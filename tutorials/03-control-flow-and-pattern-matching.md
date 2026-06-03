# Lesson 03 — Control Flow and Pattern Matching

## `if` / `else`

Rust `if` expressions work like most languages but with two differences:
1. The condition does **not** need parentheses.
2. `if` is an *expression*, meaning it produces a value.

```rust
let is_read = true;
let indicator = if is_read { "  " } else { "● " };
```

This is exactly what thrssty does in `src/ui.rs` — the indicator symbol is chosen inline
without a separate `let … if … else` block.

The two branches must produce the same type:

```rust
let n: i32 = if some_condition { 42 } else { 0 };   // both branches: i32
```

### chained `else if`

```rust
let style = if is_selected {
    t.style("article.selected")
} else if is_read {
    t.style("article.read")
} else {
    t.style("article.unread")
};
```

This is from `src/ui.rs`: the style of a row in the article list depends on three conditions
checked in priority order.

---

## `loop`, `while`, and `for`

### `loop`

`loop` runs forever until you `break` out of it. It is the right construct when you do not know
in advance how many iterations you need:

```rust
loop {
    terminal.draw(|f| thrssty::ui::draw(f, app))?;

    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && !app.handle_key(key) {
                break;   // quit when handle_key returns false
            }
        }
    }

    app.tick();
}
```

This is the main event loop in `src/main.rs`: draw a frame, wait up to 100 ms for a key press,
process it, update background state, and repeat.

`loop` can also return a value through `break`:

```rust
let result = loop {
    let n = compute_something();
    if n > 100 { break n; }   // n is the value of the whole loop expression
};
```

### `while`

```rust
while !queue.is_empty() {
    process(queue.pop());
}
```

### `for` and ranges

```rust
for i in 0..10 { println!("{}", i); }   // 0, 1, 2, …, 9
for i in 0..=10 { println!("{}", i); }  // 0, 1, 2, …, 10 (inclusive)
```

Iterating over a collection:

```rust
for url in &urls {
    fetch(url);
}
```

The `&` here borrows `urls` — we iterate over references to the items, not consuming the
collection. (Ownership and borrowing are covered in depth in Lesson 04.)

---

## `match`

`match` is Rust's most powerful control-flow construct. It is like a `switch` statement on
steroids: it matches a value against a list of **patterns** and executes the first matching arm.

```rust
match event.code {
    KeyCode::Char(c) if key_str.len() == 1 => c.to_string() == key_str,
    KeyCode::Up        => key_str == "Up",
    KeyCode::Down      => key_str == "Down",
    KeyCode::Home      => key_str == "Home",
    KeyCode::End       => key_str == "End",
    KeyCode::PageUp    => key_str == "PageUp",
    KeyCode::PageDown  => key_str == "PageDown",
    KeyCode::Enter     => key_str == "Enter",
    KeyCode::Esc       => key_str == "Esc",
    _                  => false,   // catch-all (the underscore wildcard)
}
```

This is the `matches_key` function from `src/config.rs`. Every arm has the form `pattern => expression`.

### Match is exhaustive

The compiler requires every possible value to be covered. If you forget a case, the code will not
compile. The `_` wildcard is the conventional catch-all.

### Guards

A pattern can have an additional condition after `if`:

```rust
KeyCode::Char(c) if key_str.len() == 1 => c.to_string() == key_str,
```

The arm only fires when the pattern matches *and* the guard is true.

### Matching on enum variants

`match` is the primary way to handle enums (covered in Lesson 05). Here is a preview from the
background fetch handler in `src/app.rs`:

```rust
for msg in messages {
    match msg {
        FetchMessage::Started { url } => { … }
        FetchMessage::Complete { url, articles } => { … }
        FetchMessage::Error { url, reason } => { … }
        FetchMessage::Done => { … }
    }
}
```

Each arm **destructures** the enum variant, binding its fields to local names.

---

## `if let`

`if let` is shorthand for a `match` with exactly one interesting arm:

```rust
// Verbose match:
match some_option {
    Some(value) => do_something(value),
    None => {}   // we don't care about None
}

// Equivalent if let:
if let Some(value) = some_option {
    do_something(value);
}
```

thrssty uses `if let` extensively when checking optional values:

```rust
// src/app.rs — open article in browser
let url = self.current_article().and_then(|a| a.url.clone());
if let Some(url) = url {
    let _ = open::that_detached(&url);
    self.ui.status_message = Some("Done.".into());
}
```

```rust
// src/ui.rs — only draw article header if we have an article
if let Some(article) = article {
    lines.push(…);
}
```

You can also combine `if let` with `else`:

```rust
if let Some(fetching) = &app.ui.fetching {
    (format!(" ⟳ Fetching {}…", fetching), bar_style)
} else if let Some(msg) = &app.ui.status_message {
    let style = if app.ui.status_is_error { err_style } else { bar_style };
    (format!(" {msg}"), style)
} else {
    (String::new(), bar_style)
}
```

This is the status-bar rendering from `src/ui.rs`: it tries each condition in order and returns
the first match.

---

## `while let`

Like `if let`, but keeps looping while the pattern matches:

```rust
while let Some(message) = rx.try_recv().ok() {
    process(message);
}
```

thrssty uses the iterator version instead (`.try_iter().collect()`) but the concept is the same.

---

## `let else` (early exit)

Introduced in Rust 1.65. When a pattern must match — and if it does not, you want to exit early:

```rust
let Some(feed) = app.feeds.get(*feed_index) else {
    return ListItem::new(Line::raw(""));   // bail out if no feed at this index
};
// feed is bound here and we continue
```

From `src/ui.rs`. Without `let else` you would need:

```rust
let feed = match app.feeds.get(*feed_index) {
    Some(f) => f,
    None => return ListItem::new(Line::raw("")),
};
```

`let else` is more concise and keeps the happy path un-indented.

---

## `return` vs Implicit Return

In Rust, the last expression in a block (without a semicolon) is the block's value:

```rust
fn is_positive(n: i32) -> bool {
    n > 0     // implicit return — no semicolon
}
```

Use `return` only for early exits:

```rust
fn find_first_unread(articles: &[Article], read_ids: &HashSet<String>) -> usize {
    for (i, article) in articles.iter().enumerate() {
        if !read_ids.contains(&article.id) {
            return i;   // early return as soon as we find one
        }
    }
    0   // implicit return: default to first article
}
```

---

## Putting It Together: The Feed Navigation Logic

Here is a condensed version of `next_feed()` from `src/app.rs` that uses several of these constructs:

```rust
fn next_feed(&mut self) {
    let order = &self.sidebar.feed_order;

    if order.is_empty() {   // guard: nothing to do
        return;
    }

    // Find where the current feed sits in the navigation order.
    let pos = order
        .iter()
        .position(|&i| i == self.ui.selected_feed)
        .unwrap_or(0);   // unwrap_or: provide a default when None

    if pos + 1 < order.len() {
        self.ui.selected_feed = order[pos + 1];
        self.sync_article_selection();
        self.render_current_article();
    }
}
```

- `if order.is_empty() { return; }` — early exit guard.
- `.position(|&i| i == …)` — returns `Option<usize>` (the index, or `None` if not found).
- `.unwrap_or(0)` — extract the value or use `0` as the fallback.
- `if pos + 1 < order.len()` — bounds check before indexing.

---

## Summary

- `if` is an expression: both branches must have the same type.
- `loop` runs until `break`; `while` checks a condition; `for` iterates over a range or collection.
- `match` exhaustively matches patterns; the `_` wildcard is the catch-all.
- Guards (`if cond`) add extra conditions to match arms.
- `if let` is shorthand for a one-arm match on `Option` or `Result`.
- `let else` extracts a value and exits early if it does not match.
- Functions return the last expression implicitly; `return` is for early exits.

Next: [Lesson 04 — Ownership and Borrowing](04-ownership-and-borrowing.md)
