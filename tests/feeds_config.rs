//! Integration tests for `FeedsFile` parsing, layout generation, and
//! file-level add/remove operations.

use std::io::Write as _;

use tempfile::{NamedTempFile, TempDir};
use thrssty::feeds_config::{FeedsFile, add_feed_to_file, remove_feed_from_file};

// ── Missing / empty file ──────────────────────────────────────────────────────

#[test]
fn load_missing_file_returns_empty() {
    let absent = std::path::Path::new("/tmp/thrssty_feeds_no_such_file.toml");
    let f = FeedsFile::load(absent).unwrap();
    assert!(f.categories.is_empty());
    assert!(f.uncategorized.is_empty());
}

#[test]
fn load_empty_file_returns_empty() {
    let f = NamedTempFile::new().unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    assert!(feeds.categories.is_empty());
    assert!(feeds.uncategorized.is_empty());
}

// ── Flat feed list ────────────────────────────────────────────────────────────

const FLAT: &str = r#"
[[feed]]
url  = "https://blog.rust-lang.org/feed.xml"
name = "Rust Blog"

[[feed]]
url = "https://this-week-in-rust.org/rss.xml"
"#;

#[test]
fn flat_feed_list_parsed_correctly() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{FLAT}").unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    assert!(feeds.categories.is_empty());
    assert_eq!(feeds.uncategorized.len(), 2);
    assert_eq!(feeds.uncategorized[0].url, "https://blog.rust-lang.org/feed.xml");
    assert_eq!(feeds.uncategorized[0].name.as_deref(), Some("Rust Blog"));
    assert_eq!(feeds.uncategorized[1].name, None); // no name override
}

#[test]
fn flat_layout_has_no_categories() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{FLAT}").unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    let (feed_configs, layout) = feeds.into_layout("No category");
    assert_eq!(feed_configs.len(), 2);
    assert!(!layout.has_categories);
    assert_eq!(layout.feed_order, vec![0, 1]);
}

// ── Categorised feed list ─────────────────────────────────────────────────────

const CATEGORISED: &str = r#"
[category.1]
title = "Rust"

[[category.1.feed]]
url  = "https://blog.rust-lang.org/feed.xml"
name = "Rust Blog"

[[category.1.feed]]
url = "https://this-week-in-rust.org/rss.xml"
name = "TWIR"

[category.2]
title = "Linux"

[[category.2.feed]]
url = "https://lwn.net/headlines/rss"
"#;

#[test]
fn categorised_feed_list_parsed_correctly() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{CATEGORISED}").unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    assert_eq!(feeds.categories.len(), 2);
    assert_eq!(feeds.categories[0].title, "Rust");
    assert_eq!(feeds.categories[0].feeds.len(), 2);
    assert_eq!(feeds.categories[1].title, "Linux");
    assert_eq!(feeds.categories[1].feeds.len(), 1);
    assert!(feeds.uncategorized.is_empty());
}

#[test]
fn categorised_layout_has_categories_flag_set() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{CATEGORISED}").unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    let (feed_configs, layout) = feeds.into_layout("No category");
    assert_eq!(feed_configs.len(), 3);
    assert!(layout.has_categories);
}

#[test]
fn mixed_layout_places_uncategorized_at_end() {
    let mixed = r#"
[category.1]
title = "Tech"

[[category.1.feed]]
url = "https://example.com/feed1"

[[feed]]
url = "https://example.com/feed2"
"#;
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "{mixed}").unwrap();
    let feeds = FeedsFile::load(f.path()).unwrap();
    let (feed_configs, _layout) = feeds.into_layout("No category");
    // feed2 (uncategorized) should appear after feed1 (categorized).
    assert_eq!(feed_configs[0].url, "https://example.com/feed1");
    assert_eq!(feed_configs[1].url, "https://example.com/feed2");
}

// ── add_feed_to_file ──────────────────────────────────────────────────────────

#[test]
fn add_feed_creates_file_when_absent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("feeds.toml");
    assert!(!path.exists());

    add_feed_to_file(&path, "https://example.com/feed.xml", None, "").unwrap();

    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("https://example.com/feed.xml"));
}

#[test]
fn add_feed_with_name_writes_name_field() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("feeds.toml");

    add_feed_to_file(&path, "https://example.com/feed.xml", Some("Example"), "").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Example"));
}

#[test]
fn add_feed_appends_to_existing_file() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "[[feed]]\nurl = \"https://first.com/feed\"").unwrap();
    let path = f.path().to_path_buf();

    add_feed_to_file(&path, "https://second.com/feed", None, "").unwrap();

    let feeds = FeedsFile::load(&path).unwrap();
    assert_eq!(feeds.uncategorized.len(), 2);
}

// ── remove_feed_from_file ─────────────────────────────────────────────────────

#[test]
fn remove_feed_eliminates_the_entry() {
    let mut f = NamedTempFile::new().unwrap();
    write!(
        f,
        "[[feed]]\nurl = \"https://keep.com/feed\"\n\n\
         [[feed]]\nurl = \"https://remove.com/feed\"\n"
    )
    .unwrap();
    let path = f.path().to_path_buf();

    remove_feed_from_file(&path, "https://remove.com/feed").unwrap();

    let feeds = FeedsFile::load(&path).unwrap();
    assert_eq!(feeds.uncategorized.len(), 1);
    assert_eq!(feeds.uncategorized[0].url, "https://keep.com/feed");
}

#[test]
fn remove_nonexistent_feed_is_a_no_op() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "[[feed]]\nurl = \"https://keep.com/feed\"").unwrap();
    let path = f.path().to_path_buf();

    remove_feed_from_file(&path, "https://nothere.com/feed").unwrap();

    let feeds = FeedsFile::load(&path).unwrap();
    assert_eq!(feeds.uncategorized.len(), 1);
}
