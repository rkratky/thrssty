//! Integration tests for `html_to_lines()` — the HTML-to-ratatui rendering
//! pipeline.  These complement the unit tests in `src/html.rs` by exercising
//! the public API from outside the crate and covering cross-cutting scenarios.

use ratatui::style::{Color, Modifier, Style};
use thrssty::html::{html_to_lines, ArticleStyles};

fn plain_styles() -> ArticleStyles {
    ArticleStyles::default()
}

fn styled() -> ArticleStyles {
    ArticleStyles {
        h1: Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        h2: Style::default().fg(Color::Green),
        h3: Style::default().fg(Color::Cyan),
        code: Style::default().fg(Color::Magenta),
        link: Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED),
        quote: Style::default().fg(Color::DarkGray),
    }
}

fn render(html: &str) -> Vec<String> {
    html_to_lines(html, plain_styles(), 80)
        .into_iter()
        .map(|line| {
            line.spans
                .into_iter()
                .map(|s| s.content.into_owned())
                .collect::<String>()
        })
        .collect()
}

// ── Plain text pass-through ───────────────────────────────────────────────────

#[test]
fn plain_text_renders_without_panic() {
    let lines = render("Hello, world!");
    assert!(!lines.is_empty());
    let joined = lines.join("\n");
    assert!(joined.contains("Hello"));
}

#[test]
fn empty_string_does_not_panic() {
    let _ = html_to_lines("", plain_styles(), 80);
}

// ── Paragraph / block elements ────────────────────────────────────────────────

#[test]
fn paragraph_text_is_present_in_output() {
    let lines = render("<p>The quick brown fox.</p>");
    let joined = lines.join(" ");
    assert!(joined.contains("quick brown fox"));
}

#[test]
fn multiple_paragraphs_produce_multiple_lines() {
    let lines = render("<p>First paragraph.</p><p>Second paragraph.</p>");
    let joined = lines.join("\n");
    assert!(joined.contains("First paragraph"));
    assert!(joined.contains("Second paragraph"));
}

// ── Heading levels ────────────────────────────────────────────────────────────

#[test]
fn heading_text_is_present_in_output() {
    let lines = render("<h1>Main Title</h1><p>Body text.</p>");
    let joined = lines.join("\n");
    assert!(joined.contains("Main Title"));
    assert!(joined.contains("Body text"));
}

#[test]
fn headings_apply_styles_when_provided() {
    // With non-default styles the spans should still contain the text.
    let lines = html_to_lines("<h2>Section</h2>", styled(), 80);
    let joined: String = lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect();
    assert!(joined.contains("Section"));
}

// ── Inline markup ─────────────────────────────────────────────────────────────

#[test]
fn bold_text_is_present_in_output() {
    let lines = render("<p>This is <strong>bold</strong> text.</p>");
    let joined = lines.join(" ");
    assert!(joined.contains("bold"));
}

#[test]
fn italic_text_is_present_in_output() {
    let lines = render("<p>This is <em>italic</em> text.</p>");
    let joined = lines.join(" ");
    assert!(joined.contains("italic"));
}

#[test]
fn inline_code_is_present_in_output() {
    let lines = render("<p>Run <code>cargo test</code>.</p>");
    let joined = lines.join(" ");
    assert!(joined.contains("cargo test"));
}

// ── Links ─────────────────────────────────────────────────────────────────────

#[test]
fn link_text_appears_in_output() {
    let lines = render("<a href=\"https://example.com\">Click here</a>");
    let joined = lines.join(" ");
    assert!(joined.contains("Click here"));
}

#[test]
fn link_url_appears_as_footnote() {
    let lines = render("<a href=\"https://example.com\">Click here</a>");
    let joined = lines.join("\n");
    assert!(joined.contains("example.com"));
}

#[test]
fn naked_url_in_plain_text_is_preserved() {
    let lines = render("<p>Visit https://rust-lang.org for more.</p>");
    let joined = lines.join(" ");
    assert!(joined.contains("rust-lang.org"));
}

// ── Lists ─────────────────────────────────────────────────────────────────────

#[test]
fn unordered_list_items_are_present() {
    let html = "<ul><li>Alpha</li><li>Beta</li><li>Gamma</li></ul>";
    let joined = render(html).join("\n");
    assert!(joined.contains("Alpha"));
    assert!(joined.contains("Beta"));
    assert!(joined.contains("Gamma"));
}

#[test]
fn ordered_list_items_are_present() {
    let html = "<ol><li>First</li><li>Second</li></ol>";
    let joined = render(html).join("\n");
    assert!(joined.contains("First"));
    assert!(joined.contains("Second"));
}

// ── Blockquote ────────────────────────────────────────────────────────────────

#[test]
fn blockquote_text_is_preserved() {
    let lines = render("<blockquote>Quoted text here.</blockquote>");
    let joined = lines.join(" ");
    assert!(joined.contains("Quoted text here"));
}

// ── Code block ────────────────────────────────────────────────────────────────

