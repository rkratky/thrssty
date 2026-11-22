//! Integration tests for `Theme` loading — builtin defaults, user TOML files,
//! and Helix-compatible scope resolution.

use ratatui::style::{Color, Modifier, Style};
use tempfile::{NamedTempFile, TempDir};
use thrssty::theme::Theme;

// ── Builtin theme ─────────────────────────────────────────────────────────────

#[test]
fn builtin_theme_loads_without_panic() {
    let _ = Theme::load_builtin();
}

#[test]
fn builtin_theme_returns_non_default_style_for_known_scopes() {
    let theme = Theme::load_builtin();
    let bg = theme.style("ui.background");
    let text = theme.style("ui.text");
    // Both should be non-equal to the completely empty Style (each has at least one colour).
    assert!(bg != Style::default() && text != Style::default());
}

#[test]
fn unknown_scope_returns_default_style() {
    let theme = Theme::load_builtin();
    let s = theme.style("completely.unknown.scope.xyz");
    assert_eq!(s, Style::default());
}

// ── Load from file ────────────────────────────────────────────────────────────

#[test]
fn load_from_absent_path_falls_back_to_builtin() {
    let dir = TempDir::new().unwrap();
    let nonexistent = dir.path().join("no_such_theme_xyzzy.toml");
    let s = Theme::load(nonexistent.to_str().unwrap(), None);
    // Just verify it returns something usable (can call .style() without panic).
    let _ = s.style("ui.background");
}

#[test]
fn load_custom_theme_overrides_ui_text_colour() {
    let f = NamedTempFile::new().unwrap();
    // Scope definitions MUST come before [palette] in TOML — once [palette]
    // is opened, subsequent key-value pairs belong inside that table.
    std::fs::write(
        f.path(),
        r##"
"ui.text" = { fg = "myfg" }

[palette]
myfg = "#ff0000"
"##,
    )
    .unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let text_style = theme.style("ui.text");
    assert_eq!(text_style.fg, Some(Color::Rgb(255, 0, 0)));
}

#[test]
fn load_theme_by_name_from_user_dir() {
    let dir = TempDir::new().unwrap();
    let theme_path = dir.path().join("mytheme.toml");
    std::fs::write(
        &theme_path,
        r##"
"ui.text" = { fg = "accent" }

[palette]
accent = "#00ff00"
"##,
    )
    .unwrap();

    let theme = Theme::load("mytheme", Some(dir.path()));
    let text_style = theme.style("ui.text");
    assert_eq!(text_style.fg, Some(Color::Rgb(0, 255, 0)));
}

// ── Helix scope auto-mapping ──────────────────────────────────────────────────

#[test]
fn helix_ui_background_maps_to_thrssty_background() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(
        f.path(),
        r##"
"ui.background" = { bg = "mybg" }

[palette]
mybg = "#1e1e2e"
"##,
    )
    .unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let bg = theme.style("ui.background");
    assert_eq!(bg.bg, Some(Color::Rgb(0x1e, 0x1e, 0x2e)));
}

#[test]
fn helix_ui_selection_maps_to_article_selected() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(
        f.path(),
        r##"
"ui.selection" = { bg = "sel" }

[palette]
sel = "#313244"
"##,
    )
    .unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    // `article.selected` should pick up the selection colour via auto-mapping.
    let sel = theme.style("article.selected");
    assert_eq!(sel.bg, Some(Color::Rgb(0x31, 0x32, 0x44)));
}

// ── Palette hex colour parsing ────────────────────────────────────────────────

#[test]
fn palette_six_digit_hex_colour_parsed() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(
        f.path(),
        r##"
"ui.text" = { fg = "col" }

[palette]
col = "#aabbcc"
"##,
    )
    .unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let s = theme.style("ui.text");
    assert_eq!(s.fg, Some(Color::Rgb(0xaa, 0xbb, 0xcc)));
}

#[test]
fn palette_named_colour_parsed() {
    let f = NamedTempFile::new().unwrap();
    // No palette needed; use a terminal colour name directly.
    std::fs::write(f.path(), r#""ui.text" = { fg = "red" }"#).unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let s = theme.style("ui.text");
    assert_eq!(s.fg, Some(Color::Red));
}

#[test]
fn inline_hex_colour_without_palette_parsed() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r##""ui.text" = { fg = "#ff00ff" }"##).unwrap();

    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let s = theme.style("ui.text");
    assert_eq!(s.fg, Some(Color::Rgb(255, 0, 255)));
}

