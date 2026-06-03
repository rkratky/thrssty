# Lesson 0 — Introduction: What We Are Building and Why Rust

## Welcome

This series of tutorials teaches Rust by building a real, working program from
scratch — a terminal-based RSS news reader called **thrssty**. By the final
lesson you will have read, understood, and mentally reconstructed every line of
the program.

The tutorials assume you are comfortable at a Linux or macOS command line
(navigating directories, editing files, running commands) and have programming experience in at least one other language (Python, JavaScript, Go, C, or similar), but are new to Rust. Every new Rust concept is introduced with a
short explanation and immediately grounded in code that ends up in the finished
application.

- - -
## What Is thrssty?

thrssty is a **TUI** (Terminal User Interface) RSS/Atom news-reader modelled on
the Mutt email client. The screen is divided into three panels:

```
**┌────────────────┬───────────────────────────────────┐**
**│ Feed sidebar   │ Article list                      │**
**│                ├───────────────────────────────────┤**
**│  ● Rust Blog   │                                   │**
**│  ● Ubuntu      │ Article view                      │**
**│    News        │                                   │**
**├────────────────┴───────────────────────────────────┤**
**│ Status bar         q:quit  r:refresh  o:open …     │**
**└────────────────────────────────────────────────────┘**
```
- **Feed sidebar** (left): lists every configured RSS/Atom feed, grouped into
  categories. Unread feeds are highlighted. `j`/`k` moves the cursor between
  feeds.
- **Article list** (top right): articles from the selected feed, with title,
  author and date. Arrow keys move the cursor; switching articles marks them as
  read.
- **Article view** (bottom right): the body of the selected article, with full
  HTML-to-text conversion and scrolling.
- **Status bar**: current operation (fetching, errors) on the left; keyboard
  shortcuts on the right.

Key capabilities:

- Parses both RSS 2.0 and Atom feeds.
- Fetches all feeds in the background so the UI stays responsive.
- Persists a read/unread cache across sessions (
  `~/.local/share/thrssty/cache.json`).
- Supports custom colour themes in the Helix editor theme format.
- Automatically detects the terminal's background (dark or light) and selects a matching built-in theme.
- CLI options: `--config`, `--theme`, `--refresh`, `--init`, `--help`, `--version`.
- Fully configurable: panel sizes, colours, and key bindings live in 
  `~/.config/thrssty/config.toml`.

- - -
## Why Rust?

You already know that a program is just instructions the CPU executes and that
those instructions manipulate memory. Most high-level languages (Python, Java,
JavaScript) manage memory automatically with a *garbage collector* — a
background process that periodically finds and frees unused memory. Garbage
collection adds runtime overhead and can cause unpredictable pauses.

Rust takes a different approach: it guarantees memory safety *at compile time*
through a set of rules called the **ownership system**. There is no garbage
collector, no runtime, and no null-pointer crashes at runtime — the compiler
simply refuses to compile code that would violate the rules. You pay the cost
at development time (the compiler is strict), not at run time.

For a terminal application like thrssty this means:

- **No pauses** from a garbage collector while you are reading an article.
- **Tiny binary** (the final executable is self-contained, with no VM or runtime).
- **Predictable memory use** — the program cleans up exactly when variables go
  out of scope.
- **Fearless concurrency** — Rust's type system prevents data races between the
  background fetch thread and the UI thread without needing a lock on every
  access.

- - -
## The Source Tree at a Glance

After completing the build you will have this layout:

```
**thrssty/**
**├── Cargo.toml          # Package manifest: name, version, dependencies**
**├── Cargo.lock          # Exact dependency versions (committed for binaries)**
**├── config.toml         # Example config (documents all options)**
**├── feeds.toml          # Example feeds file (with categories)**
**├── man/**
**│   └── thrssty.1       # Linux manpage (roff format)**
**├── themes/             # Bundled example themes (Helix-compatible TOML)**
**└── src/**
**    ├── main.rs         # Entry point: CLI parsing, terminal setup, event loop**
**    ├── lib.rs          # Declares all modules (makes them usable from main.rs)**
**    ├── cli.rs          # Command-line argument parser (no external crate)**
**    ├── error.rs        # Unified error type for the whole application**
**    ├── config.rs       # Configuration structs and TOML loading**
**    ├── state.rs        # Persistent cache and ephemeral UI state**
**    ├── theme.rs        # Helix-compatible colour theme parser**
**    ├── terminal.rs     # Terminal background detection (dark / light)**
**    ├── feeds_config.rs # Feed/category config and sidebar layout builder**
**    ├── app.rs          # Central application state and event handling**
**    ├── ui.rs           # TUI rendering (ratatui widgets and layout)**
**    ├── html.rs         # HTML → styled terminal text converter**
**    └── feed/**
**        ├── mod.rs      # Article and Feed structs**
**        └── fetcher.rs  # HTTP fetch + feed-rs parsing + background thread**
```
Each source file corresponds to one or more tutorial lessons. We will build
them in order, starting with the simplest pieces.

- - -
## Tutorial Roadmap


|Lesson|Topic                                               |
|------|----------------------------------------------------|
|01    |Project setup: Cargo, `Cargo.toml`, `rustup`        |
|02    |Variables, types, and functions                     |
|03    |Control flow and pattern matching                   |
|04    |Ownership and borrowing                             |
|05    |Structs, enums, and methods — modelling the data    |
|06    |Error handling with `Option`, `Result`, and `thiserror`|
|07    |Traits, generics, and the standard library          |
|08    |Modules and project structure                       |
|09    |Collections, closures, and iterators                |
|10    |Serialisation with `serde`: TOML config and JSON cache|
|11    |Concurrency: threads and channels                   |
|12    |External crates: HTTP and RSS/Atom parsing          |
|13    |Building a terminal UI with `ratatui`               |
|14    |Architecture: putting the whole application together|

- - -
## Before You Start

You need:

- A Linux or macOS machine (thrssty uses a Unix terminal).
- An internet connection for downloading the Rust toolchain and dependencies.
- A text editor — any editor works, but one with Rust support (VS Code +
  rust-analyzer, Helix, Neovim, or JetBrains RustRover) will make the
  experience much smoother.

Move on to [Lesson 01 — Project Setup](01-setup.md) to install Rust and create
the project.

