# Lesson 13 — Building a Terminal UI with ratatui

`ratatui` is a Rust TUI (Terminal User Interface) library. It provides an *immediate-mode*
rendering model: every frame is drawn from scratch by calling a series of widget render calls.
`crossterm` handles the low-level terminal interactions: enabling raw mode, reading key events,
drawing to the alternate screen.

---

## Terminal Setup and Teardown

Before drawing anything, you must put the terminal in *raw mode* (key presses are sent directly
to the program, not buffered by the OS line editor) and switch to the *alternate screen* (a
separate screen buffer, so the normal shell output is not disturbed).

```rust
// src/main.rs
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
```

`execute!` is a crossterm macro that sends escape sequences to the terminal. Always call
`restore_terminal` before exiting — even on errors — or the terminal will be left in raw mode,
making it unusable until the user manually resets it.

---

## The Draw–Poll–Handle Loop

```rust
fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // 1. Draw one frame.
        terminal.draw(|f| thrssty::ui::draw(f, app))?;

        // 2. Wait up to 100 ms for a key press.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && !app.handle_key(key) {
                    break;   // handle_key returned false → quit
                }
            }
        }

        // 3. Advance background state (drain fetch messages, etc.).
        app.tick();
    }
    Ok(())
}
```

This is the **immediate-mode** pattern:
- Draw: render the current state to the terminal.
- Poll: check for user input (with a timeout so `tick()` runs regularly).
- Handle: update the application state.
- Repeat.

`terminal.draw(|f| …)` passes a `Frame` to the closure. The closure calls ratatui render
functions to draw widgets into the frame's buffer. At the end of the closure, ratatui
computes the diff from the previous frame and sends only the changes to the terminal.

---

## Layout: Splitting the Screen

ratatui's `Layout` divides a rectangular area into sub-areas:

```rust
use ratatui::layout::{Constraint, Layout};

// Split the whole screen into [main area | 1-row status bar]
let outer = Layout::vertical([
    Constraint::Min(1),       // takes all remaining space
    Constraint::Length(1),    // exactly 1 row
]).split(area);

let main_area   = outer[0];
let status_area = outer[1];

// Split main area into [sidebar | right content]
let main_chunks = Layout::horizontal([
    Constraint::Percentage(sidebar_pct),   // e.g. 20%
    Constraint::Min(0),
]).split(main_area);

// Split right content into [article list | article view]
let content_chunks = Layout::vertical([
    Constraint::Percentage(list_pct),      // e.g. 40%
    Constraint::Min(0),
]).split(main_chunks[1]);
```

`Constraint` variants:
- `Length(n)` — exactly `n` rows/columns.
- `Percentage(p)` — `p` percent of the available space.
- `Min(n)` — at least `n` rows/columns; takes all remaining space.
- `Max(n)` — at most `n` rows/columns.

`split` returns a `Vec<Rect>` (or slice); index into it to get the sub-areas.

---

## Styles and Colours

A `Style` describes how text looks: foreground colour, background colour, and modifiers (bold,
italic, underline, …):

```rust
use ratatui::style::{Color, Modifier, Style};

let style = Style::default()
    .fg(Color::Rgb(0xCB, 0xA6, 0xF7))   // purple foreground
    .bg(Color::Rgb(0x1E, 0x1E, 0x2E))   // dark background
    .add_modifier(Modifier::BOLD);
```

thrssty's `Theme` struct maps scope names to `Style` values:

```rust
t.style("article.unread")    // → Style with bright green foreground
t.style("article.selected")  // → Style with reversed colours
t.style("ui.background")     // → Style with the dark background
```

---

## Spans, Lines, and Text

ratatui's text model has three levels:

| Type | Description |
|------|-------------|
| `Span<'a>` | A styled string fragment — text + style. |
| `Line<'a>` | A row on screen: a `Vec<Span>`, all on one line. |
| `Text<'a>` | Multiple `Line`s forming a paragraph. |

```rust
use ratatui::text::{Line, Span};

let span1 = Span::raw("plain text");
let span2 = Span::styled("bold text", Style::default().add_modifier(Modifier::BOLD));

let line = Line::from(vec![span1, span2]);
```

thrssty assembles article headers, sidebar rows, and status-bar content by building `Vec<Span>`:

```rust
let mut spans = vec![
    Span::styled(indicator.to_owned(), indicator_style),
    Span::styled(
        format!("{:<title_width$} ", title, title_width = title_width),
        title_style,
    ),
];
if author_col > 0 {
    spans.push(Span::styled(
        format!("{:>author_col$}  ", raw_author, author_col = author_col),
        t.style("article.author"),
    ));
}
spans.push(Span::styled(date_str, t.style("article.date")));
ListItem::new(Line::from(spans))
```

---

## Widgets

A widget is anything that can be rendered into a `Rect`. The most important ones in thrssty:

### `Block`

A decorative border with an optional title:

```rust
use ratatui::widgets::{Block, Borders};

let block = Block::default()
    .borders(Borders::ALL)
    .title(Span::styled(" Feeds ", t.style("ui.text")))
    .style(bg_style)
    .padding(Padding::horizontal(1));   // 1 space on left and right inside the border

frame.render_widget(block, area);
```

`block.inner(area)` gives you the rectangle inside the border, accounting for padding —
useful when you need to draw the divider line at the correct width.

### `List` and `ListState`

A scrollable list of `ListItem`s:

```rust
use ratatui::widgets::{List, ListItem, ListState};

let items = vec![
    ListItem::new(Span::styled("Item 1", style1)),
    ListItem::new(Line::from(vec![span_a, span_b])),
];

let list = List::new(items)
    .block(block)
    .highlight_style(t.style("article.selected"));

let mut list_state = ListState::default();
list_state.select(Some(selected_index));   // which row to highlight

frame.render_stateful_widget(list, area, &mut list_state);
```

`render_stateful_widget` (not `render_widget`) is used when the widget has mutable state,
such as scroll position. ratatui uses the `ListState` to keep the selected item visible.

### `Paragraph`

Multi-line text with optional word-wrapping and scrolling:

```rust
use ratatui::widgets::{Paragraph, Wrap};

let lines: Vec<Line<'static>> = build_article_lines();

let paragraph = Paragraph::new(lines)
    .block(block)
    .wrap(Wrap { trim: false })          // wrap long lines; don't trim leading whitespace
    .scroll((app.ui.article_scroll, 0)); // vertical scroll offset

frame.render_widget(paragraph, area);
```

`.scroll((row, col))` shifts the visible content by `row` rows and `col` columns.

---

## Stateful vs Stateless Rendering

| Function | Use when |
|----------|----------|
| `frame.render_widget(widget, area)` | Widget has no runtime state (Block, Paragraph) |
| `frame.render_stateful_widget(widget, area, &mut state)` | Widget tracks scroll/selection (List, Table) |

thrssty uses `render_stateful_widget` for both the feed sidebar and the article list so ratatui
can auto-scroll to keep the selected row visible.

---

## The `'static` Lifetime Requirement

ratatui stores rendered content between frames for diffing. Pre-built `Line<'a>` values can only
be stored if their lifetime is `'static` (data is owned, not borrowed from a stack frame that
might disappear).

thrssty pre-renders article content into `Vec<Line<'static>>` by building `Span`s from owned
`String`s:

```rust
pub article_lines: Vec<Line<'static>>,

// Building a Line<'static>:
let line = Line::from(Span::styled(
    text.to_owned(),   // owned String → 'static Span → 'static Line
    style,
));
```

`Span::raw(string)` where `string: String` produces `Span<'static>` because the data is owned.
`Span::raw(&str)` produces `Span<'a>` borrowing from the string — which cannot be `'static`
unless it comes from a string literal.

---

## The HTML Rendering Pipeline

`src/html.rs` converts HTML article content into `Vec<Line<'static>>` through a pipeline that
preserves semantic styling:

1. **Pre-processing**: entity-encoded HTML (`&lt;em&gt;`) is decoded to real HTML tags.
   Markdown-formatted feeds (no `<` characters) are converted to HTML so the pipeline is uniform.

2. **Link numbering**: all `<a href="url">text</a>` links are rewritten to
   `<a href="url">text[n]</a>` before rendering, and the URLs are collected into a footnote list.
   This happens *before* `html2text` so that column-width accounting includes the `[n]` markers.

3. **html2text rendering**: `html2text` with `RichDecorator` converts the HTML into annotated
   lines. Each line is a list of `(text, Vec<RichAnnotation>)` pairs — annotations mark bold,
   italic, links, headings, code, etc.

4. **Style mapping**: annotations are translated to ratatui `Style`s drawn from the active theme:

   ```rust
   RichAnnotation::Strong   → Modifier::BOLD
   RichAnnotation::Emphasis → Modifier::ITALIC
   RichAnnotation::Code     → styles.code
   RichAnnotation::Link(u)  → styles.link  (plus collect URL for footnotes)
   ```

5. **Footnote URLs**: collected link URLs are appended as `[n] https://…` lines at the end of
   the article — readable without a pointing device.

The result is a `Vec<Line<'static>>` built from owned `String`s that can be stored in `App`
and rendered cheaply on every frame.

### Why `'static` for pre-rendered lines?

Pre-rendering means `article_lines` is computed once (when the article selection changes) and
kept in `App`. ratatui's `Line<'a>` borrows string data for lifetime `'a`. Since the `App`
struct owns these lines past the render closure, the lines must not borrow from anything that
could go away — so thrssty converts all strings to owned `String`s and passes them to
`Span::raw(string)`, which produces `Line<'static>`.

---

## Scroll Clamping

The article view height is captured from the layout on every frame and stored in `UiState`:

```rust
app.ui.article_view_height = content_chunks[1].height.saturating_sub(2); // minus borders
```

When the user scrolls, the offset is clamped so they cannot scroll past the end:

```rust
fn scroll_down(&mut self) {
    let page = self.ui.article_view_height.max(1);
    let max_scroll =
        (self.article_lines.len() as u16).saturating_sub(self.ui.article_view_height);
    self.ui.article_scroll = (self.ui.article_scroll + page).min(max_scroll);
}
```

`saturating_sub` prevents unsigned integer underflow: `5u16.saturating_sub(10) == 0`, whereas
`5u16 - 10` would panic in debug mode or silently wrap in release mode.

---

## Summary

- `crossterm` puts the terminal in raw mode and manages the alternate screen.
- `terminal.draw(|f| …)` passes a `Frame` to the draw closure; ratatui diffs frames for efficiency.
- `Layout::vertical/horizontal([constraints]).split(area)` divides the screen.
- `Span` → `Line` → `Paragraph/List/Block` is the text composition hierarchy.
- `render_stateful_widget` is used for scrollable/selectable widgets.
- Pre-built `Line<'static>` values use owned `String`s in their `Span`s.
- `saturating_sub` and `.min()` keep numeric computations safe against overflow.

Next: [Lesson 14 — Architecture: Putting the Application Together](14-complete-application-architecture.md)
