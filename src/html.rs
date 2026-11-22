use html2text::{config, render::RichAnnotation};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::fmt::Write as _;

/// Styles passed in from the active theme for rendering article content.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArticleStyles {
    pub h1: Style,
    pub h2: Style,
    pub h3: Style,
    pub code: Style,
    pub link: Style,
    pub quote: Style,
}

/// Convert an HTML string into styled ratatui `Line`s.
///
/// Pipeline:
/// 1. Entity-decode entity-encoded HTML (`&lt;em&gt;` → `<em>`).
/// 2. Pre-process links: `<a href="url">text</a>` → `<a href="url">text[n]</a>`.
///    This happens **before** html2text so table cell widths account for `[n]`.
/// 3. html2text with `RichDecorator` converts HTML to annotated lines.
/// 4. Map `RichAnnotation`s to ratatui `Style`s.
/// 5. Append footnote URLs as `[n] url` lines.
///
/// `width` is the available display width in columns.  Pass `0` before the
/// first frame — html2text uses a large fallback and ratatui soft-wraps.
pub fn html_to_lines(html: &str, styles: ArticleStyles, width: u16) -> Vec<Line<'static>> {
    // Normalise the HTML source:
    // • Entity-encoded HTML (`&lt;em&gt;`) → real HTML tags.
    // • Pure Markdown (no HTML at all) → convert Markdown links to HTML
    //   so html2text can process them (e.g. KDE store feed descriptions).
    let src: String = if !html.contains('<') {
        if html.contains("&lt;") {
            decode_html_entities(html)
        } else if html.contains("](http") || html.contains("](https") || html.contains("](/") {
            markdown_links_to_html(html)
        } else {
            html.to_owned()
        }
    } else {
        html.to_owned()
    };

    // Pre-process links into footnote references BEFORE html2text renders,
    // so table layout accounts for the `[n]` suffix width.
    let (src, footnote_urls) = number_links(&src);

    let render_width = if width > 0 { width as usize } else { 100_000 };
    // min_wrap_width raised from the default of 3 to reduce aggressive
    // word-breaking in table cells.
    let rich_lines = config::rich()
        .min_wrap_width(20)
        .lines_from_read(src.as_bytes(), render_width)
        .unwrap_or_default();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut last_was_list = false;

    for (i, tagged_line) in rich_lines.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();

        for ts in tagged_line.tagged_strings() {
            if ts.s.is_empty() {
                continue;
            }
            let style = annotation_style(&ts.tag, &styles);
            let only_default = ts.tag.iter().all(|a| matches!(a, RichAnnotation::Default));
            if only_default {
                // `[n]` footnote markers and naked URLs arrive with Default annotation.
                // Split them out and apply appropriate styling.
                let with_refs = if ts.s.contains('[') {
                    split_footnote_refs(&ts.s, style, dim)
                } else {
                    vec![Span::styled(ts.s.clone(), style)]
                };
                for span in with_refs {
                    if span.style == style
                        && (span.content.contains("http://") || span.content.contains("https://"))
                    {
                        spans.extend(highlight_naked_urls(&span.content, style, styles.link));
                    } else {
                        spans.push(span);
                    }
                }
            } else {
                spans.push(Span::styled(ts.s.clone(), style));
            }
        }

        if spans.is_empty() {
            last_was_list = false;
            lines.push(Line::raw(""));
            continue;
        }

        let first_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let this_is_list = is_list_item(&first_text);

        // Blank before a list block if not already there.
        if this_is_list && !last_was_list {
            ensure_blank_before(&mut lines);
        }
        // Blank after a list block if not already there (detected when next non-list arrives).
        if !this_is_list && last_was_list {
            ensure_blank_before(&mut lines);
        }

        if let Some(level) = heading_level(&first_text) {
            let hstyle = match level {
                1 => styles.h1,
                2 => styles.h2,
                _ => styles.h3,
            };
            lines.push(Line::from(
                spans
                    .into_iter()
                    .map(|s| Span::styled(s.content, hstyle))
                    .collect::<Vec<_>>(),
            ));
            let next_is_blank = rich_lines
                .get(i + 1)
                .map(|nl| nl.tagged_strings().all(|ts| ts.s.trim().is_empty()))
                .unwrap_or(false);
            if !next_is_blank {
                lines.push(Line::raw(""));
            }
            last_was_list = false;
        } else {
            lines.push(Line::from(spans));
            last_was_list = this_is_list;
        }
    }

    // Footnotes section.
    if !footnote_urls.is_empty() {
        if !lines
            .last()
            .map(|l: &Line| l.spans.is_empty())
            .unwrap_or(true)
        {
            lines.push(Line::raw(""));
        }
        lines.push(Line::raw("─".repeat(40)));
        for (i, url) in footnote_urls.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("[{}] ", i + 1), dim),
                Span::styled(url.clone(), styles.link),
            ]));
        }
    }

    if lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines
}

