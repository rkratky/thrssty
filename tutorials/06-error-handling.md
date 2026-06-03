# Lesson 06 — Error Handling

Errors are a fact of life in any program that talks to the outside world — reads files, makes
network requests, parses user input. Rust has no exceptions. Instead, functions that can fail
return a `Result<T, E>` value, making it impossible to accidentally ignore an error.

---

## `Result<T, E>`

`Result` is another built-in enum:

```rust
pub enum Result<T, E> {
    Ok(T),    // success: contains the value of type T
    Err(E),   // failure: contains the error of type E
}
```

A function that might fail returns `Result`:

```rust
fn read_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
```

The caller must handle both cases:

```rust
match read_file("config.toml") {
    Ok(content) => println!("Got {} bytes", content.len()),
    Err(e)      => println!("Failed: {}", e),
}
```

---

## The `?` Operator

Writing `match` for every fallible call is verbose. The `?` operator is shorthand: it unwraps
an `Ok` value or *immediately returns* the `Err` from the current function:

```rust
fn load_and_parse(path: &str) -> Result<Config, Error> {
    let content = std::fs::read_to_string(path)?;   // returns early on Err
    let config = toml::from_str(&content)?;          // returns early on Err
    Ok(config)
}
```

`?` can only be used inside functions that return `Result` (or `Option`). It is by far the most
common pattern in thrssty for propagating errors upward to the caller.

---

## A Unified Error Type

When a function can fail in multiple ways — IO errors, parse errors, network errors — the
return type needs an error type broad enough to hold all of them. thrssty defines one error enum
to cover the entire application: `src/error.rs`.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Config serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    #[error("Cache error: {0}")]
    Cache(#[from] serde_json::Error),

    #[error("Feed fetch error for '{url}': {reason}")]
    FeedFetch { url: String, reason: String },

    #[error("Theme error in '{file}': {reason}")]
    ThemeParse { file: String, reason: String },

    #[error("Feeds file error in '{file}': {reason}")]
    FeedsFileParse { file: String, reason: String },

    #[error("Config directory not found")]
    MissingConfigDir,

    #[error("Data directory not found")]
    MissingDataDir,
}

/// Convenience Result type using AppError.
pub type Result<T> = std::result::Result<T, AppError>;
```

### What `thiserror` does

`thiserror` is a crate that generates boilerplate error trait implementations via `#[derive(Error)]`.
Without it you would write dozens of lines of repetitive `impl std::error::Error for AppError` code.

The `#[error("…")]` attribute on each variant defines the human-readable error message. The
format string is like `format!`:
- `{0}` refers to the first field (for tuple-like variants like `Io(std::io::Error)`).
- `{url}`, `{reason}` refer to named fields.

### `#[from]`

The `#[from]` attribute on a field means: automatically implement `From<FieldType> for AppError`,
which allows `?` to convert those errors into `AppError` automatically:

```rust
let content = std::fs::read_to_string(&path)?;
//                                           ^ std::io::Error is converted to AppError::Io
```

Without `#[from]`, you would write:

```rust
let content = std::fs::read_to_string(&path).map_err(AppError::Io)?;
```

### Type alias

```rust
pub type Result<T> = std::result::Result<T, AppError>;
```

This defines `thrssty::error::Result<T>` as shorthand for `std::result::Result<T, AppError>`.
Every function in thrssty uses this alias:

```rust
pub fn load() -> Result<Self> { … }
pub fn save(&self) -> Result<()> { … }
```

---

## Constructing Errors Manually

Variants with named fields are constructed directly:

```rust
return Err(AppError::FeedFetch {
    url: url.to_owned(),
    reason: e.to_string(),
});
```

Or using `map_err` to convert a different error type:

```rust
let response = ureq::get(url).call().map_err(|e| AppError::FeedFetch {
    url: url.to_owned(),
    reason: e.to_string(),
})?;
```

`map_err` transforms the `Err` variant while leaving `Ok` unchanged — then `?` propagates it.

---

## Handling Errors at the Top Level

The `?` operator propagates errors upward. Eventually they reach `main`, which must handle them.
thrssty's `main` uses a wrapper function `run()` that returns `Result<()>`:

```rust
fn main() {
    if let Err(e) = run() {
        eprintln!("thrssty error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = Config::load()?;
    let cache = Cache::load()?;
    // …
    Ok(())
}
```

When `run()` returns `Err`, `main` prints the error message (which `thiserror` made human-readable)
and exits with status code 1. The user sees one clean error line, not a stack trace.

---

## Ignoring Errors Deliberately

Sometimes an error genuinely does not matter — you try something and move on if it fails. Use
`let _ = …`:

```rust
let _ = std::fs::remove_file(&path);   // ignore error: if we can't delete it, no big deal
let _ = app.on_exit();                 // ignore error: we're quitting anyway
```

`let _ = …` explicitly tells the compiler (and future readers) "I know this might fail and I
am intentionally not handling the error".

---

## The Cache Schema Example

The cache migration code demonstrates all error-handling patterns together:

```rust
pub fn load() -> Result<Self> {
    let path = Self::cache_path()?;        // propagate directory errors

    if !path.exists() {
        return Ok(Self::default());        // early Ok return
    }

    let content = std::fs::read_to_string(&path)?;  // propagate IO errors

    match serde_json::from_str::<Self>(&content) {
        Ok(cache) => Ok(cache),
        Err(e) => {
            // Schema mismatch — print a warning and start fresh.
            println!(
                "thrssty: cache schema has changed ({}); discarding stale cache.",
                e
            );
            let _ = std::fs::remove_file(&path);   // ignore removal error
            Ok(Self::default())                    // recover gracefully
        }
    }
}
```

- `?` propagates unrecoverable errors (missing home directory, disk error).
- `match` handles the one case where we can recover (parse failure → start fresh).
- `let _ =` ignores the error from a best-effort cleanup.
- All return paths are `Ok(…)` or propagate through `?` — a `Result<Self>` is always produced.

---

## `unwrap` and `expect` — and When Not to Use Them

Two convenience methods exist for extracting `Ok` or `Some` values:

```rust
let value = some_result.unwrap();          // panics if Err
let value = some_result.expect("my msg"); // panics with "my msg" if Err
```

**Panics terminate the program immediately.** Use `unwrap`/`expect` only when:
- You have *proven* (by logic) that the value cannot be `None`/`Err`, or
- You are writing a quick prototype and will replace it later.

In thrssty, `unwrap` appears once — for the built-in theme, which is parsed from a compile-time
constant and therefore guaranteed to be valid:

```rust
pub fn load_builtin() -> Self {
    Self::parse(DEFAULT_THEME).unwrap_or_else(|_| Self::empty())
}
```

Note: even here we use `unwrap_or_else` (fall back to an empty theme) rather than `unwrap`
(panic). This is the correct pattern for library/application code.

---

## Summary

- Functions that can fail return `Result<T, E>`.
- The `?` operator propagates errors; it can only be used in functions returning `Result`.
- Define a single application-level error enum and use `#[from]` to convert standard errors automatically.
- `thiserror`'s `#[derive(Error)]` generates boilerplate so you do not have to.
- `type Result<T> = std::result::Result<T, AppError>` is a useful shorthand alias.
- Use `let _ = …` to explicitly ignore errors; avoid `unwrap()` in production code.

Next: [Lesson 07 — Traits, Generics, and the Standard Library](07-traits-generics-and-stdlib.md)
