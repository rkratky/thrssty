use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph,
              Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use crate::app::App;
use crate::feed::Article;
use crate::feeds_config::SidebarItem;
use crate::state::InputMode;

/// Draw the entire TUI for one frame.
///
/// Layout:
/// ```text
/// ┌──────────────┬────────────────────────────────┐
/// │              │  Article List                  │
/// │  Feed        ├────────────────────────────────┤
/// │  Sidebar     │                                │
/// │              │  Article View                  │
/// │              │                                │
/// ├──────────────┴────────────────────────────────┤
/// │  Input / Status line                          │
/// │  Key hints                                    │
/// └───────────────────────────────────────────────┘
/// ```
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Outer: main content | input/status line | key-hints line.
    let outer = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    let main_area   = outer[0];
    let input_area  = outer[1];
    let hints_area  = outer[2];

    // Main: feed sidebar | right content.
    let sidebar_pct = app.config.layout.sidebar_width;
    let main_chunks = Layout::horizontal([Constraint::Percentage(sidebar_pct), Constraint::Min(0)])
        .split(main_area);

    // Right content: article list | article view.
    let list_pct = app.config.layout.article_list_height;
    let content_chunks = Layout::vertical([Constraint::Percentage(list_pct), Constraint::Min(0)])
        .split(main_chunks[1]);

    // Expose the article view dimensions so the App can clamp scrolling and size renders.
    app.ui.article_view_height = content_chunks[1].height.saturating_sub(2); // minus borders
    // Inner width: area minus 2 border cols minus 2 padding cols (Padding::horizontal(1)).
    app.ui.article_view_width = content_chunks[1].width.saturating_sub(4);

    draw_feed_sidebar(frame, app, main_chunks[0]);
    draw_article_list(frame, app, content_chunks[0]);
    draw_article_view(frame, app, content_chunks[1]);
    draw_input_line(frame, app, input_area);
    draw_hints_line(frame, app, hints_area);
}

// ── Feed sidebar ─────────────────────────────────────────────────────────────

