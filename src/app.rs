use std::collections::HashSet;
use std::sync::mpsc;

use chrono::Utc;
use crossterm::event::KeyCode;
use ratatui::text::Line;
use tui_input::backend::crossterm::to_input_request;
use tui_input::Input as TuiInput;
use crossterm::event::Event as CrosstermEvent;

use crate::config::{matches_key, Config};
use crate::error::Result;
use crate::feed::fetcher::{probe_feed, spawn_fetch, FetchMessage};
use crate::feed::{Article, Feed};
use crate::feeds_config::{add_feed_to_file, remove_feed_from_file, rename_feed_in_file, FeedConfig, SidebarItem, SidebarLayout};
use crate::html::{html_to_lines, ArticleStyles};
use crate::state::{Cache, InputMode, UiState};
use crate::theme::Theme;

/// Central application state.
pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub cache: Cache,
    pub ui: UiState,

    /// Flat feed list; ordering matches `sidebar.feed_order`.
    pub feeds: Vec<Feed>,

    /// Pre-computed sidebar display structure (category headers + feed rows).
    pub sidebar: SidebarLayout,

    /// Pre-rendered lines for the currently displayed article.
    pub article_lines: Vec<Line<'static>>,

    /// Pre-rendered lines for the summary/description of the current article.
    pub description_lines: Vec<Line<'static>>,

    /// Feed URLs whose most recent fetch resulted in an error.
    pub feed_errors: HashSet<String>,

    /// Receives results from the background fetch thread.
    fetch_rx: Option<mpsc::Receiver<FetchMessage>>,
    /// Receives the result of a single-URL validation probe (add-feed flow).
    /// Carries `Ok(feed_title)` on success or `Err(reason)` on failure.
    validate_rx: Option<mpsc::Receiver<Result<Option<String>>>>,

    /// Width (in columns) that was used for the last `render_current_article()` call.
    /// Compared against `ui.article_view_width` each frame to detect when a re-render
    /// is needed (e.g. after first frame or after terminal resize).
    rendered_at_width: u16,
}

impl App {
    /// Build the application.
    ///
    /// `feed_configs` and `sidebar` come from `FeedsFile::into_layout()`.
    pub fn new(
        config: Config,
        theme: Theme,
        mut cache: Cache,
        feed_configs: Vec<FeedConfig>,
        sidebar: SidebarLayout,
    ) -> Self {
        // Purge old articles before building the feed list.
        cache.purge_old_articles(config.max_article_age_days);

        let feeds = build_feeds(feed_configs, &cache);

        let mut app = Self {
            config,
            theme,
            cache,
            ui: UiState::default(),
            feeds,
            sidebar,
            article_lines: Vec::new(),
            description_lines: Vec::new(),
            feed_errors: HashSet::new(),
            fetch_rx: None,
            validate_rx: None,
            rendered_at_width: 0,
        };

        // Select the first feed and its first unread article.
        app.sync_article_selection();
        app.render_current_article();

        if app.feeds.is_empty() {
            app.ui.status_message = Some(
                "No feeds configured — press 'a' to add a feed, or supply a feeds.toml file."
                    .into(),
            );
        }

        app
    }

    /// Trigger a background refresh of all configured feeds.
    pub fn refresh_all_feeds(&mut self) {
        let urls: Vec<String> = self.feeds.iter().map(|f| f.url.clone()).collect();
        if urls.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.fetch_rx = Some(rx);
        spawn_fetch(urls, tx);
        self.ui.status_message = Some("Fetching all feeds…".into());
        self.ui.status_is_error = false;
    }

    /// Trigger a background refresh of the currently selected feed only.
    pub fn refresh_current_feed(&mut self) {
        let url = match self.feeds.get(self.ui.selected_feed) {
            Some(f) => f.url.clone(),
            None => return,
        };
        let (tx, rx) = mpsc::channel();
        self.fetch_rx = Some(rx);
        spawn_fetch(vec![url.clone()], tx);
        self.ui.status_message = Some(format!("Fetching {}…", url));
        self.ui.status_is_error = false;
    }

    /// Open the current article's URL in the system browser.
    pub fn open_browser(&mut self) {
        let url = self.current_article().and_then(|a| a.url.clone());
        if let Some(url) = url {
            let scheme_ok = url.starts_with("https://") || url.starts_with("http://");
            if !scheme_ok {
                self.ui.status_message = Some("URL scheme not allowed (only http/https)".into());
                self.ui.status_is_error = true;
                return;
            }
            let _ = open::that_detached(&url);
            self.ui.status_message = Some("Done.".into());
            self.ui.status_is_error = false;
        }
    }

