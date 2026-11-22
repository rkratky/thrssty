use std::path::Path;

use crate::error::{AppError, Result};

/// A single configured feed entry.
#[derive(Debug, Clone)]
pub struct FeedConfig {
    pub url: String,
    /// Optional display-name override; feed's own title is used when absent.
    pub name: Option<String>,
}

/// A category from feeds.toml with its associated feeds.
#[derive(Debug, Clone)]
pub struct CategoryConfig {
    pub title: String,
    pub feeds: Vec<FeedConfig>,
}

/// Parsed representation of a feeds.toml file.
#[derive(Debug, Default)]
pub struct FeedsFile {
    /// Categories in document order (sorted numerically by key when keys are integers).
    pub categories: Vec<CategoryConfig>,
    /// Feeds declared with `[[feed]]` (not under any category).
    pub uncategorized: Vec<FeedConfig>,
}

// ── Sidebar layout ────────────────────────────────────────────────────────────

/// A single displayable row in the feed sidebar.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    /// A category header row (not selectable by the cursor).
    Category {
        title: String,
        /// Indices into `App::feeds` for computing aggregate article counts.
        feed_indices: Vec<usize>,
    },
    /// A feed row.
    Feed {
        /// Index into `App::feeds`.
        feed_index: usize,
        /// True when any category exists; controls the indent prefix.
        indented: bool,
    },
}

/// Pre-computed sidebar display structure derived from `FeedsFile`.
#[derive(Debug, Default)]
pub struct SidebarLayout {
    /// All sidebar rows (category headers + feed entries) in display order.
    pub items: Vec<SidebarItem>,
    /// True when the feeds file defines at least one `[category.N]` section.
    pub has_categories: bool,
    /// Feed indices in display order; used for j/k cursor navigation.
    /// The cursor (`UiState::selected_feed`) is always an index into `App::feeds`,
    /// and this vec maps display positions to those indices while skipping categories.
    pub feed_order: Vec<usize>,
}

// ── FeedsFile ─────────────────────────────────────────────────────────────────

impl FeedsFile {
    /// Load and parse a `feeds.toml` file.
    ///
    /// Returns an empty `FeedsFile` (no categories, no feeds) when the path
    /// does not exist; this lets thrssty start without a feeds file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        parse_feeds_content(&content).map_err(|reason| AppError::FeedsFileParse {
            file: path.display().to_string(),
            reason,
        })
    }

    /// Consume `self` and produce a flat feed list + sidebar layout.
    ///
    /// Feed ordering in the returned `Vec<FeedConfig>`:
    /// 1. Feeds under each category in document order.
    /// 2. Uncategorized feeds at the end (under a "No category" header when
    ///    any categories are defined).
    ///
    /// `uncategorized_label` is the display title of the auto-generated
    /// "No category" category.
    pub fn into_layout(self, uncategorized_label: &str) -> (Vec<FeedConfig>, SidebarLayout) {
        let has_categories = !self.categories.is_empty();
        let mut all_feeds: Vec<FeedConfig> = Vec::new();
        let mut items: Vec<SidebarItem> = Vec::new();
        let mut feed_order: Vec<usize> = Vec::new();

        if has_categories {
            // ── Categorised feeds ────────────────────────────────────────────
            for cat in self.categories {
                // Collect feed indices for this category before pushing items
                // so the Category header can reference them in one shot.
                let mut cat_feed_indices: Vec<usize> = Vec::new();
                for feed in &cat.feeds {
                    let _ = feed; // borrow check: we only count here
                    cat_feed_indices.push(all_feeds.len() + cat_feed_indices.len());
                }

                items.push(SidebarItem::Category {
                    title: cat.title,
                    feed_indices: cat_feed_indices.clone(),
                });

                for feed in cat.feeds {
                    let idx = all_feeds.len();
                    feed_order.push(idx);
                    items.push(SidebarItem::Feed {
                        feed_index: idx,
                        indented: true,
                    });
                    all_feeds.push(feed);
                }
            }

            // ── Uncategorised feeds → "No category" ─────────────────────────
            if !self.uncategorized.is_empty() {
                let mut cat_feed_indices: Vec<usize> = Vec::new();

                for feed in &self.uncategorized {
                    let _ = feed;
                    cat_feed_indices.push(all_feeds.len() + cat_feed_indices.len());
                }

                items.push(SidebarItem::Category {
                    title: uncategorized_label.to_owned(),
                    feed_indices: cat_feed_indices,
                });

                for feed in self.uncategorized {
                    let idx = all_feeds.len();
                    feed_order.push(idx);
                    items.push(SidebarItem::Feed {
                        feed_index: idx,
                        indented: true,
                    });
                    all_feeds.push(feed);
                }
            }
        } else {
            // ── Flat list (no categories) ────────────────────────────────────
            for feed in self.uncategorized {
                let idx = all_feeds.len();
                feed_order.push(idx);
                items.push(SidebarItem::Feed {
                    feed_index: idx,
                    indented: false,
                });
                all_feeds.push(feed);
            }
        }

        let layout = SidebarLayout {
            items,
            has_categories,
            feed_order,
        };
        (all_feeds, layout)
    }
}

