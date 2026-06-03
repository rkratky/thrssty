use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// Top-level configuration, loaded from `~/.config/thrssty/config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Theme to use. Accepts four forms:
    /// - `"auto"` (default) — detects the terminal background and selects
    ///   `"default_dark"` (Ao-inspired) or `"default_light"` automatically.
    /// - `"default_dark"` / `"default_light"` — built-in themes selected explicitly.
    /// - `"name"` — a file named `<name>.toml` in `~/.config/thrssty/themes/`.
    /// - A file path (absolute or containing `/`) — loaded directly, e.g.
    ///   `/usr/lib/helix/runtime/themes/ayu_evolve.toml`.
    ///
    /// Helix theme files are supported natively, including `inherits` chains.
    /// Standard Helix scopes (`markup.*`, `ui.*`) are automatically mapped to
    /// thrssty scopes so stock Helix themes render well out of the box.
    #[serde(default = "defaults::theme")]
    pub theme: String,

    /// Fetch all feeds on startup. Set to `false` to disable.
    #[serde(default = "defaults::refresh_on_start")]
    pub refresh_on_start: bool,

    /// Path to the feeds definition file.
    /// Relative paths are resolved relative to the config directory.
    /// Defaults to `feeds.toml` in the same directory as `config.toml`.
    #[serde(default)]
    pub feeds_file: Option<String>,

    /// Display label for feeds not assigned to any category.
    #[serde(default = "defaults::uncategorized_label")]
    pub uncategorized_label: String,

    /// Remove articles older than this many days from the cache on startup.
    /// Set to 0 to never remove old articles.
    #[serde(default = "defaults::max_article_age_days")]
    pub max_article_age_days: u64,

    #[serde(default)]
    pub layout: LayoutConfig,

    #[serde(default)]
    pub keys: KeysConfig,
}

/// Proportional size of each panel (percentages).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayoutConfig {
    /// Width of the feed sidebar as a percentage of total viewport width.
    #[serde(default = "defaults::sidebar_width")]
    pub sidebar_width: u16,

    /// Height of the article list as a percentage of the right-panel height.
    #[serde(default = "defaults::article_list_height")]
    pub article_list_height: u16,

    /// Maximum width of the author column in the article list as a percentage
    /// of the list panel's inner width.
    #[serde(default = "defaults::max_author_col_pct")]
    pub max_author_col_pct: u16,
}

/// Key binding strings. Each value is a key name such as `"q"`, `"Down"`, `"PageUp"`, etc.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeysConfig {
    #[serde(default = "defaults::quit")]
    pub quit: String,
    #[serde(default = "defaults::refresh_feed")]
    pub refresh_feed: String,
    #[serde(default = "defaults::refresh_all")]
    pub refresh_all: String,
    #[serde(default = "defaults::open_browser")]
    pub open_browser: String,
    #[serde(default = "defaults::feed_down")]
    pub feed_down: String,
    #[serde(default = "defaults::feed_up")]
    pub feed_up: String,
    #[serde(default = "defaults::article_down")]
    pub article_down: String,
    #[serde(default = "defaults::article_up")]
    pub article_up: String,
    #[serde(default = "defaults::article_home")]
    pub article_home: String,
    #[serde(default = "defaults::article_end")]
    pub article_end: String,
    #[serde(default = "defaults::scroll_down")]
    pub scroll_down: String,
    #[serde(default = "defaults::scroll_up")]
    pub scroll_up: String,
    #[serde(default = "defaults::scroll_line_down")]
    pub scroll_line_down: String,
    #[serde(default = "defaults::scroll_line_up")]
    pub scroll_line_up: String,
    #[serde(default = "defaults::reload_config")]
    pub reload_config: String,
    #[serde(default = "defaults::add_feed")]
    pub add_feed: String,
    #[serde(default = "defaults::delete_feed")]
    pub delete_feed: String,
    #[serde(default = "defaults::edit_feed")]
    pub edit_feed: String,
}

// ── Default value functions ──────────────────────────────────────────────────

