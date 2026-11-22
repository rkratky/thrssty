use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::feed::{Article, Feed};

/// Persistent application state: cached feed content and per-feed read status.
///
/// Saved to `~/.local/share/thrssty/cache.json` on exit and after each fetch.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    /// Cached feeds with their articles.
    pub feeds: Vec<Feed>,
    /// Per-feed read article IDs: `feed_url → {article_id, ...}`.
    pub read_ids: HashMap<String, HashSet<String>>,
}

impl Cache {
    /// Path to the cache file: `~/.local/share/thrssty/cache.json`.
    pub fn cache_path() -> Result<PathBuf> {
        Ok(dirs::data_dir()
            .ok_or(AppError::MissingDataDir)?
            .join("thrssty")
            .join("cache.json"))
    }

    /// Load the cache from disk. Returns an empty cache if the file does not exist.
    /// If the file exists but cannot be deserialised (e.g. the schema changed),
    /// the stale file is deleted and an empty cache is returned; a notice is
    /// printed to stdout so the user can see it before the TUI starts.
    pub fn load() -> Result<Self> {
        let path = Self::cache_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        match serde_json::from_str::<Self>(&content) {
            Ok(cache) => Ok(cache),
            Err(e) => {
                // Schema mismatch or corruption — start fresh and re-fetch everything.
                println!(
                    "thrssty: cache schema has changed ({}); discarding stale cache and re-fetching all feeds.",
                    e
                );
                // Best-effort removal; ignore errors (we'll overwrite on next save).
                let _ = std::fs::remove_file(&path);
                Ok(Self::default())
            }
        }
    }

    /// Persist the cache to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::cache_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Returns `true` if the article has been read.
    pub fn is_read(&self, feed_url: &str, article_id: &str) -> bool {
        self.read_ids
            .get(feed_url)
            .map(|ids| ids.contains(article_id))
            .unwrap_or(false)
    }

    /// Mark an article as read.
    pub fn mark_read(&mut self, feed_url: &str, article_id: &str) {
        self.read_ids
            .entry(feed_url.to_owned())
            .or_default()
            .insert(article_id.to_owned());
    }

    /// Count unread articles in a feed.
    pub fn unread_count(&self, feed_url: &str, articles: &[Article]) -> usize {
        articles
            .iter()
            .filter(|a| !self.is_read(feed_url, &a.id))
            .count()
    }

    /// Remove articles older than `max_age_days` from all feeds in the cache.
    ///
    /// Also removes their IDs from `read_ids` to avoid stale entries.
    /// Does nothing when `max_age_days == 0`.
    pub fn purge_old_articles(&mut self, max_age_days: u64) {
        use chrono::Utc;
        if max_age_days == 0 {
            return;
        }
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days.min(i64::MAX as u64) as i64);
        for feed in &mut self.feeds {
            let before = feed.articles.len();
            let surviving: Vec<_> = feed
                .articles
                .iter()
                .filter(|a| a.published.map(|d| d > cutoff).unwrap_or(true))
                .cloned()
                .collect();

            // Don't empty a feed: if purging would remove every article, keep
            // the stale ones so the user sees content until the next refresh.
            if surviving.is_empty() {
                continue;
            }

            feed.articles = surviving;
            // Clean up stale read-IDs for articles that were actually removed.
            if feed.articles.len() < before {
                if let Some(ids) = self.read_ids.get_mut(&feed.url) {
                    let keep: std::collections::HashSet<String> =
                        feed.articles.iter().map(|a| a.id.clone()).collect();
                    ids.retain(|id| keep.contains(id));
                }
            }
        }
    }
}

/// Tracks whether the bottom input line is collecting user input.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum InputMode {
    /// Normal navigation — no input is being collected.
    #[default]
    Idle,
    /// Collecting a new feed URL.
    AddFeedUrl,
    /// Probing the URL in a background thread to confirm it is a valid feed.
    ValidatingFeedUrl { url: String },
    /// Collecting the category name for a new feed.
    ///
    /// `pending_url` — the validated URL from the previous step.
    /// `feed_title` — title retrieved from the feed during validation (used as the
    ///                display name in feeds.toml when present).
    AddFeedCategory {
        pending_url: String,
        feed_title: Option<String>,
    },
    /// Awaiting Enter/Esc confirmation to delete a feed.
    DeleteConfirm { feed_url: String, feed_name: String },
    /// Editing the display name of an existing feed.
    ///
    /// `feed_url` — URL key used to locate the entry in `feeds.toml`.
    EditFeedName { feed_url: String },
}

/// Ephemeral UI state (not persisted between sessions).
#[derive(Debug)]
pub struct UiState {
    /// Index of the currently selected feed in the sidebar.
    pub selected_feed: usize,
    /// Index of the currently selected article in the list.
    pub selected_article: usize,
    /// Vertical scroll offset for the article view (in raw Line units).
    pub article_scroll: u16,
    /// Height of the article view area (updated each frame, used for scroll clamping).
    pub article_view_height: u16,
    /// Width of the article view area (updated each frame, used to size html2text output).
    pub article_view_width: u16,
    /// Short message shown in the status bar (e.g. fetch status or errors).
    pub status_message: Option<String>,
    /// When `true`, `status_message` should be rendered with the error style.
    pub status_is_error: bool,
    /// URL of the feed currently being fetched, if any.
    pub fetching: Option<String>,
    /// Current input mode (Idle = normal navigation).
    pub input_mode: InputMode,
    /// Text input field (URL prompt, feed name, category name).
    pub input: tui_input::Input,
    /// Total number of `Line` objects in the currently rendered article view
    /// (header + description + body). Updated by `draw_article_view` each frame.
    pub article_content_height: usize,
    /// Scroll offset of the article-list `List` widget (updated each frame from
    /// `ListState::offset()` after rendering). Used for accurate scroll arrows.
    pub article_list_offset: usize,
    /// Scroll offset of the sidebar `List` widget (updated each frame).
    pub sidebar_list_offset: usize,
    /// Cached maximum author name length (in chars) for the current feed.
    ///
    /// Set to `usize::MAX` as a sentinel meaning "not yet computed". Invalidated
    /// (reset to `usize::MAX`) whenever the selected feed changes so that
    /// `draw_article_list` re-scans only when necessary rather than every frame.
    pub raw_author_col: usize,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_feed: 0,
            selected_article: 0,
            article_scroll: 0,
            article_view_height: 0,
            article_view_width: 0,
            status_message: None,
            status_is_error: false,
            fetching: None,
            input_mode: InputMode::default(),
            input: tui_input::Input::default(),
            article_content_height: 0,
            article_list_offset: 0,
            sidebar_list_offset: 0,
            raw_author_col: usize::MAX, // sentinel: "not yet computed"
        }
    }
}
