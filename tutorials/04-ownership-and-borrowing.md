# Lesson 04 — Ownership and Borrowing

Ownership is the feature that makes Rust unique. It is the mechanism by which Rust guarantees
memory safety at compile time — without a garbage collector.

If you have used C, you have manually `malloc`'d and `free`'d memory, and have experienced
use-after-free bugs or double-frees. If you have used Python or Java, a garbage collector freed
memory for you, but at the cost of runtime overhead and unpredictable pauses. Rust takes a third
path: the *compiler itself* tracks when every piece of memory should be freed, using the ownership
rules.

---

## The Three Rules of Ownership

1. Every value in Rust has exactly **one owner**.
2. When the owner goes out of scope, the value is **dropped** (freed).
3. Ownership can be **moved** to a new owner — after which the old owner can no longer use it.

```rust
{
    let s = String::from("hello");   // s is the owner of the String
    // … use s …
}   // s goes out of scope here → the String is freed automatically
```

No `free()`. No garbage collector. The compiler inserts the deallocation at the closing brace.

### Move semantics

When you assign a `String` to another variable, ownership *moves*:

```rust
let s1 = String::from("hello");
let s2 = s1;        // s1 is MOVED into s2
println!("{}", s1); // COMPILE ERROR: s1 is no longer valid
```

This prevents the same memory from being freed twice (a double-free bug). `s2` is now the sole
owner and will free the memory when it goes out of scope.

Types that are cheap to copy (integers, booleans, `char`) implement the `Copy` trait and are
implicitly duplicated instead of moved:

```rust
let x = 5;
let y = x;          // x is COPIED (integers are Copy)
println!("{}", x);  // OK: x is still valid
```

---

## Cloning

If you need a deep copy of a `String` (or any `Clone` type), call `.clone()`:

```rust
let s1 = String::from("hello");
let s2 = s1.clone();   // s2 gets its own copy of the data
println!("{} {}", s1, s2);   // both are valid
```

Cloning costs memory and CPU. thrssty clones values only when necessary:

```rust
// src/app.rs — we need the URL after iterating, so we clone it out of the reference
let url = feed.url.clone();
```

```rust
// src/app.rs — cloning the entire key-config struct so we can call &mut self methods later
let keys = self.config.keys.clone();
```

---

## Borrowing: Shared References (`&T`)

You can *lend* a value without giving up ownership by taking a **reference**. A reference is
like a pointer but guaranteed safe: it is always valid, and it does not own the data.

```rust
fn print_length(s: &String) {   // s is a reference to a String, not an owner
    println!("Length: {}", s.len());
}

let s = String::from("hello");
print_length(&s);   // borrow s
println!("{}", s);  // s is still valid — we only lent it
```

A `&T` is a **shared reference** — you can have as many as you like at the same time, but
you cannot modify the value through them.

### In thrssty

Functions that only need to *read* data take references:

```rust
pub fn is_read(&self, feed_url: &str, article_id: &str) -> bool {
    self.read_ids
        .get(feed_url)
        .map(|ids| ids.contains(article_id))
        .unwrap_or(false)
}
```

`feed_url: &str` is a reference to string data. The caller keeps ownership.

`&self` (which appears in every method) is a shared reference to the struct itself — the method
can read the struct but not modify it.

---

## Mutable References (`&mut T`)

To modify through a reference, use `&mut T`. The rule: **at any time, there can be at most one
mutable reference to a value** (and no shared references simultaneously). This rule eliminates
data races.

```rust
fn append_exclamation(s: &mut String) {
    s.push('!');
}

let mut greeting = String::from("hello");
append_exclamation(&mut greeting);
println!("{}", greeting);   // "hello!"
```

### `&mut self`

Methods that modify the struct take `&mut self`:

```rust
pub fn mark_read(&mut self, feed_url: &str, article_id: &str) {
    self.read_ids
        .entry(feed_url.to_owned())
        .or_default()
        .insert(article_id.to_owned());
}
```

If you call a `&self` method and a `&mut self` method at the same time, the compiler rejects it.
This comes up in thrssty's key handler:

```rust
// Clone the key bindings first (giving us an independent owned value),
// then we can call &mut self methods freely.
let keys = self.config.keys.clone();

if matches_key(&event, &keys.quit) { … }
```