mod defaults {
    pub fn theme() -> String {
        "auto".into()
    }
    pub fn refresh_on_start() -> bool {
        true
    }
    pub fn uncategorized_label() -> String {
        "No category".into()
    }
    pub fn max_article_age_days() -> u64 {
        100
    }
    pub fn sidebar_width() -> u16 {
        20
    }
    pub fn article_list_height() -> u16 {
        40
    }
    pub fn max_author_col_pct() -> u16 {
        25
    }
    pub fn quit() -> String {
        "q".into()
    }
    pub fn refresh_feed() -> String {
        "r".into()
    }
    pub fn refresh_all() -> String {
        "R".into()
    }
    pub fn open_browser() -> String {
        "o".into()
    }
    pub fn feed_down() -> String {
        "j".into()
    }
    pub fn feed_up() -> String {
        "k".into()
    }
    pub fn article_down() -> String {
        "Right".into()
    }
    pub fn article_up() -> String {
        "Left".into()
    }
    pub fn article_home() -> String {
        "Home".into()
    }
    pub fn article_end() -> String {
        "End".into()
    }
    pub fn scroll_down() -> String {
        "PageDown".into()
    }
    pub fn scroll_up() -> String {
        "PageUp".into()
    }
    pub fn scroll_line_down() -> String {
        "Down".into()
    }
    pub fn scroll_line_up() -> String {
        "Up".into()
    }
    pub fn reload_config() -> String {
        "F5".into()
    }
    pub fn add_feed() -> String {
        "a".into()
    }
    pub fn delete_feed() -> String {
        "d".into()
    }
    pub fn edit_feed() -> String {
        "e".into()
    }
}

// ── Default trait implementations ─────────────────────────────────────────

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            sidebar_width: defaults::sidebar_width(),
            article_list_height: defaults::article_list_height(),
            max_author_col_pct: defaults::max_author_col_pct(),
        }
    }
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            quit: defaults::quit(),
            refresh_feed: defaults::refresh_feed(),
            refresh_all: defaults::refresh_all(),
            open_browser: defaults::open_browser(),
            feed_down: defaults::feed_down(),
            feed_up: defaults::feed_up(),
            article_down: defaults::article_down(),
            article_up: defaults::article_up(),
            article_home: defaults::article_home(),
            article_end: defaults::article_end(),
            scroll_down: defaults::scroll_down(),
            scroll_up: defaults::scroll_up(),
            scroll_line_down: defaults::scroll_line_down(),
            scroll_line_up: defaults::scroll_line_up(),
            reload_config: defaults::reload_config(),
            add_feed: defaults::add_feed(),
            delete_feed: defaults::delete_feed(),
            edit_feed: defaults::edit_feed(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: defaults::theme(),
            refresh_on_start: defaults::refresh_on_start(),
            feeds_file: None,
            uncategorized_label: defaults::uncategorized_label(),
            max_article_age_days: defaults::max_article_age_days(),
            layout: LayoutConfig::default(),
            keys: KeysConfig::default(),
        }
    }
}

// ── Config file I/O ──────────────────────────────────────────────────────────

