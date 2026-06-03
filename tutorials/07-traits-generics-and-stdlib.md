# Lesson 07 — Traits, Generics, and the Standard Library

## What Is a Trait?

A **trait** defines a set of methods that a type must implement. It is similar to an interface in
Java or Go, but more powerful. Once a type implements a trait, it can be used anywhere that trait
is expected.

```rust
trait Greet {
    fn hello(&self) -> String;
}

struct Person { name: String }

impl Greet for Person {
    fn hello(&self) -> String {
        format!("Hi, I'm {}!", self.name)
    }
}
```

---

## Standard Traits

The Rust standard library defines dozens of traits. The most important ones for thrssty are:

### `Debug`

Allows `{:?}` formatting — prints the struct/enum in a developer-friendly way:

```rust
#[derive(Debug)]
struct Feed { url: String, name: String }

let f = Feed { url: "…".into(), name: "Rust Blog".into() };
println!("{:?}", f);   // Feed { url: "…", name: "Rust Blog" }
```

Every public type in thrssty derives `Debug` so you can print them during development.

### `Clone`

Allows explicit deep copying with `.clone()`. Types that wrap heap data (like `String` and `Vec`)
need this; scalar types that fit on the stack are `Copy` instead.

```rust
#[derive(Clone)]
struct Article { title: String, content: String }

let a1 = Article { … };
let a2 = a1.clone();   // a1 is still valid
```

### `Default`

Provides a zero/empty value via `Type::default()`:

```rust
#[derive(Default)]
struct UiState {
    selected_feed: usize,     // default: 0
    article_scroll: u16,      // default: 0
    status_message: Option<String>,  // default: None
}

let state = UiState::default();
```

When you write `#[serde(default)]` on a field (seen in Lesson 10), serde calls `.default()` on
the field type when it is absent from the serialised data.

### `PartialEq` and `Eq`

Enable `==` and `!=` comparisons:

```rust
#[derive(PartialEq, Eq)]
struct ArticleId(String);

let id1 = ArticleId("abc".into());
let id2 = ArticleId("abc".into());
assert!(id1 == id2);
```

### `From` and `Into`

Conversion between types. Implementing `From<A> for B` automatically gives you `Into<B> for A`:

```rust
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}
```

`thiserror`'s `#[from]` generates this implementation automatically. Once it exists, `?` can
convert `std::io::Error` to `AppError` in any function returning `Result<_, AppError>`.

### `Display`

Enables `{}` formatting — the human-readable form shown to users:

```rust
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // thiserror's #[error("…")] generates this automatically.
        // A manual implementation would match on variants and write each message:
        match self {
            AppError::Io(e) => write!(f, "I/O error: {e}"),
            // … other variants
            _ => write!(f, "{:?}", self),
        }
    }
}
```

`thiserror`'s `#[error("…")]` generates `Display` for you.

---

## Generics

Generics let you write code that works with many different types without duplicating it. You have
already used them: `Option<T>`, `Result<T, E>`, and `Vec<T>` are all generic.

### Generic functions

```rust
fn first<T>(v: &[T]) -> Option<&T> {
    v.first()   // returns the first element, or None if empty
}

let nums = vec![1, 2, 3];
let strs = vec!["a", "b", "c"];
let _ = first(&nums);   // T = i32
let _ = first(&strs);   // T = &str
```

The type parameter `T` is determined at the call site. Rust generates a separate specialised copy
for each concrete type — this is called **monomorphisation** and has zero runtime cost.

### Generic structs

`FeedsFile::into_layout` accepts `uncategorized_label: &str` — but `Feed::new` accepts any type
that can be converted *into* a `String`:

```rust
pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
    Self {
        url: url.into(),
        name: name.into(),
        articles: Vec::new(),
        last_fetched: None,
    }
}
```

`impl Into<String>` is shorthand for "any type T where T: Into<String>". This means the caller
can pass a `&str` or a `String` and both work without explicit conversion:

```rust
Feed::new("https://example.com/feed", "Example")    // &str works
Feed::new(fc.url.clone(), fc.name.clone().unwrap())  // String works
```

### Trait bounds

Generics can be constrained with **trait bounds** — "T must implement these traits":