/// Add a blank line if the last emitted line is not already blank.
fn ensure_blank_before(lines: &mut Vec<Line<'static>>) {
    let last_blank = lines
        .last()
        .map(|l: &Line| l.spans.is_empty() || l.spans.iter().all(|s| s.content.trim().is_empty()))
        .unwrap_or(true);
    if !last_blank {
        lines.push(Line::raw(""));
    }
}

/// Split `text` on `[n]` footnote reference patterns, applying `dim` to the
/// marker and `base` to surrounding text.  Returns one or more `Span`s.
fn split_footnote_refs(text: &str, base: Style, dim: Style) -> Vec<Span<'static>> {
    let mut result: Vec<Span<'static>> = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let Some(lb) = rest.find('[') else {
            result.push(Span::styled(rest.to_owned(), base));
            break;
        };
        // Check if what follows is digits then ']'.
        let after = &rest[lb + 1..];
        let digit_len = after.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digit_len > 0 && after.as_bytes().get(digit_len) == Some(&b']') {
            if lb > 0 {
                result.push(Span::styled(rest[..lb].to_owned(), base));
            }
            result.push(Span::styled(
                rest[lb..lb + 1 + digit_len + 1].to_owned(),
                dim,
            ));
            rest = &rest[lb + 1 + digit_len + 1..];
        } else {
            // Not a footnote ref — include the '[' in base text and move on.
            result.push(Span::styled(rest[..lb + 1].to_owned(), base));
            rest = &rest[lb + 1..];
        }
    }
    result
}

/// Split any naked URLs (`http://…` or `https://…`) out of plain text,
/// applying `link_style` to each URL and `base` to surrounding text.
/// A URL ends at the first ASCII whitespace or end of string.
fn highlight_naked_urls(text: &str, base: Style, link_style: Style) -> Vec<Span<'static>> {
    let mut result: Vec<Span<'static>> = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let url_start = rest.find("https://").or_else(|| rest.find("http://"));

        let Some(pos) = url_start else {
            result.push(Span::styled(rest.to_owned(), base));
            break;
        };

        if pos > 0 {
            result.push(Span::styled(rest[..pos].to_owned(), base));
        }

        // URL ends at first whitespace, or end of string.
        let url_end = rest[pos..]
            .find(|c: char| c.is_ascii_whitespace())
            .map(|n| pos + n)
            .unwrap_or(rest.len());

        result.push(Span::styled(rest[pos..url_end].to_owned(), link_style));
        rest = &rest[url_end..];
    }
    result
}

fn is_list_item(text: &str) -> bool {
    let t = text.trim_start_matches(' ');
    if t.starts_with("* ") {
        return true;
    }
    // Ordered: one or more digits followed by ". "
    let digit_end = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    digit_end > 0
        && t.as_bytes().get(digit_end) == Some(&b'.')
        && t.as_bytes().get(digit_end + 1) == Some(&b' ')
}

/// Detect a heading prefix (`# `, `## `, `### `) and return the level.
fn heading_level(text: &str) -> Option<usize> {
    if text.starts_with("### ") || text == "###" {
        Some(3)
    } else if text.starts_with("## ") || text == "##" {
        Some(2)
    } else if text.starts_with("# ") || text == "#" {
        Some(1)
    } else {
        None
    }
}

/// Map `RichAnnotation`s to a ratatui `Style`.
fn annotation_style(annotations: &[RichAnnotation], styles: &ArticleStyles) -> Style {
    let mut style = Style::default();
    for ann in annotations {
        match ann {
            RichAnnotation::Default => {}
            RichAnnotation::Emphasis => {
                style = style.add_modifier(Modifier::ITALIC);
            }
            RichAnnotation::Strong => {
                style = style.add_modifier(Modifier::BOLD);
            }
            RichAnnotation::Strikeout => {
                style = style.add_modifier(Modifier::CROSSED_OUT);
            }
            RichAnnotation::Code | RichAnnotation::Preformat(_) => {
                style = style.patch(styles.code);
            }
            RichAnnotation::Link(_) => {
                style = style.patch(styles.link);
            }
            RichAnnotation::Image(_) => {
                style = style.add_modifier(Modifier::ITALIC);
            }
            RichAnnotation::Colour(_) | RichAnnotation::BgColour(_) => {}
            _ => {}
        }
    }
    style
}

