use std::sync::{mpsc, OnceLock};
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

/// Shared HTTP agent with the global fetch timeout applied.
///
/// Reusing a single agent lets ureq pool connections across feeds and refreshes
/// instead of opening a new pool for every request.
fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(FETCH_TIMEOUT))
            .build()
            .into()
    })
}

/// Messages sent from the background fetch thread to the UI thread.
#[derive(Debug)]
pub enum FetchMessage {
    /// A fetch has started for this URL.
    Started { url: String },
    /// Articles were successfully fetched.
    Complete {
        url: String,
        feed_title: Option<String>,
        articles: Vec<Article>,
    },
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
/// feed-rs 3 splits the raw author text into `name` and `email`, so `name` is
/// now authoritative. Two quirks remain worth handling:
///
/// 1. For the RSS 2.0 `email@example.com (Real Name)` form, feed-rs derives the
///    name from the parenthetical but leaves the parentheses attached, so
///    `name` arrives as `"(Real Name)"`.
/// 2. Older feeds can still deliver a bare name in the author field, which
///    feed-rs reports as an `email` without an `@`.
fn person_display_name(p: &Person) -> Option<String> {
    if let Some(name) = &p.name {
        let name = strip_duplicate_paren(strip_wrapping_parens(name));
        if !name.is_empty() && name != "author" && !name.contains('@') {
            return Some(name.to_owned());
        }
    }
    // Fallback: a bare name delivered as the author "email".
    if let Some(raw) = &p.email {
        if !raw.contains('@') {
            let name = strip_duplicate_paren(strip_wrapping_parens(raw));
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    None
}

/// Strip one layer of wrapping parentheses and surrounding whitespace:
/// `"(First Last)"` → `"First Last"`. Other input is returned trimmed.
fn strip_wrapping_parens(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(s)
        .trim()
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
    let mut response = http_agent()
        .get(url)
        .call()
        .map_err(|e| AppError::FeedFetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| AppError::FeedFetch {
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
    let mut response = http_agent()
        .get(url)
        .call()
        .map_err(|e| AppError::FeedFetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| AppError::FeedFetch {
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
            let author = entry.authors.iter().find_map(person_display_name);

            let categories: Vec<String> = entry.categories.iter().map(|c| c.term.clone()).collect();

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
    let mut seen_title_date: std::collections::HashSet<(
        String,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = std::collections::HashSet::new();
    articles.retain(|a| seen_title_date.insert((a.title.clone(), a.published)));

    // Sort newest-first.
    articles.sort_by_key(|a| std::cmp::Reverse(a.published));

    Ok((feed_title, articles))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(name: Option<&str>, email: Option<&str>) -> Person {
        Person {
            name: name.map(str::to_owned),
            uri: None,
            email: email.map(str::to_owned),
            role: None,
        }
    }

    /// What feed-rs 3 produces for each RSS `<author>` shape, as observed from
    /// the real parser, mapped to the display name we want to show.
    #[test]
    fn author_display_name_from_parsed_people() {
        let cases = [
            // `email (Name)`: feed-rs leaves the parentheses attached.
            (
                person(Some("(First Last)"), Some("user@example.com")),
                Some("First Last"),
            ),
            // `Name <email>`: both fields populated cleanly.
            (
                person(Some("First Last"), Some("user@example.com")),
                Some("First Last"),
            ),
            // Plain name.
            (person(Some("First Last"), None), Some("First Last")),
            // Bare email is not a display name.
            (person(None, Some("user@example.com")), None),
            // dc:creator.
            (person(Some("Dee See"), None), Some("Dee See")),
            // Duplicated parenthetical is collapsed.
            (person(Some("Bob (Bob)"), None), Some("Bob")),
            // Bare name delivered as the author "email".
            (
                person(Some("author"), Some("Legacy Name")),
                Some("Legacy Name"),
            ),
            // The RSS 2.0 placeholder with nothing usable.
            (person(Some("author"), None), None),
            // Whitespace-only name falls through.
            (person(Some("   "), None), None),
            // A name that is only an email address is rejected.
            (person(Some("user@example.com"), None), None),
        ];

        for (p, expected) in cases {
            assert_eq!(
                person_display_name(&p).as_deref(),
                expected,
                "unexpected display name for {p:?}"
            );
        }
    }

    #[test]
    fn strip_wrapping_parens_handles_bare_and_wrapped() {
        assert_eq!(strip_wrapping_parens("(First Last)"), "First Last");
        assert_eq!(strip_wrapping_parens("  (First Last)  "), "First Last");
        assert_eq!(strip_wrapping_parens("First Last"), "First Last");
        assert_eq!(strip_wrapping_parens("(First Last"), "(First Last");
        assert_eq!(strip_wrapping_parens("First Last)"), "First Last)");
        assert_eq!(strip_wrapping_parens(""), "");
    }
}