// ── Feed file modification ────────────────────────────────────────────────────

/// Add a new feed entry to a `feeds.toml` file.
///
/// If `category` is non-empty, the feed is placed under an existing category
/// with that title (case-sensitive), or a new category is created.  
/// If `category` is empty, the feed is added to the uncategorized section.
pub fn add_feed_to_file(
    path: &Path,
    url: &str,
    feed_title: Option<&str>,
    category: &str,
) -> Result<()> {
    let mut file = if path.exists() {
        FeedsFile::load(path)?
    } else {
        FeedsFile::default()
    };

    let new_feed = FeedConfig {
        url: url.to_owned(),
        name: feed_title.map(|s| s.to_owned()),
    };

    if category.is_empty() {
        file.uncategorized.push(new_feed);
    } else if let Some(cat) = file.categories.iter_mut().find(|c| c.title == category) {
        cat.feeds.push(new_feed);
    } else {
        file.categories.push(CategoryConfig {
            title: category.to_owned(),
            feeds: vec![new_feed],
        });
    }

    write_feeds_file(path, &file)
}

/// Remove a feed from `feeds.toml` by URL.
///
/// The feed is searched in all categories and in the uncategorized section.
/// Categories that become empty after removal are deleted automatically.
pub fn remove_feed_from_file(path: &Path, url: &str) -> Result<()> {
    let mut file = FeedsFile::load(path)?;

    // Remove from categories.
    for cat in &mut file.categories {
        cat.feeds.retain(|f| f.url != url);
    }
    // Drop categories that are now empty.
    file.categories.retain(|c| !c.feeds.is_empty());

    // Remove from uncategorized.
    file.uncategorized.retain(|f| f.url != url);

    write_feeds_file(path, &file)
}

/// Rename a feed in `feeds.toml`.
///
/// If `new_name` is `None` or an empty string the `name` field is cleared
/// (the app will fall back to the title returned by the feed on the next
/// fetch).  The feed is identified by its URL.
pub fn rename_feed_in_file(path: &Path, url: &str, new_name: Option<&str>) -> Result<()> {
    let mut file = FeedsFile::load(path)?;

    let name = new_name.filter(|s| !s.is_empty()).map(str::to_owned);

    let mut found = false;
    for cat in &mut file.categories {
        for feed in &mut cat.feeds {
            if feed.url == url {
                feed.name = name.clone();
                found = true;
            }
        }
    }
    for feed in &mut file.uncategorized {
        if feed.url == url {
            feed.name = name.clone();
            found = true;
        }
    }

    if found {
        write_feeds_file(path, &file)
    } else {
        Ok(()) // feed not in file yet — nothing to rename
    }
}

