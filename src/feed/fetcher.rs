use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use feed_rs::model::{Entry, Link, Person};
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
/// authoritative. Two quirks remain worth handling:
///
/// 1. For the RSS 2.0 `email@example.com (Real Name)` form, feed-rs derives the
///    name from the parenthetical but leaves the parentheses attached, so
///    `name` arrives as `"(Real Name)"`.
/// 2. A bare email address is not a usable display name and is rejected.
fn person_display_name(p: &Person) -> Option<String> {
    if let Some(name) = &p.name {
        let name = strip_duplicate_paren(strip_wrapping_parens(name));
        if !name.is_empty() && name != "author" && !name.contains('@') {
            return Some(name.to_owned());
        }
    }
    // Defensive fallback for malformed Atom/iTunes authors that put a bare
    // name in `email`. Well-formed RSS authors no longer land here.
    if let Some(raw) = &p.email {
        if !raw.contains('@') {
            let name = strip_duplicate_paren(strip_wrapping_parens(raw));
            if !name.is_empty() && name != "author" {
                return Some(name.to_owned());
            }
        }
    }
    None
}

/// Strip one layer of wrapping parentheses and surrounding whitespace:
/// `"(First Last)"` → `"First Last"`. Other input is returned trimmed.
///
/// The leading `(` is only removed when it is closed by the *final* `)`, so
/// `"(Reuters) (AP)"` is left intact rather than mangled into `"Reuters) (AP"`.
fn strip_wrapping_parens(s: &str) -> &str {
    let s = s.trim();
    let Some(inner) = s.strip_prefix('(') else {
        return s;
    };

    let mut depth = 1usize;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return if idx == inner.len() - 1 {
                        inner[..idx].trim()
                    } else {
                        s
                    };
                }
            }
            _ => {}
        }
    }

    // No matching close paren: leave the input alone.
    s
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
/// Parse a feed from raw bytes.
///
/// Bytes rather than a `String` are handed to the parser so that an
/// XML-declared encoding (e.g. ISO-8859-1) is honoured instead of being
/// flattened by an earlier UTF-8 decode.
///
/// Feeds with no declaration but invalid UTF-8 used to load with replacement
/// characters, so fall back to a lossy decode rather than rejecting them.
fn parse_feed(
    body: &[u8],
) -> std::result::Result<feed_rs::model::Feed, feed_rs::parser::ParseFeedError> {
    match parser::parse(body) {
        Ok(feed) => Ok(feed),
        Err(err) => {
            if std::str::from_utf8(body).is_err() {
                let lossy = String::from_utf8_lossy(body);
                parser::parse(lossy.as_bytes()).or(Err(err))
            } else {
                Err(err)
            }
        }
    }
}

