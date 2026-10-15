//! `tuios-rs --check-keys`: a short guided check of what *this* terminal
//! sends for Ctrl+B and Esc. It uses the same key conversion and the same
//! mode logic as the multiplexer, so its verdict is what the real program
//! would do in this terminal.

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::input::Key;
use crate::modal::{Modal, Mode};
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

/// Run the check. Returns true when Ctrl+B then Esc enters Window mode.
pub fn run() -> io::Result<bool> {
    let _raw = RawMode::enter()?;
    let wait = wait_time();
    let secs = wait.as_secs().max(1);
    let mut modal = Modal::new(Key::ctrl('b'));
    let (tx, events) = mpsc::channel();
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(ev).is_err() {
                break;
            }
        }
    });

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
    modal.handle(key);
    if !modal.prefix_armed() {
        say("  RESULT: FAIL - TUIOS-RS did not recognise that as the Ctrl+B prefix key.");
        return Ok(false);
    }
    say("  OK: prefix key recognised.");
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
    let outcome = modal.handle(key);
    say(&format!("  TUIOS-RS decides: {outcome:?}"));
    let entered = modal.mode() == Mode::Window;
    if entered {
        say("");
        say("RESULT: PASS - Ctrl+B then Esc enters Window mode in this terminal.");
    } else {
        say("");
        say("RESULT: FAIL - Esc arrived but did not enter Window mode (see lines above).");
    }
    Ok(entered)
}
