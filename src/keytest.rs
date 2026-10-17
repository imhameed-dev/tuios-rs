//! `tuios-rs --check-keys`: a short guided check of what *this* terminal
//! sends for Ctrl+B and Esc. The keys it receives are fed to the real
//! multiplexer (`App`, with a real pane), and the verdict is read from the
//! app's own mode and status bar, so it tests the code path the program
//! really uses.

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::app::App;
use crate::event::{AppEvent, InputEvent};
use crate::input::Key;
use crate::modal::Mode;
use crate::tty::convert_key;

/// Environment variable that shortens the wait for each key (milliseconds).
pub const TIMEOUT_ENV: &str = "TUIOS_KEYTEST_TIMEOUT_MS";

struct RawMode;

impl RawMode {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn say(text: &str) {
    let mut out = io::stdout().lock();
    // Raw mode does not turn \n into a new line by itself.
    let _ = write!(out, "{text}\r\n");
    let _ = out.flush();
}

fn wait_time() -> Duration {
    let ms = std::env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(20_000);
    Duration::from_millis(ms)
}

/// The next key press, or `None` if nothing arrives in time. Events are
/// read on a helper thread: for a byte sequence the input library cannot
/// decode, `poll` can say "ready" and `read` then blocks, which would
/// defeat the timeout.
fn next_key(events: &Receiver<Event>, wait: Duration) -> Option<(KeyEvent, Key)> {
    let end = Instant::now() + wait;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        match events.recv_timeout(left) {
            Ok(Event::Key(k)) => {
                if let Some(key) = convert_key(k) {
                    return Some((k, key));
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

/// Give the real app a key, redraw, and return the status bar text.
fn feed(app: &mut App, key: Key) -> String {
    app.handle(AppEvent::Input(InputEvent::Key(key)));
    app.render();
    app.status_text().unwrap_or_default().trim_end().to_string()
}

/// Run the check. Returns true when Ctrl+B then Esc really moves the app
/// from Terminal mode to Window mode (and its status bar says so).
pub fn run() -> io::Result<bool> {
    let _raw = RawMode::enter()?;
    let wait = wait_time();
    let secs = wait.as_secs().max(1);
    let (tx, events) = mpsc::channel();
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(ev).is_err() {
                break;
            }
        }
    });

    // The real application, with one real shell pane.
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());
    let (app_tx, _app_rx) = mpsc::channel();
    let mut app = App::new(cols, rows, shell, app_tx);
    app.open_pane();
    app.render();

    say("TUIOS-RS key check: two key presses, then it tells you the result.");
    say("");
    say(&format!(
        "Step 1: hold Ctrl and tap B. (waiting up to {secs} seconds)"
    ));
    let Some((event, key)) = next_key(&events, wait) else {
        say("  Nothing arrived. RESULT: FAIL - this terminal sent nothing for Ctrl+B.");
        return Ok(false);
    };
    say(&format!("  terminal sent: {event:?}"));
    say(&format!("  TUIOS-RS reads it as: {key:?}"));
    let status = feed(&mut app, key);
    say("  status bar now:");
    say(&format!("    {status}"));
    if !status.contains("PREFIX") {
        say("  RESULT: FAIL - TUIOS-RS did not recognise that as the Ctrl+B prefix key.");
        return Ok(false);
    }
    say("  OK: prefix key recognised (the status bar says PREFIX).");
    say("");
    say(&format!(
        "Step 2: now tap Esc (just Esc). (waiting up to {secs} seconds)"
    ));
    let Some((event, key)) = next_key(&events, wait) else {
        say("  Nothing arrived. RESULT: FAIL - the program never received the Esc key.");
        say("  (Something outside TUIOS-RS, such as the terminal or window manager, kept it,");
        say("   or the terminal sent an encoding the input library cannot read.)");
        return Ok(false);
    };
    say(&format!("  terminal sent: {event:?}"));
    say(&format!("  TUIOS-RS reads it as: {key:?}"));
    let before = app.mode();
    let status = feed(&mut app, key);
    let after = app.mode();
    // Not a command to run: a mode change is reported as "nothing for the
    // caller to do" inside the key handler, so judge by the mode itself.
    say(&format!("  mode: {before:?} -> {after:?}"));
    say("  status bar now:");
    say(&format!("    {status}"));
    let entered = before == Mode::Terminal && after == Mode::Window && status.contains("WINDOW");
    say("");
    if entered {
        say("RESULT: PASS - Ctrl+B then Esc moved the program from Terminal mode to Window mode.");
    } else {
        say("RESULT: FAIL - Esc arrived but the program did not enter Window mode (see above).");
    }
    Ok(entered)
}