/// Write a `FeedsFile` back to disk as TOML.
///
/// Comments and formatting from the original file are not preserved.
fn write_feeds_file(path: &Path, file: &FeedsFile) -> Result<()> {
    let content = serialize_feeds_file(file);
    std::fs::write(path, content).map_err(AppError::Io)
}

/// Serialize a `FeedsFile` to a TOML string using the feeds.toml format.
fn serialize_feeds_file(file: &FeedsFile) -> String {
    let mut out = String::new();

    for (i, cat) in file.categories.iter().enumerate() {
        let key = i + 1;
        out.push_str(&format!("[category.{}]\n", key));
        out.push_str(&format!("title = {}\n", toml_string(&cat.title)));
        out.push('\n');
        for feed in &cat.feeds {
            out.push_str(&format!("[[category.{}.feed]]\n", key));
            out.push_str(&format!("url = {}\n", toml_string(&feed.url)));
            if let Some(name) = &feed.name {
                out.push_str(&format!("name = {}\n", toml_string(name)));
            }
            out.push('\n');
        }
    }

    for feed in &file.uncategorized {
        out.push_str("[[feed]]\n");
        out.push_str(&format!("url = {}\n", toml_string(&feed.url)));
        if let Some(name) = &feed.name {
            out.push_str(&format!("name = {}\n", toml_string(name)));
        }
        out.push('\n');
    }

    out
}

/// Produce a valid TOML basic string literal from `s` (quoted, with escapes).
fn toml_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            // Escape remaining control characters as \uXXXX.
            c if c.is_control() => escaped.push_str(&format!("\\u{:04X}", c as u32)),
            c => escaped.push(c),
        }
    }
    format!("\"{}\"", escaped)
}