/// Pick the link that points at the article itself.
///
/// feed-rs 3 records RSS `<comments>` and `<wfw:commentRss>` as ordinary links
/// on the entry, distinguished only by `Link::target`. Taking the first link
/// would open the comments anchor or the comments feed whenever `<comments>`
/// precedes `<link>`, so prefer an untargeted `alternate` link and only then
/// fall back to any untargeted link.
fn article_link(entry: &Entry) -> Option<String> {
    let untargeted = |l: &&Link| l.target.is_none();
    entry
        .links
        .iter()
        .find(|l| untargeted(l) && matches!(l.rel.as_deref(), None | Some("alternate")))
        .or_else(|| entry.links.iter().find(untargeted))
        .map(|l| l.href.clone())
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
        .read_to_vec()
        .map_err(|e| AppError::FeedFetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;

    let feed = parse_feed(&body).map_err(|e| AppError::FeedFetch {
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
        .read_to_vec()
        .map_err(|e| AppError::FeedFetch {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;

    // Read bytes, not a `String`: ureq 3's `read_to_string()` is only lossy for
    // `text/*`, so a non-UTF-8 feed served as `application/rss+xml` would fail
    // outright, and a `String` would discard the XML-declared encoding anyway.
    let feed = parse_feed(&body).map_err(|e| AppError::FeedFetch {
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

            let url = article_link(&entry);

            // Pick the first author that yields a usable display name.
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

    /// Parse a fixture the way the fetcher does and return the display name
    /// chosen for each entry.
    fn display_names(doc: &str) -> Vec<Option<String>> {
        let feed = parser::parse(doc.as_bytes()).expect("fixture should parse");
        feed.entries
            .iter()
            .map(|e| e.authors.iter().find_map(person_display_name))
            .collect()
    }

    fn urls(doc: &str) -> Vec<Option<String>> {
        let feed = parser::parse(doc.as_bytes()).expect("fixture should parse");
        feed.entries.iter().map(article_link).collect()
    }

    /// Feed-rs 3 turns `email (Name)` into `name = "(Name)"`; the parentheses
    /// must not reach the UI. `Name <email>` used to yield no author at all.
    #[test]
    fn author_names_from_real_rss_authors() {
        let rss = r#"<?xml version="1.0"?>
            <rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/"><channel><title>T</title>
              <item><title>a</title><guid>1</guid><author>user@example.com (First Last)</author></item>
              <item><title>b</title><guid>2</guid><author>First Last &lt;user@example.com&gt;</author></item>
              <item><title>c</title><guid>3</guid><author>First Last</author></item>
              <item><title>d</title><guid>4</guid><author>user@example.com</author></item>
              <item><title>e</title><guid>5</guid><dc:creator>Dee See</dc:creator></item>
              <item><title>f</title><guid>6</guid><author>Bob (Bob)</author></item>
            </channel></rss>"#;

        assert_eq!(
            display_names(rss),
            vec![
                Some("First Last".to_owned()),
                Some("First Last".to_owned()),
                Some("First Last".to_owned()),
                None, // a bare address is not a name
                Some("Dee See".to_owned()),
                Some("Bob".to_owned()),
            ]
        );
    }

    /// Atom separates name and email, so nothing needs repairing.
    #[test]
    fn author_names_from_atom() {
        let atom = r#"<?xml version="1.0"?>
            <feed xmlns="http://www.w3.org/2005/Atom"><title>A</title><id>x</id>
              <updated>2026-01-01T00:00:00Z</updated>
              <entry><id>g</id><title>g</title><updated>2026-01-01T00:00:00Z</updated>
                <author><name>Atom Author</name><email>a@example.com</email></author></entry>
              <entry><id>h</id><title>h</title><updated>2026-01-01T00:00:00Z</updated>
                <author><name>No Email</name></author></entry>
            </feed>"#;

        assert_eq!(
            display_names(atom),
            vec![Some("Atom Author".to_owned()), Some("No Email".to_owned())]
        );
    }

    /// feed-rs 3 records `<comments>`/`<wfw:commentRss>` as entry links, so the
    /// article URL must not simply be the first one.
    #[test]
    fn article_url_skips_comment_links() {
        let rss = r#"<?xml version="1.0"?>
            <rss version="2.0" xmlns:wfw="http://wellformedweb.org/CommentAPI/"><channel><title>T</title>
              <item><title>a</title><guid>1</guid>
                <comments>https://example.com/story#comments</comments>
                <link>https://example.com/story</link></item>
              <item><title>b</title><guid>2</guid>
                <link>https://example.com/other</link>
                <comments>https://example.com/other#comments</comments></item>
              <item><title>c</title><guid>3</guid>
                <comments>https://example.com/only-comments</comments></item>
            </channel></rss>"#;

        assert_eq!(
            urls(rss),
            vec![
                Some("https://example.com/story".to_owned()),
                Some("https://example.com/other".to_owned()),
                None, // comments-only entry has no article URL
            ]
        );
    }

    #[test]
    fn strip_wrapping_parens_handles_bare_and_wrapped() {
        assert_eq!(strip_wrapping_parens("(First Last)"), "First Last");
        assert_eq!(strip_wrapping_parens("  (First Last)  "), "First Last");
        assert_eq!(strip_wrapping_parens("First Last"), "First Last");
        assert_eq!(strip_wrapping_parens("(First Last"), "(First Last");
        assert_eq!(strip_wrapping_parens("First Last)"), "First Last)");
        assert_eq!(strip_wrapping_parens(""), "");
        // Multiple groups must not be mangled into "Reuters) (AP".
        assert_eq!(strip_wrapping_parens("(Reuters) (AP)"), "(Reuters) (AP)");
        assert_eq!(
            strip_wrapping_parens("(Alice) and (Bob)"),
            "(Alice) and (Bob)"
        );
        assert_eq!(strip_wrapping_parens("((Nested))"), "(Nested)");
    }
}
