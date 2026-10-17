//! Resizing and leaving, end to end: the real binary in a pseudo-terminal.
//! Covers exit by quit, by signal, by panic and by a failed shell start
//! (terminal put back, programs stopped), and resizing down to a sliver and
//! back up (every pane's PTY gets the new size and the screen is redrawn).

mod common;

use std::thread;
use std::time::{Duration, Instant};

use common::{bin_path, number_after, process_alive, Session};

const SIGNALS: [&str; 3] = ["TERM", "HUP", "INT"];

/// The program has ended with `exit` and left the terminal as it found it.
fn assert_restored(s: &Session, exit: &str) {
    s.wait_for("AFTER_DONE");
    thread::sleep(Duration::from_millis(300));
    let t = s.text();
    assert!(t.contains(exit), "expected {exit}:\n{t}");
    assert!(
        t.contains("TTY: isig icanon echo"),
        "terminal left in raw mode:\n{t}"
    );
    let (alt, hidden, paste) = s.outer_modes();
    assert!(!alt, "alternate screen not left:\n{t}");
    assert!(!hidden, "cursor left hidden:\n{t}");
    assert!(!paste, "bracketed paste left on:\n{t}");
}

fn wait_gone(pids: &[u32], what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && pids.iter().any(|p| process_alive(*p)) {
        thread::sleep(Duration::from_millis(50));
    }
    for p in pids {
        assert!(!process_alive(*p), "{what}: process {p} still running");
    }
}

/// A background job and a foreground program started in the pane.
fn start_jobs(s: &mut Session, tag: &str) -> (u32, u32) {
    s.send(&format!("sleep 3{tag}1 & echo BG_$!\r"));
    s.send(&format!("sh -c 'echo FG_$$; exec sleep 3{tag}2'\r"));
    s.wait_until("job pids printed", |s| {
        number_after(&s.text(), "BG_").is_some() && number_after(&s.text(), "FG_").is_some()
    });
    let bg = number_after(&s.text(), "BG_").unwrap();
    let fg = number_after(&s.text(), "FG_").unwrap();
    assert!(process_alive(bg) && process_alive(fg));
    (bg, fg)
}

#[test]
fn normal_quit_puts_the_terminal_back_including_raw_mode() {
    let mut s = Session::start_wrapped(24, 80, &[], "TERMINAL");
    s.quit();
    assert_restored(&s, "EXIT_CODE_0");
}

#[test]
fn a_signal_quits_cleanly_and_stops_the_shell_and_its_jobs() {
    for (i, sig) in SIGNALS.into_iter().enumerate() {
        let mut s = Session::start_wrapped(24, 80, &[], "TERMINAL");
        let (bg, fg) = start_jobs(&mut s, &format!("0{i}"));
        s.signal_app(sig);
        assert_restored(&s, "EXIT_CODE_0");
        wait_gone(&[bg, fg], &format!("after SIG{sig}"));
    }
}

#[test]
fn quitting_stops_background_jobs_too() {
    let mut s = Session::start_wrapped(24, 80, &[], "TERMINAL");
    let (bg, fg) = start_jobs(&mut s, "10");
    s.quit();
    assert_restored(&s, "EXIT_CODE_0");
    wait_gone(&[bg, fg], "after quit");
}

#[test]
fn closing_a_pane_stops_its_background_jobs_and_keeps_the_app() {
    let mut s = Session::start(24, 80);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    let (bg, fg) = start_jobs(&mut s, "20");
    s.send("\x02x");
    s.wait_for("pane 1/1");
    wait_gone(&[bg, fg], "after closing the pane");
    s.send("echo alive_$((20+22))\r");
    s.wait_for("alive_42");
}

#[test]
fn a_panic_puts_the_terminal_back_and_the_message_stays_visible() {
    let s = Session::start_wrapped(24, 80, &[("TUIOS_TEST_PANIC", "1")], "deliberate panic");
    assert_restored(&s, "EXIT_CODE_101");
    assert!(
        s.text().contains("deliberate panic"),
        "panic message must be readable on the normal screen:\n{}",
        s.text()
    );
}

#[test]
fn a_shell_that_cannot_start_shows_a_notice_and_quits_cleanly() {
    let mut s = Session::start_wrapped(24, 80, &[("SHELL", "/nonexistent/shell")], "cannot start");
    s.wait_for("WINDOW");
    s.send("q");
    assert_restored(&s, "EXIT_CODE_0");
}

