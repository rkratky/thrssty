/// Parsed command-line arguments for thrssty.
#[derive(Debug, Default)]
pub struct Args {
    /// Path to an alternative config file (`--config`).
    pub config: Option<String>,
    /// Path / name of a theme to use (`--theme`), overriding the config value.
    pub theme: Option<String>,
    /// Force feed refresh on startup (`--refresh`), overriding the config value.
    pub refresh: bool,
    /// Write default config / theme / feeds files and exit (`--init`).
    pub init: bool,
}

/// Parse `std::env::args()` and handle `--help` / `--version` inline
/// (they print to stdout and call `std::process::exit`).
pub fn parse() -> Args {
    let raw: Vec<String> = std::env::args().collect();
    let prog = raw.first().map(|s| s.as_str()).unwrap_or("thrssty");
    let mut args = Args::default();
    let mut i = 1;

    while i < raw.len() {
        match raw[i].as_str() {
            "-h" | "--help" => {
                print_help(prog);
                std::process::exit(0);
            }
            "-v" | "--version" => {
                println!("thrssty {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-r" | "--refresh" => {
                args.refresh = true;
            }
            "-i" | "--init" => {
                args.init = true;
            }
            "-c" | "--config" => {
                i += 1;
                args.config = raw.get(i).cloned().or_else(|| {
                    eprintln!("thrssty: --config requires a path argument");
                    std::process::exit(1);
                });
            }
            "-t" | "--theme" => {
                i += 1;
                args.theme = raw.get(i).cloned().or_else(|| {
                    eprintln!("thrssty: --theme requires a path or name argument");
                    std::process::exit(1);
                });
            }
            other => {
                eprintln!("thrssty: unknown option '{other}'");
                eprintln!("Run 'thrssty --help' for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    args
}

fn print_help(prog: &str) {
    use std::io::IsTerminal as _;
    let tty = std::io::stdout().is_terminal();

    // bold green     — section headers (USAGE, OPTIONS, …)
    let hdr   = |s: &str| -> String { if tty { format!("\x1b[1;32m{s}\x1b[0m")  } else { s.to_owned() } };
    // bold           — program name / version header
    let bold  = |s: &str| -> String { if tty { format!("\x1b[1m{s}\x1b[0m")     } else { s.to_owned() } };
    // bold cyan      — command name in USAGE, option flags (-h, --help)
    let cyan  = |s: &str| -> String { if tty { format!("\x1b[1;36m{s}\x1b[0m")  } else { s.to_owned() } };
    // plain cyan     — option value placeholders (<path>), paths in precedence lists
    let pcyan = |s: &str| -> String { if tty { format!("\x1b[36m{s}\x1b[0m")    } else { s.to_owned() } };

    let ver = format!("thrssty {}", env!("CARGO_PKG_VERSION"));
    println!("{}", bold(&ver));
    println!("TUI RSS newsreader\n");

    println!("{}", hdr("USAGE"));
    println!("    {} {}\n", cyan(prog), pcyan("[OPTIONS]"));

    println!("{}", hdr("OPTIONS"));
    // Each entry: (short, long, value placeholder, description)
    let opts: &[(&str, &str, &str, &str)] = &[
        ("-h", "--help",    "",           "Show this help message and exit"),
        ("-v", "--version", "",           "Show version and exit"),
        ("-r", "--refresh", "",           "Refresh all feeds on startup"),
        ("-c", "--config",  " <path>",    "Use the specified config file"),
        ("-t", "--theme",   " <path|name>","Use the specified theme (path or name)"),
    ];
    for (short, long, val, desc) in opts {
        println!("    {}, {}{}  {desc}", cyan(short), cyan(long), pcyan(val));
    }
    println!("    {}, {}  Write default config/theme/feeds files to {} and exit",
        cyan("-i"), cyan("--init"), pcyan("~/.config/thrssty/"));
    println!();

    println!("{}  (highest \u{2192} lowest)", hdr("CONFIG PRECEDENCE"));
    println!("    1. {} {}  (command line)", cyan("--config"), pcyan("<path>"));
    println!("    2. {}",  pcyan("~/.config/thrssty/config.toml"));
    println!("    3. {}  (current directory)", pcyan("./config.toml"));
    println!("    4. Built-in defaults");
    println!("    Settings are merged; higher-priority values override lower-priority ones.");
    println!();

    println!("{}  (highest \u{2192} lowest)", hdr("THEME PRECEDENCE"));
    println!("    1. {} {}  (command line)", cyan("--theme"), pcyan("<path|name>"));
    println!("    2. {} key in the effective config", pcyan("'theme'"));
    println!("    3. Built-in default theme");
}