// ── Color name edge cases ──────────────────────────────────────────────────────

#[test]
fn named_color_grey_spelling_works() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { fg = "grey" }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert_eq!(theme.style("ui.text").fg, Some(Color::Gray));
}

#[test]
fn named_color_light_gray_maps_to_white() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { fg = "light-gray" }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    // light-gray → Color::White (brighter than Color::Gray)
    assert_eq!(theme.style("ui.text").fg, Some(Color::White));
}

#[test]
fn named_color_light_grey_maps_to_white() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { fg = "light-grey" }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert_eq!(theme.style("ui.text").fg, Some(Color::White));
}

#[test]
fn unknown_color_name_returns_reset() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { fg = "notacolor" }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    // Unknown color → Color::Reset (terminal default)
    assert_eq!(theme.style("ui.text").fg, Some(Color::Reset));
}

#[test]
fn hex_uppercase_is_parsed() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r##""ui.text" = { fg = "#AABBCC" }"##).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert_eq!(
        theme.style("ui.text").fg,
        Some(Color::Rgb(0xaa, 0xbb, 0xcc))
    );
}

#[test]
fn hex_mixed_case_is_parsed() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r##""ui.text" = { fg = "#AaBbCc" }"##).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert_eq!(
        theme.style("ui.text").fg,
        Some(Color::Rgb(0xaa, 0xbb, 0xcc))
    );
}

// ── Modifier parsing ──────────────────────────────────────────────────────────

#[test]
fn modifier_bold_applied() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { modifiers = ["bold"] }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert!(theme.style("ui.text").add_modifier == Modifier::BOLD);
}

#[test]
fn modifier_italic_applied() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(f.path(), r#""ui.text" = { modifiers = ["italic"] }"#).unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert!(theme.style("ui.text").add_modifier == Modifier::ITALIC);
}

#[test]
fn modifier_underline_both_spellings() {
    // Both "underlined" and "underline" are accepted.
    for spelling in &["underlined", "underline"] {
        let f = NamedTempFile::new().unwrap();
        std::fs::write(
            f.path(),
            format!(r#""ui.text" = {{ modifiers = ["{}"] }}"#, spelling),
        )
        .unwrap();
        let theme = Theme::load(f.path().to_str().unwrap(), None);
        assert!(
            theme
                .style("ui.text")
                .add_modifier
                .contains(Modifier::UNDERLINED),
            "spelling '{spelling}' should produce UNDERLINED modifier"
        );
    }
}

#[test]
fn multiple_modifiers_stacked() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(
        f.path(),
        r#""ui.text" = { modifiers = ["bold", "italic"] }"#,
    )
    .unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let m = theme.style("ui.text").add_modifier;
    assert!(m.contains(Modifier::BOLD), "bold should be set");
    assert!(m.contains(Modifier::ITALIC), "italic should be set");
}

#[test]
fn unknown_modifier_silently_ignored() {
    let f = NamedTempFile::new().unwrap();
    // "fakefmt" is not a recognized modifier; should not panic or affect the style.
    std::fs::write(
        f.path(),
        r#""ui.text" = { fg = "red", modifiers = ["fakefmt"] }"#,
    )
    .unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    let s = theme.style("ui.text");
    assert_eq!(s.fg, Some(Color::Red));
    assert_eq!(
        s.add_modifier,
        Modifier::empty(),
        "unknown modifier should add nothing"
    );
}

// ── Palette lookup takes precedence ───────────────────────────────────────────

#[test]
fn palette_name_resolved_before_direct_color() {
    let f = NamedTempFile::new().unwrap();
    std::fs::write(
        f.path(),
        r##"
"ui.text" = { fg = "myalias" }

[palette]
myalias = "#003366"
"##,
    )
    .unwrap();
    let theme = Theme::load(f.path().to_str().unwrap(), None);
    assert_eq!(
        theme.style("ui.text").fg,
        Some(Color::Rgb(0x00, 0x33, 0x66))
    );
}
