# Lesson 14 — Architecture: Putting the Application Together

This final lesson steps back and looks at the application as a whole. After reading the
source you will understand not just *what* the code does, but *why* it is structured the way
it is — the design decisions, trade-offs, and patterns that make a real-world Rust program work.

---

## The Module Dependency Graph

```
main.rs
├── config::Config            (config.toml → app settings)
├── theme::Theme              (Helix TOML → ratatui Styles)
├── state::Cache              (cache.json → read status + cached articles)
├── feeds_config::FeedsFile   (feeds.toml → FeedConfig list + SidebarLayout)
└── app::App
    ├── feed::Feed / Article  (the data model)
    ├── feed::fetcher         (background HTTP + parse thread)
    ├── state::UiState        (selection, scroll, status)
    ├── html::html_to_lines   (HTML → Vec<Line>)
    └── ui::draw              (ratatui rendering)
```

Module dependencies flow downward: `main` depends on `app`, `app` depends on `feed`, etc.
No circular dependencies — a design property Rust's module system does not enforce but good
design maintains.

---

## Separation of Concerns

thrssty splits responsibilities cleanly:

| File | Responsibility |
|------|---------------|
| `main.rs` | Terminal lifecycle, event loop — nothing else |
| `app.rs` | Application state and business logic — no rendering |
| `ui.rs` | All rendering — reads `App`, writes nothing to `App` (except `article_view_height`) |
| `state.rs` | Persistence (`Cache`) and transient UI state (`UiState`) |
| `config.rs` | Configuration loading and key matching |
| `cli.rs` | Command-line argument parser; handles `--help`, `--version`, `--config`, `--theme`, `--refresh`, `--init` without external crates |
| `terminal.rs` | Detect terminal background colour (dark/light) via env vars and OSC 11 probe; used for `"auto"` theme selection |
| `feeds_config.rs` | Feed/category hierarchy and sidebar structure |
| `feed/mod.rs` | Data types (`Article`, `Feed`) — pure data, no logic |
| `feed/fetcher.rs` | Network I/O — runs on a background thread |
| `html.rs` | HTML→text conversion — pure function, no side effects |
| `theme.rs` | TOML→Style mapping — pure parsing |
| `error.rs` | Error taxonomy — no behaviour |

This separation makes the code easier to test, modify, and reason about.

---

## The `App` Struct: Central State

`App` is the heart of the application. Everything that changes during the program's lifetime
lives here:

```rust
pub struct App {
    pub config: Config,              // immutable after startup
    pub theme: Theme,                // immutable after startup
    pub cache: Cache,                // read/unread tracking, persisted on exit
    pub ui: UiState,                 // transient: selection, scroll, status
    pub feeds: Vec<Feed>,            // live feed data (merged from cache + fetcher)
    pub sidebar: SidebarLayout,      // pre-computed display structure
    pub article_lines: Vec<Line<'static>>,     // pre-rendered article body
    pub description_lines: Vec<Line<'static>>, // pre-rendered summary
    pub feed_errors: HashSet<String>,// URLs that failed their most recent fetch
    fetch_rx: Option<mpsc::Receiver<FetchMessage>>,  // channel from background thread
}
```

### Why pre-render?

`article_lines` and `description_lines` are computed once when the selection changes
(`render_current_article`), not on every frame. HTML conversion involves string allocations and
parsing — doing it 60 times per second would be wasteful. The immediate-mode pattern benefits
from caching expensive computations.

---

## Initialisation Flow

```
main()
  │
  ├── cli::parse()             parse --help/--version/--config/--theme/--refresh/--init
  │     └── if --init → Config::init_files() → exit
  │
  ├── Config::load_cascade()   merge: built-in defaults ← ./config.toml ← ~/.config/thrssty/config.toml ← --config
  │
  ├── Theme::load()            resolve theme (name / path / "auto" / "default_dark" / "default_light")
  │     └── if "auto" → terminal::detect() → choose dark or light built-in theme
  │
  ├── Cache::load()            read cache.json (or empty + warning if schema changed)
  ├── FeedsFile::load()        read feeds.toml (optional — empty file is fine)
  ├── feeds_file.into_layout() → (Vec<FeedConfig>, SidebarLayout)
  │
  └── App::new(config, theme, cache, feed_configs, sidebar)
        │
        ├── cache.purge_old_articles()   remove stale articles
        ├── build_feeds()               merge feed_configs with cached articles
        ├── sync_article_selection()    jump to first unread article
        └── render_current_article()    pre-render the article body
```