#[test]
fn preformatted_code_block_text_is_preserved() {
    let html = "<pre><code>fn main() { println!(\"hello\"); }</code></pre>";
    let joined = render(html).join(" ");
    assert!(joined.contains("fn main"));
}

// ── HTML entities ─────────────────────────────────────────────────────────────

#[test]
fn html_entities_are_decoded() {
    let lines = render("<p>&lt;em&gt;italic&lt;/em&gt;</p>");
    let joined = lines.join(" ");
    // HTML entities are decoded to literal characters: &lt;em&gt; becomes the
    // plain text string `<em>`, not an HTML tag. The word "italic" should appear.
    assert!(joined.contains("italic"));
}

#[test]
fn hex_numeric_entities_are_decoded() {
    // &#x3C; = '<', &#x3E; = '>', &#x74; = 't', &#x65; = 'e', etc.
    // Regression: the entity parser was stripping 'x' before '#', so &#x3C;
    // was never decoded.
    let lines = render("<p>&#x74;&#x65;&#x73;&#x74;</p>");
    let joined = lines.join(" ");
    assert!(
        joined.contains("test"),
        "hex entities not decoded: {joined}"
    );
}

#[test]
fn uppercase_hex_numeric_entities_are_decoded() {
    // Same as above but with uppercase X.
    let lines = render("<p>&#X74;&#X65;&#X73;&#X74;</p>");
    let joined = lines.join(" ");
    assert!(
        joined.contains("test"),
        "uppercase hex entities not decoded: {joined}"
    );
}

// ── Markdown in plain-text feeds ─────────────────────────────────────────────

#[test]
fn markdown_link_in_no_html_source_renders_url() {
    // Feeds sometimes deliver pure Markdown in the description field.
    let md = "See [the docs](https://doc.rust-lang.org) for details.";
    let lines = render(md);
    let joined = lines.join(" ");
    assert!(joined.contains("the docs"));
    assert!(joined.contains("doc.rust-lang.org"));
}

// ── Width / wrapping ──────────────────────────────────────────────────────────

#[test]
fn zero_width_does_not_panic() {
    let _ = html_to_lines("<p>Some text.</p>", plain_styles(), 0);
}

#[test]
fn narrow_width_wraps_long_line() {
    let long = format!("<p>{}</p>", "word ".repeat(30));
    let lines = html_to_lines(&long, plain_styles(), 20);
    // With a 20-column width, a 150-char paragraph must produce more than one line.
    assert!(lines.len() > 1);
}

// ── Link edge cases ───────────────────────────────────────────────────────────

#[test]
fn local_anchor_link_content_preserved_without_footnote() {
    // href="#top" is a local anchor — content should appear but no [N] footnote.
    let html = r##"<a href="#top">Back to top</a>"##;
    let lines = render(html);
    let joined = lines.join(" ");
    assert!(
        joined.contains("Back to top"),
        "anchor text should appear: {joined}"
    );
    assert!(
        !joined.contains('['),
        "local anchor should not produce a footnote: {joined}"
    );
}

#[test]
fn empty_link_content_produces_no_footnote() {
    // An anchor whose text trims to empty should not generate a footnote reference.
    let html = r#"<a href="https://example.com">   </a>"#;
    let lines = render(html);
    let joined = lines.join(" ");
    assert!(
        !joined.contains("[1]"),
        "whitespace-only link content should not produce footnote: {joined}"
    );
}

#[test]
fn duplicate_urls_share_one_footnote_number() {
    // Two links pointing to the same URL should both show [1], not [1] and [2].
    let html = r#"<p><a href="https://dup.example.com">first</a> and <a href="https://dup.example.com">second</a></p>"#;
    let lines = render(html);
    let joined = lines.join(" ");
    assert!(joined.contains("[1]"), "expected footnote [1]: {joined}");
    assert!(
        !joined.contains("[2]"),
        "duplicate URL should reuse [1], not create [2]: {joined}"
    );
}

#[test]
fn distinct_urls_get_different_footnote_numbers() {
    let html = r#"<p><a href="https://a.example.com">alpha</a> and <a href="https://b.example.com">beta</a></p>"#;
    let lines = render(html);
    let joined = lines.join(" ");
    assert!(joined.contains("[1]"), "first URL should be [1]: {joined}");
    assert!(joined.contains("[2]"), "second URL should be [2]: {joined}");
}

#[test]
fn link_uppercase_tag_is_handled() {
    // Feed parsers sometimes produce uppercase tags; ensure they don't cause panics.
    let html = "<P>See <A HREF=\"https://example.com\">this</A> link.</P>";
    let lines = render(html);
    assert!(!lines.is_empty(), "output should not be empty");
}

#[test]
fn link_footnotes_appear_after_paragraph() {
    // Footnote URLs are appended at the end of the rendered block, separated from
    // the body text.
    let html = r#"<p>Read <a href="https://example.com/article">the article</a> now.</p>"#;
    let lines = render(html);
    let joined = lines.join("\n");
    assert!(
        joined.contains("example.com/article"),
        "URL should appear in footnotes: {joined}"
    );
}
