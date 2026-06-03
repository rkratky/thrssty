# Lesson 11 — Concurrency: Threads and Channels

Fetching RSS feeds over HTTP can take seconds — you do not want the user interface to freeze while
waiting for a network response. thrssty solves this by moving all network I/O to a **background
thread**, while the UI thread continues to draw frames and process key presses.

Rust's ownership system makes this safe: the type system prevents sharing data unsafely between
threads, and you get a compile-time error instead of a runtime data race.

---

## Spawning a Thread

`std::thread::spawn` creates an OS thread. It takes a closure that the thread will execute:

```rust
use std::thread;

thread::spawn(|| {
    println!("hello from a thread!");
});
```

The closure is **`move`** — it can capture values from the outer scope by moving ownership into
the thread:

```rust
let url = "https://example.com/feed".to_owned();

thread::spawn(move || {
    // url was moved into this closure; the original binding is gone
    fetch_and_process(url);
});
```

The `move` keyword is typically required when a spawned thread's closure captures owned values,
because the thread may outlive the parent scope and needs to own its data. Closures that capture
only references with a `'static` lifetime (e.g., a `&'static str` string literal) do not need
`move`; in practice, closures passed to `thread::spawn` almost always capture owned values, so
`move` is the norm.

---

## Channels: Communicating Between Threads

Threads should not share mutable data directly (that would be a data race). Instead, Rust
provides **channels**: a typed pipe from sender to receiver.

```rust
use std::sync::mpsc;  // multiple-producer, single-consumer

let (tx, rx) = mpsc::channel::<String>();

// Send from any thread:
tx.send("hello".to_owned()).unwrap();

// Receive on the owner thread:
let msg = rx.recv().unwrap();   // blocks until a message arrives
println!("{}", msg);
```

`mpsc` = *multiple producer, single consumer*: you can clone `tx` and send from many threads,
but there is only one `rx`.

`send` and `recv` return `Result` because:
- `send` fails if the receiver was dropped.
- `recv` fails if all senders were dropped (channel closed).

---

## thrssty's Fetch Architecture

The fetch thread sends messages through a channel; the UI thread drains the channel on every
`tick()` call (which happens every 100 ms).

### The message type

```rust
// src/feed/fetcher.rs
pub enum FetchMessage {
    Started  { url: String },
    Complete { url: String, feed_title: Option<String>, articles: Vec<Article> },
    Error    { url: String, reason: String },
    Done,
}
```

Each variant carries exactly the data the UI needs. Note that `Vec<Article>` is *moved* from the
thread — there is no shared pointer, no lock, no risk of races.

### Spawning the fetch thread

```rust
pub fn spawn_fetch(urls: Vec<String>, tx: mpsc::Sender<FetchMessage>) {
    thread::spawn(move || {          // moves `urls` and `tx` into the thread
        for url in &urls {
            let _ = tx.send(FetchMessage::Started { url: url.clone() });

            match fetch_feed(url) {
                Ok((feed_title, articles)) => {
                    let _ = tx.send(FetchMessage::Complete {
                        url: url.clone(),
                        feed_title,    // Optional feed title from the parsed feed
                        articles,      // Vec<Article> moved into the message
                    });
                }
                Err(AppError::FeedFetch { reason, .. }) => {
                    let _ = tx.send(FetchMessage::Error {
                        url: url.clone(),
                        reason,
                    });
                }
                Err(e) => {
                    let _ = tx.send(FetchMessage::Error {
                        url: url.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }
        let _ = tx.send(FetchMessage::Done);
    });
    // The thread runs independently. `spawn_fetch` returns immediately.
}
```

Key points:
- `thread::spawn(move || { … })` — the closure *moves* `urls` and `tx` in.
- The function returns immediately; the thread runs in the background.
- `let _ = tx.send(…)` — the send might fail if the UI quit; we ignore that error.
- The thread sends `Done` as its last message so the UI knows everything finished.

### Triggering a refresh from the App

```rust
pub fn refresh_all_feeds(&mut self) {
    let urls: Vec<String> = self.feeds.iter().map(|f| f.url.clone()).collect();
    if urls.is_empty() { return; }

    let (tx, rx) = mpsc::channel();    // create a fresh channel
    self.fetch_rx = Some(rx);          // store receiver in App
    spawn_fetch(urls, tx);             // sender goes to the thread

    self.ui.status_message = Some("Fetching all feeds…".into());
}
```

The `App` struct stores the receiving end:

```rust
fetch_rx: Option<mpsc::Receiver<FetchMessage>>,
```

`Option` here means "is a fetch currently in progress?" — `None` means idle, `Some(rx)` means
a fetch thread is running and sending messages.

### Polling on every tick

