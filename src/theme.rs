use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::style::{Color, Modifier, Style};

use crate::error::{AppError, Result};
use crate::terminal;

/// Built-in dark default theme (ao-inspired), embedded at compile time.
const DEFAULT_THEME_DARK: &str = include_str!("default_theme.toml");

/// Built-in light default theme, embedded at compile time.
const DEFAULT_THEME_LIGHT: &str = include_str!("default_theme_light.toml");

/// Maximum inheritance depth to prevent infinite loops.
const MAX_INHERIT_DEPTH: usize = 8;

/// Resolved theme, mapping scope names to ratatui Styles.
///
/// Supports the Helix editor theme format:
/// - `fg`, `bg`, and `modifiers` per scope
/// - `[palette]` table for named colors
/// - `inherits` to chain from a parent theme
///
/// Helix-specific scopes (`markup.*`, `ui.*`) are automatically mapped to
/// thrssty scopes so that stock Helix themes render reasonably without any
/// manual customisation.
#[derive(Debug)]
pub struct Theme {
    styles: HashMap<String, Style>,
}

impl Theme {
    /// Load the built-in dark default theme.
    pub fn load_builtin_dark() -> Self {
        Self::parse_with_base(DEFAULT_THEME_DARK, None, 0).unwrap_or_else(|_| Self::empty())
    }

    /// Load the built-in light default theme.
    pub fn load_builtin_light() -> Self {
        Self::parse_with_base(DEFAULT_THEME_LIGHT, None, 0).unwrap_or_else(|_| Self::empty())
    }

    /// Load the built-in default theme, choosing dark or light based on the
    /// terminal background.  Kept for backward compatibility.
    pub fn load_builtin() -> Self {
        match terminal::detect() {
            terminal::Background::Light => Self::load_builtin_light(),
            terminal::Background::Dark => Self::load_builtin_dark(),
        }
    }

    /// Resolve and load a theme from the `theme` config value.
    ///
    /// Resolution order:
    /// 1. `"auto"` / `"default"` → detect terminal background, load the
    ///    matching built-in theme (`default_dark` or `default_light`).
    /// 2. `"default_dark"` / `"default_light"` → explicit built-in selection.
    /// 3. Value contains `/`, `\`, or ends with `.toml` → treat as a file
    ///    path (absolute or relative).
    /// 4. Otherwise → `<user_themes_dir>/<name>.toml`.
    ///
    /// `inherits` chains are resolved in the same directory as the theme file.
    pub fn load(theme_str: &str, user_themes_dir: Option<&Path>) -> Self {
        match theme_str {
            "auto" | "default" => return Self::load_builtin(),
            "default_dark" => return Self::load_builtin_dark(),
            "default_light" => return Self::load_builtin_light(),
            _ => {}
        }

        // Detect explicit path: absolute, contains a separator, or .toml suffix.
        let is_path =
            theme_str.contains('/') || theme_str.contains('\\') || theme_str.ends_with(".toml");

        if is_path {
            let path = PathBuf::from(theme_str);
            return Self::load_from_file_recursive(&path, 0)
                .unwrap_or_else(|_| Self::load_builtin());
        }

        // Name-based lookup in the user themes directory.
        if let Some(dir) = user_themes_dir {
            let path = dir.join(format!("{}.toml", theme_str));
            if path.exists() {
                return Self::load_from_file_recursive(&path, 0)
                    .unwrap_or_else(|_| Self::load_builtin());
            }
        }

        Self::load_builtin()
    }