/// Parse the text of a feeds.toml file into a `FeedsFile`.
/// Returns `Err(String)` with a human-readable description on failure.
fn parse_feeds_content(content: &str) -> std::result::Result<FeedsFile, String> {
    let value: toml::Value = toml::from_str(content).map_err(|e| e.to_string())?;
    let table = value.as_table().ok_or("feeds file must be a TOML table")?;

    // ── Categories ───────────────────────────────────────────────────────────
    let mut categories: Vec<CategoryConfig> = Vec::new();

    if let Some(cat_value) = table.get("category") {
        let cat_table = cat_value
            .as_table()
            .ok_or("`category` must be a TOML table")?;

        // Collect and sort by numeric key so [category.1] < [category.2] < …
        // Non-numeric keys fall after all numeric ones (sorted lexicographically).
        let mut pairs: Vec<(&str, &toml::Value)> =
            cat_table.iter().map(|(k, v)| (k.as_str(), v)).collect();

        pairs.sort_by(
            |(a, _), (b, _)| match (a.parse::<u64>(), b.parse::<u64>()) {
                (Ok(na), Ok(nb)) => na.cmp(&nb),
                (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                (Err(_), Err(_)) => a.cmp(b),
            },
        );

        for (key, val) in pairs {
            let tbl = val
                .as_table()
                .ok_or_else(|| format!("category.{key} must be a TOML table"))?;

            let title = tbl
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(key)
                .to_owned();

            let feeds = parse_feed_array(tbl.get("feed"))
                .map_err(|e| format!("category.{key}.feed: {e}"))?;

            categories.push(CategoryConfig { title, feeds });
        }
    }

    // ── Uncategorised feeds ──────────────────────────────────────────────────
    let uncategorized =
        parse_feed_array(table.get("feed")).map_err(|e| format!("[[feed]]: {e}"))?;

    Ok(FeedsFile {
        categories,
        uncategorized,
    })
}

/// Parse a `[[feed]]` array value into a list of `FeedConfig`s.
/// Entries without a `url` key are silently skipped.
fn parse_feed_array(val: Option<&toml::Value>) -> std::result::Result<Vec<FeedConfig>, String> {
    let Some(val) = val else {
        return Ok(Vec::new());
    };
    let arr = val
        .as_array()
        .ok_or("feed entries must be a TOML array of tables")?;

    let mut feeds = Vec::new();
    for v in arr {
        let Some(t) = v.as_table() else { continue };
        let Some(url) = t.get("url").and_then(|v| v.as_str()) else {
            continue; // url is required; skip malformed entries
        };
        let name = t.get("name").and_then(|v| v.as_str()).map(|s| s.to_owned());
        feeds.push(FeedConfig {
            url: url.to_owned(),
            name,
        });
    }
    Ok(feeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[category.1]
title = "Rust"

[[category.1.feed]]
url = "https://blog.rust-lang.org/feed.xml"
name = "Rust Blog"

[category.2]
title = "Linux"

[[category.2.feed]]
url = "https://www.kernel.org/feeds/kdiff.xml"

[[feed]]
url = "https://this-week-in-rust.org/rss.xml"
name = "TWIR"
"#;

    // ── toml_string() ─────────────────────────────────────────────────────────

    #[test]
    fn toml_string_plain_ascii() {
        assert_eq!(toml_string("Hello World"), "\"Hello World\"");
    }

    #[test]
    fn toml_string_empty() {
        assert_eq!(toml_string(""), "\"\"");
    }

    #[test]
    fn toml_string_escapes_backslash() {
        assert_eq!(toml_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn toml_string_escapes_double_quote() {
        assert_eq!(toml_string("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn toml_string_escapes_newline() {
        assert_eq!(toml_string("line1\nline2"), "\"line1\\nline2\"");
    }

    #[test]
    fn toml_string_escapes_carriage_return() {
        assert_eq!(toml_string("foo\rbar"), "\"foo\\rbar\"");
    }

    #[test]
    fn toml_string_escapes_tab() {
        assert_eq!(toml_string("col1\tcol2"), "\"col1\\tcol2\"");
    }

    #[test]
    fn toml_string_escapes_null_byte() {
        assert_eq!(toml_string("\x00"), "\"\\u0000\"");
    }

    #[test]
    fn toml_string_escapes_other_control_char() {
        // U+0001 CONTROL CHARACTER
        assert_eq!(toml_string("\x01"), "\"\\u0001\"");
    }

    #[test]
    fn toml_string_unicode_passthrough() {
        assert_eq!(toml_string("café"), "\"café\"");
    }

    #[test]
    fn toml_string_multiple_escapes_in_one_string() {
        assert_eq!(toml_string("a\nb\\c\"d"), "\"a\\nb\\\\c\\\"d\"");
    }

    // ── parse_categories_and_uncategorized ────────────────────────────────────

    #[test]
    fn parse_categories_and_uncategorized() {
        let f = parse_feeds_content(EXAMPLE).unwrap();
        assert_eq!(f.categories.len(), 2);
        assert_eq!(f.categories[0].title, "Rust");
        assert_eq!(f.categories[0].feeds.len(), 1);
        assert_eq!(f.categories[1].title, "Linux");
        assert_eq!(f.uncategorized.len(), 1);
    }

    #[test]
    fn into_layout_categorised() {
        let f = parse_feeds_content(EXAMPLE).unwrap();
        let (feeds, layout) = f.into_layout("No category");
        // 3 feeds total: Rust Blog, Kernel.org, TWIR
        assert_eq!(feeds.len(), 3);
        assert!(layout.has_categories);
        // feed_order: Rust Blog(0), Kernel.org(1), TWIR(2)
        assert_eq!(layout.feed_order, vec![0, 1, 2]);
    }

    #[test]
    fn into_layout_flat() {
        let content = r#"
[[feed]]
url = "https://a.com/feed"
[[feed]]
url = "https://b.com/feed"
"#;
        let f = parse_feeds_content(content).unwrap();
        let (feeds, layout) = f.into_layout("No category");
        assert_eq!(feeds.len(), 2);
        assert!(!layout.has_categories);
        assert_eq!(layout.feed_order, vec![0, 1]);
    }
}
