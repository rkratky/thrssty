# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Edit feed name** (`e` key, configurable): pressing `e` on the selected feed opens an
  in-place input prompt pre-populated with the current display name; Enter confirms and
  persists the new name to `feeds.toml`; Esc cancels; empty input resets the name to the
  feed's own title on the next refresh
- **Coloured `--help` output**: `thrssty --help` now uses ANSI colours (bold section
  headers, green option flags, yellow placeholders, dim supplementary text); colours are
  automatically disabled when stdout is not a TTY
- **Terminal background detection** (`src/terminal`): at startup, thrssty checks `$TERM_BACKGROUND`
  (Kitty), `$COLORFGBG` (rxvt/xterm), and the OSC 11 escape sequence (200 ms timeout) to determine
  whether the terminal uses a dark or light background. The result is used to select the appropriate
  built-in theme when `theme = "auto"` (the new default)
- **Built-in light theme** (`default_light`): a clean light-background theme; selected automatically
  on light terminals when `theme = "auto"` or explicitly with `theme = "default_light"`
- **`"auto"` theme value**: detects the terminal background and loads `default_dark` or
  `default_light`; this is now the default value for `theme` in `config.toml`. The previous
  `"default"` value remains accepted as an alias
- **`"purple"` example theme** (`themes/purple.toml`): the former built-in Catppuccin Mocha-inspired
  dark theme, saved as an installable example; copy to `~/.config/thrssty/themes/purple.toml`
- **Helix theme file support**: `theme` in `config.toml` now accepts an absolute or relative
  path to any Helix-editor `.toml` theme file (e.g. `/usr/lib/helix/runtime/themes/ayu_evolve.toml`).
  `inherits` chains are resolved recursively (up to 8 levels). Unmapped Helix scopes are
  automatically mapped to the nearest thrssty equivalent
- **CLI options**: `-h/--help`, `-v/--version`, `-r/--refresh` (force refresh on startup),
  `-c/--config <path>` (alternate config file), `-t/--theme <path|name>` (override theme),
  `-i/--init` (write default config/theme/feeds files to `~/.config/thrssty/` and exit)
- **Config cascade**: settings are merged from built-in defaults < `./config.toml` <
  `~/.config/thrssty/config.toml` < `--config <path>`; missing keys fall back to the
  lower-priority source
- **In-app feed management**: `a` prompts for a URL in the status bar, validates it in the
  background, optionally asks for a category name, and writes the feed to `feeds.toml`
  (creating the file if absent); `d` deletes the selected feed after confirmation
- **No-feeds startup hint**: when the app starts with an empty feed list, the status bar
  displays a prompt explaining how to add feeds via `a` or `feeds.toml`
- **Linux manpage**: `man/thrssty.1` covering all options, key bindings, files, configuration,
  and theming; installable to `~/.local/share/man/man1/`
- `[package.metadata.deb]` section in `Cargo.toml` for building Debian packages with
  `cargo-deb` (binary, manpage, example configs, README, and CHANGELOG as assets)
- **Integration test suite** in `tests/` (138 tests across 5 files):
  `config.rs` (cascade, defaults, key matching), `feeds_config.rs` (parse, add, remove),
  `html_rendering.rs` (all element types, link edge cases), `state.rs` (cache, read tracking, purge),
  `theme.rs` (loading, colour parsing, modifiers, scope mapping)
- Feed categories in the sidebar with aggregate article counts and distinct styling
- Separate `feeds.toml` file for feed and category definitions (path configurable via
  `feeds_file` in `config.toml`)
- `uncategorized_label` config option to rename the auto-generated "No category" category
- `max_article_age_days` config option (default: 100); articles older than the configured
  threshold are purged from the cache on startup; set to 0 to disable
- Two new theme scopes: `category.unread` and `category.read`
- `feed.error` theme scope for feeds whose last fetch resulted in an error
- `ui.statusline.error` theme scope for error messages in the status bar
- Status bar now shows static key-binding hints right-aligned at all times; operation
  status (fetching, errors, "Done.") is shown left-aligned and truncated if necessary
- Naked URL highlighting: bare `http://` / `https://` URLs in article text are
  automatically detected and rendered in the `article.link` colour
- Markdown-style links (`[text](url)`) in feeds that deliver plain Markdown descriptions
  (e.g. KDE store feeds) are now rendered as proper hyperlinks with footnote references

### Changed

- **Default theme replaced**: the built-in dark theme is now based on Ao (by YardQuit) — a
  deep-navy dark theme with blue/green accents. The former Catppuccin Mocha-inspired theme is
  preserved as `themes/purple.toml`
- **Scrollbar thumb symbol**: changed from the default solid block to `░` (U+2591 LIGHT SHADE)
  for a lighter visual weight
- **Default `theme` value**: changed from `"default"` to `"auto"` in `config.toml` and embedded
  defaults. `"default"` remains accepted as an alias for `"auto"`
- **Arrow key defaults swapped**: `←`/`→` now navigate the article list (up/down);
  `↑`/`↓` now scroll the article body one line at a time
- **Scrollbar**: replaced hand-rolled up/down arrows with ratatui's `Scrollbar` widget;
  all three panels now show a proportional thumb that travels the full scroll range;
  arrows are placed one row inside the panel borders; track is hidden so the border
  shows through with the thumb overlaid
- **Key hint styling**: separator characters (`/`, `(`, `)`) between keys are no longer
  highlighted — only the key token itself is styled
- **Key chip colours in Helix themes**: key chips in the help line use the `keyword`
  accent colour for the foreground and the global background colour for consistency
  across themes
- `feeds.toml` is **optional**: the app starts with an empty sidebar when the file is
  absent and creates it automatically when a feed is added via `a`