    /// Poll the fetch channel for new messages; returns when the channel is empty.
    pub fn tick(&mut self) {
        // ── Drain the single-URL validation channel (add-feed flow) ──────────
        if let Some(rx) = &self.validate_rx {
            if let Ok(result) = rx.try_recv() {
                self.validate_rx = None;
                // Only act if we're still waiting (user may have pressed Esc).
                if let InputMode::ValidatingFeedUrl { url } = self.ui.input_mode.clone() {
                    match result {
                        Ok(feed_title) => {
                            let category = self.current_feed_category();
                            self.ui.input = TuiInput::new(category);
                            self.ui.input_mode =
                                InputMode::AddFeedCategory { pending_url: url, feed_title };
                            self.ui.status_message = None;
                            self.ui.status_is_error = false;
                        }
                        Err(e) => {
                            self.ui.input_mode = InputMode::Idle;
                            self.ui.input = TuiInput::default();
                            self.ui.status_message = Some(format!("Invalid feed: {e}"));
                            self.ui.status_is_error = true;
                        }
                    }
                }
            }
        }

        // ── Drain the background fetch channel ───────────────────────────────
        let messages: Vec<FetchMessage> = self
            .fetch_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        for msg in messages {
            match msg {
                FetchMessage::Started { url } => {
                    self.ui.fetching = Some(url.clone());
                    self.ui.status_message = Some(format!("Fetching {}…", url));
                    self.ui.status_is_error = false;
                }
                FetchMessage::Complete { url, feed_title, articles } => {
                    // Clear any previous error for this feed on success.
                    self.feed_errors.remove(&url);
                    self.apply_fetch_result(&url, feed_title, articles);
                }
                FetchMessage::Error { url, reason } => {
                    self.feed_errors.insert(url.clone());
                    self.ui.status_message =
                        Some(format!("Error fetching {}: {}", url, reason));
                    self.ui.status_is_error = true;
                }
                FetchMessage::Done => {
                    self.ui.fetching = None;
                    if !self.ui.status_is_error {
                        self.ui.status_message = Some("Done.".into());
                    }
                    self.fetch_rx = None;
                    // Persist updated articles to disk.
                    self.sync_cache_feeds();
                    let _ = self.cache.save();
                }
            }
        }
    }

    /// Handle a key press. Returns `false` when the application should quit.
    pub fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> bool {
        // When collecting input, all keys are consumed by the input handler.
        if self.ui.input_mode != InputMode::Idle {
            return self.handle_input_key(event);
        }

        // Clone key binding strings so we can also call `&mut self` methods.
        let keys = self.config.keys.clone();

        if matches_key(&event, &keys.quit) {
            return false;
        }

        if matches_key(&event, &keys.feed_down) {
            self.next_feed();
        } else if matches_key(&event, &keys.feed_up) {
            self.prev_feed();
        } else if matches_key(&event, &keys.article_down) {
            self.next_article();
        } else if matches_key(&event, &keys.article_up) {
            self.prev_article();
        } else if matches_key(&event, &keys.article_home) {
            self.first_article();
        } else if matches_key(&event, &keys.article_end) {
            self.last_article();
        } else if matches_key(&event, &keys.scroll_down) {
            self.scroll_down();
        } else if matches_key(&event, &keys.scroll_up) {
            self.scroll_up();
        } else if matches_key(&event, &keys.scroll_line_down) {
            self.scroll_line_down();
        } else if matches_key(&event, &keys.scroll_line_up) {
            self.scroll_line_up();
        } else if matches_key(&event, &keys.refresh_feed) {
            self.refresh_current_feed();
        } else if matches_key(&event, &keys.refresh_all) {
            self.refresh_all_feeds();
        } else if matches_key(&event, &keys.reload_config) {
            self.reload_config();
        } else if matches_key(&event, &keys.add_feed) {
            self.start_add_feed();
        } else if matches_key(&event, &keys.delete_feed) {
            self.start_delete_feed();
        } else if matches_key(&event, &keys.edit_feed) {
            self.start_edit_feed();
        } else if matches_key(&event, &keys.open_browser) {
            self.open_browser();
        }

        true
    }