// ── Link pre-processing ───────────────────────────────────────────────────────

/// Convert Markdown-style links `[text](url)` to HTML `<a href="url">text</a>`.
/// Used for feeds (e.g. KDE store) that deliver plain Markdown in descriptions.
fn markdown_links_to_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while !rest.is_empty() {
        let Some(lb) = rest.find('[') else {
            out.push_str(rest);
            break;
        };

        // Find the matching `]` — allow any content inside (no nesting).
        let after_lb = &rest[lb + 1..];
        let Some(rb) = after_lb.find(']') else {
            out.push_str(&rest[..lb + 1]);
            rest = after_lb;
            continue;
        };
        let link_text = &after_lb[..rb];
        let after_rb = &after_lb[rb + 1..];

        // Must be immediately followed by `(url)`.
        if let Some(inner) = after_rb.strip_prefix('(') {
            if let Some(rp) = inner.find(')') {
                let url = inner[..rp].trim();
                if url.starts_with("http") || url.starts_with('/') {
                    out.push_str(&rest[..lb]);
                    out.push_str(&format!(r#"<a href="{}">{}</a>"#, url, link_text));
                    rest = &inner[rp + 1..];
                    continue;
                }
            }
        }

        // Not a valid Markdown link — emit `[` and continue.
        out.push_str(&rest[..lb + 1]);
        rest = after_lb;
    }
    out
}

/// Replace `<a href="url">content</a>` with `<a href="url">content[n]</a>`
/// and return the list of footnote URLs.
///
/// Deduplicates: the same URL always reuses the same `[n]`.
/// Local anchors (`href="#..."`) are left unchanged with no footnote.
/// Empty-content links (CMS navigation anchors) are dropped entirely.
fn number_links(html: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(html.len() + 64);
    let mut urls: Vec<String> = Vec::new();
    // O(1) lookup by URL string; value is the 1-based footnote index.
    let mut url_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut pos = 0;

    while pos < html.len() {
        let Some(rel_start) = find_a_open(&html[pos..]) else {
            out.push_str(&html[pos..]);
            break;
        };

        out.push_str(&html[pos..pos + rel_start]);
        let tag_start = pos + rel_start;

        let Some((href, tag_end)) = parse_a_open(&html[tag_start..]) else {
            out.push('<');
            pos = tag_start + 1;
            continue;
        };

        let content_start = tag_start + tag_end;
        let Some((close_start, close_end)) = find_a_close(&html[content_start..]) else {
            out.push_str(&html[tag_start..]);
            break;
        };

        let content = &html[content_start..content_start + close_start];

        if let Some(url) = href {
            if !url.is_empty() && !url.starts_with('#') && !content.trim().is_empty() {
                // O(1) lookup; insert only if new, cloning the key into urls once.
                let n = match url_index.entry(url) {
                    std::collections::hash_map::Entry::Occupied(e) => *e.get(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        let idx = urls.len() + 1;
                        urls.push(e.key().clone());
                        e.insert(idx);
                        idx
                    }
                };
                // Keep the <a> tag for link styling; [n] goes OUTSIDE so it
                // renders in dim rather than link colour.
                out.push_str(&html[tag_start..content_start]);
                out.push_str(content);
                out.push_str("</a>");
                let _ = write!(out, "[{}]", n);
            } else {
                // Local anchor or empty link: keep content, drop the <a> wrapper.
                out.push_str(content);
            }
        } else {
            out.push_str(content);
        }

        pos = content_start + close_end;
    }

    (out, urls)
}

/// Find the next `<a` opening tag offset in `s`.
fn find_a_open(s: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < s.len() {
        let p = s[pos..].find('<')?;
        let abs = pos + p;
        let b = s.as_bytes();
        let next = b.get(abs + 1).copied();
        if matches!(next, Some(b'a') | Some(b'A')) {
            let boundary = b.get(abs + 2).copied();
            if matches!(
                boundary,
                Some(b' ')
                    | Some(b'\t')
                    | Some(b'\n')
                    | Some(b'\r')
                    | Some(b'>')
                    | Some(b'/')
                    | None
            ) {
                return Some(abs);
            }
        }
        pos = abs + 1;
    }
    None
}

/// Parse `<a ...>` starting at `s`. Returns `(href, bytes_consumed)`.
fn parse_a_open(s: &str) -> Option<(Option<String>, usize)> {
    let bytes = s.as_bytes();
    let mut pos = 2; // skip "<a"
    let mut href: Option<String> = None;

    while pos < bytes.len() {
        match bytes[pos] {
            b'>' => return Some((href, pos + 1)),
            b'/' if bytes.get(pos + 1) == Some(&b'>') => return Some((href, pos + 2)),
            b'\'' | b'"' => {
                let q = bytes[pos];
                pos += 1;
                while pos < bytes.len() && bytes[pos] != q {
                    pos += 1;
                }
                if pos < bytes.len() {
                    pos += 1;
                }
            }
            c if c.eq_ignore_ascii_case(&b'h')
                && s.len() > pos + 4
                && s[pos..pos + 4].eq_ignore_ascii_case("href") =>
            {
                pos += 4;
                while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
                    pos += 1;
                }
                if bytes.get(pos) == Some(&b'=') {
                    pos += 1;
                    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
                        pos += 1;
                    }
                    let (val, consumed) = parse_attr_val(&s[pos..]);
                    href = Some(val);
                    pos += consumed;
                }
            }
            _ => pos += 1,
        }
    }
    None
}

