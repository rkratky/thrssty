use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use thrssty::{
    app::App, config::Config, error::Result, feeds_config::FeedsFile, state::Cache, theme::Theme,
};

mod cli;

fn main() {
    if let Err(e) = run() {
        eprintln!("thrssty error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = cli::parse();

    // --init: write default files and exit.
    if args.init {
        println!("Initialising thrssty config files…");
        Config::init_files()?;
        println!("Done.");
        return Ok(());
    }

    // Build the config cascade (lowest priority first):
    //   4. built-in defaults           (serde field defaults — no file needed)
    //   3. ./config.toml               (current working directory)
    //   2. ~/.config/thrssty/config.toml
    //   1. --config <path>             (command line)
    let xdg_config = Config::config_path().ok();
    let cwd_config = Path::new("config.toml")
        .exists()
        .then(|| Path::new("config.toml").to_path_buf());
    let cli_config = args.config.as_deref().map(Path::new);

    let config = Config::load_cascade(&[cwd_config.as_deref(), xdg_config.as_deref(), cli_config]);

    // --theme overrides the config's theme field.
    // --refresh overrides the config's refresh_on_start field.
    let theme_source = args.theme.as_deref().unwrap_or(&config.theme);
    let refresh_on_start = args.refresh || config.refresh_on_start;

    // Resolve theme (supports name, path, inherits).
    let themes_dir = Config::themes_dir().ok();
    let theme = Theme::load(theme_source, themes_dir.as_deref());

    let cache = Cache::load()?;

    // feeds_file_path: resolve relative to the CLI config file's directory
    // so that a self-contained config bundle works correctly.
    let feeds_path = if let Some(cli_path) = args.config.as_deref() {
        let base = Path::new(cli_path).parent();
        config.feeds_file_path_relative_to(base)?
    } else {
        config.feeds_file_path()?
    };

    let feeds_file = FeedsFile::load(&feeds_path)?;
    let (feed_configs, sidebar) = feeds_file.into_layout(&config.uncategorized_label);

    let mut app = App::new(config, theme, cache, feed_configs, sidebar);

    if refresh_on_start {
        app.refresh_all_feeds();
    }

    let mut terminal = setup_terminal()?;

    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;

    let _ = app.on_exit();

    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| thrssty::ui::draw(f, app))?;
        app.rerender_if_width_changed();

        // Drain all queued events before re-rendering. This ensures that
        // rapid sequences (e.g. paste arriving as multiple Char events on
        // terminals that do not honour bracketed paste) are all processed in
        // one frame rather than one character per 100 ms tick.
        let mut should_quit = false;
        if event::poll(Duration::from_millis(100))? {
            while event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press && !app.handle_key(key) {
                            should_quit = true;
                            break;
                        }
                    }
                    Event::Paste(s) => {
                        app.handle_paste(&s);
                    }
                    _ => {}
                }
            }
        }
        if should_quit {
            break;
        }

        app.tick();
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;
    Ok(())
}
