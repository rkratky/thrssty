# Lesson 01 — Project Setup: Cargo, Packages, and the Build System

## Installing Rust

Rust is distributed through **rustup**, a command-line installer that also manages toolchain
versions. Run this in your terminal:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The installer adds three commands to your `PATH`:

| Command | Purpose |
|---------|---------|
| `rustup` | Manages Rust toolchain versions |
| `rustc` | The Rust compiler |
| `cargo` | The build system and package manager |

After installation, open a new shell (or run `source ~/.cargo/env`) and verify:

```bash
rustc --version   # e.g. rustc 1.78.0
cargo --version   # e.g. cargo 1.78.0
```

---

## What Is Cargo?

Every Rust project is a **crate** (Rust's word for a package). Cargo is the tool that:

1. Creates new projects (`cargo new`).
2. Downloads dependencies from [crates.io](https://crates.io).
3. Compiles everything (`cargo build`).
4. Runs tests (`cargo test`).
5. Lints the code (`cargo clippy`).
6. Produces release binaries (`cargo build --release`).

You will use Cargo for every step of building thrssty.

---

## Creating the Project

```bash
cargo new thrssty
cd thrssty
```

Cargo creates this skeleton:

```
thrssty/
├── Cargo.toml
└── src/
    └── main.rs
```

`src/main.rs` is a working "Hello, world!" program. Build and run it:

```bash
cargo run
# Hello, world!
```

`cargo run` compiles (if needed) and executes. The compiled output lands in `target/debug/thrssty`.

---

## Understanding `Cargo.toml`

Open `Cargo.toml`. It is a [TOML](https://toml.io) file (Tom's Obvious Minimal Language — you
will parse this format yourself later in the project). The initial content is:

```toml
[package]
name = "thrssty"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### `[package]` section

| Key | Meaning |
|-----|---------|
| `name` | The crate name; also the binary name after `cargo build`. |
| `version` | Follows [Semantic Versioning](https://semver.org): MAJOR.MINOR.PATCH. |
| `edition` | The Rust edition. Use `2021` (current). |

Optionally add metadata — thrssty's final manifest has these:

```toml
[package]
name = "thrssty"
version = "0.1.0"
edition = "2021"
description = "TUI RSS news reader in Rust"
repository = "https://github.com/rkratky/thrssty"
license = "MIT OR Apache-2.0"
keywords = ["rss", "atom", "tui", "newsreader", "terminal"]
categories = ["command-line-utilities"]
```

### `[dependencies]` section

External libraries are called **crates** and are listed here. Let's add all the crates thrssty
needs:

```toml
[dependencies]
# TUI rendering
ratatui    = { version = "0.30", features = ["unstable-rendered-line-info"] }
crossterm  = "0.28"

# RSS/Atom feed parsing
feed-rs    = "2"

# HTTP client (sync, minimal — fetches run on a background thread)
ureq       = "2"

# Serialisation: config (TOML) and cache (JSON)
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
toml       = "0.8"

# Error handling
thiserror  = "2"

# HTML → terminal text conversion for article body
html2text  = "0.16"

# Dates with serde support
chrono     = { version = "0.4", features = ["serde"] }

# Platform config/data directory resolution
dirs       = "5"

# Open article URLs in the system browser
open       = "5"
```

Version strings follow [Semantic Versioning](https://semver.org):

- `"0.29"` means "any compatible 0.29.x release" (patch updates accepted).
- `"2"` means "any compatible 2.x.y release" (minor and patch updates accepted).
- `{ version = "1", features = ["derive"] }` enables an optional *feature flag* within the crate.

After editing `Cargo.toml`, run:

```bash
cargo build
```

Cargo downloads every dependency and their transitive dependencies (you will see a lot of lines
the first time) and places the results in `target/`. `Cargo.lock` is generated to record the
exact versions resolved — commit this file for binary crates like thrssty so builds are
reproducible.

---

## The `target/` Directory

After a build, `target/` holds:

```
target/
├── debug/          # Debug builds: fast to compile, slow to run
│   ├── thrssty     # The compiled binary
│   └── ...
└── release/        # Release builds: slow to compile, fast to run
    └── thrssty
```

Use `cargo build` during development and `cargo build --release` for the final binary you
actually want to use. The debug binary includes symbols for debugging but is not optimised.

---

## A Library and a Binary

The `src/main.rs` file is the binary entry point. As the project grows we will also create
`src/lib.rs`, which turns the crate into a *library* that `main.rs` uses. This separation is a
Rust best practice: it makes the logic testable independently of the binary entry point.
thrssty also has a `cli.rs` module for command-line argument parsing — it is declared only in
`main.rs` (with `mod cli;`) rather than in `lib.rs`, because it is only needed by the binary
entry point and not by the testable library. All other modules (`config.rs`, `state.rs`,
`theme.rs`, `terminal.rs`, etc.) are declared in `lib.rs` so integration tests can use them.

When both exist, Cargo builds two outputs:
- `thrssty` (binary, from `main.rs`)
- `libthrssty` (library, from `lib.rs`, used only at link time by `main.rs`)

The binary imports the library with:

```rust
use thrssty::app::App;
use thrssty::config::Config;
// … and so on
```

---

## Useful Cargo Commands

```bash
cargo build           # Compile in debug mode
cargo build --release # Compile with optimisations
cargo run             # Compile and run
cargo test            # Run all tests
cargo clippy          # Lint: catches common mistakes and style issues
cargo fmt             # Auto-format all source files
cargo doc --open      # Generate and open documentation for your crate
cargo clean           # Delete the target/ directory
```

You will use all of these throughout the tutorial series.

---

## Summary

- Rust is installed through `rustup`; the three main tools are `rustup`, `rustc`, and `cargo`.
- Every Rust project is a **crate**; its manifest is `Cargo.toml`.
- `[dependencies]` lists external crates; Cargo resolves and downloads them automatically.
- `cargo build` / `cargo run` / `cargo test` are the commands you will use most.
- A crate can have both `src/main.rs` (binary) and `src/lib.rs` (library); thrssty uses both.

Next: [Lesson 02 — Variables, Types, and Functions](02-variables-types-functions.md)
