# thrssty

[![Crates.io](https://img.shields.io/crates/v/thrssty.svg)](https://crates.io/crates/thrssty)
[![Docs.rs](https://docs.rs/thrssty/badge.svg)](https://docs.rs/thrssty)
[![CI](https://github.com/rkratky/thrssty/workflows/CI/badge.svg)](https://github.com/rkratky/thrssty/actions)

A terminal RSS/Atom newsreader written in Rust, styled after the [Mutt](http://www.mutt.org/) email
client. Supports Helix-compatible color themes and a fully configurable keyboard layout.

## Features

- **Three-panel layout**: feed sidebar · article list · article view
- **RSS and Atom** feed support (via [feed-rs](https://crates.io/crates/feed-rs))
- **HTML rendering** via [html2text](https://crates.io/crates/html2text): headings, bold, italic,
  links (with footnote references), inline code, blockquotes, tables, and nested lists
- **Naked URL detection** — bare URLs in article text are highlighted as links
- **Read/unread tracking** persisted across sessions
- **Background fetching** — UI stays responsive during network I/O
- **Proportional scrollbar** with position indicator in all three panels
- **Helix-compatible themes** — use any stock Helix `.toml` theme directly by path, or place
  custom themes in `~/.config/thrssty/themes/`; `inherits` chains are resolved automatically
- **Cascading config** — CLI flags → XDG config → working-dir config → built-in defaults
- **Fully configurable** key bindings, panel proportions, and colors

## Layout

```
┌──────────────┬─────────────────────────────────┐
│              │  Article List         date      │
│  Feed        │  ● Unread title       2026-03-01│
│  Sidebar     │    Read title         2026-02-28│
│              ├─────────────────────────────────┤
│  ● Feed 1    │  Article title                  │
│    Feed 2    │  Published: Sunday, 01 March…   │
│              │                                 │
│              │  Article body text here with    │
│              │  **bold** and *italic* support. │
├──────────────┴─────────────────────────────────┤
│  q:quit  r:refresh  R:refresh-all  o:open  …   │
└────────────────────────────────────────────────┘
```

## Key Bindings (defaults)

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down/up in the feed sidebar |
| `←` / `→` | Move cursor up/down in the article list |
| `Home` / `End` | Jump to first/last article |
| `↑` / `↓` | Scroll the article view one line |
| `PgUp` / `PgDn` | Scroll the article view one page |
| `r` | Refresh the current feed |
| `R` | Refresh all feeds |
| `a` | Add a new feed (prompts for URL and optional category) |
| `d` | Delete the selected feed (prompts for confirmation) |
| `e` | Edit the display name of the selected feed |
| `o` | Open the article URL in the system browser |
| `F5` | Reload config and feeds files without restarting |
| `q` | Quit |

All keys are configurable — see [Configuration](#configuration) below.

Navigation is global: feed, article, and scroll keys work at all times without switching focus
between panels.

## Installation

### From source

```bash
# Requires Rust 1.75+ (install via https://rustup.rs)
cargo install --path .
```

### Via Cargo

```bash
cargo install thrssty
```

### Manual page

The repository ships a pre-written manpage at `man/thrssty.1`.
Install it to your local man directory so `man thrssty` works without root:

```bash
mkdir -p ~/.local/share/man/man1
cp man/thrssty.1 ~/.local/share/man/man1/

# Make sure the directory is on your MANPATH (add to ~/.bashrc / ~/.zshrc):
export MANPATH="$HOME/.local/share/man:$MANPATH"

# Verify
man thrssty
```

When building a Debian package with
[cargo-deb](https://github.com/kornelski/cargo-deb), the manpage is
installed to `/usr/share/man/man1/` automatically via the
`[package.metadata.deb]` section in `Cargo.toml`.

## Quick start

```bash
# Write default config, theme, and example feeds files to ~/.config/thrssty/
thrssty --init

# Edit the feeds file to add your own feeds
$EDITOR ~/.config/thrssty/feeds.toml

# Launch
thrssty
```

## Usage

```
thrssty [OPTIONS]

OPTIONS
  -h, --help               Show help and exit
  -v, --version            Show version and exit
  -r, --refresh            Refresh all feeds on startup
  -c, --config <path>      Use the specified config file
  -t, --theme  <path|name> Override the theme from the config
  -i, --init               Write default config/theme/feeds to
                           ~/.config/thrssty/ and exit
```

### Config precedence (highest → lowest)

1. `--config <path>` (command line)
2. `~/.config/thrssty/config.toml`
3. `./config.toml` (current working directory)
4. Built-in defaults

Settings are merged across these sources — a value in a higher-priority source overrides the same
value in a lower-priority source; absent keys fall back to defaults.

### Theme precedence (highest → lowest)

1. `--theme <path|name>` (command line)
2. `theme` key in the effective config
3. Built-in default theme

## Configuration

The configuration file lives at `~/.config/thrssty/config.toml`. Run `thrssty --init` to
generate the file with all default values and annotations, or copy the bundled
[`config.toml`](config.toml) manually.

All options are optional; omitting any key restores its default.

### Full example

```toml
theme               = "auto"         # "auto", "default", name, or path; see Theming below
refresh_on_start    = true           # fetch all feeds on launch (default: true)
feeds_file          = "feeds.toml"   # path to feeds definition file (default: feeds.toml)
uncategorized_label = "No category"  # label for feeds not in any category
max_article_age_days = 100           # purge cached articles older than N days (0 = never)

[layout]
sidebar_width       = 20     # feed sidebar width as % of terminal width  (default: 20)
article_list_height = 40     # article list height as % of right panel    (default: 40)
max_author_col_pct  = 25     # max width of author column as % of list panel (default: 25)

[keys]
quit              = "q"
refresh_feed      = "r"
refresh_all       = "R"
open_browser      = "o"
reload_config     = "F5"
add_feed          = "a"
delete_feed       = "d"
edit_feed         = "e"
feed_down         = "j"
feed_up           = "k"
article_down      = "Right"
article_up        = "Left"
article_home      = "Home"
article_end       = "End"
scroll_down       = "PageDown"
scroll_up         = "PageUp"
scroll_line_down  = "Down"
scroll_line_up    = "Up"
```

### Key syntax

Single characters are written as-is (`"q"`, `"j"`, `"R"`). Named keys: `"Up"`, `"Down"`,
`"Left"`, `"Right"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`, `"Enter"`, `"Esc"`,
`"Tab"`, `"BackTab"`, `"Backspace"`, `"Delete"`, `"Insert"`, `"Space"`, `"F1"`–`"F12"`.

## Feeds configuration

Feeds and categories are defined in a separate file, by default
`~/.config/thrssty/feeds.toml` (configurable via `feeds_file` in `config.toml`).
A ready-to-edit example is included in the repository as [`feeds.toml`](feeds.toml).

The file is **optional**: thrssty starts with an empty sidebar if it does not
exist. Press `a` in the UI to add feeds interactively — they are written to the
feeds file automatically (creating it if necessary).

All text prompts (`a`, `e`, category name) support standard readline-style
editing: arrow keys, `Ctrl+A`/`Ctrl+E` (start/end), `Ctrl+W` (delete word),
`Ctrl+U` (clear line), `Ctrl+K` (kill to end).

> **Tip:** Run `thrssty --init` to generate a commented example `feeds.toml`
> alongside the default config at `~/.config/thrssty/`.

### Flat feed list (no categories)

When no categories are defined, feeds are shown as a simple list in the sidebar:

```toml
[[feed]]
url  = "https://this-week-in-rust.org/rss.xml"
name = "This Week in Rust"        # optional display-name override

[[feed]]
url = "https://blog.rust-lang.org/feed.xml"
```

### Feeds with categories

Use `[category.N]` sections (numeric keys control display order) to group feeds.
Feeds without a category appear under a configurable "No category" header at the bottom.

```toml
[category.1]
title = "Rust"

[[category.1.feed]]
url  = "https://blog.rust-lang.org/feed.xml"
name = "Rust Blog"

[[category.1.feed]]
url = "https://this-week-in-rust.org/rss.xml"

[category.2]
title = "Linux"

[[category.2.feed]]
url = "https://www.kernel.org/feeds/kdiff.xml"

# Uncategorized feeds (shown last, under "No category"):
[[feed]]
url = "https://lwn.net/headlines/rss"
name = "LWN.net"
```

**Sidebar appearance with categories:**

```
┌─────────────────────────┐
│ Feeds                   │
│ ● Rust               3/1│
│   ● Rust Blog        2/1│
│     This Week in…    1/0│
│ ● Linux              5/2│
│   ● Kernel.org       5/2│
│   No category        1/0│
│     LWN.net          1/0│
└─────────────────────────┘
```

- Category names are **bold and highlighted** when they have unread articles.
- Feed names are indented under their category.
- `j`/`k` navigation skips category header rows — only feeds are selectable.
- All defined categories are shown even if they contain no feeds.

## Theming

thrssty uses the [Helix editor theme format](https://docs.helix-editor.com/master/themes.html).
The `theme` config key accepts:

| Value | Effect |
|-------|--------|
| `"auto"` | Detect terminal background; load `default_dark` or `default_light` (default) |
| `"default_dark"` | Built-in dark theme (Ao-inspired) |
| `"default_light"` | Built-in light theme |
| `"purple"` | Catppuccin Mocha-inspired (copy `themes/purple.toml` to `~/.config/thrssty/themes/`) |
| `"name"` | `~/.config/thrssty/themes/name.toml` |
| `"/path/to/theme.toml"` | Direct file path |

```toml
theme = "auto"                                           # auto-detect (default)
theme = "default_dark"                                   # always dark
theme = "default_light"                                  # always light
theme = "purple"                                         # user-installed example theme
theme = "/usr/lib/helix/runtime/themes/ayu_evolve.toml" # any Helix theme by path
```

**Terminal background detection** checks `$TERM_BACKGROUND` (Kitty), `$COLORFGBG` (rxvt/xterm),
then probes via the OSC 11 escape sequence (200 ms timeout), and falls back to dark if all three
fail. You can bypass detection entirely with `"default_dark"` or `"default_light"`.

**Stock Helix themes work out of the box.** Standard Helix scopes (`ui.*`, `markup.*`) are
automatically mapped to thrssty's display scopes, so any Helix theme renders reasonably without
manual customisation. `inherits` chains are resolved from the same directory as the theme file,
matching Helix's own behaviour.

### Available scopes

| Scope | Purpose |
|-------|---------|
| `ui.background` | Terminal background |
| `ui.text` | Default foreground text |
| `ui.border` | Panel borders |
| `ui.statusline` | Status / help bar |
| `ui.statusline.key` | Key chips in the help bar |
| `ui.statusline.error` | Error messages in the status bar |
| `ui.selection` | Generic highlight background |
| `category.unread` | Category header with unread articles |
| `category.read` | Category header with no unread articles |
| `feed.unread` | Feed with unread articles |
| `feed.read` | Feed with no unread articles |
| `feed.selected` | Currently selected feed row |
| `feed.error` | Feed whose last fetch produced an error |
| `article.unread` | Unread article title |
| `article.read` | Read article title |
| `article.selected` | Currently selected article row |
| `article.date` | Publication date |
| `article.author` | Author name in article header |
| `article.category` | Category / tag line in article header |
| `article.description` | Summary / description text |
| `article.bold` | Bold text in article body |
| `article.italic` | Italic text in article body |
| `article.link` | Hyperlinks (and naked URLs) |
| `article.h1` / `h2` / `h3` | Heading levels |
| `article.code` | Inline and block code |
| `article.quote` | Blockquotes |

### Example theme

```toml
# ~/.config/thrssty/themes/nord.toml

[palette]
bg      = "#2e3440"
fg      = "#d8dee9"
surface = "#3b4252"
blue    = "#81a1c1"
green   = "#a3be8c"
dim     = "#4c566a"

"ui.background"    = { bg = "bg" }
"ui.text"          = { fg = "fg" }
"ui.border"        = { fg = "dim" }
"ui.statusline"    = { fg = "fg", bg = "surface" }
"ui.statusline.key"= { fg = "blue", bg = "surface", modifiers = ["bold"] }

"category.unread"  = { fg = "blue", modifiers = ["bold"] }
"category.read"    = { fg = "dim" }

"feed.unread"      = { fg = "blue", modifiers = ["bold"] }
"feed.read"        = { fg = "dim" }
"feed.selected"    = { fg = "fg", bg = "surface" }

"article.unread"   = { fg = "fg", modifiers = ["bold"] }
"article.read"     = { fg = "dim" }
"article.selected" = { fg = "fg", bg = "surface" }
"article.date"     = { fg = "green" }
"article.bold"     = { modifiers = ["bold"] }
"article.italic"   = { modifiers = ["italic"] }
"article.link"     = { fg = "blue", modifiers = ["underlined"] }
```

## Cache and data

Fetched articles and read/unread state are persisted to `~/.local/share/thrssty/cache.json`.
The cache is updated after every successful fetch and on clean exit.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).