/// Parse a quoted or unquoted HTML attribute value.
fn parse_attr_val(s: &str) -> (String, usize) {
    match s.as_bytes().first() {
        Some(&b'"') => {
            let end = s[1..].find('"').unwrap_or(s.len().saturating_sub(1));
            (s[1..1 + end].to_owned(), end + 2)
        }
        Some(&b'\'') => {
            let end = s[1..].find('\'').unwrap_or(s.len().saturating_sub(1));
            (s[1..1 + end].to_owned(), end + 2)
        }
        _ => {
            let end = s
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(s.len());
            (s[..end].to_owned(), end)
        }
    }
}

/// Find `</a>` (case-insensitive) in `s`. Returns `(start, end_past_>)`.
fn find_a_close(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        // Find the next '<' by byte — always a valid boundary since '<' is ASCII.
        let lt = bytes[pos..].iter().position(|&b| b == b'<')?;
        let abs = pos + lt;
        let rest = &bytes[abs..];
        // Compare as bytes so multi-byte UTF-8 characters after '<' never
        // cause a slice-not-on-char-boundary panic.
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case(b"</a>") {
            return Some((abs, abs + 4));
        }
        if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case(b"</a") {
            if let Some(gt) = rest[3..].iter().position(|&b| b == b'>') {
                if rest[3..3 + gt].iter().all(|b| b.is_ascii_whitespace()) {
                    return Some((abs, abs + 3 + gt + 1));
                }
            }
        }
        pos = abs + 1;
    }
    None
}

// ── HTML entity pre-processing ────────────────────────────────────────────────

