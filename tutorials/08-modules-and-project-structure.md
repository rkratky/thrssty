# Lesson 08 — Modules and Project Structure

As a program grows it needs to be split into logical units. Rust's module system controls both
the **structure** of the source tree and the **visibility** of every item within it.

---

## Modules

A **module** is a named namespace. Items declared inside a module are only accessible from
outside it if they are marked `pub`.

```rust
mod greeting {
    pub fn hello() -> &'static str { "Hello!" }
    fn secret() -> &'static str { "shh" }    // private: only usable inside this module
}

greeting::hello();   // OK: pub
greeting::secret();  // COMPILE ERROR: not pub
```

---

## `mod` in Files vs Inline

There are two ways to define a module:

### 1. Inline (inside the same file)

```rust
// src/config.rs
mod defaults {
    pub fn sidebar_width() -> u16 { 20 }
    pub fn quit() -> String { "q".into() }
    // …
}
```

This is how thrssty defines its default-value functions. The `mod defaults` block is *private* to
`config.rs` — nothing outside can reach it — while the functions inside are `pub` so the rest
of `config.rs` can call them.

### 2. In a separate file

```rust
// src/lib.rs
pub mod config;    // tells the compiler to load src/config.rs as the `config` module
pub mod error;
pub mod app;
pub mod feed;
// …
```

When Cargo sees `pub mod config;` in `lib.rs`, it looks for:
- `src/config.rs`, or
- `src/config/mod.rs` (for a module that is itself a directory).

thrssty uses both forms: the flat modules (`config`, `error`, `app`, …) are individual files,
and `feed` is a directory with a `mod.rs`:

```
src/
├── feed/
│   ├── mod.rs       ← the `feed` module
│   └── fetcher.rs   ← a submodule of `feed`
└── lib.rs
```

Inside `src/feed/mod.rs`:

```rust
pub mod fetcher;   // loads src/feed/fetcher.rs as feed::fetcher
```

---

## `lib.rs` and `main.rs`: the Two Entry Points

thrssty is both a library and a binary:

- `src/lib.rs` declares all public modules and makes them accessible as `thrssty::…`.
- `src/main.rs` contains `fn main()` and imports from the library with `use thrssty::…`.

```rust
// src/lib.rs
pub mod app;
pub mod config;
pub mod error;
pub mod feed;
pub mod feeds_config;
pub mod html;
pub mod state;
pub mod theme;
pub mod ui;
```

```rust
// src/main.rs (excerpt)
use thrssty::{
    app::App,
    config::Config,
    error::Result,
    feeds_config::FeedsFile,
    state::Cache,
    theme::Theme,
};
```

This separation pays off during testing: tests can import from the library crate without running
`main()`.

---

## Visibility (`pub`)

Every item (struct, function, enum, constant, module) is **private by default**. Privacy is
relative to the module tree: a private item is visible within its own module and any of its
descendants.

| Modifier | Visible from |
|----------|-------------|
| (nothing) | Only within the current module and its descendants |
| `pub` | Anywhere the module itself is accessible |
| `pub(crate)` | Anywhere within this crate, but not outside |
| `pub(super)` | The parent module |

thrssty uses:
- `pub` on all structs and methods that `main.rs` or other modules need.
- No modifier on internal helpers like `build_feeds`, `truncate`, `parse_feeds_content` —
  they are implementation details.

```rust
// src/app.rs
pub fn refresh_all_feeds(&mut self) { … }  // used from main.rs via handle_key
fn build_feeds(…) -> Vec<Feed> { … }       // private: only called from within app.rs

// src/ui.rs
pub fn draw(frame: &mut Frame, app: &mut App) { … }  // called from main.rs
fn draw_feed_sidebar(…) { … }                         // private: only called from draw()
fn draw_article_list(…) { … }                         // private
fn draw_article_view(…) { … }                         // private
fn draw_status_bar(…) { … }                           // private
```

---

## `use` and Paths

Paths in Rust are written with `::` separators: `crate::error::AppError`,
`std::collections::HashMap`, `ratatui::text::Line`.

The `use` statement brings an item into scope so you can refer to it by its short name:

```rust
// Without use:
let x: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

// With use:
use std::collections::HashMap;
let x: HashMap<String, usize> = HashMap::new();
```

### Glob imports (`use module::*`)

Imports everything exported from a module. Used sparingly because it makes it harder to see where
names come from:

```rust
use ratatui::prelude::*;   // common in ratatui examples; thrssty avoids it
```

thrssty always uses explicit imports:

```rust
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{block::Padding, Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
```

The brace syntax groups multiple items from the same module on one line.

### `use` in tests

Inside `#[cfg(test)]` modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;   // import everything from the parent module (the file being tested)

    #[test]
    fn parse_categories_and_uncategorized() {
        let f = parse_feeds_content(EXAMPLE).unwrap();
        assert_eq!(f.categories.len(), 2);
    }
}
```

`use super::*` imports private items from the parent — tests are a special context where this is
allowed and expected.

---

## Re-exports

A module can re-export items from submodules to present a cleaner public API:

```rust
// src/feed/mod.rs
pub mod fetcher;           // expose the submodule

pub struct Article { … }  // defined here
pub struct Feed { … }     // defined here
```

From outside `src/feed/`:

```rust
use crate::feed::{Article, Feed};               // types from mod.rs
use crate::feed::fetcher::{spawn_fetch, FetchMessage};  // from fetcher.rs
```

---

## The `crate` Keyword

Within library code, `crate` refers to the root of the current crate (the library itself):

```rust
// src/feed/fetcher.rs — referring to types defined in sibling modules
use crate::error::AppError;
use crate::feed::Article;
```

In `main.rs`, the same thing is expressed as `thrssty::`:

```rust
use thrssty::app::App;   // from outside the crate (the binary uses the library)
```

---

## Practical Layout: thrssty's Module Tree

```
thrssty (crate root = lib.rs)
├── app           (src/app.rs)        — App struct and event handler
├── config        (src/config.rs)     — Config, LayoutConfig, KeysConfig
│   └── defaults  (inline mod)        — default value functions
├── error         (src/error.rs)      — AppError, Result alias
├── feed          (src/feed/mod.rs)   — Article, Feed
│   └── fetcher   (src/feed/fetcher.rs) — spawn_fetch, FetchMessage
├── feeds_config  (src/feeds_config.rs) — FeedConfig, SidebarLayout, …
├── html          (src/html.rs)       — html_to_lines, ArticleStyles
├── state         (src/state.rs)      — Cache, UiState
├── theme         (src/theme.rs)      — Theme
└── ui            (src/ui.rs)         — draw()
```

And separately:
```
thrssty (binary = main.rs)  — event_loop(), setup_terminal(), restore_terminal()
```

---

## Summary

- A **module** is a namespace; `mod name;` loads `name.rs` or `name/mod.rs`.
- Items are private by default; `pub` makes them accessible from outside the module.
- `use` brings items into scope under short names.
- `lib.rs` is the library root; `main.rs` is the binary entry point.
- `crate::` refers to the library root; `super::` refers to the parent module.
- Test modules use `use super::*` to access private implementation details.

Next: [Lesson 09 — Collections, Closures, and Iterators](09-collections-closures-and-iterators.md)