    /// Load a theme from a TOML file on disk, resolving `inherits` chains.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        Self::load_from_file_recursive(path, 0)
    }

    /// Return the resolved ratatui `Style` for the given scope name.
    ///
    /// Falls back to the default (empty) style if the scope is not defined.
    pub fn style(&self, scope: &str) -> Style {
        self.styles.get(scope).copied().unwrap_or_default()
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn empty() -> Self {
        Self {
            styles: HashMap::new(),
        }
    }

    fn load_from_file_recursive(path: &Path, depth: usize) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| AppError::ThemeParse {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;

        // Determine the directory so we can resolve the parent theme.
        let dir = path.parent().map(|p| p.to_owned());

        Self::parse_with_base(&content, dir.as_deref(), depth).map_err(|e| AppError::ThemeParse {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }

    /// Parse a TOML string in Helix theme format into a Theme.
    ///
    /// `search_dir` is the directory in which to look for `inherits` parents.
    fn parse_with_base(
        content: &str,
        search_dir: Option<&Path>,
        depth: usize,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let value: toml::Value = toml::from_str(content)?;
        let table = value
            .as_table()
            .ok_or("Expected a TOML table at the top level")?;

        // Resolve `inherits` → load the parent theme first as our base.
        let mut styles: HashMap<String, Style> = if depth < MAX_INHERIT_DEPTH {
            if let Some(parent_name) = table.get("inherits").and_then(|v| v.as_str()) {
                resolve_parent(parent_name, search_dir, depth + 1)
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        // Extract palette.
        let palette: HashMap<String, String> = table
            .get("palette")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();

        // Merge current theme's scopes on top of the inherited base.
        for (key, val) in table.iter() {
            if matches!(key.as_str(), "palette" | "inherits" | "rainbow") {
                continue;
            }
            styles.insert(key.clone(), resolve_toml_value(val, &palette));
        }

        // Derive missing thrssty scopes from Helix scopes where sensible.
        apply_helix_fallbacks(&mut styles);

        Ok(Self { styles })
    }
}

/// Try to load a parent theme by name from `search_dir`.
/// Returns an empty style map on any failure (graceful degradation).
fn resolve_parent(name: &str, search_dir: Option<&Path>, depth: usize) -> HashMap<String, Style> {
    if let Some(dir) = search_dir {
        let path = dir.join(format!("{}.toml", name));
        if let Ok(theme) = Theme::load_from_file_recursive(&path, depth) {
            return theme.styles;
        }
    }
    HashMap::new()
}

/// Map well-known Helix scopes onto thrssty-specific scopes that a Helix theme
/// file will not contain.  Only fills in scopes that are not already set.
fn apply_helix_fallbacks(styles: &mut HashMap<String, Style>) {
    // (thrssty scope, &[helix scopes to try in priority order])
    let mappings: &[(&str, &[&str])] = &[
        // Article inline styles
        ("article.link", &["markup.link.url", "markup.link"]),
        ("article.h1", &["markup.heading.1", "markup.heading"]),
        ("article.h2", &["markup.heading.2", "markup.heading"]),
        ("article.h3", &["markup.heading.3", "markup.heading"]),
        (
            "article.code",
            &["markup.raw.inline", "markup.raw.block", "markup.raw"],
        ),
        ("article.quote", &["markup.quote"]),
        ("article.bold", &["markup.bold"]),
        ("article.italic", &["markup.italic"]),
        // Article list rows
        ("article.unread", &["ui.text"]),
        ("article.read", &["ui.text.inactive", "comment"]),
        ("article.selected", &["ui.selection"]),
        ("article.date", &["constant.numeric", "constant"]),
        ("article.author", &["variable.other.member", "function"]),
        ("article.category", &["tag", "attribute", "label"]),
        ("article.description", &["comment", "ui.text.inactive"]),
        // Feed sidebar
        ("feed.unread", &["ui.text"]),
        ("feed.read", &["ui.text.inactive", "comment"]),
        ("feed.selected", &["ui.selection"]),
        ("feed.error", &["error", "diagnostic.error"]),
        // Category rows
        ("category.unread", &["ui.text"]),
        ("category.read", &["ui.text.inactive", "comment"]),
        // UI chrome
        ("ui.border", &["ui.window", "ui.popup"]),
        ("ui.statusline.error", &["error", "diagnostic.error"]),
    ];

    for &(thrssty, fallbacks) in mappings {
        if styles.contains_key(thrssty) {
            continue;
        }
        for &fb in fallbacks {
            if let Some(&s) = styles.get(fb) {
                styles.insert(thrssty.to_owned(), s);
                break;
            }
        }
    }

    // ui.statusline and ui.statusline.key: for Helix themes (which define
    // ui.statusline with their own bg but leave our key scope absent) the
    // statusline's bg often clashes with the key-chip fg, making them
    // invisible.  Normalise both to use the global background colour.
    if !styles.contains_key("ui.statusline.key") {
        let global_bg = styles.get("ui.background").and_then(|s| s.bg);

        // Patch ui.statusline's bg to match the global background so the bar
        // blends into the rest of the UI.
        if let Some(bg) = global_bg {
            let sl = styles.entry("ui.statusline".to_owned()).or_default();
            sl.bg = Some(bg);
        }

        // Derive key-chip style: accent fg on global bg.
        // Try keyword first — it's an accent colour in virtually every Helix
        // theme and is readable against the editor background, giving key chips
        // a distinct appearance from surrounding description text (ui.text).
        let fg_source = ["keyword", "ui.text"]
            .iter()
            .find_map(|&fb| styles.get(fb).and_then(|s| s.fg));

        let mut derived = Style::default().add_modifier(Modifier::BOLD);
        if let Some(fg) = fg_source {
            derived = derived.fg(fg);
        }
        if let Some(bg) = global_bg {
            derived = derived.bg(bg);
        }
        styles.insert("ui.statusline.key".to_owned(), derived);
    }
}

// ── Value resolution ─────────────────────────────────────────────────────────

fn resolve_toml_value(val: &toml::Value, palette: &HashMap<String, String>) -> Style {
    match val {
        toml::Value::String(s) => Style::default().fg(parse_color(s, palette)),
        toml::Value::Table(t) => {
            let mut style = Style::default();
            if let Some(fg) = t.get("fg").and_then(|v| v.as_str()) {
                style = style.fg(parse_color(fg, palette));
            }
            if let Some(bg) = t.get("bg").and_then(|v| v.as_str()) {
                style = style.bg(parse_color(bg, palette));
            }
            if let Some(mods) = t.get("modifiers").and_then(|v| v.as_array()) {
                for m in mods {
                    if let Some(s) = m.as_str() {
                        style = apply_modifier(style, s);
                    }
                }
            }
            style
        }
        _ => Style::default(),
    }
}

/// Resolve a color string: first look it up in the palette, then parse as a
/// hex literal (`#rrggbb`) or a named terminal color.
fn parse_color(s: &str, palette: &HashMap<String, String>) -> Color {
    if let Some(hex) = palette.get(s) {
        return parse_hex_or_named(hex);
    }
    parse_hex_or_named(s)
}

fn parse_hex_or_named(s: &str) -> Color {
    // Six-digit hex: #rrggbb
    if s.starts_with('#') && s.len() == 7 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[1..3], 16),
            u8::from_str_radix(&s[3..5], 16),
            u8::from_str_radix(&s[5..7], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }

    match s {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "light-red" => Color::LightRed,
        "light-green" => Color::LightGreen,
        "light-yellow" => Color::LightYellow,
        "light-blue" => Color::LightBlue,
        "light-magenta" => Color::LightMagenta,
        "light-cyan" => Color::LightCyan,
        "light-gray" | "light-grey" => Color::White,
        "white" => Color::White,
        _ => Color::Reset,
    }
}

fn apply_modifier(style: Style, m: &str) -> Style {
    match m {
        "bold" => style.add_modifier(Modifier::BOLD),
        "dim" => style.add_modifier(Modifier::DIM),
        "italic" => style.add_modifier(Modifier::ITALIC),
        "underlined" | "underline" => style.add_modifier(Modifier::UNDERLINED),
        "slow_blink" => style.add_modifier(Modifier::SLOW_BLINK),
        "rapid_blink" => style.add_modifier(Modifier::RAPID_BLINK),
        "reversed" => style.add_modifier(Modifier::REVERSED),
        "hidden" => style.add_modifier(Modifier::HIDDEN),
        "crossed_out" => style.add_modifier(Modifier::CROSSED_OUT),
        _ => style,
    }
}
