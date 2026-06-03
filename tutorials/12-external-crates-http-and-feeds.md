# Lesson 12 — External Crates: HTTP and RSS/Atom Parsing

Two of the most interesting crates in thrssty are `ureq` (HTTP client) and `feed-rs` (feed
parser). This lesson shows how to evaluate, integrate, and work around the quirks of external
crates — a major part of practical Rust development.

---

## Choosing External Crates

Rust's ecosystem is large and quality varies. When picking a crate, consider:

| Question | Where to look |
|----------|--------------|
| Is it actively maintained? | GitHub commit history, open issues |
| Is it widely used? | [crates.io](https://crates.io) download stats |
| Does it have docs? | [docs.rs](https://docs.rs) |
| Does it pull in many dependencies? | `cargo tree` |

thrssty's choices for HTTP and parsing:

- **`ureq`**: synchronous HTTP client, single-threaded, minimal dependencies. Perfect for
  fetches that run on a background thread — no async runtime needed.
- **`feed-rs`**: parses RSS 0.9, RSS 1.0, RSS 2.0, Atom, and JSON Feed. Returns a unified
  `Feed` model regardless of the input format.

---

## Making an HTTP Request with `ureq`

```rust
use ureq;

let response = ureq::get("https://blog.rust-lang.org/feed.xml")
    .call()
    .map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;

let body: String = response
    .into_string()
    .map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;
```

`ureq::get(url).call()` returns `Result<Response, ureq::Error>`. Since `ureq::Error` is not
in thrssty's `AppError`, we use `.map_err(|e| AppError::FeedFetch { … })` to convert it before
propagating with `?`.

`response.into_string()` reads the response body as a `String`. It can fail (IO error or
non-UTF-8 content), so we handle that separately.

---

## Parsing a Feed with `feed-rs`

```rust
use feed_rs::parser;

let feed = parser::parse(body.as_bytes())
    .map_err(|e| AppError::FeedFetch {
        url: url.to_owned(),
        reason: e.to_string(),
    })?;
```

`parser::parse` accepts a `&[u8]` (byte slice) and returns a `feed_rs::model::Feed`. It
auto-detects the format (RSS or Atom) from the content.

### The `feed_rs::model` types

```rust
// Simplified from feed-rs's model
struct Feed {
    entries: Vec<Entry>,
    title: Option<Text>,
    // …
}

struct Entry {
    id: String,
    title: Option<Text>,
    authors: Vec<Person>,
    links: Vec<Link>,
    published: Option<DateTime<FixedOffset>>,
    updated: Option<DateTime<FixedOffset>>,
    summary: Option<Text>,
    content: Option<Content>,
    categories: Vec<Category>,
}

struct Person {
    name: String,
    email: Option<String>,
    uri: Option<String>,
}
```

These are the *external* types — they come from `feed-rs`, not from thrssty. We extract the
data we need and convert it into our own `Article` type.

---

## Mapping External Types to Internal Types

This is the core of `fetch_feed`:

```rust
let mut articles: Vec<Article> = feed
    .entries
    .into_iter()
    .map(|entry| {
        let description = entry.summary.as_ref().map(|s| s.content.clone());
        let content = entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_deref())
            .unwrap_or("")
            .to_owned();

        let published = entry
            .published
            .or(entry.updated)
            .map(|dt| dt.with_timezone(&Utc));

        let url = entry.links.into_iter().next().map(|l| l.href);

        let author = entry.authors.iter().find_map(person_display_name);

        let categories: Vec<String> =
            entry.categories.iter().map(|c| c.term.clone()).collect();

        Article {
            id: entry.id,
            title: entry.title.map(|t| t.content).unwrap_or_else(|| "Untitled".into()),
            url,
            author,
            published,
            description,
            categories,
            content,
        }
    })
    .collect();
```

Each external field requires some translation:

| External field | Translation | Reason |
|----------------|-------------|--------|
| `entry.summary` | `.map(|s| s.content.clone())` | `Text` → `String` |
| `entry.content.body` | `.as_ref().and_then(…).unwrap_or("")` | nested `Option<Option<&str>>` |
| `entry.published` | `.or(entry.updated).map(|dt| dt.with_timezone(&Utc))` | fall back to `updated`; convert timezone |
| `entry.links[0]` | `.into_iter().next().map(|l| l.href)` | take the first link's URL |
| `entry.authors` | `find_map(person_display_name)` | custom extraction (see below) |
| `entry.categories[*].term` | `.iter().map(|c| c.term.clone()).collect()` | flatten to `Vec<String>` |

---

## Working Around External Crate Quirks

External crates often have idiosyncrasies that do not match your expectations. Handling them is a
normal part of integration work.

### The RSS 2.0 author quirk

RSS 2.0 specifies that `<author>` should be an email address. `feed-rs` follows the spec:

```rust
fn handle_contact(role: &str, element: Element<R>) -> Option<Person> {
    element.child_as_text().map(|email| {
        let mut person = Person::new(role);   // name defaults to the role: "author"
        person.email = Some(email);           // raw text goes into email field
        person
    })
}
```

So for the Ubuntu blog's `<author>Benjamin Ryzman (Benjamin Ryzman)</author>`, feed-rs produces:

```
Person { name: "author", email: Some("Benjamin Ryzman (Benjamin Ryzman)") }
```

The actual name is in the `email` field, not `name`. thrssty's `person_display_name` function
handles this:

```rust
fn person_display_name(p: &Person) -> Option<String> {
    if let Some(raw) = &p.email {
        if raw.contains('@') {
            // Standard `email@example.com (Real Name)` format.
            if let Some(name) = extract_paren_suffix(raw) {
                return Some(name.to_owned());
            }
        } else {
            // Plain name, possibly with a duplicate "(Name)" suffix.
            let name = strip_duplicate_paren(raw);
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    // Fall back to `person.name` if it is a real name.
    if !p.name.is_empty() && p.name != "author" && !p.name.contains('@') {
        return Some(p.name.clone());
    }
    None
}

fn extract_paren_suffix(s: &str) -> Option<&str> {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") {
            return Some(&s[pos + 2..s.len() - 1]);
        }
    }
    None
}

fn strip_duplicate_paren(s: &str) -> &str {
    if s.ends_with(')') {
        if let Some(pos) = s.rfind(" (") {
            let before = &s[..pos];
            let inside = &s[pos + 2..s.len() - 1];
            if before == inside { return before; }
        }
    }
    s
}
```

The lesson: **read the crate's source** when the output does not match your expectations. The
`feed-rs` source (in `~/.cargo/registry/src/`) showed exactly what `handle_contact` does.

---

## The `chrono` and `Utc` Timezone

Feed timestamps come as `chrono::DateTime<FixedOffset>` (a timezone-aware datetime). thrssty
normalises them all to UTC for consistent storage and display:

```rust
.map(|dt| dt.with_timezone(&Utc))
```

`with_timezone` converts between timezone representations; the underlying instant in time is
unchanged.

---

## HTML to Text: `html2text`

Article content arrives as HTML. thrssty converts it to styled ratatui `Line`s using `html2text`
with its `RichDecorator`, which produces annotation-tagged output instead of plain text:

```rust
use html2text::{config, render::RichAnnotation};

let annotated = config::rich()
    .string_from_read(html_bytes, width)?;
```

`html2text` processes the HTML and emits lines of `(String, Vec<RichAnnotation>)` tuples —
each annotation marks a span of text as a heading level, a link, bold, italic, or code.
thrssty's `html_to_lines()` in `src/html.rs` maps each annotation to a ratatui `Style`:

```rust
for ann in annotations {
    match ann {
        RichAnnotation::Emphasis      => style = style.add_modifier(Modifier::ITALIC),
        RichAnnotation::Strong        => style = style.add_modifier(Modifier::BOLD),
        RichAnnotation::Code          => style = styles.code,
        RichAnnotation::Link(url)     => { style = styles.link; link_url = Some(url); }
        RichAnnotation::Colour(r,g,b) => { /* skip feed-defined colours */ }
        _                             => {}
    }
}
```

Links are also collected and appended as numbered footnotes (`[1] https://…`) so the reader
can see the URL without hovering — a natural affordance for a terminal environment.

Choosing `html2text` over a Markdown-intermediate approach (like the earlier `htmd` crate) means
thrssty can handle proper HTML tables, nested lists, and inline annotations without a lossy
Markdown conversion step.

---

## Opening URLs: `open`

thrssty uses the `open` crate to launch the system browser:

```rust
let _ = open::that_detached(&url);
```

`that_detached` opens the URL in the default browser and returns immediately — the browser
process is fully independent of thrssty. `that` (without `_detached`) would wait for the
browser to close.

---

## Inspecting the Dependency Tree

To see all transitive dependencies:

```bash
cargo tree
```

To find what pulls in a specific crate:

```bash
cargo tree -i serde
```

To check for outdated dependencies:

```bash
cargo install cargo-outdated
cargo outdated
```

---

## Summary

- Evaluate crates on docs quality, maintenance status, and transitive dependencies.
- Use `.map_err(|e| YourError { … })` to convert external error types to your unified error type.
- Read external crate source code (in `~/.cargo/registry/src/`) when behaviour is unexpected.
- Write adapter/translation functions to convert external types into your own domain types.
- Prefer `that_detached` over `that` when you do not want to block on an external process.
- `cargo tree` shows the full dependency graph.

Next: [Lesson 13 — Building a Terminal UI with ratatui](13-building-a-tui-with-ratatui.md)
