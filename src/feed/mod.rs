use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod fetcher;

/// A single article from an RSS or Atom feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Article {
    /// Unique identifier from the feed (entry id / guid).
    pub id: String,
    /// Article title.
    pub title: String,
    /// Primary URL (link to the full article).
    pub url: Option<String>,
    /// Author name (first human-readable author; skips email-only entries).
    pub author: Option<String>,
    /// Publication or update timestamp.
    pub published: Option<DateTime<Utc>>,
    /// Short description / abstract (RSS `<description>` / Atom `<summary>`).
    pub description: Option<String>,
    /// Categories / tags attached to this entry.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Body content as HTML (preferred) or plain text.
    pub content: String,
}

/// An RSS/Atom feed with its fetched articles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    /// Feed source URL.
    pub url: String,
    /// Display name (from config override, or parsed from the feed itself).
    pub name: String,
    /// Articles, sorted newest-first.
    pub articles: Vec<Article>,
    /// When this feed was last successfully fetched.
    pub last_fetched: Option<DateTime<Utc>>,
}

impl Feed {
    /// Create an empty feed (articles not yet loaded).
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            name: name.into(),
            articles: Vec::new(),
            last_fetched: None,
        }
    }
}
