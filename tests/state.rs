//! Integration tests for `Cache` — read/unread tracking, unread counts,
//! cache serialisation round-trip, and article purging.

use chrono::{DateTime, Duration, Utc};

use thrssty::feed::{Article, Feed};
use thrssty::state::Cache;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_article(id: &str, days_ago: Option<i64>) -> Article {
    Article {
        id: id.to_owned(),
        title: format!("Article {id}"),
        url: None,
        author: None,
        published: days_ago.map(|d| Utc::now() - Duration::days(d)),
        description: None,
        categories: vec![],
        content: String::new(),
    }
}

fn make_feed(url: &str, articles: Vec<Article>) -> Feed {
    Feed {
        url: url.to_owned(),
        name: "Test Feed".to_owned(),
        articles,
        last_fetched: None,
    }
}

// ── mark_read / is_read ───────────────────────────────────────────────────────

#[test]
fn is_read_returns_false_for_fresh_cache() {
    let cache = Cache::default();
    assert!(!cache.is_read("https://example.com/feed", "art-1"));
}

#[test]
fn mark_read_makes_is_read_true() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    cache.mark_read(url, "art-1");
    assert!(cache.is_read(url, "art-1"));
}

#[test]
fn is_read_is_false_for_different_feed_url() {
    let mut cache = Cache::default();
    cache.mark_read("https://feed-a.example.com/", "art-1");
    assert!(!cache.is_read("https://feed-b.example.com/", "art-1"));
}

#[test]
fn is_read_is_false_for_different_article_id() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    cache.mark_read(url, "art-1");
    assert!(!cache.is_read(url, "art-2"));
}

#[test]
fn mark_read_is_idempotent() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    cache.mark_read(url, "art-1");
    cache.mark_read(url, "art-1"); // second call should not panic or duplicate
    assert!(cache.is_read(url, "art-1"));
}

#[test]
fn mark_read_multiple_articles_same_feed() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    cache.mark_read(url, "a");
    cache.mark_read(url, "b");
    assert!(cache.is_read(url, "a"));
    assert!(cache.is_read(url, "b"));
    assert!(!cache.is_read(url, "c"));
}

// ── unread_count ──────────────────────────────────────────────────────────────

#[test]
fn unread_count_all_unread() {
    let cache = Cache::default();
    let url = "https://example.com/feed";
    let articles = vec![
        make_article("a", None),
        make_article("b", None),
        make_article("c", None),
    ];
    assert_eq!(cache.unread_count(url, &articles), 3);
}

#[test]
fn unread_count_some_read() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    cache.mark_read(url, "a");
    let articles = vec![
        make_article("a", None),
        make_article("b", None),
        make_article("c", None),
    ];
    assert_eq!(cache.unread_count(url, &articles), 2);
}

#[test]
fn unread_count_all_read() {
    let mut cache = Cache::default();
    let url = "https://example.com/feed";
    let articles = vec![make_article("a", None), make_article("b", None)];
    for a in &articles {
        cache.mark_read(url, &a.id);
    }
    assert_eq!(cache.unread_count(url, &articles), 0);
}

// ── purge_old_articles ────────────────────────────────────────────────────────

#[test]
fn purge_max_age_zero_is_no_op() {
    let old = make_article("old", Some(1000));
    let mut cache = Cache {
        feeds: vec![make_feed("https://f.example.com/", vec![old.clone()])],
        ..Default::default()
    };
    cache.purge_old_articles(0);
    assert_eq!(cache.feeds[0].articles.len(), 1, "purge with 0 should not remove anything");
}

#[test]
fn purge_removes_articles_older_than_max_age() {
    let recent = make_article("new", Some(5));   // 5 days old
    let stale  = make_article("old", Some(100)); // 100 days old
    let mut cache = Cache {
        feeds: vec![make_feed("https://f.example.com/", vec![recent.clone(), stale.clone()])],
        ..Default::default()
    };
    cache.purge_old_articles(30); // keep articles younger than 30 days
    let ids: Vec<_> = cache.feeds[0].articles.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"new"),  "recent article should be kept");
    assert!(!ids.contains(&"old"), "stale article should be removed");
}

