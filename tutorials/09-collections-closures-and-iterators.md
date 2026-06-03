# Lesson 09 — Collections, Closures, and Iterators

The three topics of this lesson work together constantly in thrssty. Collections hold the data,
iterators traverse it, and closures provide the logic for each step.

---

## Collections

### `Vec<T>` — Dynamic Array

`Vec<T>` is the workhorse collection: a heap-allocated, contiguous, growable array. Think of it
as a managed version of a C array, or Python's list.

```rust
let mut v: Vec<i32> = Vec::new();       // empty, explicit type
let mut v = Vec::new();                  // empty, type inferred later
let v = vec![1, 2, 3];                  // filled with the vec! macro

v.push(4);           // append
v.pop();             // remove last (returns Option<T>)
let len = v.len();   // number of elements
let first = &v[0];   // index (panics on out-of-bounds!)
let first = v.get(0); // safe index (returns Option<&T>)
```

thrssty uses `Vec` for almost everything:

```rust
pub feeds: Vec<Feed>,
pub articles: Vec<Article>,
pub categories: Vec<String>,
pub items: Vec<SidebarItem>,   // sidebar rows
pub feed_order: Vec<usize>,    // navigation cursor positions
```

### `HashMap<K, V>` — Hash Map

A hash table mapping keys to values. Lookup, insert, and remove are all O(1) average.

```rust
use std::collections::HashMap;

let mut map: HashMap<String, usize> = HashMap::new();
map.insert("alpha".to_owned(), 1);
map.insert("beta".to_owned(), 2);

let v = map.get("alpha");     // Option<&usize>
let v = map["alpha"];         // usize (panics if key absent)
map.remove("beta");
```

thrssty uses `HashMap` for two things:

```rust
// Cache: per-feed read article IDs — feed URL → set of article IDs
pub read_ids: HashMap<String, HashSet<String>>,

// Theme: scope name → ratatui Style
styles: HashMap<String, Style>,
```

#### The `entry` API

A common pattern: look up a key and modify it in place (or insert a default if absent):

```rust
self.read_ids
    .entry(feed_url.to_owned())  // get the entry for this key (existing or vacant)
    .or_default()                // if vacant, insert HashSet::default() (empty set)
    .insert(article_id.to_owned()); // then insert into the set
```

Without the `entry` API you would need two lookups: one to check existence, one to modify.

### `HashSet<T>` — Hash Set

A collection of unique values — like `HashMap<T, ()>` but with a cleaner API.

```rust
use std::collections::HashSet;

let mut set: HashSet<String> = HashSet::new();
set.insert("abc".to_owned());
set.insert("abc".to_owned());   // duplicate — silently ignored
set.contains("abc");            // true
set.remove("abc");
```

thrssty uses `HashSet` in two places:

```rust
pub read_ids: HashMap<String, HashSet<String>>,  // per-feed read article IDs
pub feed_errors: HashSet<String>,                // URLs that failed to fetch
```

---

## Closures

A **closure** is an anonymous function that can *capture* variables from its surrounding scope.
Closures are the glue between iterators — you pass a closure to `.map()`, `.filter()`, etc. to
describe what to do with each element.

```rust
let double = |x| x * 2;         // closure taking one argument
let add = |a, b| a + b;         // two arguments
let greet = |name: &str| format!("Hello, {name}!");  // explicit type annotation
```

The `|…|` delimiters replace `fn`'s parentheses.

### Capturing from the environment

```rust
let threshold = 5;
let above = |x| x > threshold;   // captures `threshold` by reference
assert!(above(10));
assert!(!above(3));
```

C functions cannot do this — they have no access to the local scope where they are called.
Closures can, and the compiler tracks *what* is captured and *how*.

### Closure traits: `Fn`, `FnMut`, `FnOnce`

| Trait | Meaning |
|-------|---------|
| `FnOnce` | Can be called **at most once**; may consume (move) captured values |
| `FnMut` | Can be called multiple times; may mutate captured values |
| `Fn` | Can be called multiple times; does not mutate captured values |

Most closures are `Fn`. thrssty uses `FnMut` implicitly when the draw closure captures `app`:

```rust
terminal.draw(|f| thrssty::ui::draw(f, app))?;
//            ^^^ closure passed to draw()
```

The closure captures `app` mutably because `draw` takes `&mut App`.

---

## Iterators

Any type implementing `Iterator` gains a rich set of adapter methods. Iterators are **lazy**:
building a chain of adapters does no work. Work happens only when the chain is consumed.

### Creating an iterator

| Expression | Iterator over |
|------------|---------------|
| `v.iter()` | `&T` (shared references) |
| `v.iter_mut()` | `&mut T` (mutable references) |
| `v.into_iter()` | `T` (moves elements out of the collection) |
| `0..n` | integers 0, 1, …, n−1 |
| `0..=n` | integers 0, 1, …, n |