    /// Handle a terminal paste event (requires bracketed paste mode).
    ///
    /// When an input prompt is active, the pasted text is appended to the
    /// input buffer with control characters stripped.  Paste events outside
    /// of an input prompt are silently ignored.
    pub fn handle_paste(&mut self, text: &str) {
        if self.ui.input_mode == InputMode::Idle {
            return;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            self.ui.input.handle(tui_input::InputRequest::InsertChar(c));
        }
    }

    /// Handle a key press while an input prompt is active.
    fn handle_input_key(&mut self, event: crossterm::event::KeyEvent) -> bool {
        match event.code {
            KeyCode::Esc => {
                self.ui.input_mode = InputMode::Idle;
                self.ui.input = TuiInput::default();
                self.ui.status_message = None;
                self.ui.status_is_error = false;
            }
            KeyCode::Enter => {
                self.submit_input();
            }
            _ => {
                if let Some(req) = to_input_request(&CrosstermEvent::Key(event)) {
                    self.ui.input.handle(req);
                }
            }
        }
        true
    }

    /// Begin the add-feed flow: prompt the user for a feed URL.
    fn start_add_feed(&mut self) {
        self.ui.input = TuiInput::default();
        self.ui.input_mode = InputMode::AddFeedUrl;
        self.ui.status_message = None;
        self.ui.status_is_error = false;
    }

    /// Begin the delete-feed flow: prompt the user for confirmation.
    fn start_delete_feed(&mut self) {
        if let Some(feed) = self.current_feed() {
            let feed_url = feed.url.clone();
            let feed_name = feed.name.clone();
            self.ui.input = TuiInput::default();
            self.ui.input_mode = InputMode::DeleteConfirm { feed_url, feed_name };
            self.ui.status_message = None;
            self.ui.status_is_error = false;
        }
    }

    /// Begin the edit-feed-name flow: pre-fill the input buffer with the
    /// current display name so the user can edit it in place.
    fn start_edit_feed(&mut self) {
        let Some(feed) = self.feeds.get(self.ui.selected_feed) else {
            return;
        };
        let url = feed.url.clone();
        let current_name = feed.name.clone();
        self.ui.input = TuiInput::new(current_name);
        self.ui.input_mode = InputMode::EditFeedName { feed_url: url };
        self.ui.status_message = Some("Edit feed name (Enter to confirm, Esc to cancel)".into());
        self.ui.status_is_error = false;
    }

    /// Process an Enter keypress in the active input mode.
    fn submit_input(&mut self) {
        // Clone so we can borrow `self` mutably below.
        let mode = self.ui.input_mode.clone();

        match mode {
            InputMode::Idle => {}

            InputMode::AddFeedUrl => {
                let url = self.ui.input.value().trim().to_owned();
                if url.is_empty() {
                    self.ui.input_mode = InputMode::Idle;
                    return;
                }
                // Spawn a background thread to probe the URL so the UI can
                // show "Loading…" without blocking the draw loop.
                let (tx, rx) = mpsc::channel();
                let probe_url = url.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(probe_feed(&probe_url));
                });
                self.validate_rx = Some(rx);
                self.ui.input_mode = InputMode::ValidatingFeedUrl { url };
                self.ui.input = TuiInput::default();
            }

            InputMode::ValidatingFeedUrl { .. } => {
                // Validation is in progress; ignore spurious Enter presses.
            }

            InputMode::AddFeedCategory { pending_url, feed_title } => {
                let category = self.ui.input.value().trim().to_owned();
                self.ui.input_mode = InputMode::Idle;
                self.ui.input = TuiInput::default();

                match self.config.feeds_file_path() {
                    Ok(path) => {
                        let title_ref = feed_title.as_deref();
                        match add_feed_to_file(&path, &pending_url, title_ref, &category) {
                            Ok(()) => {
                                self.reload_config();
                                self.ui.status_message =
                                    Some(format!("Feed added: {pending_url}"));
                                self.ui.status_is_error = false;
                            }
                            Err(e) => {
                                self.ui.status_message =
                                    Some(format!("Failed to save feed: {e}"));
                                self.ui.status_is_error = true;
                            }
                        }
                    }
                    Err(e) => {
                        self.ui.status_message = Some(format!("Config error: {e}"));
                        self.ui.status_is_error = true;
                    }
                }
            }