Without the clone, we would hold `&self.config.keys` (a shared borrow) while trying to call
`&mut self` methods — a compile error, because you cannot have both simultaneously.

---

## The Borrow Checker in Action

The borrow checker is the part of the compiler that enforces ownership rules. When it rejects
your code it is preventing a bug. Here is a toy example:

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];   // shared borrow of v
v.push(4);           // COMPILE ERROR: cannot mutably borrow v while first (a shared borrow) is alive
println!("{}", first);
```

The `v.push(4)` might reallocate the vector's internal buffer, invalidating `first`. The borrow
checker catches this at compile time instead of silently corrupting memory at runtime.

---

## Lifetimes: How Long Is a Reference Valid?

Every reference has a **lifetime**: the span of code during which it is valid. The compiler
usually infers lifetimes automatically. When it cannot, you annotate them with `'a` syntax.

thrssty mostly avoids explicit lifetime annotations by storing owned `String` values instead of
references in structs. The one place where lifetimes matter explicitly is the `Line<'static>`
type used for pre-rendered TUI text:

```rust
pub article_lines: Vec<Line<'static>>,
```

`'static` means "lives for the entire program". The pre-rendered lines are built from owned
`String`s converted via `Span::raw(string)` — ratatui's API requires `'static` for stored lines.

---

## Slices: References to Parts of Collections

A **slice** is a reference to a contiguous sequence of elements:

```rust
let v = vec![1, 2, 3, 4, 5];
let mid: &[i32] = &v[1..4];   // a slice containing values 2, 3, 4 (at indices 1, 2, 3)
```

`&str` is itself a string slice — a view into string data. That is why you can take a `&str`
from a `String` with `&`:

```rust
let s: String = "hello world".to_owned();
let word: &str = &s[0..5];   // "hello"
```

### In thrssty

`current_articles()` returns a slice of the current feed's article vector:

```rust
pub fn current_articles(&self) -> &[Article] {
    self.current_feed()
        .map(|f| f.articles.as_slice())
        .unwrap_or_default()
}
```

`&[Article]` is a slice — it borrows into the feed's internal `Vec<Article>` without copying
any articles.

---

## Ownership in Practice: Building the Feed List

`build_feeds` in `src/app.rs` illustrates many ownership concepts at once:

```rust
fn build_feeds(feed_configs: Vec<FeedConfig>, cache: &Cache) -> Vec<Feed> {
    // Build a lookup map: URL → &Feed (borrowed from cache).
    let cache_map: HashMap<&str, &Feed> =
        cache.feeds.iter().map(|f| (f.url.as_str(), f)).collect();

    feed_configs
        .into_iter()                    // consumes (moves) feed_configs
        .map(|fc| {
            if let Some(cached) = cache_map.get(fc.url.as_str()) {
                let mut feed = (*cached).clone();   // clone the borrowed Feed into an owned one
                if let Some(name) = &fc.name {
                    feed.name = name.clone();        // clone the name String
                }
                feed                                 // return the owned Feed
            } else {
                Feed::new(&fc.url, fc.name.clone().unwrap_or_else(|| fc.url.clone()))
            }
        })
        .collect()                       // collect into Vec<Feed>
}
```

- `cache: &Cache` — borrowed; we only read the cache, not own it.
- `feed_configs: Vec<FeedConfig>` — owned; we consume it with `.into_iter()`.
- `(*cached).clone()` — `cached` is `&&Feed` (reference to a reference); `*cached` dereferences
  one level to `&Feed`, then `.clone()` produces an owned `Feed`.
- The returned `Vec<Feed>` is owned by the caller.

---

## Summary

| Concept | Syntax | Rules |
|---------|--------|-------|
| Ownership | `let x = value;` | One owner; dropped at end of scope |
| Move | `let y = x;` | x is invalid after the move |
| Clone | `x.clone()` | Deep copy; both remain valid |
| Shared borrow | `&x` / `&T` | Many allowed simultaneously; no mutation |
| Mutable borrow | `&mut x` / `&mut T` | Exactly one; no shared borrows at the same time |
| Slice | `&v[1..4]` / `&str` | Reference to a contiguous sub-sequence |

The ownership system eliminates use-after-free, double-free, and data races — the most common
classes of memory bugs in systems programming — by making them *compile errors*.

Next: [Lesson 05 — Structs, Enums, and Methods](05-structs-enums-and-methods.md)