fn draw_feed_sidebar(frame: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let bg_style = t.style("ui.background");

    // Inner width available for text (subtract 2 border columns).
    let inner_width = area.width.saturating_sub(2) as usize;

    // Find which sidebar row the currently selected feed is on (for ListState scroll).
    let selected_row = app.sidebar.items.iter().position(|item| {
        matches!(item, SidebarItem::Feed { feed_index, .. } if *feed_index == app.ui.selected_feed)
    });

    let items: Vec<ListItem> = app
        .sidebar
        .items
        .iter()
        .map(|item| match item {
            SidebarItem::Category { title, feed_indices } => {
                // Aggregate total and unread counts across all feeds in this category.
                let (total, unread) =
                    feed_indices.iter().fold((0usize, 0usize), |(t, u), &idx| {
                        if let Some(feed) = app.feeds.get(idx) {
                            let feed_unread = app.cache.unread_count(&feed.url, &feed.articles);
                            (t + feed.articles.len(), u + feed_unread)
                        } else {
                            (t, u)
                        }
                    });

                let indicator = if unread > 0 { "● " } else { "  " };
                let counts = format!(" ({unread})/{total}");
                // 0-indent for category headers; "indicator" (2) + name + counts.
                let name_width = inner_width
                    .saturating_sub(2) // indicator
                    .saturating_sub(counts.len());
                let name = truncate(title, name_width);

                let style = if unread > 0 {
                    t.style("category.unread")
                } else {
                    t.style("category.read")
                };

                let text = format!("{indicator}{name:<name_width$}{counts}");
                ListItem::new(Span::styled(text, style))
            }

            SidebarItem::Feed { feed_index, indented } => {
                let Some(feed) = app.feeds.get(*feed_index) else {
                    return ListItem::new(Line::raw(""));
                };
                let unread = app.cache.unread_count(&feed.url, &feed.articles);
                let total = feed.articles.len();
                let is_selected = *feed_index == app.ui.selected_feed;

                let indent = if *indented { "  " } else { "" };
                let indicator = if unread > 0 { "● " } else { "  " };
                let prefix_len = indent.len() + 2; // indent + indicator (2 chars)
                let counts = format!(" ({unread})/{total}");
                let name_width = inner_width
                    .saturating_sub(prefix_len)
                    .saturating_sub(counts.len());
                let name = truncate(&feed.name, name_width);

                let has_error = app.feed_errors.contains(&feed.url);
                let style = if is_selected {
                    t.style("feed.selected")
                } else if has_error {
                    t.style("feed.error")
                } else if unread > 0 {
                    t.style("feed.unread")
                } else {
                    t.style("feed.read")
                };

                let text = format!("{indent}{indicator}{name:<name_width$}{counts}");
                ListItem::new(Span::styled(text, style))
            }
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(selected_row);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Feeds ", t.style("ui.text").add_modifier(Modifier::BOLD)))
        .style(bg_style);

    // All item styling is applied explicitly above; highlight_style adds nothing extra.
    let list = List::new(items).block(block);

    frame.render_stateful_widget(list, area, &mut list_state);

    // Store scroll offset for accurate scrollbar display next frame.
    let offset = list_state.offset();
    app.ui.sidebar_list_offset = offset;

    let viewport_rows = area.height.saturating_sub(2) as usize;
    let total_rows = app.sidebar.items.len();
    draw_scrollbar(frame, area, total_rows, offset, viewport_rows, t.style("ui.text"));
}

// ── Article list ──────────────────────────────────────────────────────────────

fn draw_article_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let bg_style = t.style("ui.background");

    // Use direct field access so the compiler can see that articles/feed_name/feed_url
    // borrow only from app.feeds, leaving app.ui free for mutation (author col cache).
    let feed_idx = app.ui.selected_feed;
    let (feed_name, feed_url, articles): (&str, &str, &[Article]) =
        match app.feeds.get(feed_idx) {
            Some(f) => (f.name.as_str(), f.url.as_str(), f.articles.as_slice()),
            None    => ("(no feeds)", "", &[]),
        };

    // ── Column widths ─────────────────────────────────────────────────────────
    // Layout: [borders 2][indicator 2][title + space 1][author col][2 sep][date 10][slack 2]
    let date_width: usize = 10;
    let slack: usize = 2; // small right-margin buffer
    let inner_width = (area.width as usize).saturating_sub(2); // minus borders

    // Max author column is capped at a configurable percentage of inner width.
    let max_author_pct = app.config.layout.max_author_col_pct as usize;

    // Compute the widest author name across all articles in this feed, capped.
    // The raw max is cached in UiState (sentinel usize::MAX = not yet computed)
    // and only recomputed when the selected feed changes or new articles arrive.
    // Direct field access above lets the borrow checker see articles ∈ app.feeds
    // and app.ui.raw_author_col ∈ app.ui — disjoint, so the assignment is legal.
    if app.ui.raw_author_col == usize::MAX {
        app.ui.raw_author_col = articles
            .iter()
            .filter_map(|a| a.author.as_deref())
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0);
    }
    let author_col: usize = app.ui.raw_author_col.min(inner_width * max_author_pct / 100);

    // 2 separator spaces between author column and date (only when column is used).
    let author_section: usize = if author_col > 0 { author_col + 2 } else { 0 };

    let title_width = (area.width as usize)
        .saturating_sub(2 + 2 + 1 + author_section + date_width + slack);

    let items: Vec<ListItem> = articles
        .iter()
        .enumerate()
        .map(|(i, article)| {
            let is_selected = i == app.ui.selected_article;
            let is_read = app.cache.is_read(feed_url, &article.id);

            let indicator = if is_read { "  " } else { "● " };

            let date_str = article
                .published
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| " ".repeat(date_width));

            let title = truncate(&article.title, title_width);

            let indicator_style = if is_read {
                t.style("article.read")
            } else {
                t.style("article.unread")
            };
            let title_style = if is_selected {
                t.style("article.selected")
            } else if is_read {
                t.style("article.read")
            } else {
                t.style("article.unread")
            };

            let mut spans = vec![
                Span::styled(indicator.to_owned(), indicator_style),
                Span::styled(
                    format!("{:<title_width$} ", title, title_width = title_width),
                    title_style,
                ),
            ];

            if author_col > 0 {
                let raw_author = article.author.as_deref().unwrap_or("");
                // Truncate to the capped column width, then right-align with two sep spaces.
                let author_str = truncate(raw_author, author_col);
                spans.push(Span::styled(
                    format!("{:>author_col$}  ", author_str, author_col = author_col),
                    t.style("article.author"),
                ));
            }

            spans.push(Span::styled(date_str, t.style("article.date")));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.ui.selected_article));

    let title_text = format!(" {} ({} articles) ", feed_name, articles.len());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title_text, t.style("ui.text").add_modifier(Modifier::BOLD)))
        .style(bg_style);

    let list = if items.is_empty() {
        List::new([ListItem::new(Span::styled(
            " (no articles — press r to refresh)",
            t.style("article.read"),
        ))])
        .block(block)
    } else {
        List::new(items)
            .block(block)
            .highlight_style(t.style("article.selected"))
    };

    frame.render_stateful_widget(list, area, &mut list_state);

    // Compute total while the `articles` borrow is still live.
    let total = articles.len();

    // Store scroll offset for accurate scrollbar display next frame.
    let offset = list_state.offset();
    app.ui.article_list_offset = offset;

    let viewport_rows = area.height.saturating_sub(2) as usize;
    draw_scrollbar(frame, area, total, offset, viewport_rows, t.style("ui.text"));
}