            InputMode::DeleteConfirm { feed_url, .. } => {
                self.ui.input_mode = InputMode::Idle;
                self.ui.input = TuiInput::default();

                match self.config.feeds_file_path() {
                    Ok(path) => match remove_feed_from_file(&path, &feed_url) {
                        Ok(()) => {
                            self.reload_config();
                            self.ui.status_message = Some("Feed deleted.".into());
                            self.ui.status_is_error = false;
                        }
                        Err(e) => {
                            self.ui.status_message =
                                Some(format!("Failed to delete feed: {e}"));
                            self.ui.status_is_error = true;
                        }
                    },
                    Err(e) => {
                        self.ui.status_message = Some(format!("Config error: {e}"));
                        self.ui.status_is_error = true;
                    }
                }
            }

            InputMode::EditFeedName { feed_url } => {
                let url = feed_url.clone();
                let new_name = self.ui.input.value().trim().to_owned();
                // Update in memory.
                if let Some(feed) = self.feeds.iter_mut().find(|f| f.url == url) {
                    feed.name = if new_name.is_empty() {
                        // Reset to feed URL as a temporary placeholder until next refresh.
                        url.clone()
                    } else {
                        new_name.clone()
                    };
                }
                // Persist to feeds.toml.
                let name_opt: Option<&str> = if new_name.is_empty() { None } else { Some(&new_name) };
                match self.config.feeds_file_path() {
                    Ok(path) => {
                        if let Err(e) = rename_feed_in_file(&path, &url, name_opt) {
                            self.ui.status_message = Some(format!("Failed to save: {e}"));
                            self.ui.status_is_error = true;
                        } else {
                            self.ui.status_message = None;
                        }
                    }
                    Err(e) => {
                        self.ui.status_message = Some(format!("Config error: {e}"));
                        self.ui.status_is_error = true;
                    }
                }
                self.ui.input_mode = InputMode::Idle;
                self.ui.input = TuiInput::default();
                // Invalidate column cache since the name width may have changed.
                self.ui.raw_author_col = usize::MAX;
            }
        }
    }

    /// Return the category title of the currently selected feed, or an empty
    /// string when the feed is uncategorized or no categories exist.
    fn current_feed_category(&self) -> String {
        let feed_idx = self.ui.selected_feed;
        let mut last_category = String::new();
        for item in &self.sidebar.items {
            match item {
                SidebarItem::Category { title, .. } => {
                    last_category = title.clone();
                }
                SidebarItem::Feed { feed_index, .. } => {
                    if *feed_index == feed_idx {
                        return last_category;
                    }
                }
            }
        }
        String::new()
    }


    /// Reload `config.toml` and `feeds.toml` without restarting.
    ///
    /// - Re-reads both files from disk (falling back to defaults on error).
    /// - Rebuilds the sidebar/feed list, preserving cached articles and read status.
    /// - Keeps the current feed selection if the URL is still present; otherwise
    ///   falls back to the first feed.
    pub fn reload_config(&mut self) {
        // Re-load config.
        let new_config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                self.ui.status_message = Some(format!("Config reload failed: {e}"));
                self.ui.status_is_error = true;
                return;
            }
        };

        // Re-load feeds file.
        let feeds_path = match new_config.feeds_file_path() {
            Ok(p) => p,
            Err(e) => {
                self.ui.status_message = Some(format!("Config reload failed: {e}"));
                self.ui.status_is_error = true;
                return;
            }
        };
        let feeds_file = match crate::feeds_config::FeedsFile::load(&feeds_path) {
            Ok(f) => f,
            Err(e) => {
                self.ui.status_message = Some(format!("Feeds reload failed: {e}"));
                self.ui.status_is_error = true;
                return;
            }
        };

        // Rebuild sidebar and feed list.
        let (feed_configs, new_sidebar) =
            feeds_file.into_layout(&new_config.uncategorized_label);

        // Remember which feed URL was selected so we can restore selection.
        let selected_url = self
            .current_feed()
            .map(|f| f.url.clone())
            .unwrap_or_default();

        // Rebuild feed list (merges new config with existing cached articles).
        let new_feeds = build_feeds(feed_configs, &self.cache);

        // Re-load theme if it changed.
        let themes_dir = Config::themes_dir().ok();
        let new_theme = crate::theme::Theme::load(&new_config.theme, themes_dir.as_deref());

        self.config = new_config;
        self.theme = new_theme;
        self.sidebar = new_sidebar;
        self.feeds = new_feeds;

        // Restore feed selection by URL, or default to 0.
        let new_idx = self
            .feeds
            .iter()
            .position(|f| f.url == selected_url)
            .unwrap_or(0);
        self.ui.selected_feed = new_idx;

        self.sync_article_selection();
        self.render_current_article();
        self.ui.status_message = Some("Config reloaded.".into());
        self.ui.status_is_error = false;
    }

    /// Save the cache on clean exit.
    pub fn on_exit(&mut self) -> Result<()> {
        self.sync_cache_feeds();
        self.cache.save()
    }

    // ── Navigation helpers ───────────────────────────────────────────────────

    fn next_feed(&mut self) {
        let order = &self.sidebar.feed_order;
        if order.is_empty() {
            return;
        }
        // Find current position in the ordered feed list, then advance one step.
        let pos = order
            .iter()
            .position(|&i| i == self.ui.selected_feed)
            .unwrap_or(0);
        if pos + 1 < order.len() {
            self.ui.selected_feed = order[pos + 1];
            self.ui.raw_author_col = usize::MAX; // invalidate column cache for new feed
            self.sync_article_selection();
            self.render_current_article();
        }
    }

    fn prev_feed(&mut self) {
        let order = &self.sidebar.feed_order;
        if order.is_empty() {
            return;
        }
        let pos = order
            .iter()
            .position(|&i| i == self.ui.selected_feed)
            .unwrap_or(0);
        if pos > 0 {
            self.ui.selected_feed = order[pos - 1];
            self.ui.raw_author_col = usize::MAX; // invalidate column cache for new feed
            self.sync_article_selection();
            self.render_current_article();
        }
    }

    fn next_article(&mut self) {
        let max = self.current_article_count().saturating_sub(1);
        self.ui.selected_article = (self.ui.selected_article + 1).min(max);
        self.ui.article_scroll = 0;
        self.mark_current_read();
        self.render_current_article();
    }

    fn prev_article(&mut self) {
        self.ui.selected_article = self.ui.selected_article.saturating_sub(1);
        self.ui.article_scroll = 0;
        self.mark_current_read();
        self.render_current_article();
    }

    fn first_article(&mut self) {
        self.ui.selected_article = 0;
        self.ui.article_scroll = 0;
        self.mark_current_read();
        self.render_current_article();
    }

    fn last_article(&mut self) {
        let max = self.current_article_count().saturating_sub(1);
        self.ui.selected_article = max;
        self.ui.article_scroll = 0;
        self.mark_current_read();
        self.render_current_article();
    }

    fn scroll_down(&mut self) {
        let page = self.ui.article_view_height.max(1);
        let max_scroll = (self.ui.article_content_height)
            .saturating_sub(self.ui.article_view_height as usize)
            .min(u16::MAX as usize) as u16;
        self.ui.article_scroll = (self.ui.article_scroll + page).min(max_scroll);
    }

    fn scroll_up(&mut self) {
        let page = self.ui.article_view_height.max(1);
        self.ui.article_scroll = self.ui.article_scroll.saturating_sub(page);
    }

    fn scroll_line_down(&mut self) {
        let max_scroll = (self.ui.article_content_height)
            .saturating_sub(self.ui.article_view_height as usize)
            .min(u16::MAX as usize) as u16;
        self.ui.article_scroll = (self.ui.article_scroll + 1).min(max_scroll);
    }

    fn scroll_line_up(&mut self) {
        self.ui.article_scroll = self.ui.article_scroll.saturating_sub(1);
    }

    // ── State helpers ────────────────────────────────────────────────────────

    /// Return the currently selected feed, if any.
    pub fn current_feed(&self) -> Option<&Feed> {
        self.feeds.get(self.ui.selected_feed)
    }

    /// Return the articles of the currently selected feed.
    pub fn current_articles(&self) -> &[Article] {
        self.current_feed()
            .map(|f| f.articles.as_slice())
            .unwrap_or_default()
    }

    /// Return the currently selected article, if any.
    pub fn current_article(&self) -> Option<&Article> {
        self.current_feed()
            .and_then(|f| f.articles.get(self.ui.selected_article))
    }

    fn current_article_count(&self) -> usize {
        self.current_feed().map(|f| f.articles.len()).unwrap_or(0)
    }

    /// After switching feeds: jump to the first unread article (or 0), reset scroll.
    fn sync_article_selection(&mut self) {
        let feed = match self.feeds.get(self.ui.selected_feed) {
            Some(f) => f,
            None => return,
        };
        let url = feed.url.clone();
        let first_unread = feed
            .articles
            .iter()
            .position(|a| !self.cache.is_read(&url, &a.id))
            .unwrap_or(0);
        self.ui.selected_article = first_unread;
        self.ui.article_scroll = 0;
        self.mark_current_read();
    }

    fn mark_current_read(&mut self) {
        if let Some(feed) = self.feeds.get(self.ui.selected_feed) {
            let url = feed.url.clone();
            if let Some(article) = feed.articles.get(self.ui.selected_article) {
                let id = article.id.clone();
                self.cache.mark_read(&url, &id);
            }
        }
    }

    /// Re-render the article body for the current selection into `self.article_lines`.
    fn render_current_article(&mut self) {
        // Build styles before borrowing self.feeds so the self.theme borrow is
        // fully released before we take references into self.feeds.
        let styles = {
            let t = &self.theme;
            ArticleStyles {
                h1:    t.style("article.h1"),
                h2:    t.style("article.h2"),
                h3:    t.style("article.h3"),
                code:  t.style("article.code"),
                link:  t.style("article.link"),
                quote: t.style("article.quote"),
            }
        };
        let width = self.ui.article_view_width;

        // Direct field access (self.feeds.get()) gives the compiler split-borrow
        // visibility so it can prove content/description don't alias article_lines
        // or description_lines — avoiding clone() of potentially large HTML strings.
        let feed_idx    = self.ui.selected_feed;
        let article_idx = self.ui.selected_article;
        let (content, description): (&str, &str) = self.feeds
            .get(feed_idx)
            .and_then(|f| f.articles.get(article_idx))
            .map(|a| (a.content.as_str(), a.description.as_deref().unwrap_or("")))
            .unwrap_or(("", ""));

        self.article_lines      = html_to_lines(content,     styles, width);
        self.description_lines  = html_to_lines(description, styles, width);
        self.rendered_at_width  = width;
    }

    /// Re-render the article if the panel width changed since the last render.
    /// Called from the event loop after each frame so the first frame's width
    /// is used immediately (without waiting for a keypress).
    pub fn rerender_if_width_changed(&mut self) {
        if self.ui.article_view_width != 0 && self.ui.article_view_width != self.rendered_at_width {
            self.render_current_article();
        }
    }

    /// Merge freshly fetched articles into the correct feed slot.
    fn apply_fetch_result(&mut self, url: &str, feed_title: Option<String>, articles: Vec<Article>) {
        if let Some(feed) = self.feeds.iter_mut().find(|f| f.url == url) {
            feed.articles = articles;
            feed.last_fetched = Some(Utc::now());
            // When no name was configured in feeds.toml the feed starts with its
            // URL as a placeholder name.  Update it with the title from the feed
            // itself as soon as we have one.
            if feed.name == url {
                if let Some(title) = feed_title {
                    feed.name = title;
                }
            }
        }

        // If the fetch was for the currently displayed feed, re-sync the selection.
        if self.current_feed().map(|f| f.url == url).unwrap_or(false) {
            self.ui.raw_author_col = usize::MAX; // new articles may have different author widths
            self.sync_article_selection();
            self.render_current_article();
        }
    }

    /// Copy current in-memory feeds back into `cache.feeds` before saving.
    fn sync_cache_feeds(&mut self) {
        self.cache.feeds = self.feeds.clone();
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Build the live feed list by merging `feed_configs` entries with cached article data.
fn build_feeds(feed_configs: Vec<FeedConfig>, cache: &Cache) -> Vec<Feed> {
    let cache_map: std::collections::HashMap<&str, &Feed> =
        cache.feeds.iter().map(|f| (f.url.as_str(), f)).collect();

    feed_configs
        .into_iter()
        .map(|fc| {
            if let Some(cached) = cache_map.get(fc.url.as_str()) {
                let mut feed = (*cached).clone();
                // Config name takes precedence over any cached name.
                if let Some(name) = &fc.name {
                    feed.name = name.clone();
                }
                // Defensive dedup: drop any articles whose ID has already
                // been seen.  Normally articles are dedup'd in fetch_feed(),
                // but old caches written before that fix may still contain
                // duplicates.
                let mut seen = std::collections::HashSet::new();
                feed.articles.retain(|a| seen.insert(a.id.clone()));
                feed
            } else {
                Feed::new(&fc.url, fc.name.clone().unwrap_or_else(|| fc.url.clone()))
            }
        })
        .collect()
}