#[test]
fn purge_keeps_all_if_all_would_be_removed() {
    // When purging would leave an empty feed, the feed's articles are preserved
    // so the user sees something until the next refresh.
    let stale = make_article("old", Some(200));
    let mut cache = Cache {
        feeds: vec![make_feed("https://f.example.com/", vec![stale])],
        ..Default::default()
    };
    cache.purge_old_articles(30);
    assert_eq!(cache.feeds[0].articles.len(), 1, "should preserve last article to avoid blank feed");
}

#[test]
fn purge_preserves_articles_without_published_date() {
    let no_date = make_article("nodateart", None); // published = None
    let mut cache = Cache {
        feeds: vec![make_feed("https://f.example.com/", vec![no_date])],
        ..Default::default()
    };
    cache.purge_old_articles(30);
    assert_eq!(cache.feeds[0].articles.len(), 1, "undated articles should survive purge");
}

#[test]
fn purge_cleans_stale_read_ids() {
    let url = "https://f.example.com/";
    let recent = make_article("new", Some(5));
    let stale  = make_article("old", Some(100));
    let mut cache = Cache {
        feeds: vec![make_feed(url, vec![recent.clone(), stale.clone()])],
        ..Default::default()
    };
    cache.mark_read(url, "new");
    cache.mark_read(url, "old");
    cache.purge_old_articles(30);
    // "old" article is removed; its read-ID should also be gone.
    assert!(!cache.is_read(url, "old"), "stale read-ID should be purged");
    assert!(cache.is_read(url, "new"),  "recent read-ID should be kept");
}

#[test]
fn purge_handles_multiple_feeds_independently() {
    let url_a = "https://a.example.com/";
    let url_b = "https://b.example.com/";
    let old_a = make_article("old-a", Some(100));
    let new_b = make_article("new-b", Some(5));
    let mut cache = Cache {
        feeds: vec![
            make_feed(url_a, vec![old_a.clone(), make_article("new-a", Some(5))]),
            make_feed(url_b, vec![new_b.clone()]),
        ],
        ..Default::default()
    };
    cache.purge_old_articles(30);
    let ids_a: Vec<_> = cache.feeds[0].articles.iter().map(|a| a.id.as_str()).collect();
    let ids_b: Vec<_> = cache.feeds[1].articles.iter().map(|a| a.id.as_str()).collect();
    assert!(!ids_a.contains(&"old-a"), "stale article in feed A should be removed");
    assert!(ids_a.contains(&"new-a"),  "recent article in feed A should remain");
    assert_eq!(ids_b, vec!["new-b"],   "feed B should be untouched");
}

// ── cache serialisation round-trip ────────────────────────────────────────────

#[test]
fn cache_json_roundtrip_preserves_read_state() {
    let url = "https://rt.example.com/";
    let mut original = Cache {
        feeds: vec![make_feed(url, vec![make_article("a", None)])],
        ..Default::default()
    };
    original.mark_read(url, "a");

    let json = serde_json::to_string(&original).unwrap();
    let restored: Cache = serde_json::from_str(&json).unwrap();

    assert!(restored.is_read(url, "a"), "read state should survive JSON round-trip");
    assert!(!restored.is_read(url, "b"), "unread state should also survive");
}

#[test]
fn cache_article_dates_survive_roundtrip() {
    let published: DateTime<Utc> = "2024-01-15T12:00:00Z".parse().unwrap();
    let mut article = make_article("dated", None);
    article.published = Some(published);

    let original = Cache {
        feeds: vec![make_feed("https://rt.example.com/", vec![article])],
        ..Default::default()
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: Cache = serde_json::from_str(&json).unwrap();

    assert_eq!(
        restored.feeds[0].articles[0].published,
        Some(published),
        "publication date should survive JSON round-trip"
    );
}