// ── Article view ──────────────────────────────────────────────────────────────

fn draw_article_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let t = &app.theme;
    let bg_style = t.style("ui.background");

    let article = app.current_article();

    let title_text = article
        .map(|a| format!(" {} ", a.title))
        .unwrap_or_else(|| " thrssty ".into());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title_text,
            t.style("ui.text").add_modifier(Modifier::BOLD),
        ))
        .style(bg_style)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);

    // Build content: article metadata header + body lines.
    // Divider width accounts for the 1-space padding on each side.
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(article) = article {
        let divider = Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            t.style("ui.border"),
        ));

        // Header: author, categories, date, URL.
        if let Some(author) = &article.author {
            lines.push(Line::from(Span::styled(
                format!("Author: {}", author),
                t.style("article.author"),
            )));
        }
        if !article.categories.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("Categories: {}", article.categories.join(", ")),
                t.style("article.category"),
            )));
        }
        if let Some(pub_date) = article.published {
            lines.push(Line::from(Span::styled(
                pub_date
                    .format("Published: %A, %d %B %Y %H:%M UTC")
                    .to_string(),
                t.style("article.date"),
            )));
        }
        if let Some(url) = &article.url {
            lines.push(Line::from(Span::styled(
                format!("URL: {}", url),
                t.style("article.link"),
            )));
        }
        // Separator below header.
        lines.push(divider.clone());

        // Find the first summary line that has actual visible text.
        let first_summary_idx = app.description_lines.iter().position(|l| {
            l.spans.iter().any(|s| !s.content.trim().is_empty())
        });
        // Check whether the article body has visible content.
        let has_body = app
            .article_lines
            .iter()
            .any(|l| l.spans.iter().any(|s| !s.content.trim().is_empty()));

        if let Some(idx) = first_summary_idx {
            lines.push(Line::raw(""));
            if has_body {
                // Body follows — show description, then divider before main body.
                lines.extend(app.description_lines[idx..].iter().cloned());
                lines.push(Line::raw(""));
                lines.push(divider);
            } else {
                // No body — the summary IS the article; render without prefix.
                lines.extend(app.description_lines[idx..].iter().cloned());
            }
        }

        if has_body {
            lines.push(Line::raw(""));
            lines.extend(app.article_lines.clone());
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Add feeds to ~/.config/thrssty/feeds.toml to get started.",
            t.style("article.read"),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.ui.article_scroll, 0));

    // line_count() accounts for soft-wrapping; call before moving paragraph.
    let display_rows = paragraph.line_count(inner.width);
    frame.render_widget(paragraph, area);

    // Store display row count so scroll_down/scroll_up clamp correctly.
    app.ui.article_content_height = display_rows;

    // Scroll indicator: use display row count vs viewport height.
    let vh = app.ui.article_view_height as usize;
    let scroll = app.ui.article_scroll as usize;
    draw_scrollbar(frame, area, display_rows, scroll, vh, t.style("ui.text"));
}