```rust
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    // T must support comparison with PartialOrd
    let mut biggest = &list[0];
    for item in list {
        if item > biggest {
            biggest = item;
        }
    }
    biggest
}
```

thrssty uses trait bounds in the sorting of articles:

```rust
articles.sort_by(|a, b| b.published.cmp(&a.published));
```

`cmp` requires `T: Ord` on `Option<DateTime<Utc>>`, which is satisfied because `Option<T>` is
`Ord` when `T` is `Ord`.

---

## Trait Objects: `dyn Trait`

Sometimes you do not know the concrete type at compile time — you only know it implements a
trait. A **trait object** (`dyn Trait`) lets you use the trait through a pointer at runtime:

```rust
fn parse(content: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
    let value: toml::Value = toml::from_str(content)?;
    // …
}
```

`Box<dyn std::error::Error>` is "a pointer to any type that implements `std::error::Error`".
The internal `parse` helper in `src/theme.rs` uses this so it can return errors from different
crates (`toml::de::Error`, a string error, etc.) without a unified error enum just for that one
private function.

---

## The `Iterator` Trait

The `Iterator` trait is one of the most important in Rust. Any type that implements it gains
access to a huge library of adapter methods (`.map()`, `.filter()`, `.fold()`, `.collect()`, …).

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    // hundreds of default methods built on top of next()
}
```

Most collections provide `.iter()` which returns an iterator:

```rust
for article in articles.iter() {
    // article: &Article
}
```

Iterators are **lazy** — they do no work until consumed. Building an iterator chain costs nothing;
only calling `.collect()` or using the value in a `for` loop actually runs the computation:

```rust
let unread_count = articles
    .iter()                          // create an iterator (no work yet)
    .filter(|a| !self.is_read(…))   // attach a filter (no work yet)
    .count();                         // consume: now the work happens
```

Iterators are covered in much more depth in Lesson 09.

---

## The `Write` Trait and `format!`

The `format!` macro builds a `String`. Under the hood it uses the `Display` trait. The related
`write!` and `writeln!` macros write to anything implementing the `Write` or `fmt::Write` trait:

```rust
use std::fmt::Write as _;   // String implements fmt::Write, not io::Write

let mut s = String::new();
write!(&mut s, "Hello, {}!", "world").unwrap();   // s is now "Hello, world!"
```

---

## Implementing Traits for External Types: The Orphan Rule

Rust has the **orphan rule**: you can implement a trait for a type only if either the trait or
the type is defined in your crate. This prevents two crates from providing conflicting
implementations.

thrssty never violates this rule. Where it needs custom behaviour on an external type, it writes
a wrapper function instead:

```rust
// Instead of implementing Display for feed_rs::Person (not allowed — both are external),
// we write a local helper function:
fn person_display_name(p: &Person) -> Option<String> { … }
```

---

## Key Traits Used in thrssty

| Trait | Source | Used for |
|-------|--------|----------|
| `Debug` | `std` | Printing structs during development |
| `Clone` | `std` | Copying `Config`, `ArticleStyles`, cached feeds |
| `Default` | `std` | Zero-initialising `UiState`, `Cache`, `SidebarLayout` |
| `PartialEq`, `Eq` | `std` | Comparing articles (for deduplication) |
| `From` | `std` | Auto-converting errors with `#[from]` and `?` |
| `Display` | `std` | Human-readable error messages via `thiserror` |
| `Iterator` | `std` | Transforming collections (feeds, articles, sidebar items) |
| `Into<String>` | `std` | Flexible `Feed::new` constructor parameters |
| `Serialize`, `Deserialize` | `serde` | TOML config and JSON cache I/O |
| `Error` | `std` | Marking types as errors; `thiserror` implements this |

---

## Summary

- Traits define shared behaviour; `impl Trait for Type` provides the implementation.
- `#[derive(…)]` generates trait implementations automatically for common cases.
- Generics (`<T>`, `impl Trait`) write code that works for many types without duplication.
- Trait bounds constrain generics: `T: PartialOrd` means "T must be comparable".
- `dyn Trait` is a trait object — a runtime-polymorphic pointer to any implementing type.
- `Iterator` is the central trait for processing collections; it is lazy and composable.
- The orphan rule prevents conflicting implementations from different crates.

Next: [Lesson 08 — Modules and Project Structure](08-modules-and-project-structure.md)