After `App::new`, the program sets up the terminal and enters the event loop.

---

## The Event Loop in Detail

```
loop
  │
  ├── terminal.draw(|f| ui::draw(f, app))
  │     ├── draw_feed_sidebar    renders sidebar items
  │     ├── draw_article_list    renders article list with columns
  │     ├── draw_article_view    renders header + pre-rendered lines
  │     └── draw_status_bar     renders left status + right key hints
  │
  ├── event::poll(100ms)         wait for input
  │     └── app.handle_key(key)
  │           ├── j / k         → next/prev_feed → sync_article_selection → render_current_article
  │           ├── ↑ / ↓         → scroll_line_up / scroll_line_down (article view)
  │           ├── ← / →         → prev_article / next_article → mark_current_read → render_current_article
  │           ├── r / R         → refresh_current_feed / refresh_all_feeds → spawn_fetch
  │           ├── o             → open::that_detached(url)
  │           ├── PgUp/PgDn     → scroll_up / scroll_down
  │           └── q             → return false (breaks the loop)
  │
  └── app.tick()
        └── drain fetch_rx
              ├── Started  → update status_message
              ├── Complete → apply_fetch_result → update feeds → render_current_article
              ├── Error    → record in feed_errors → set error status
              └── Done     → save cache, clear fetch_rx
```

Every key press calls at most one or two helper methods. Each helper modifies `UiState`
(or `feeds`, or `cache`) and returns. The next `terminal.draw` call then renders the updated
state — there is no direct coupling between input handling and rendering.

---

## Data Flow for a Feed Refresh

1. User presses `R`.
2. `handle_key` calls `refresh_all_feeds`.
3. `refresh_all_feeds`:
   - Creates `(tx, rx)` channel pair.
   - Stores `rx` in `self.fetch_rx`.
   - Calls `spawn_fetch(urls, tx)` — creates a background thread.
   - Sets status message to "Fetching all feeds…".
4. Background thread (independently):
   - For each URL: sends `Started`, fetches + parses, sends `Complete` or `Error`.
   - Sends `Done`.
5. On every `tick()` call (every ~100 ms):
   - `try_iter()` collects any available messages.
   - Each `Complete` message: updates `self.feeds[i].articles`, calls `render_current_article` if
     this is the current feed.
   - Each `Error` message: inserts URL into `feed_errors`, sets error status.
   - `Done`: saves cache, clears `fetch_rx`.

At no point does the UI thread block waiting for the network. This is why the interface remains
responsive during long fetches.

---

## Key Design Decisions

### 1. No async runtime

Rust has excellent async support via `tokio` or `async-std`. thrssty deliberately avoids it.
The fetch work happens on a regular OS thread; the UI thread is single-threaded. This keeps the
dependency tree small and the code straightforward — you never deal with `.await`, `Pin`, or
`Future` complexity.

**Trade-off**: fetching multiple feeds is sequential within the background thread. Parallelising
would require either multiple threads or an async runtime. For a news reader, sequential is fine.

### 2. `mpsc` channels instead of `Arc<Mutex<_>>`

The background thread owns its data. When done with a feed, it moves the result into a message
and sends it. There is no shared mutable state between threads, so no locks are needed.

**Trade-off**: the UI only sees results when messages arrive on `tick()`. There is up to 100 ms
of latency between a fetch completing and the UI updating. That is imperceptible to humans.

### 3. Pre-rendered `Vec<Line<'static>>`

HTML conversion happens once per article selection change. Rendering reads the pre-built lines
directly — no allocation per frame.

**Trade-off**: if the terminal is resized, the line breaks do not adapt immediately. ratatui's
`Wrap` handles this, but the summary/body split logic in `draw_article_view` uses the
pre-rendered `description_lines`, which would need to be re-rendered on resize for perfect
wrapping. For this application the simplification is acceptable.

### 4. Single `AppError` enum

Every fallible function in the library returns `crate::error::Result<T>`. This means the error
type flows through `?` without conversion in most places.

**Trade-off**: individual errors carry less type information. A `Cache` read error and a `Config`
read error are both `AppError`, distinguished only by variant. For a GUI application this is
usually fine; for a library you would want finer-grained error types.