// ── Input / status line ───────────────────────────────────────────────────────

/// Draw the interactive input / status line (second-to-last row).
///
/// In input modes this shows a prompt and the current input buffer with a
/// cursor character.  In idle mode it shows the operation status message.
fn draw_input_line(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let bar_style = t.style("ui.statusline");
    let err_style = t.style("ui.statusline.error");
    let input_style = t.style("ui.statusline.info");

    let line = match &app.ui.input_mode {
        InputMode::AddFeedUrl => {
            let text = format!(" New feed URL: {}", app.ui.input.value());
            Line::from(Span::styled(text, input_style))
        }
        InputMode::ValidatingFeedUrl { url } => {
            let text = format!(" Loading '{url}'…");
            Line::from(Span::styled(text, input_style))
        }
        InputMode::AddFeedCategory { .. } => {
            let prompt = "Category (empty = uncategorized): ";
            let text = format!(" {}{}", prompt, app.ui.input.value());
            Line::from(Span::styled(text, input_style))
        }
        InputMode::DeleteConfirm { feed_name, .. } => {
            let text = format!(
                " Delete '{feed_name}'? [Enter: confirm • Esc: cancel]"
            );
            Line::from(Span::styled(text, input_style))
        }
        InputMode::EditFeedName { .. } => {
            let text = format!(" Feed name: {}", app.ui.input.value());
            Line::from(Span::styled(text, input_style))
        }
        InputMode::Idle => {
            // Show operation status (fetch progress, errors, "Done.", …).
            let (text, style) = if let Some(fetching) = &app.ui.fetching {
                (format!(" ⟳ Fetching {}…", fetching), bar_style)
            } else if let Some(msg) = &app.ui.status_message {
                let s = if app.ui.status_is_error { err_style } else { bar_style };
                (format!(" {msg}"), s)
            } else {
                (String::new(), bar_style)
            };
            Line::from(Span::styled(text, style))
        }
    };

    frame.render_widget(Paragraph::new(line).style(bar_style), area);

    // Show the real terminal cursor at the input position when collecting input.
    let cursor_col = match &app.ui.input_mode {
        InputMode::AddFeedUrl => {
            let prompt_chars = " New feed URL: ".chars().count();
            Some(prompt_chars + app.ui.input.cursor())
        }
        InputMode::AddFeedCategory { .. } => {
            let prompt_chars = " Category (empty = uncategorized): ".chars().count();
            Some(prompt_chars + app.ui.input.cursor())
        }
        InputMode::EditFeedName { .. } => {
            let prompt_chars = " Feed name: ".chars().count();
            Some(prompt_chars + app.ui.input.cursor())
        }
        _ => None,
    };
    if let Some(col) = cursor_col {
        let x = (area.x + col as u16).min(area.x + area.width.saturating_sub(1));
        frame.set_cursor_position((x, area.y));
    }
}

// ── Key hints line ────────────────────────────────────────────────────────────

