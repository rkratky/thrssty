# Lesson 05 — Structs, Enums, and Methods

Programs model the real world as data. In Rust, the two primary tools for defining custom data
types are **structs** (named collections of fields) and **enums** (a type that can be one of
several variants). Both can have methods attached to them.

---

## Structs

A struct groups related data under one name. Every field has a name and a type:

```rust
struct Point {
    x: f64,
    y: f64,
}
```

Create an instance by naming each field:

```rust
let p = Point { x: 1.0, y: 2.5 };
println!("{}", p.x);   // field access with dot notation
```

### thrssty's core structs

`src/feed/mod.rs` defines the two data-carrying types at the heart of the application:

```rust
pub struct Article {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub content: String,
}

pub struct Feed {
    pub url: String,
    pub name: String,
    pub articles: Vec<Article>,
    pub last_fetched: Option<DateTime<Utc>>,
}
```

Key observations:
- `pub` before a field makes it accessible from outside the module (visibility is covered in
  Lesson 08).
- `Option<T>` represents a value that may or may not exist — the Rust replacement for null
  (covered in Lesson 06).
- `Vec<Article>` is a dynamic array of articles. `Vec<T>` is covered in Lesson 09.
- `DateTime<Utc>` comes from the `chrono` crate — Rust's crates compose naturally into types.

---

## `impl` Blocks: Attaching Methods

Methods are functions that are associated with a type. They live in an `impl` block:

```rust
impl Feed {
    /// Create an empty feed (articles not yet loaded).
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            name: name.into(),
            articles: Vec::new(),
            last_fetched: None,
        }
    }
}
```

### `Self` and `self`

- `Self` (capital S) refers to the type being implemented — here, `Feed`.
- `self` (lowercase) is the receiver — the instance the method is called on.

| Signature | Meaning |
|-----------|---------|
| `fn f(self)` | Takes ownership of the instance (consumed after the call). |
| `fn f(&self)` | Borrows the instance immutably (read-only access). |
| `fn f(&mut self)` | Borrows the instance mutably (read-write access). |
| `fn f()` — no `self` | Associated function, called as `Type::f()` (like a static method). |

`Feed::new` is an associated function — you call it as `Feed::new(url, name)`, not on an
instance.

### Multiple `impl` blocks

A type can have multiple `impl` blocks. thrssty's `Cache` has one block with several methods:

```rust
impl Cache {
    pub fn cache_path() -> Result<PathBuf> { … }   // associated function
    pub fn load() -> Result<Self> { … }            // associated function
    pub fn save(&self) -> Result<()> { … }         // shared borrow — reads cache
    pub fn is_read(&self, feed_url: &str, article_id: &str) -> bool { … }
    pub fn mark_read(&mut self, feed_url: &str, article_id: &str) { … }  // mutable borrow
    pub fn unread_count(&self, feed_url: &str, articles: &[Article]) -> usize { … }
    pub fn purge_old_articles(&mut self, max_age_days: u64) { … }
}
```

---

## Nested Structs and Composition

Structs can contain other structs:

```rust
pub struct Config {
    pub theme: String,
    pub refresh_on_start: bool,
    pub feeds_file: Option<String>,
    pub uncategorized_label: String,
    pub max_article_age_days: u64,
    pub layout: LayoutConfig,   // nested struct
    pub keys: KeysConfig,       // nested struct
}

pub struct LayoutConfig {
    pub sidebar_width: u16,
    pub article_list_height: u16,
}

pub struct KeysConfig {
    pub quit: String,
    pub refresh_feed: String,
    // … more key bindings …
}
```

Nested structs are accessed with chained dot notation:

```rust
let width = config.layout.sidebar_width;
let quit_key = &config.keys.quit;
```

---

## Struct Update Syntax

When creating a new struct instance and most fields come from an existing one:

```rust
let new_feed = Feed {
    name: "Override Name".to_owned(),
    ..old_feed         // fill remaining fields from old_feed (moves or copies them)
};
```

---

## Enums

An **enum** (enumeration) defines a type that can be one of several named **variants**. Unlike a
C enum (which is just an integer alias), Rust enum variants can carry data.

### Simple enum (no data)

```rust
enum Direction { North, South, East, West }

let dir = Direction::North;
match dir {
    Direction::North => println!("going north"),
    Direction::South => println!("going south"),
    Direction::East  => println!("going east"),
    Direction::West  => println!("going west"),
}
```

### Variants with data

