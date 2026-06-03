# Lesson 10 — Serialisation with Serde

Programs need to save data to disk and read it back. thrssty does this in two places:

- **Config** (`~/.config/thrssty/config.toml`) — read on startup, in TOML format.
- **Cache** (`~/.local/share/thrssty/cache.json`) — read on startup, written on exit and
  after each fetch, in JSON format.

The [`serde`](https://serde.rs) crate handles both. It is Rust's standard serialisation
framework: fast, zero-copy where possible, and completely type-driven.

---

## The Core Idea

`serde` works through two traits:

- `Serialize` — convert a Rust value *to* a data format.
- `Deserialize` — construct a Rust value *from* a data format.

And two format-specific crates:

- `serde_json` — JSON support.
- `toml` — TOML support.

Together they look like this:

```rust
// Serialise: Rust struct → JSON string
let json = serde_json::to_string(&my_value)?;

// Deserialise: JSON string → Rust struct
let value: MyType = serde_json::from_str(&json)?;

// Serialise: Rust struct → TOML string
let toml_str = toml::to_string(&my_value)?;

// Deserialise: TOML string → Rust struct
let value: MyType = toml::from_str(&toml_str)?;
```

The format-specific details are hidden inside `serde_json` and `toml`. Your struct code is
identical regardless of format.

---

## Deriving `Serialize` and `Deserialize`

The most common way to make a type serialisable is with `#[derive]`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    // …
}
```

That is all. The `serde` procedural macro generates the implementation automatically, handling
every field. The `serde = { version = "1", features = ["derive"] }` entry in `Cargo.toml` enables
these macros.

---

## The Cache: JSON

### Saving

```rust
pub fn save(&self) -> Result<()> {
    let path = Self::cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;   // create dirs if needed
    }
    let content = serde_json::to_string_pretty(self)?;   // Rust → JSON
    std::fs::write(&path, content)?;
    Ok(())
}
```

`to_string_pretty` produces indented JSON for human readability. `to_string` would produce
compact JSON with no whitespace.

### Loading

```rust
pub fn load() -> Result<Self> {
    let path = Self::cache_path()?;
    if !path.exists() {
        return Ok(Self::default());   // no cache file yet → empty cache
    }
    let content = std::fs::read_to_string(&path)?;
    match serde_json::from_str::<Self>(&content) {
        Ok(cache) => Ok(cache),
        Err(e) => {
            println!("thrssty: cache schema has changed ({}); discarding stale cache.", e);
            let _ = std::fs::remove_file(&path);
            Ok(Self::default())
        }
    }
}
```

`from_str::<Self>` is *turbofish syntax* — the `::<Self>` tells the compiler what type to
deserialise into. Without it, the compiler could not infer the target type.

---

## The Config: TOML

### Loading

```rust
pub fn load() -> Result<Self> {
    let path = Self::config_path()?;
    if !path.exists() {
        return Ok(Self::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}
```

The TOML library deserialises directly into `Config`:

```toml
# config.toml
theme = "auto"          # "auto" detects dark/light terminal; or a theme name/path
refresh_on_start = true
max_article_age_days = 100

[layout]
sidebar_width = 20
article_list_height = 40
max_author_col_pct = 25   # max % of article-list width given to the author column

[keys]
quit = "q"
feed_down = "j"
feed_up = "k"
```

Each TOML key maps to a struct field by name. Nested tables (`[layout]`, `[keys]`) map to nested
structs (`LayoutConfig`, `KeysConfig`).

---

## Serde Attributes

Serde's behaviour can be customised with field-level and container-level attributes.

### `#[serde(default)]` — use `Default` when the field is absent

```rust
#[serde(default = "defaults::sidebar_width")]
pub sidebar_width: u16,
```

When the field is missing from the TOML file, call `defaults::sidebar_width()` to get the value.
Without this attribute, a missing field would be a deserialisation error.

```rust
#[serde(default)]
pub categories: Vec<String>,
```

When `categories` is absent (older cache files), use `Vec::default()` — an empty vec. This is
how thrssty stays backward-compatible when new fields are added to a struct: old cache files
that do not have the field simply get the default value.

### `#[serde(default = "function")]` vs `#[serde(default)]`

| Attribute | Calls |
|-----------|-------|
| `#[serde(default)]` | `Default::default()` for the field's type |
| `#[serde(default = "path")]` | The specified function (must return the field's type) |

thrssty uses named functions so the defaults are self-documenting and centralised:

```rust
mod defaults {
    pub fn sidebar_width() -> u16 { 20 }
    pub fn article_list_height() -> u16 { 40 }
    pub fn quit() -> String { "q".into() }
    // …
}

pub struct LayoutConfig {
    #[serde(default = "defaults::sidebar_width")]
    pub sidebar_width: u16,
    #[serde(default = "defaults::article_list_height")]
    pub article_list_height: u16,
}
```

### `#[serde(rename = "…")]` — use a different key name in the serialised format

Not used in thrssty, but common in APIs where the JSON field names use camelCase while Rust
conventions use snake_case:

```rust
#[serde(rename = "feedUrl")]
pub feed_url: String,
```

---

## The `toml::Value` Escape Hatch

Sometimes you need to parse TOML without knowing the exact schema upfront — for example, when
parsing the feeds file, which has a flexible structure:

```rust
let value: toml::Value = toml::from_str(content).map_err(|e| e.to_string())?;
let table = value.as_table().ok_or("feeds file must be a TOML table")?;
```

`toml::Value` is an enum representing any TOML value:

```rust
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Datetime(Datetime),
    Array(Array),
    Table(Table),
}
```

You can inspect it with `.as_str()`, `.as_table()`, `.as_array()`, etc., which all return
`Option<…>`. This is how `src/feeds_config.rs` parses the category/feed hierarchy manually,
because the `[category.1]`, `[category.2]`, … structure does not map naturally to a fixed-layout
Rust struct.

---

## Schema Evolution: Adding New Fields Safely

When you add a new field to a serialised struct, old files (without the field) will fail to
deserialise unless you mark it with `#[serde(default)]`.

The pattern thrssty uses:

1. Add the field with `#[serde(default)]`.
2. The new field gets a sensible default from old files.
3. If the schema change is more drastic (e.g. a field type changes), the cache
   deserialisation will fail and fall back to a fresh empty cache with a printed warning.

```rust
/// Categories / tags attached to this entry.
#[serde(default)]
pub categories: Vec<String>,
```

This was added after the initial release. Old cache files without `categories` get an empty vec
automatically.

---

## Finding Config Directories with `dirs`

Rust programs should not hardcode paths like `/home/user/.config`. The `dirs` crate provides
platform-appropriate directories:

```rust
use dirs;

dirs::config_dir()   // ~/.config on Linux, ~/Library/Application Support on macOS
dirs::data_dir()     // ~/.local/share on Linux, ~/Library/Application Support on macOS
```

thrssty uses these to build its paths:

```rust
pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .ok_or(AppError::MissingConfigDir)?  // dirs returns Option, convert to Result
        .join("thrssty")
        .join("config.toml"))
}

pub fn cache_path() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .ok_or(AppError::MissingDataDir)?
        .join("thrssty")
        .join("cache.json"))
}
```

`.join()` extends a `PathBuf` with a new component, handling OS-specific separators.

---

## Summary

- `serde` provides `Serialize` and `Deserialize` traits; `#[derive]` generates them automatically.
- `serde_json` handles JSON; `toml` handles TOML — the struct code is the same for both.
- `#[serde(default)]` marks fields that are optional in the file, using `Default::default()`.
- `#[serde(default = "fn")]` calls a named function for the default.
- `toml::Value` is an untyped TOML tree for flexible/dynamic parsing.
- Use `dirs` for platform-correct config and data directories.
- Always add `#[serde(default)]` to new fields in existing serialised structs for backward compatibility.

Next: [Lesson 11 — Concurrency: Threads and Channels](11-concurrency-threads-and-channels.md)
