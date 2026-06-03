# Learning Rust by Building thrssty

A tutorial series that teaches Rust by building **thrssty** — a real, working terminal-based
RSS newsreader — from scratch. The goal is to cover the Rust language comprehensively while
keeping every concept grounded in actual application code: no toy examples that get thrown away,
no artificial exercises.

The series is aimed at developers who already know at least one other programming language
(Python, JavaScript, Go, C, or similar) but are new to Rust. It covers the full journey from
installing the toolchain to understanding the complete application architecture — ownership and
borrowing, error handling, traits and generics, concurrency, serialisation, TUI rendering, and
more.

By the final lesson you will have read and understood every source file in thrssty.

## Lessons

| # | File | Topics |
|---|------|--------|
| 00 | [Introduction](00-introduction.md) | What is Rust, what we're building, how to use the tutorials |
| 01 | [Project Setup](01-setup.md) | `cargo`, toolchain, project structure, `Cargo.toml` |
| 02 | [Variables, Types, and Functions](02-variables-types-functions.md) | Variables, mutability, scalar types, strings, functions |
| 03 | [Control Flow and Pattern Matching](03-control-flow-and-pattern-matching.md) | `if`/`else`, `loop`/`for`, `match`, `if let` |
| 04 | [Ownership and Borrowing](04-ownership-and-borrowing.md) | Ownership, moves, references, lifetimes |
| 05 | [Structs, Enums, and Methods](05-structs-enums-and-methods.md) | Structs, enums, `impl`, `Option`, `#[derive]` |
| 06 | [Error Handling](06-error-handling.md) | `Result`, `?`, `thiserror`, unified error types |
| 07 | [Traits, Generics, and the Standard Library](07-traits-generics-and-stdlib.md) | Traits, generics, `Iterator`, standard traits |
| 08 | [Modules and Project Structure](08-modules-and-project-structure.md) | `mod`, `pub`, `use`, `lib.rs` vs `main.rs` |
| 09 | [Collections, Closures, and Iterators](09-collections-closures-and-iterators.md) | `Vec`, `HashMap`, closures, iterator chains |
| 10 | [Serialisation with Serde](10-serialisation-with-serde.md) | `serde`, TOML/JSON, `#[serde(default)]`, schema migration |
| 11 | [Concurrency: Threads and Channels](11-concurrency-threads-and-channels.md) | `std::thread`, `mpsc`, `Send`/`Sync` |
| 12 | [External Crates: HTTP and Feeds](12-external-crates-http-and-feeds.md) | `ureq`, `feed-rs`, `html2text`, integrating crates |
| 13 | [Building a TUI with ratatui](13-building-a-tui-with-ratatui.md) | `ratatui`, `crossterm`, layouts, widgets, `Span`/`Line` |
| 14 | [Complete Application Architecture](14-complete-application-architecture.md) | Full architecture, design decisions, next steps |

Start with [Lesson 00 — Introduction](00-introduction.md).
