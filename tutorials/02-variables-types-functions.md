# Lesson 02 — Variables, Types, and Functions

## Variables and Mutability

In Rust, every variable is **immutable by default**. You cannot change its value after it is set
unless you explicitly opt in with `mut`.

```rust
let x = 5;        // immutable — trying to reassign is a compile error
let mut y = 10;   // mutable — can be reassigned
y = 20;           // OK
```

This default-immutability might feel restrictive, but it is one of the key ways Rust prevents
bugs: if you never intended to change a value, the compiler enforces that intent.

### In thrssty

Look at `src/config.rs` — every field we read from the config file never needs to change after
loading, so they are all immutable:

```rust
let config = Config::load()?;   // config is immutable
```

The `mut` keyword appears only where we explicitly need to modify something, such as the list of
articles after a fetch:

```rust
let mut articles: Vec<Article> = feed.entries.into_iter().map(|entry| { … }).collect();
articles.sort_by(|a, b| b.published.cmp(&a.published));   // we sort in place → needs mut
```

---

## Scalar Types

Rust has a small set of **primitive scalar types** — a *scalar* is a single indivisible value (one number, one boolean, one character), as opposed to compound types that group several values together:

### Integers

| Type | Size | Range |
|------|------|-------|
| `i8` | 8-bit | −128 to 127 |
| `i32` | 32-bit | −2 billion to 2 billion |
| `i64` | 64-bit | ≈ −9.2 × 10¹⁸ to 9.2 × 10¹⁸ |
| `u8` | 8-bit unsigned | 0 to 255 |
| `u16` | 16-bit unsigned | 0 to 65,535 |
| `u64` | 64-bit unsigned | 0 to ≈ 1.8 × 10¹⁹ |
| `usize` | pointer-width | 0 to architecture max |

`usize` is the natural indexing type — use it for collection lengths and indices.

In thrssty, `u16` is used for percentages (always 0–100) and terminal dimensions (terminals
rarely exceed 65,535 columns):

```rust
pub sidebar_width: u16,        // percentage of viewport width
pub article_list_height: u16,  // percentage of right-panel height
pub max_author_col_pct: u16,   // max author column width as % of list panel
```

And `u64` for a potentially large day count:

```rust
pub max_article_age_days: u64,
```

### Booleans

```rust
let is_read: bool = true;
let has_categories: bool = false;
```

### Floating point

`f32` (32-bit) and `f64` (64-bit). thrssty does not use floating point — all percentages and
counts are integers.

---

## The String Types

Rust has two main string types:

| Type | Meaning |
|------|---------|
| `String` | An owned, heap-allocated, growable UTF-8 string. |
| `&str` | A borrowed reference to a string slice (no allocation). |

Think of `String` as a value you own and can modify, and `&str` as a read-only view into an
existing string (which might be a `String`, a string literal, or part of another string).

*Two memory regions matter here: the **stack** holds data of fixed, known size — integers, references, struct instances with no heap data. The **heap** is a larger pool for dynamic allocations. `String` stores its characters on the heap because a string can grow or shrink; `&str` is just a pointer (on the stack) into wherever that string data lives — it owns nothing.*

```rust
let owned: String = "hello".to_owned();   // or: String::from("hello")
let borrowed: &str = "world";             // string literals are &'static str
let also_borrowed: &str = &owned;         // borrow a view of the String
```

**String literals** (`"hello"`) have type `&'static str` — they are baked into the binary and
live for the entire program lifetime. A *literal* in general is any value written directly in source code — `42`, `true`, and `"hello"` are all literals; the compiler converts them to constants in the compiled output.

### In thrssty

Feed URLs, names, and article titles are `String` because we own them and store them in structs:

```rust
pub struct Feed {
    pub url: String,
    pub name: String,
    // …
}
```

Function parameters that only need to *read* a string use `&str` to avoid unnecessary cloning:

```rust
pub fn is_read(&self, feed_url: &str, article_id: &str) -> bool { … }
```

The key-matching helper uses `&str` too:

```rust
pub fn matches_key(event: &KeyEvent, key_str: &str) -> bool { … }
```

### String formatting

Use the `format!` macro to build strings from parts:

```rust
let msg = format!("Fetching {}…", url);
let counts = format!("({unread})/{total}");  // named capture from variable
```

`format!` is like `sprintf` in C but type-safe and UTF-8 aware.

---

## Compound Types

### Tuples

A fixed-size collection of values of possibly different types:

```rust
let pair: (usize, usize) = (total, unread);
let (total, unread) = pair;   // destructuring
```

thrssty uses tuples in iterator folds to carry two running totals:

```rust
let (total, unread) = feed_indices.iter().fold((0usize, 0usize), |(t, u), &idx| {
    // t = running total, u = running unread count
    (t + feed.articles.len(), u + feed_unread)
});
```

### Arrays

A fixed-size collection of values of the *same* type:

```rust
let rgb: [u8; 3] = [0xCB, 0xA6, 0xF7];   // length is part of the type
```

thrssty does not use arrays directly — it uses `Vec<T>` (dynamic arrays) instead.

---

## Functions

Functions are declared with `fn`:

```rust
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

Key rules:
- Parameter types are **always** required; there is no type inference for function signatures.
- The return type follows `->`. If a function returns nothing, the arrow is omitted.
- The last expression in a function body is its return value (no `return` keyword needed),
  unless you want to return early with `return`.

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b          // no semicolon → this is the return value
}

fn print_sum(a: i32, b: i32) {
    let s = a + b;
    println!("{}", s);   // semicolon → this is a statement, not a return value
    // the function returns () (unit), the empty type
}
```

### In thrssty

The `truncate` helper in `src/ui.rs` is a good example:

```rust
/// Truncate a string to at most `max_chars` characters, appending `…` if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();        // early return
    }
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        // There were more characters — truncate and add ellipsis.
        let mut truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        truncated.push('…');
        truncated                    // implicit return — no semicolon
    } else {
        collected                    // implicit return
    }
}
```

Notice `return` is only used for the early exit; the two branches at the end are implicit returns.

### The `///` doc-comment

Lines starting with `///` are **documentation comments**. They use Markdown syntax and appear in
the output of `cargo doc`. Every public function in thrssty has one.

---

## Type Inference

Rust infers types from context whenever possible:

```rust
let x = 42;              // Rust infers i32
let s = "hello".to_owned();   // Rust infers String
let v = vec![1, 2, 3];  // Rust infers Vec<i32>
```

You only need to annotate when inference is ambiguous:

```rust
let n: u16 = 20;         // must be u16, not i32
let content = std::fs::read_to_string(&path)?;  // inferred as String
```

---

## Constants and Statics

```rust
const MAX_AUTHOR_LEN: usize = 20;   // compile-time constant, must have explicit type
static GREETING: &str = "hello";    // static — lives for the program lifetime
```

thrssty uses `include_str!`, a macro that reads a file at compile time and produces a
`&'static str`:

```rust
// src/theme.rs
const DEFAULT_THEME: &str = include_str!("default_theme.toml");
```

The default theme TOML is baked into the binary — the program always has a fallback theme even
if no theme file exists on disk.

---

## Shadowing

You can re-declare a variable with the same name using `let`. This is called **shadowing**:

```rust
let x = 5;
let x = x * 2;   // a new variable, shadowing the old one
let x = "now it's a string";   // different type allowed via shadowing
```

Shadowing is intentional re-use of a name, not mutation. The original variable is not changed;
a new one is created. thrssty uses this to refine a `PathBuf` step by step:

```rust
let config_dir = Self::config_path()?;       // PathBuf to the file
let config_dir = config_dir                  // shadow to get the parent directory
    .parent()
    .unwrap_or(std::path::Path::new("."))
    .to_owned();
```

---

## Summary

- Variables are immutable by default; use `mut` to allow mutation.
- Key scalar types: integers (`i32`, `u16`, `usize`, …), `bool`, `f64`.
- Two string types: `String` (owned) and `&str` (borrowed view).
- `format!` builds owned `String`s with interpolation.
- Functions always declare parameter and return types; the last expression is the return value.
- `///` comments are documentation; `include_str!` embeds files at compile time.
- Shadowing lets you re-use a name for a new, possibly differently-typed variable.

Next: [Lesson 03 — Control Flow and Pattern Matching](03-control-flow-and-pattern-matching.md)
