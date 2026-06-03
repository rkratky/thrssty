use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use feed_rs::model::Person;
use feed_rs::parser;

use crate::error::AppError;
use crate::feed::Article;
use crate::html::decode_html_entities;

/// Timeout for all outbound HTTP requests (both validation probes and feed fetches).
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Messages sent from the background fetch thread to the UI thread.
#[derive(Debug)]
pub enum FetchMessage {
    /// A fetch has started for this URL.
    Started { url: String },
    /// Articles were successfully fetched.
    Complete { url: String, feed_title: Option<String>, articles: Vec<Article> },
    /// The fetch failed.
    Error { url: String, reason: String },
    /// All requested URLs have been processed.
    Done,
}

/// Spawn a background thread that fetches each URL in order and sends
/// `FetchMessage` values back through `tx`.
pub fn spawn_fetch(urls: Vec<String>, tx: mpsc::Sender<FetchMessage>) {
    thread::spawn(move || {
        for url in &urls {
            let _ = tx.send(FetchMessage::Started { url: url.clone() });
            match fetch_feed(url) {
                Ok(articles) => {
                    let _ = tx.send(FetchMessage::Complete {
                        url: url.clone(),
                        feed_title: articles.0,
                        articles: articles.1,
                    });
                }
                Err(AppError::FeedFetch { reason, .. }) => {
                    let _ = tx.send(FetchMessage::Error {
                        url: url.clone(),
                        reason,
                    });
                }
                Err(e) => {
                    let _ = tx.send(FetchMessage::Error {
                        url: url.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }
        let _ = tx.send(FetchMessage::Done);
    });
}

/// Extract a human-readable display name from a feed-rs `Person`.
///
/// Feed-rs RSS 2.0 stores the raw `<author>` text in `person.email` (because
/// that element is specified as an email field in RSS 2.0) and uses the XML
/// element name as the default for `person.name` (always `"author"`).
///
/// Resolution order:
/// 1. If `email` exists and contains `@`: look for a `(Name)` suffix and use it.
/// 2. If `email` exists and has no `@`: it's a plain name or "Name (Name)" duplicate
///    — strip the duplicate parenthetical and use the result.
/// 3. Otherwise fall back to `person.name` if it is not the placeholder `"author"`.
fn person_display_name(p: &Person) -> Option<String> {
    if let Some(raw) = &p.email {
        if raw.contains('@') {
            // Standard `email@example.com (Real Name)` format.
            if let Some(name) = extract_paren_suffix(raw) {
                return Some(name.to_owned());
            }
            // Bare email address — not useful as a display name.
        } else {
            // Plain name, possibly with a duplicate "(Name)" suffix.
            let name = strip_duplicate_paren(raw);
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    // Fall back to `person.name` if it is a real name (not the RSS 2.0 default).
    if !p.name.is_empty() && p.name != "author" && !p.name.contains('@') {
        return Some(p.name.clone());
    }
    None
}

/// If `s` ends with ` (something)`, return `something`.
fn extract_paren_suffix(s: &str) -> Option<&str> {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") {
            return Some(&s[pos + 2..s.len() - 1]);
        }
    }
    None
}

/// If `s` is `"Name (Name)"` (exact duplicate), return `"Name"`, else return `s` unchanged.
fn strip_duplicate_paren(s: &str) -> &str {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") {
            let before = &s[..pos];
            let inside = &s[pos + 2..s.len() - 1];
            if before == inside {
                return before;
            }
        }
    }
    s
}
/// Fetch a feed URL to validate it and obtain its title.
///
/// Used synchronously when the user adds a new feed to verify the URL is
/// reachable and points to a valid feed before writing `feeds.toml`.
///
/// Returns `Ok(Some(title))` on success with a title, `Ok(None)` when the
/// feed parses but has no title, or an error on network / parse failure.
pub fn probe_feed(url: &str) -> crate::error::Result<Option<String>> {
    let response = ureq::get(url)
        .timeout(FETCH_TIMEOUT)
        .call()
        .map_err(|e| AppError::FeedFetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;

    let body = response.into_string().map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;

    let feed = parser::parse(body.as_bytes()).map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: format!("not a valid feed: {e}"),
    })?;

    Ok(feed.title.map(|t| t.content))
}

fn fetch_feed(url: &str) -> crate::error::Result<(Option<String>, Vec<Article>)> {
    let response = ureq::get(url)
        .timeout(FETCH_TIMEOUT)
        .call()
        .map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;

    let body = response.into_string().map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;

    let feed = parser::parse(body.as_bytes()).map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;

    // Capture title before consuming feed into entries iterator.
    let feed_title = feed
        .title
        .as_ref()
        .map(|t| decode_html_entities(&t.content));

    let mut articles: Vec<Article> = feed
        .entries
        .into_iter()
        .map(|entry| {
            // Prefer content body; do NOT fall back to summary here —
            // summary is stored separately in `description` and displayed independently.
            let description = entry.summary.as_ref().map(|s| s.content.clone());
            let content = entry
                .content
                .as_ref()
                .and_then(|c| c.body.as_deref())
                .unwrap_or("")
                .to_owned();

            // If the feed populates both fields with the same text (common in
            // RSS 2.0 feeds that mirror <description> into <content:encoded>),
            // drop the description so we don't display the same content twice.
            let description = description.filter(|desc| {
                let norm = |s: &str| s.split_whitespace().collect::<String>();
                norm(desc) != norm(&content)
            });

            let published = entry
                .published
                .or(entry.updated)
                .map(|dt| dt.with_timezone(&Utc));

            let url = entry.links.into_iter().next().map(|l| l.href);

            // Pick the first author that yields a usable display name.
            // `person_display_name` handles the RSS 2.0 quirk where the raw
            // <author> text ends up in `person.email`, not `person.name`.
            let author = entry
                .authors
                .iter()
                .find_map(person_display_name);

            let categories: Vec<String> =
                entry.categories.iter().map(|c| c.term.clone()).collect();

            Article {
                id: entry.id,
                title: entry
                    .title
                    .map(|t| decode_html_entities(&t.content))
                    .unwrap_or_else(|| "Untitled".into()),
                url,
                author,
                published,
                description,
                categories,
                content,
            }
        })
        .collect();

    // Deduplicate by ID: some feeds (e.g. those without <guid>) produce
    // colliding IDs for distinct-looking entries when feed-rs falls back to
    // hash(link + title).  Keep the first occurrence; duplicates would cause
    // marking one article read to silently mark its twin read too.
    let mut seen_ids = std::collections::HashSet::new();
    articles.retain(|a| seen_ids.insert(a.id.clone()));

    // Secondary dedup by (title, date): some sources (e.g. BBC News) republish
    // the same story with a new GUID after minor edits, resulting in two entries
    // with identical titles and publication timestamps.  Keep the first
    // (typically the original); the replacement entry has less reader value since
    // the story was already seen.
    let mut seen_title_date: std::collections::HashSet<(String, Option<chrono::DateTime<chrono::Utc>>)> = std::collections::HashSet::new();
    articles.retain(|a| seen_title_date.insert((a.title.clone(), a.published)));

    // Sort newest-first.
    articles.sort_by(|a, b| b.published.cmp(&a.published));

    Ok((feed_title, articles))
}