impl Config {
    /// Path to the config file: `~/.config/thrssty/config.toml`.
    pub fn config_path() -> Result<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or(AppError::MissingConfigDir)?
            .join("thrssty")
            .join("config.toml"))
    }

    /// Path to the user themes directory: `~/.config/thrssty/themes/`.
    pub fn themes_dir() -> Result<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or(AppError::MissingConfigDir)?
            .join("thrssty")
            .join("themes"))
    }

    /// Load config from disk. Returns `Config::default()` if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Load config by merging multiple files in ascending priority order.
    ///
    /// `sources` is a slice of `Option<&Path>` values from **lowest to highest**
    /// priority.  `None` entries are skipped; missing files are silently
    /// ignored.  Any key present in a higher-priority file wins over the same
    /// key in a lower-priority file.  Keys absent from all files fall back to
    /// the serde field defaults defined on [`Config`].
    pub fn load_cascade(sources: &[Option<&Path>]) -> Self {
        let mut merged = toml::Value::Table(Default::default());
        for &maybe_path in sources {
            let Some(path) = maybe_path else { continue };
            let Ok(content) = std::fs::read_to_string(path) else { continue };
            let Ok(val) = toml::from_str::<toml::Value>(&content) else { continue };
            merge_toml(&mut merged, val);
        }
        merged.try_into().unwrap_or_default()
    }

    /// Write default configuration, theme, and example feeds files to the
    /// user config directory (`~/.config/thrssty/`).
    ///
    /// Existing files are never overwritten; the user is notified for each.
    pub fn init_files() -> Result<()> {
        use std::fs;

        let config_dir = dirs::config_dir()
            .ok_or(AppError::MissingConfigDir)?
            .join("thrssty");
        let themes_dir = config_dir.join("themes");

        fs::create_dir_all(&themes_dir)?;

        let files: &[(&str, &str)] = &[
            ("config.toml",         include_str!("default_config.toml")),
            ("feeds.toml",          include_str!("example_feeds.toml")),
            ("themes/default.toml", include_str!("default_theme.toml")),
        ];

        for &(relative, content) in files {
            let dest = config_dir.join(relative);
            if dest.exists() {
                println!("  skip  {}", dest.display());
            } else {
                fs::write(&dest, content)?;
                println!("  write {}", dest.display());
            }
        }
        Ok(())
    }

    /// Resolve the absolute path to the feeds definition file.
    ///
    /// `base_dir` is the directory used to resolve relative `feeds_file` paths.
    /// When `None`, falls back to the XDG config dir.
    ///
    /// - `feeds_file` is `None`: `<base_dir>/feeds.toml`.
    /// - `feeds_file` is a relative path: relative to `<base_dir>`.
    /// - `feeds_file` is an absolute path: used as-is.
    pub fn feeds_file_path(&self) -> Result<PathBuf> {
        self.feeds_file_path_relative_to(None)
    }

    /// Like [`feeds_file_path`] but with an explicit `base_dir`.
    pub fn feeds_file_path_relative_to(&self, base_dir: Option<&Path>) -> Result<PathBuf> {
        let fallback = Self::config_path()?;
        let base = base_dir.unwrap_or_else(|| {
            fallback.parent().unwrap_or(std::path::Path::new("."))
        });

        let path = match &self.feeds_file {
            None => base.join("feeds.toml"),
            Some(f) => {
                let p = PathBuf::from(f);
                if p.is_absolute() { p } else { base.join(p) }
            }
        };
        Ok(path)
    }
}

// ── TOML merge ────────────────────────────────────────────────────────────────

/// Merge `src` into `dst`, with `src` taking precedence.
///
/// For top-level tables, keys in `src` overwrite the same keys in `dst`.
/// Non-table values are replaced wholesale.
fn merge_toml(dst: &mut toml::Value, src: toml::Value) {
    match (dst, src) {
        (toml::Value::Table(d), toml::Value::Table(s)) => {
            for (k, v) in s {
                let entry = d.entry(k).or_insert(toml::Value::Table(Default::default()));
                merge_toml(entry, v);
            }
        }
        (dst, src) => *dst = src,
    }
}



/// Returns `true` if `event` matches the key described by `key_str`.
///
/// `key_str` is one of: a single character (`"q"`, `"j"`, `"R"`), or a named
/// key (`"Up"`, `"Down"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`, `"Enter"`, `"Esc"`,
/// `"F1"`–`"F12"`).
pub fn matches_key(event: &KeyEvent, key_str: &str) -> bool {
    match event.code {
        KeyCode::Char(c) if key_str.len() == 1 => c.to_string() == key_str,
        KeyCode::Up => key_str == "Up",
        KeyCode::Down => key_str == "Down",
        KeyCode::Left => key_str == "Left",
        KeyCode::Right => key_str == "Right",
        KeyCode::Home => key_str == "Home",
        KeyCode::End => key_str == "End",
        KeyCode::PageUp => key_str == "PageUp",
        KeyCode::PageDown => key_str == "PageDown",
        KeyCode::Enter => key_str == "Enter",
        KeyCode::Esc => key_str == "Esc",
        KeyCode::Tab => key_str == "Tab",
        KeyCode::BackTab => key_str == "BackTab",
        KeyCode::Backspace => key_str == "Backspace",
        KeyCode::Delete => key_str == "Delete",
        KeyCode::Insert => key_str == "Insert",
        KeyCode::Char(' ') => key_str == " " || key_str == "Space",
        KeyCode::F(n) => key_str == format!("F{n}"),
        _ => false,
    }
}
