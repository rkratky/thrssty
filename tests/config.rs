//! Integration tests for `Config` loading, cascade merging, and key-binding
//! defaults.  These tests exercise the public API from outside the crate,
//! complementing the unit tests that live alongside the source.

use std::io::Write as _;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::NamedTempFile;
use thrssty::config::{matches_key, Config};

// ── Defaults ──────────────────────────────────────────────────────────────────

#[test]
fn default_config_has_expected_key_bindings() {
    let cfg = Config::default();
    assert_eq!(cfg.keys.quit, "q");
    assert_eq!(cfg.keys.feed_down, "j");
    assert_eq!(cfg.keys.feed_up, "k");
    assert_eq!(cfg.keys.article_down, "Right");
    assert_eq!(cfg.keys.article_up, "Left");
    assert_eq!(cfg.keys.scroll_down, "PageDown");
    assert_eq!(cfg.keys.scroll_up, "PageUp");
    assert_eq!(cfg.keys.scroll_line_down, "Down");
    assert_eq!(cfg.keys.scroll_line_up, "Up");
    assert_eq!(cfg.keys.refresh_feed, "r");
    assert_eq!(cfg.keys.refresh_all, "R");
    assert_eq!(cfg.keys.open_browser, "o");
    assert_eq!(cfg.keys.add_feed, "a");
    assert_eq!(cfg.keys.delete_feed, "d");
}

#[test]
fn default_config_layout_proportions_are_in_valid_range() {
    let cfg = Config::default();
    assert!((1..=99).contains(&cfg.layout.sidebar_width));
    assert!((1..=99).contains(&cfg.layout.article_list_height));
}

#[test]
fn default_config_refresh_on_start_is_bool() {
    // Just assert it doesn't panic and has a concrete value.
    let _ = Config::default().refresh_on_start;
}

// ── Cascade loading ───────────────────────────────────────────────────────────

#[test]
fn cascade_with_no_sources_returns_defaults() {
    let cfg = Config::load_cascade(&[]);
    assert_eq!(cfg.keys.quit, Config::default().keys.quit);
    assert_eq!(cfg.layout.sidebar_width, Config::default().layout.sidebar_width);
}

#[test]
fn cascade_missing_file_is_silently_skipped() {
    let dir = tempfile::TempDir::new().unwrap();
    let absent = dir.path().join("thrssty_no_such_file_xyzzy.toml");
    let cfg = Config::load_cascade(&[Some(absent.as_path())]);
    // Falls back to defaults without panicking.
    assert_eq!(cfg.keys.quit, "q");
}

#[test]
fn cascade_single_file_overrides_one_key() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "[keys]\nquit = \"x\"").unwrap();
    let cfg = Config::load_cascade(&[Some(f.path())]);
    assert_eq!(cfg.keys.quit, "x");
    // Unspecified keys still come from defaults.
    assert_eq!(cfg.keys.feed_down, "j");
}

#[test]
fn cascade_higher_priority_source_wins() {
    let mut low = NamedTempFile::new().unwrap();
    writeln!(low, "[keys]\nquit = \"low\"").unwrap();

    let mut high = NamedTempFile::new().unwrap();
    writeln!(high, "[keys]\nquit = \"high\"").unwrap();

    let cfg = Config::load_cascade(&[Some(low.path()), Some(high.path())]);
    assert_eq!(cfg.keys.quit, "high");
}

#[test]
fn cascade_lower_priority_key_not_overridden_by_higher() {
    // Low sets feed_down; high does not touch feed_down.
    let mut low = NamedTempFile::new().unwrap();
    writeln!(low, "[keys]\nfeed_down = \"G\"").unwrap();

    let mut high = NamedTempFile::new().unwrap();
    writeln!(high, "[keys]\nquit = \"Q\"").unwrap();

    let cfg = Config::load_cascade(&[Some(low.path()), Some(high.path())]);
    assert_eq!(cfg.keys.feed_down, "G"); // preserved from low
    assert_eq!(cfg.keys.quit, "Q");      // taken from high
}

#[test]
fn cascade_layout_values_can_be_overridden() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "[layout]\nsidebar_width = 40").unwrap();
    let cfg = Config::load_cascade(&[Some(f.path())]);
    assert_eq!(cfg.layout.sidebar_width, 40);
}

#[test]
fn cascade_invalid_toml_file_is_silently_skipped() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "this is not valid TOML !!!").unwrap();
    // Should not panic; falls back to defaults.
    let cfg = Config::load_cascade(&[Some(f.path())]);
    assert_eq!(cfg.keys.quit, "q");
}

// ── matches_key() ─────────────────────────────────────────────────────────────

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn matches_single_char_key() {
    assert!(matches_key(&key(KeyCode::Char('q')), "q"));
    assert!(matches_key(&key(KeyCode::Char('j')), "j"));
    assert!(matches_key(&key(KeyCode::Char('R')), "R"));
}

#[test]
fn does_not_match_wrong_char() {
    assert!(!matches_key(&key(KeyCode::Char('q')), "Q")); // case-sensitive
    assert!(!matches_key(&key(KeyCode::Char('j')), "k"));
}

#[test]
fn matches_named_arrow_keys() {
    assert!(matches_key(&key(KeyCode::Up),    "Up"));
    assert!(matches_key(&key(KeyCode::Down),  "Down"));
    assert!(matches_key(&key(KeyCode::Left),  "Left"));
    assert!(matches_key(&key(KeyCode::Right), "Right"));
}

#[test]
fn matches_named_navigation_keys() {
    assert!(matches_key(&key(KeyCode::Home),     "Home"));
    assert!(matches_key(&key(KeyCode::End),      "End"));
    assert!(matches_key(&key(KeyCode::PageUp),   "PageUp"));
    assert!(matches_key(&key(KeyCode::PageDown), "PageDown"));
}

#[test]
fn matches_named_editing_keys() {
    assert!(matches_key(&key(KeyCode::Enter),     "Enter"));
    assert!(matches_key(&key(KeyCode::Esc),       "Esc"));
    assert!(matches_key(&key(KeyCode::Tab),       "Tab"));
    assert!(matches_key(&key(KeyCode::BackTab),   "BackTab"));
    assert!(matches_key(&key(KeyCode::Backspace), "Backspace"));
    assert!(matches_key(&key(KeyCode::Delete),    "Delete"));
    assert!(matches_key(&key(KeyCode::Insert),    "Insert"));
}

#[test]
fn matches_space_key_as_space_char_and_alias() {
    assert!(matches_key(&key(KeyCode::Char(' ')), " "));
    assert!(matches_key(&key(KeyCode::Char(' ')), "Space"));
}

#[test]
fn matches_function_keys() {
    assert!(matches_key(&key(KeyCode::F(1)),  "F1"));
    assert!(matches_key(&key(KeyCode::F(5)),  "F5"));
    assert!(matches_key(&key(KeyCode::F(12)), "F12"));
}

#[test]
fn does_not_match_wrong_named_key() {
    assert!(!matches_key(&key(KeyCode::Up),   "Down"));
    assert!(!matches_key(&key(KeyCode::Home), "End"));
    assert!(!matches_key(&key(KeyCode::F(1)), "F2"));
}

#[test]
fn does_not_match_named_key_wrong_case() {
    // Named keys are case-sensitive: "up" is not the same as "Up".
    assert!(!matches_key(&key(KeyCode::Up), "up"));
    assert!(!matches_key(&key(KeyCode::Enter), "enter"));
}