- Feeds are now defined in `feeds.toml` instead of `[[feeds]]` blocks in `config.toml`
- Default feed sidebar width changed from 30 % to 20 %
- Article count display format changed to `(unread)/total`
- Opening an article in the browser now shows "Done." immediately instead of "Opening …"
- Feed fetch errors are shown in the error color and the affected feed is highlighted
  in the sidebar with the `feed.error` color
- **BREAKING** (rendering): replaced `htmd` + `pulldown-cmark` HTML-to-Markdown pipeline
  with `html2text 0.16`. Rendering is now done entirely in HTML without an intermediate
  Markdown step. Tables are sized to the panel width; list blocks gain blank-line
  spacing; heading `#` prefixes are kept and styled bold. Removes two dependencies.
- Key-hint separator in the status bar changed from `▌` to `●`
- **`feed-rs` 3 migration**: feed-rs now splits an author's name from their email, so the
  display-name heuristics are much simpler. Author names in the `Name <email@example.com>`
  form are shown for the first time; previously they were dropped entirely. Article URLs
  now ignore the `<comments>` / `<wfw:commentRss>` links that feed-rs 3 adds to the entry, so
  an entry whose comments element precedes its link no longer opens the comments anchor.
  Feeds are now passed to the parser as raw bytes, so the XML-declared encoding is honoured
  and non-UTF-8 feeds served as `application/rss+xml` load again
- Dependency refresh: `html2text` 0.17, `ureq` 3 (with a shared, connection-pooled HTTP
  agent), `toml` 1, `dirs` 7, and `ratatui` 0.30.2. The minimum supported Rust version is
  now 1.96.1

### Fixed

- **Duplicate articles from republished entries**: feeds that republish articles with updated
  GUIDs (e.g. BBC News) produced duplicate titles; added a secondary dedup pass keyed on
  `(title, published_date)` so only the first-seen version of an identical story is kept
- **Paste in URL prompt**: pasting a URL into the `a`-prompt (add feed) sent each character
  individually; enabled terminal bracketed paste mode (`EnableBracketedPaste`) and handle
  `Event::Paste` to append the full pasted string to the input buffer at once
- **Hex entity decoding**: `&#xNN;` and `&#xNNNN;` numeric entities were silently dropped
  because the `#` prefix was not stripped before checking for the `x` / `X` hex marker;
  all such entities now decode correctly
- **`matches_key()` completeness**: `Tab`, `BackTab`, `Backspace`, `Delete`, `Insert`, and
  `Space` were not recognised as key names; they can now be used in key binding config
- **`light-gray` / `light-grey` color names**: mapped to `Color::Gray` (same as `gray`);
  corrected to map to `Color::White` (ANSI bright-white), matching terminal convention
- **Feed fetch timeout**: `fetch_feed()` had no timeout; it now shares the `FETCH_TIMEOUT`
  constant (30 s) with `probe_feed()`, preventing hung background fetch threads
- **Integer overflow in `purge_old_articles`**: `max_age_days as i64` could overflow for
  extreme `u64` values; now clamped to `i64::MAX` before casting
- **TOML escaping in `toml_string()`**: newlines, carriage returns, tabs, and other control
  characters in feed names were written unescaped, producing invalid TOML; they are now
  properly escaped as `\n`, `\r`, `\t`, and `\uXXXX`
- **Scroll clamp overflow**: `article_content_height as u16` truncated silently at 65 535
  for very long articles; clamped to `u16::MAX` in `usize` arithmetic before casting
- **Scrollbar arrows**: changed from `↑`/`↓` to `▲`/`▼` (filled triangles) for bolder
  visual indication of scroll direction
- Scrollbar thumb position: now correctly reaches the bottom of the track when content
  is fully scrolled in both the article list and article view panels
- **Debian packaging metadata**: `cargo-deb` referenced `LICENSE-MIT`, `config.toml`, and
  `feeds.toml`, none of which existed; the assets now point at the real files, both licence
  texts ship in the package, and the copyright field names the actual holder

### Security

- Updated `quick-xml` to 0.42, `rustls` to 0.23.46, `rustls-webpki` to 0.103.17,
  `webpki-roots` to 1.0.12, and `url` to 2.5.9, resolving 13 RustSec advisories —
  including two high-severity denial-of-service bugs reachable from untrusted feeds

## [0.1.0] - 2026-03-01

### Added

- Three-panel TUI layout (feed sidebar, article list, article view) inspired by Mutt
- RSS 1.0/2.0 and Atom feed support via `feed-rs`
- HTML article body rendering: bold, italic, inline code, links (`[text](url)`),
  headings (h1–h3), blockquotes, and bullet/numbered lists via `htmd`
- Persistent read/unread state and article cache stored in `~/.local/share/thrssty/cache.json`
- Background feed fetching on a worker thread; UI remains responsive during network I/O
- Auto-refresh of all feeds on startup (configurable via `refresh_on_start = false`)
- Manual feed refresh: `r` refreshes current feed, `R` refreshes all feeds
- Open article URL in system browser with `o` (uses `$BROWSER` / `xdg-open`)
- Global key bindings — no panel focus switching required:
  - `j`/`k` — navigate the feed sidebar
  - `↑`/`↓`, `Home`/`End` — navigate the article list
  - `PgUp`/`PgDn` — scroll the article view
  - `q` — quit
- Auto-selection of the first unread article when switching feeds
- Status bar showing key hints, active fetch progress, and error messages
- Helix-compatible TOML theme system with palette support; 17 named scopes
- Built-in dark theme (Catppuccin-inspired)
- Fully configurable layout proportions, key bindings, and theme via
  `~/.config/thrssty/config.toml`