#[test]
fn without_a_terminal_it_fails_with_a_message_and_no_escape_codes() {
    let out = std::process::Command::new(bin_path())
        .stdin(std::process::Stdio::null())
        .env("TERM", "xterm")
        .output()
        .expect("run the binary");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("tuios-rs:"), "{stderr}");
    assert!(
        !out.stdout.contains(&0x1b) && !out.stderr.contains(&0x1b),
        "no terminal control codes may be written when there is no terminal"
    );
}

// ---- resizing -------------------------------------------------------------

/// The (rows, cols) the shell in the focused pane reports, measured with a
/// fresh tag so older output on the screen is never mistaken for it.
fn pane_size(s: &mut Session, tag: &str) -> Option<(u16, u16)> {
    s.send(&format!("echo {tag}_$(stty size | tr ' ' x)_END\r"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
        for line in s.text().lines() {
            let mut rest = line;
            while let Some(i) = rest.find(&format!("{tag}_")) {
                rest = &rest[i + tag.len() + 1..];
                let rows: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let after = &rest[rows.len()..];
                if let Some(cols) = after.strip_prefix('x') {
                    let cols: String = cols.chars().take_while(char::is_ascii_digit).collect();
                    if let (Ok(r), Ok(c)) = (rows.parse(), cols.parse()) {
                        return Some((r, c));
                    }
                }
            }
        }
    }
    None
}

/// Both panes' sizes, left then right, once they match `rows`/`cols` of the
/// whole terminal (the resize arrives asynchronously, so ask until it does).
fn both_sizes(s: &mut Session, round: u32, rows: u16, cols: u16) -> ((u16, u16), (u16, u16)) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut n = 0;
    loop {
        n += 1;
        s.send("\x02\t"); // focus the left pane
        s.wait_for("pane 1/2");
        let left = pane_size(s, &format!("SZL{round}x{n}"));
        s.send("\x02\t"); // and the right one
        s.wait_for("pane 2/2");
        let right = pane_size(s, &format!("SZR{round}x{n}"));
        if let (Some(l), Some(r)) = (left, right) {
            if l.0 == rows - 1 && l.1 + 1 + r.1 == cols {
                return (l, r);
            }
        }
        assert!(
            Instant::now() < deadline,
            "panes never matched {rows}x{cols}; last: {left:?} {right:?}\n{}",
            s.text()
        );
    }
}

#[test]
fn two_panes_follow_a_shrink_to_a_sliver_and_growing_back() {
    let mut s = Session::start(24, 80);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    let (l, r) = both_sizes(&mut s, 0, 24, 80);
    assert_eq!(l.0, 23);
    assert_eq!(r.0, 23);

    // Bigger, then a sliver, then bigger than the start.
    s.resize(40, 120);
    let (l, r) = both_sizes(&mut s, 1, 40, 120);
    assert_eq!((l.0, r.0), (39, 39));

    s.resize(3, 7);
    s.settle();
    s.resize(30, 100);
    let (l, r) = both_sizes(&mut s, 2, 30, 100);
    assert_eq!((l.0, r.0), (29, 29), "each shell got the new height");

    // The screen was redrawn for the new size: separator in the right
    // column, status bar on the last row.
    let sep = s.row_text(0).chars().nth(usize::from(l.1)).unwrap();
    assert_eq!(sep, '│', "separator after the left pane:\n{}", s.text());
    assert!(
        s.row_text(29).contains("TERMINAL") && s.row_text(29).contains("pane 2/2"),
        "status bar on the last row:\n{}",
        s.text()
    );
    s.send("echo still_ok_$((20+22))\r");
    s.wait_for("still_ok_42");
}

#[test]
fn a_resize_right_after_launch_is_never_lost() {
    // The resize signal used to be missed if it arrived before the input
    // thread had started listening. Do it right after the first frame, many
    // times: every run must end up knowing the new size.
    for round in 0..15 {
        let mut s = Session::start(24, 80);
        s.resize(30, 100);
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut n = 0;
        loop {
            n += 1;
            if pane_size(&mut s, &format!("RSZ{round}x{n}")) == Some((29, 100)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "round {round}: the program never learned the new size:\n{}",
                s.text()
            );
        }
    }
}