pub(crate) fn decode_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];

        let end = rest[1..].find(';').map(|i| i + 1);
        let Some(end) = end else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };

        let entity = &rest[1..end];
        if let Some(tail) = entity.strip_prefix('#') {
            // Numeric entity: &#NNN; (decimal) or &#xNNN; / &#XNNN; (hex).
            let ch = if let Some(hex) = tail.strip_prefix('x').or_else(|| tail.strip_prefix('X')) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else {
                tail.parse::<u32>().ok().and_then(char::from_u32)
            };
            match ch {
                // U+00A0 NO-BREAK SPACE → regular space (avoids wrapping surprises).
                Some('\u{00A0}') => out.push(' '),
                Some(c) => out.push(c),
                None => out.push_str(&rest[..=end]),
            }
        } else {
            match named_entity(entity) {
                Some(s) => out.push_str(s),
                None => out.push_str(&rest[..=end]),
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "mdash" => "—",
        "ndash" => "–",
        "minus" => "−",
        "hellip" => "…",
        "laquo" => "«",
        "raquo" => "»",
        "ldquo" => "\u{201C}",
        "rdquo" => "\u{201D}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "sbquo" => "\u{201A}",
        "bdquo" => "\u{201E}",
        "lsaquo" => "‹",
        "rsaquo" => "›",
        "prime" => "′",
        "Prime" => "″",
        "thinsp" => "\u{2009}",
        "ensp" => "\u{2002}",
        "emsp" => "\u{2003}",
        "zwj" => "\u{200D}",
        "zwnj" => "\u{200C}",
        "shy" => "",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "euro" => "€",
        "pound" => "£",
        "yen" => "¥",
        "cent" => "¢",
        "deg" => "°",
        "plusmn" => "±",
        "times" => "×",
        "divide" => "÷",
        "middot" => "·",
        "bull" => "•",
        "sdot" => "⋅",
        "frac12" => "½",
        "frac14" => "¼",
        "frac34" => "¾",
        "sup1" => "¹",
        "sup2" => "²",
        "sup3" => "³",
        "infin" => "∞",
        "sum" => "∑",
        "prod" => "∏",
        "radic" => "√",
        "ne" => "≠",
        "le" => "≤",
        "ge" => "≥",
        "asymp" => "≈",
        "equiv" => "≡",
        "permil" => "‰",
        "micro" => "µ",
        "para" => "¶",
        "sect" => "§",
        "dagger" => "†",
        "Dagger" => "‡",
        "clubs" => "♣",
        "diams" => "♦",
        "hearts" => "♥",
        "spades" => "♠",
        "larr" => "←",
        "uarr" => "↑",
        "rarr" => "→",
        "darr" => "↓",
        "harr" => "↔",
        "lArr" => "⇐",
        "uArr" => "⇑",
        "rArr" => "⇒",
        "dArr" => "⇓",
        "hArr" => "⇔",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> ArticleStyles {
        ArticleStyles::default()
    }

    fn collect_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn plain_text_round_trip() {
        let lines = html_to_lines("Hello world", s(), 0);
        assert!(collect_text(&lines).contains("Hello world"));
    }

    #[test]
    fn strips_simple_tags() {
        let lines = html_to_lines("<p>Hello <b>world</b></p>", s(), 0);
        assert!(collect_text(&lines).contains("world"));
    }

    #[test]
    fn entity_encoded_html_is_rendered() {
        // Entity-encoded HTML in RSS description fields.
        let lines = html_to_lines("&lt;em&gt;italic&lt;/em&gt;", s(), 0);
        assert!(collect_text(&lines).contains("italic"));
    }

    #[test]
    fn anchor_links_in_headings_are_dropped() {
        // Discourse-style: empty <a> navigation anchor inside a heading.
        let lines = html_to_lines(
            "<h2><a name=\"section\" href=\"#section\"></a>My Heading</h2>",
            s(),
            0,
        );
        let text = collect_text(&lines);
        assert!(text.contains("My Heading"), "got: {text}");
        assert!(!text.contains("#section"), "anchor leaked: {text}");
    }

    #[test]
    fn nested_lists_render() {
        let html = "<ul><li>Top<ul><li>Nested</li></ul></li></ul>";
        let text = collect_text(&html_to_lines(html, s(), 0));
        assert!(text.contains("Top"), "got: {text}");
        assert!(text.contains("Nested"), "got: {text}");
    }

    #[test]
    fn headings_detected() {
        let lines = html_to_lines("<h1>Title</h1><h2>Subtitle</h2>", s(), 0);
        let text = collect_text(&lines);
        assert!(text.contains("Title"), "got: {text}");
        assert!(text.contains("Subtitle"), "got: {text}");
    }

    #[test]
    fn link_footnotes_replace_inline_urls() {
        let html = r#"<p>See <a href="https://example.com">this page</a> for details.</p>"#;
        let lines = html_to_lines(html, s(), 0);
        let text = collect_text(&lines);
        // Link text + footnote number in body, URL in footnote section.
        assert!(text.contains("this page[1]"), "link text not found: {text}");
        assert!(
            text.contains("https://example.com"),
            "url not in footnotes: {text}"
        );
        // The URL should appear AFTER the separator, not inline.
        assert!(
            !text.contains("this page https://"),
            "url leaked inline: {text}"
        );
    }

    #[test]
    fn duplicate_links_reuse_footnote_number() {
        let html = r#"<p><a href="https://example.com">first</a> and <a href="https://example.com">second</a></p>"#;
        let text = collect_text(&html_to_lines(html, s(), 0));
        assert!(text.contains("first[1]"), "first link: {text}");
        assert!(text.contains("second[1]"), "second reuses [1]: {text}");
        // Only one footnote entry for the same URL.
        assert_eq!(
            text.matches("https://example.com").count(),
            1,
            "should be one footnote: {text}"
        );
    }

    #[test]
    fn markdown_links_converted() {
        let md = "See [the Dracula theme](https://draculatheme.com/) for details.";
        let text = collect_text(&html_to_lines(md, s(), 0));
        assert!(
            text.contains("the Dracula theme"),
            "link text missing: {text}"
        );
        assert!(
            text.contains("draculatheme.com"),
            "url in footnote missing: {text}"
        );
        // Should not show raw markdown syntax.
        assert!(!text.contains("](http"), "raw markdown leaked: {text}");
    }

    #[test]
    fn markdown_links_non_link_brackets_preserved() {
        let md = "This [not a link] stays literal.";
        let out = markdown_links_to_html(md);
        assert!(
            out.contains("[not a link]"),
            "bracket content should be preserved: {out}"
        );
        assert!(!out.contains("<a"), "should not be an anchor: {out}");
    }

    #[test]
    fn naked_url_highlighted() {
        let lines = html_to_lines("See https://example.com for details.", s(), 0);
        let text = collect_text(&lines);
        assert!(text.contains("https://example.com"), "url missing: {text}");
        // The URL must be isolated in its own span (not merged with surrounding text).
        let url_span = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|sp| sp.content.as_ref() == "https://example.com");
        assert!(
            url_span.is_some(),
            "url not in a dedicated span; spans: {:?}",
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
        );
    }

    // ── decode_html_entities() ────────────────────────────────────────────────

    #[test]
    fn entity_named_common() {
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&lt;"), "<");
        assert_eq!(decode_html_entities("&gt;"), ">");
        assert_eq!(decode_html_entities("&quot;"), "\"");
        assert_eq!(decode_html_entities("&apos;"), "'");
    }

    #[test]
    fn entity_decimal_numeric() {
        assert_eq!(decode_html_entities("&#65;"), "A");
        assert_eq!(decode_html_entities("&#8212;"), "\u{2014}"); // em dash
    }

    #[test]
    fn entity_hex_lowercase_x() {
        assert_eq!(decode_html_entities("&#x41;"), "A");
        assert_eq!(decode_html_entities("&#x2014;"), "\u{2014}");
    }

    #[test]
    fn entity_hex_uppercase_x() {
        assert_eq!(decode_html_entities("&#X41;"), "A");
        assert_eq!(decode_html_entities("&#X2014;"), "\u{2014}");
    }

    #[test]
    fn entity_nbsp_becomes_space() {
        assert_eq!(decode_html_entities("&nbsp;"), " ");
        assert_eq!(decode_html_entities("&#160;"), " ");
        assert_eq!(decode_html_entities("&#xA0;"), " ");
    }

    #[test]
    fn entity_consecutive_decoded() {
        assert_eq!(decode_html_entities("&lt;&gt;"), "<>");
        assert_eq!(decode_html_entities("A&amp;B&amp;C"), "A&B&C");
    }

    #[test]
    fn entity_unknown_named_left_as_is() {
        // Unknown entity: not in the lookup table → kept verbatim
        let s = "&notanentity;";
        let out = decode_html_entities(s);
        assert_eq!(out, s);
    }

    #[test]
    fn entity_incomplete_no_semicolon_left_as_is() {
        // & without ; = literal &
        assert_eq!(decode_html_entities("A&B"), "A&B");
    }

    #[test]
    fn entity_mixed_text_and_entities() {
        let out = decode_html_entities("Hello &amp; Goodbye &lt;World&gt;");
        assert_eq!(out, "Hello & Goodbye <World>");
    }

    #[test]
    fn entity_hex_leading_zero() {
        assert_eq!(decode_html_entities("&#x0041;"), "A");
    }

    // ── multibyte_char_after_link_open_does_not_panic ─────────────────────────

    #[test]
    fn multibyte_char_after_link_open_does_not_panic() {
        // Regression: find_a_close() was slicing `rest[..4]` as a &str byte
        // index, which panics when a multi-byte character (e.g. U+201C " = 3
        // bytes) immediately follows the `<` that was found.
        let html = r#"<p><a href="https://example.com"><u>"goose-stepping"</u></a> text.</p>"#;
        // Must not panic.
        let lines = html_to_lines(html, s(), 80);
        let text = collect_text(&lines);
        assert!(text.contains("goose-stepping"), "content missing: {text}");
    }
}