```rust
pub fn tick(&mut self) {
    // Drain available messages WITHOUT blocking.
    let messages: Vec<FetchMessage> = self
        .fetch_rx
        .as_ref()
        .map(|rx| rx.try_iter().collect())  // try_iter: returns available messages, no wait
        .unwrap_or_default();

    for msg in messages {
        match msg {
            FetchMessage::Started { url } => {
                self.ui.fetching = Some(url.clone());
                self.ui.status_message = Some(format!("Fetching {}…", url));
                self.ui.status_is_error = false;
            }
            FetchMessage::Complete { url, feed_title, articles } => {
                self.feed_errors.remove(&url);
                self.apply_fetch_result(&url, feed_title, articles);
            }
            FetchMessage::Error { url, reason } => {
                self.feed_errors.insert(url.clone());
                self.ui.status_message = Some(format!("Error fetching {}: {}", url, reason));
                self.ui.status_is_error = true;
            }
            FetchMessage::Done => {
                self.ui.fetching = None;
                if !self.ui.status_is_error {
                    self.ui.status_message = Some("Done.".into());
                }
                self.fetch_rx = None;    // drop the receiver (channel is exhausted)
                self.sync_cache_feeds();
                let _ = self.cache.save();
            }
        }
    }
}
```

`try_iter()` returns an iterator over all *currently available* messages — it does not block.
If no messages have arrived since the last tick, it returns nothing. This is why the UI stays
responsive: `tick()` takes microseconds, never seconds.

`tick()` also polls a second channel, `validate_rx`, used during the "add feed" flow (`a` key). When the user types a URL, a background thread validates it; `tick()` reads the result and transitions the UI mode accordingly. The principle is the same: a non-blocking `try_recv()` check on every tick, with the main thread remaining responsive throughout.

---

## `Send` and `Sync`: Thread Safety at the Type Level

Rust has two marker traits that govern thread safety:

| Trait | Meaning |
|-------|---------|
| `Send` | A value of this type can be *transferred* to another thread. |
| `Sync` | A value of this type can be *referenced* from multiple threads simultaneously. |

Most types are automatically `Send` and `Sync`. Types that contain raw pointers, reference counts
without locks (`Rc`), or similar constructs are not.

`mpsc::Sender<T>` is `Send` only if `T: Send`. Because `FetchMessage` contains `String` and
`Vec<Article>` — which are both `Send` — the compiler accepts moving the sender into the thread.

If you accidentally tried to send a non-`Send` type (such as an `Rc<…>`), you would get a
compile error explaining why. This is Rust preventing a data race before it can happen.

---

## The Alternative: `Arc<Mutex<T>>` for Shared State

thrssty's design avoids shared state by passing *messages* (channel). Another common pattern for
concurrency is `Arc<Mutex<T>>`:

- `Arc<T>` — *Atomic Reference Counted* pointer: multiple threads can hold a clone of the
  `Arc`, and the inner value is freed when the last `Arc` is dropped.
- `Mutex<T>` — mutual exclusion lock: only one thread can access the inner value at a time.

```rust
use std::sync::{Arc, Mutex};

let shared = Arc::new(Mutex::new(Vec::<String>::new()));
let shared2 = Arc::clone(&shared);

std::thread::spawn(move || {
    let mut vec = shared2.lock().unwrap();   // acquire the lock
    vec.push("from thread".to_owned());
    // lock released automatically when `vec` drops (end of scope)
});

let vec = shared.lock().unwrap();
println!("{:?}", *vec);
```

thrssty chose the channel/message-passing approach because it avoids locks entirely — the
thread exclusively owns its data and sends it when done. This design choice is described by the
Go proverb: *"Do not communicate by sharing memory; share memory by communicating."*

---

## Why the Borrow Checker Helps Here

Consider what would happen without the ownership rules:

```rust
// Hypothetical unsafe code
let articles: Vec<Article> = Vec::new();
thread::spawn(|| {
    articles.push(fetch_from_network());   // thread mutates articles
});
println!("{}", articles.len());            // main reads articles simultaneously
// DATA RACE → undefined behaviour in C/C++
```

In Rust, this does not compile. `articles` cannot be borrowed mutably by the thread while also
being borrowed by `println!`. The compiler forces you to choose one of the safe patterns:
move into the thread (gives up the data in main), channel (copy/move data out when done),
or `Arc<Mutex<_>>` (lock-protected sharing).

---

## Summary

- `std::thread::spawn(move || { … })` starts an OS thread; the closure must be `move`.
- `std::sync::mpsc::channel()` creates a typed `(Sender<T>, Receiver<T>)` pair.
- `rx.try_iter()` drains available messages without blocking — keeps the UI responsive.
- Design pattern: the background thread owns its data; when done, it *moves* results into a
  message and sends it. No locks needed.
- `Send` and `Sync` are compile-time markers that prevent sharing non-thread-safe types.
- Alternatives to channels: `Arc<Mutex<T>>` for shared mutable state.

Next: [Lesson 12 — External Crates: HTTP and RSS/Atom Parsing](12-external-crates-http-and-feeds.md)