/// Draw the permanent key-hints line (bottom row).
fn draw_hints_line(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let bar_style = t.style("ui.statusline");
    let key_style = t.style("ui.statusline.key");
    let keys = &app.config.keys;

    // Each entry: (key_text, description_text).
    // Sections are separated by a " ● " divider.
    let sep = " ● ";

    // Build combined key strings from config values.
    let refresh_key = format!("{}({})", keys.refresh_feed, keys.refresh_all);
    let feed_nav_key = format!("{}/{}", keys.feed_down, keys.feed_up);
    let article_nav_key = format!("{}/{}", pretty_key(&keys.article_up), pretty_key(&keys.article_down));
    let scroll_line_key = format!("{}/{}", pretty_key(&keys.scroll_line_up), pretty_key(&keys.scroll_line_down));
    let scroll_page_key = format!("{}/{}", pretty_key(&keys.scroll_up), pretty_key(&keys.scroll_down));

    let entries: &[Option<(&str, &str)>] = &[
        // App controls
        Some((&keys.quit,          "quit")),
        Some((&refresh_key,        "refresh(all)")),
        Some((&keys.reload_config, "reload config")),
        None, // section separator
        // Feed controls
        Some((&feed_nav_key,     "feeds")),
        Some((&keys.add_feed,    "add")),
        Some((&keys.delete_feed, "delete")),
        Some((&keys.edit_feed,   "edit name")),
        None, // section separator
        // Article navigation
        Some((&article_nav_key,   "articles")),
        Some((&keys.open_browser, "open")),
        Some((&scroll_line_key,   "line up/down")),
        Some((&scroll_page_key,   "page up/down")),
    ];

    // Build spans: key (bold+highlight) then ": " then description, space-separated.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut first_in_section = true;
    for entry in entries {
        match entry {
            None => {
                spans.push(Span::styled(sep.to_owned(), bar_style));
                first_in_section = true;
            }
            Some((key, desc)) => {
                if !first_in_section {
                    spans.push(Span::styled("  ".to_owned(), bar_style));
                }
                first_in_section = false;
                spans.extend(key_spans(key, key_style, bar_style));
                spans.push(Span::styled(": ".to_owned(), bar_style));
                spans.push(Span::styled(desc.to_string(), bar_style));
            }
        }
    }
    // Trailing space so the last entry isn't flush against the right edge.
    spans.push(Span::styled(" ".to_owned(), bar_style));

    // Right-align: measure total width then prepend padding.
    let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let area_width = area.width as usize;
    let pad = " ".repeat(area_width.saturating_sub(content_width));
    let mut padded = vec![Span::styled(pad, bar_style)];
    padded.extend(spans);

    frame.render_widget(
        Paragraph::new(Line::from(padded)).style(bar_style),
        area,
    );
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Split a compound key string (e.g. `"j/k"`, `"PgUp/PgDn"`, `"r(R)"`) into
/// spans, applying `key_style` to the individual key tokens and `bar_style` to
/// the punctuation separators (`/`, `(`, `)`).
fn key_spans(key: &str, key_style: Style, bar_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut token = String::new();
    for ch in key.chars() {
        if matches!(ch, '/' | '(' | ')') {
            if !token.is_empty() {
                spans.push(Span::styled(token.clone(), key_style));
                token.clear();
            }
            spans.push(Span::styled(ch.to_string(), bar_style));
        } else {
            token.push(ch);
        }
    }
    if !token.is_empty() {
        spans.push(Span::styled(token, key_style));
    }
    spans
}

/// Map a config key name to a compact display symbol where one exists.
fn pretty_key(key: &str) -> &str {
    match key {
        "Up"       => "↑",
        "Down"     => "↓",
        "Left"     => "←",
        "Right"    => "→",
        "PageUp"   => "PgUp",
        "PageDown" => "PgDn",
        "Home"     => "Home",
        "End"      => "End",
        "Enter"    => "↵",
        "Esc"      => "Esc",
        "Tab"      => "Tab",
        other      => other,
    }
}

/// Render a ratatui `Scrollbar` on the right border of `area`.
///
/// The thumb travels from `position = 0` to `position = max_scroll`
/// (`content_len - viewport`). The track is hidden so the single border
/// character shows through; only the thumb and ▲/▼ arrows are drawn.
/// Arrows are placed one row inside the corners so they don't overwrite them.
fn draw_scrollbar(
    frame: &mut Frame,
    area: Rect,
    content_len: usize,
    position: usize,
    viewport: usize,
    style: Style,
) {
    let max_scroll = content_len.saturating_sub(viewport);
    if max_scroll == 0 || area.height < 4 {
        return;
    }

    // content_length = max_scroll + 1 so that position == max_scroll puts
    // the thumb exactly at the bottom of the track.
    let mut state = ScrollbarState::new(max_scroll + 1)
        .position(position.min(max_scroll));

    // Inset the scrollbar area by one row top and bottom so the ▲/▼ arrows
    // appear just inside the panel corners rather than on them.
    let scrollbar_area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(2),
        ..area
    };

    // track_symbol(None): don't draw track characters so the border line
    // shows through, avoiding a double-line appearance.
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"))
        .track_symbol(None)
        .thumb_symbol("░")
        .thumb_style(style);

    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
}

/// Truncate a string to at most `max_chars` characters, appending `…` if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        // There were more characters — truncate and add ellipsis.
        let mut truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    } else {
        collected
    }
}