### 5. Helix-compatible theme format

Using an established format (Helix themes) means users can import existing community themes
instead of learning a new syntax.

**Trade-off**: thrssty only uses a subset of Helix scopes. The parse ignores unknown keys
silently, so full Helix themes work but most of their scopes have no effect.

---

## Testing Strategy

thrssty has unit tests for the two most complex pure-logic components:

`src/feeds_config.rs` tests parsing and layout construction:

```rust
#[test]
fn parse_categories_and_uncategorized() {
    let f = parse_feeds_content(EXAMPLE).unwrap();
    assert_eq!(f.categories.len(), 2);
    assert_eq!(f.categories[0].title, "Rust");
    assert_eq!(f.uncategorized.len(), 1);
}
```

`src/html.rs` tests the HTML conversion:

```rust
#[test]
fn plain_text_round_trip() {
    let lines = html_to_lines("Hello world", styles());
    assert_eq!(lines.len(), 1);
}
```

The TUI rendering and event handling are *not* unit tested — testing ratatui output correctly
requires either a mock terminal or visual inspection. Integration testing a TUI application is
an advanced topic and out of scope here.

---

## Building and Running the Finished Program

```bash
# Development build (fast compile, debug symbols)
cargo build
./target/debug/thrssty

# Production build (optimised)
cargo build --release
./target/release/thrssty

# Run tests
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

Set up feeds:

```bash
mkdir -p ~/.config/thrssty

cat > ~/.config/thrssty/feeds.toml << 'EOF'
[category.1]
title = "Rust"

[[category.1.feed]]
url = "https://blog.rust-lang.org/feed.xml"
name = "Rust Blog"

[[category.1.feed]]
url = "https://this-week-in-rust.org/rss.xml"
name = "This Week in Rust"

[category.2]
title = "Linux"

[[category.2.feed]]
url = "https://ubuntu.com/blog/feed"
name = "Ubuntu Blog"
EOF
```

Run thrssty — it will fetch all configured feeds on startup.

---

## What You Have Learned

Over 14 lessons you have covered:

| Lesson | Concepts |
|--------|---------|
| 01 | Cargo, crates, `Cargo.toml`, dependency management |
| 02 | Variables, mutability, types, strings, functions, doc comments |
| 03 | `if`/`else`, `loop`, `for`, `match`, `if let`, `let else` |
| 04 | Ownership, borrowing, lifetimes, slices |
| 05 | Structs, enums, `impl` blocks, `Option<T>`, `#[derive]` |
| 06 | `Result<T, E>`, `?`, `thiserror`, error hierarchies |
| 07 | Traits, generics, `impl Trait`, `dyn Trait`, `Iterator` |
| 08 | `mod`, `pub`, `use`, `lib.rs` vs `main.rs`, visibility |
| 09 | `Vec`, `HashMap`, `HashSet`, closures, iterator adapters |
| 10 | `serde`, TOML/JSON, `#[serde(default)]`, `dirs` |
| 11 | `std::thread`, `mpsc` channels, `Send`/`Sync`, `try_iter` |
| 12 | Integrating external crates, error conversion, `html2text`, `open` |
| 13 | `ratatui`, `crossterm`, layouts, widgets, `Line`/`Span` |
| 14 | Architecture, separation of concerns, design trade-offs |

The program you have studied is approximately 1,400 lines of library code plus 100 lines of
binary entry point. It is complete, tested, and production-quality. Studying real programs is
the fastest way to internalise how Rust is used in practice.

---

## Where to Go Next

- **The Rust Book** (free online): https://doc.rust-lang.org/book/ — the official reference.
- **Rust by Example**: https://doc.rust-lang.org/rust-by-example/ — concise examples for every concept.
- **Rustlings**: https://github.com/rust-lang/rustlings — hands-on exercises.
- **crates.io**: browse the ecosystem; everything you have used here has excellent documentation on docs.rs.
- **Async Rust**: if you want concurrent network I/O without threads, study `tokio` and `async`/`await`.
- **Writing a CLI tool**: `clap` for argument parsing; extend thrssty with a `--import-opml` flag.
- **Writing tests**: `assert_eq!`, `assert!`, `#[should_panic]`, integration tests in `tests/`.

Congratulations on completing the series.