```rust
pub enum SidebarItem {
    /// A category header row (not selectable by the cursor).
    Category {
        title: String,
        feed_indices: Vec<usize>,
    },
    /// A feed row.
    Feed {
        feed_index: usize,
        indented: bool,
    },
}
```

From `src/feeds_config.rs`. A `SidebarItem` is either a `Category` (with a title and a list of
feed indices) or a `Feed` (with an index and an indent flag). The two variants have completely
different shapes.

Matching against such an enum destructures the contained data:

```rust
match item {
    SidebarItem::Category { title, feed_indices } => {
        // title: String, feed_indices: Vec<usize>
        let name = truncate(title, name_width);
    }
    SidebarItem::Feed { feed_index, indented } => {
        let feed = app.feeds.get(*feed_index);
        let indent = if *indented { "  " } else { "" };
    }
}
```

### Tuple-like variants

Variants can also carry unnamed (positional) data:

```rust
enum Color {
    Rgb(u8, u8, u8),
    Named(String),
}

match color {
    Color::Rgb(r, g, b) => println!("#{:02X}{:02X}{:02X}", r, g, b),
    Color::Named(s)     => println!("{}", s),
}
```

---

## `Option<T>`: The Absence of a Value

`Option` is a built-in enum defined in the standard library:

```rust
pub enum Option<T> {
    None,        // nothing here
    Some(T),     // there is a value of type T
}
```

Every place where a value might be absent, thrssty uses `Option` instead of nullable pointers or
sentinel values:

```rust
pub author: Option<String>,        // an article might not have an author
pub published: Option<DateTime<Utc>>,  // it might not have a publish date
pub url: Option<String>,           // it might not have a URL
```

Working with `Option`:

```rust
// if let — the most common pattern
if let Some(author) = &article.author {
    lines.push(format!("Author: {}", author));
}

// map — transform the inner value if present
let date_str = article.published
    .map(|d| d.format("%Y-%m-%d").to_string())
    .unwrap_or_else(|| " ".repeat(10));   // fallback when None

// unwrap_or — provide a default
let name = fc.name.clone().unwrap_or_else(|| fc.url.clone());
```

---

## `FetchMessage`: an Enum as a Message Protocol

The background fetch thread communicates with the UI thread by sending `FetchMessage` values
through a channel. The enum is the communication protocol:

```rust
pub enum FetchMessage {
    Started  { url: String },
    Complete { url: String, feed_title: Option<String>, articles: Vec<Article> },
    Error    { url: String, reason: String },
    Done,
}
```

When the UI thread receives a message, it `match`es on it to decide what to do:

```rust
match msg {
    FetchMessage::Started { url } => {
        self.ui.status_message = Some(format!("Fetching {}…", url));
    }
    FetchMessage::Complete { url, feed_title, articles } => {
        self.apply_fetch_result(&url, feed_title, articles);
    }
    FetchMessage::Error { url, reason } => {
        self.feed_errors.insert(url.clone());
        self.ui.status_message = Some(format!("Error: {}", reason));
        self.ui.status_is_error = true;
    }
    FetchMessage::Done => {
        self.fetch_rx = None;
        let _ = self.cache.save();
    }
}
```

Using an enum here instead of a struct with a `type: String` field makes exhaustiveness
checking work: if we add a new variant later, the compiler will point out every `match` that
does not handle it yet.

---

## Derived Traits

Many standard behaviours are available automatically via `#[derive(…)]`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Article { … }
```

| Trait | What it provides |
|-------|-----------------|
| `Debug` | `{:?}` formatting — print the struct for debugging |
| `Clone` | `.clone()` — deep copy |
| `PartialEq`, `Eq` | `==` and `!=` comparison |
| `Default` | `Type::default()` — a zero/empty value |
| `Serialize`, `Deserialize` | JSON/TOML read and write (from `serde`) |

The `#[derive]` attribute is a **procedural macro** that generates the implementation
automatically. You will meet more macros and traits in Lessons 07 and 10.

---

## Summary

- **Structs** group named fields into a single type; methods are added with `impl` blocks.
- Methods take `self`, `&self`, or `&mut self` as their first parameter; associated functions take none.
- **Enums** define a type with multiple variants, each optionally carrying different data.
- `Option<T>` is the standard way to express "this value might not be present".
- `#[derive(…)]` automatically implements common traits.
- Matching on enums is exhaustive — the compiler catches missing cases.

Next: [Lesson 06 — Error Handling](06-error-handling.md)