### Key adapter methods

#### `.map(f)` — transform each element

```rust
let urls: Vec<String> = self.feeds.iter().map(|f| f.url.clone()).collect();
//                                        ^^^ closure: &Feed → String
```

#### `.filter(predicate)` — keep elements where predicate is true

```rust
let unread: Vec<&Article> = articles
    .iter()
    .filter(|a| !self.is_read(url, &a.id))
    .collect();
```

#### `.filter_map(f)` — map and filter in one step (keeps `Some`, drops `None`)

```rust
let author_widths: Vec<usize> = articles
    .iter()
    .filter_map(|a| a.author.as_deref())   // Option<&str>: None is dropped
    .map(|s| s.chars().count())
    .collect();
```

#### `.enumerate()` — attach an index

```rust
for (i, article) in articles.iter().enumerate() {
    let is_selected = i == app.ui.selected_article;
    // …
}
```

#### `.position(predicate)` — find the first matching index

```rust
let pos = order
    .iter()
    .position(|&i| i == self.ui.selected_feed)
    .unwrap_or(0);
```

Returns `Option<usize>`.

#### `.fold(initial, f)` — reduce to a single value

```rust
let (total, unread) = feed_indices.iter().fold((0usize, 0usize), |(t, u), &idx| {
    if let Some(feed) = app.feeds.get(idx) {
        let feed_unread = app.cache.unread_count(&feed.url, &feed.articles);
        (t + feed.articles.len(), u + feed_unread)
    } else {
        (t, u)
    }
});
```

`fold` threads an accumulator (`(t, u)`) through each element and returns the final value.

#### `.any(predicate)` and `.all(predicate)` — short-circuit boolean tests

```rust
let has_body = app.article_lines
    .iter()
    .any(|l| l.spans.iter().any(|s| !s.content.trim().is_empty()));
```

`.any()` returns `true` as soon as one element satisfies the predicate; it does not process
remaining elements.

#### `.count()` — count elements

```rust
let unread = articles
    .iter()
    .filter(|a| !self.is_read(feed_url, &a.id))
    .count();
```

#### `.max()` — find the largest element

```rust
let author_col: usize = articles
    .iter()
    .filter_map(|a| a.author.as_deref())
    .map(|s| s.chars().count())
    .max()
    .unwrap_or(0);
```

#### `.collect()` — materialise the iterator into a collection

```rust
let urls: Vec<String> = feeds.iter().map(|f| f.url.clone()).collect();
let styles: HashMap<String, Style> = …iter().collect();
```

The target collection type is inferred from context or annotated explicitly.

### Chaining: a real example from thrssty

Building the cache URL lookup map (`src/app.rs`):

```rust
let cache_map: HashMap<&str, &Feed> =
    cache.feeds
        .iter()                         // Iterator<Item = &Feed>
        .map(|f| (f.url.as_str(), f))  // Iterator<Item = (&str, &Feed)>
        .collect();                     // HashMap<&str, &Feed>
```

Three lines: borrow each feed, pair it with its URL as a key, collect into a hash map.

Building sidebar items with `map` and `collect` (`src/ui.rs`):

```rust
let items: Vec<ListItem> = app.sidebar.items
    .iter()
    .map(|item| match item {
        SidebarItem::Category { … } => { … }
        SidebarItem::Feed { … } => { … }
    })
    .collect();
```

---

## Retaining Elements: `.retain()`

`Vec` and `HashSet` have `.retain(predicate)` — removes elements where the predicate is false:

```rust
feed.articles.retain(|a| {
    a.published.map(|d| d > cutoff).unwrap_or(true)
});
```

This is how thrssty purges old articles: keep articles newer than the cutoff date (or articles
with no publish date, which we cannot age-check).

---

## Sorting

```rust
articles.sort_by(|a, b| b.published.cmp(&a.published));   // newest-first
```

`.sort_by` takes a closure returning `std::cmp::Ordering`. Because `b` is on the left,
descending order is produced. This uses the `Ord` trait's `.cmp()` method.

---

## Summary

- `Vec<T>` — dynamic array, the most used collection.
- `HashMap<K, V>` — O(1) key–value lookup; the `entry` API handles insert-or-update cleanly.
- `HashSet<T>` — unique-value collection, used for read tracking and error sets.
- Closures are anonymous functions that capture their surrounding scope with `|params| body`.
- Iterators are lazy chains of adapters; call `.collect()` to materialise the result.
- Key adapters: `.map`, `.filter`, `.filter_map`, `.enumerate`, `.position`, `.fold`,
  `.any`, `.count`, `.max`, `.retain`.

Next: [Lesson 10 — Serialisation with Serde](10-serialisation-with-serde.md)
