/// Terminal background-color detection.
///
/// The detection strategy is:
/// 1. Check `$TERM_BACKGROUND` — set by Kitty and some other emulators to
///    `"dark"` or `"light"`.
/// 2. Check `$COLORFGBG` — set by rxvt/xterm family; the last `;`-delimited
///    component is a terminal-color index where < 8 means a dark background.
/// 3. Probe via the OSC 11 escape sequence (query terminal background color).
///    The probe is run before entering TUI raw mode and has a 200 ms timeout
///    so it is invisible to the user on supported terminals.
/// 4. Fall back to `Dark` if none of the above succeeds.
pub enum Background {
    Dark,
    Light,
}

/// Detect whether the terminal uses a dark or light background.
///
/// Returns `Background::Dark` on any ambiguity or failure.
pub fn detect() -> Background {
    if let Some(bg) = from_env() {
        return bg;
    }
    if let Some(bg) = probe_osc11() {
        return bg;
    }
    Background::Dark
}

// ── Environment variable checks ───────────────────────────────────────────────

fn from_env() -> Option<Background> {
    // Kitty (and some others) expose this directly.
    if let Ok(val) = std::env::var("TERM_BACKGROUND") {
        return match val.to_lowercase().as_str() {
            "light" => Some(Background::Light),
            "dark" => Some(Background::Dark),
            _ => None,
        };
    }

    // rxvt / some xterms: "fg;bg" where the bg index < 8 means dark.
    if let Ok(val) = std::env::var("COLORFGBG") {
        if let Some(bg_str) = val.split(';').next_back() {
            if let Ok(n) = bg_str.trim().parse::<u8>() {
                return Some(if n < 8 {
                    Background::Dark
                } else {
                    Background::Light
                });
            }
        }
    }

    None
}

// ── OSC 11 probe ──────────────────────────────────────────────────────────────

/// Try to query the terminal's background color via the OSC 11 escape sequence.
///
/// Requires a real TTY and a terminal that responds to OSC 11. If the terminal
/// does not respond within `TIMEOUT_MS`, the function returns `None`.
fn probe_osc11() -> Option<Background> {
    use std::io::Write as _;
    use std::sync::mpsc;
    use std::time::Duration;

    const TIMEOUT_MS: u64 = 200;

    // Only probe when we have a real interactive terminal.
    if std::env::var("TERM").is_err() {
        return None;
    }

    crossterm::terminal::enable_raw_mode().ok()?;

    // OSC 11 query: "\e]11;?\e\\" (OSC 11, query character, ST terminator).
    let query_ok = std::io::stdout()
        .write_all(b"\x1b]11;?\x1b\\")
        .and_then(|_| std::io::stdout().flush())
        .is_ok();

    if !query_ok {
        let _ = crossterm::terminal::disable_raw_mode();
        return None;
    }

    // Read the response in a background thread so we can time out cleanly.
    // The OSC 11 response arrives as raw bytes on stdin, not as a crossterm
    // Event, so we cannot use crossterm::event::poll() here.
    //
    // The thread holds stdin.lock() while reading. On timeout the lock is
    // released when the thread eventually exits (raw mode is off, so the next
    // stdin.read() will return 0 bytes or an error immediately on most
    // terminals). The thread handle is dropped here, but the thread itself is
    // short-lived by design: raw mode is disabled below and the terminal
    // stops buffering, so the in-progress read returns quickly.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        use std::io::Read as _;
        let mut buf = [0u8; 1];
        let mut resp = Vec::with_capacity(32);
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        while let Ok(1) = stdin.read(&mut buf) {
            resp.push(buf[0]);
            // OSC response ends with BEL (0x07) or ST (ESC \\).
            let n = resp.len();
            let done = buf[0] == 0x07 || (n >= 2 && resp[n - 2] == 0x1b && resp[n - 1] == 0x5c);
            if done || n > 64 {
                break;
            }
        }
        let _ = tx.send(resp);
    });

    let resp = rx.recv_timeout(Duration::from_millis(TIMEOUT_MS));
    // Disable raw mode before returning so the thread's next read returns
    // immediately, releasing stdin.lock().
    let _ = crossterm::terminal::disable_raw_mode();

    let bytes = resp.ok()?;
    let text = String::from_utf8_lossy(&bytes);
    parse_osc11_background(&text)
}

/// Parse an OSC 11 response into a `Background` variant.
///
/// Expected format: `\e]11;rgb:RRRR/GGGG/BBBB\e\\` or `\e]11;rgb:RR/GG/BB\x07`.
fn parse_osc11_background(resp: &str) -> Option<Background> {
    let rgb_pos = resp.find("rgb:")?;
    let after = &resp[rgb_pos + 4..];
    let parts: Vec<&str> = after.splitn(4, '/').collect();
    if parts.len() < 3 {
        return None;
    }
    // Take the two most-significant hex digits of each channel.
    let r = u8::from_str_radix(parts[0].get(..2)?, 16).ok()?;
    let g = u8::from_str_radix(parts[1].get(..2)?, 16).ok()?;
    let b_str = parts[2].split(|c: char| !c.is_ascii_hexdigit()).next()?;
    let b = u8::from_str_radix(b_str.get(..2)?, 16).ok()?;

    // Standard relative luminance approximation.
    let lum = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    Some(if lum < 128.0 {
        Background::Dark
    } else {
        Background::Light
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc11_dark_background_parsed() {
        // Simulated OSC 11 response for a very dark background (#0a0a0a).
        let resp = "\x1b]11;rgb:0a0a/0a0a/0a0a\x07";
        assert!(matches!(
            parse_osc11_background(resp),
            Some(Background::Dark)
        ));
    }

    #[test]
    fn osc11_light_background_parsed() {
        // Simulated OSC 11 response for white (#fafafa).
        let resp = "\x1b]11;rgb:fafa/fafa/fafa\x07";
        assert!(matches!(
            parse_osc11_background(resp),
            Some(Background::Light)
        ));
    }

    #[test]
    fn osc11_malformed_returns_none() {
        assert!(parse_osc11_background("garbage").is_none());
        assert!(parse_osc11_background("rgb:gg/gg/gg").is_none());
    }
}
