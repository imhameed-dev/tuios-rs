//! Compatibility tests: real programs (vim, less, htop, bash) running in a
//! pane, plus paste, Unicode, colors, resizing and cleanup behavior.
//!
//! Programs that are not installed are skipped locally; CI sets
//! `TUIOS_REQUIRE_TOOLS=1` so a missing program fails the test instead.

mod common;

use common::{need, number_after, process_alive, tmp_path, Session};
use std::thread;
use std::time::{Duration, Instant};

/// Run a marker command so a later `wait_for("mk_42")` can only match
/// output, never the typed command line.
fn mark(s: &mut Session) {
    s.send("echo mk_$((6*7))\r");
    s.wait_for("mk_42");
}

// ---- vim ----------------------------------------------------------------

#[test]
fn vim_edits_with_arrow_keys_saves_and_restores_the_screen() {
    if !need("vim") {
        return;
    }
    let file = tmp_path("vim_edit");
    let mut s = Session::start(24, 80);
    mark(&mut s);
    s.send(&format!("vim -u NONE -N -n {file}\r"));
    s.wait_for("~"); // empty-buffer filler lines
    s.send("iab");
    s.settle();
    s.send("\x1b[D"); // Left arrow, still in insert mode
    s.settle();
    s.send("X");
    s.settle();
    s.send("\x1b"); // Esc
    s.settle();
    s.send(":wq\r");
    // Back on the main screen: the earlier output must be there again.
    s.wait_until("shell screen restored after vim", |s| {
        s.text().contains("mk_42") && !s.text().contains('~')
    });
    s.send(&format!("echo got_$(cat {file})\r"));
    s.wait_for("got_aXb");
    let _ = std::fs::remove_file(&file);
}

#[test]
fn vim_scrolls_with_g_and_page_keys() {
    if !need("vim") {
        return;
    }
    let file = tmp_path("vim_scroll");
    let mut s = Session::start(24, 80);
    s.send(&format!("seq 1 200 > {file}\r"));
    mark(&mut s);
    s.send(&format!("vim -u NONE -N -n -R {file}\r"));
    s.wait_for_line("20");
    s.send("G");
    s.wait_for_line("200");
    s.send("gg");
    s.wait_for_line("1");
    s.send("\x1b[6~"); // Page Down
    s.wait_until("page down scrolled away from line 1", |s| !s.has_line("1"));
    s.send(":q\r");
    s.wait_until("shell screen restored", |s| s.text().contains("mk_42"));
    let _ = std::fs::remove_file(&file);
}

// ---- less ---------------------------------------------------------------

#[test]
fn less_pages_to_the_end_and_quits_cleanly() {
    if !need("less") {
        return;
    }
    let file = tmp_path("less");
    let mut s = Session::start(24, 80);
    s.send(&format!("seq 1 200 > {file}\r"));
    mark(&mut s);
    s.send(&format!("LESSHISTFILE=- less {file}\r"));
    s.wait_for_line("20");
    s.send(" "); // next page
    s.wait_until("second page", |s| s.has_line("40") && !s.has_line("1"));
    s.send("G");
    s.wait_for("(END)");
    s.send("q");
    s.wait_until("shell back after less", |s| {
        s.text().contains("mk_42") && !s.text().contains("(END)")
    });
    s.send("echo after_less_$((1+1))\r");
    s.wait_for("after_less_2");
    let _ = std::fs::remove_file(&file);
}

// ---- htop ---------------------------------------------------------------

#[test]
fn htop_draws_in_color_follows_resize_and_quits() {
    if !need("htop") {
        return;
    }
    let mut s = Session::start(24, 80);
    mark(&mut s);
    s.send("htop\r");
    s.wait_for("PID");
    assert!(
        s.any_cell_with_background(),
        "htop uses colored backgrounds; screen:\n{}",
        s.text()
    );
    // htop's function-key bar is the last row of its pane: row 22 of 24
    // (one row is our status line).
    s.wait_until("F10 bar on the last pane row", |s| {
        s.row_text(22).contains("F10")
    });
    s.resize(30, 100);
    s.wait_until("F10 bar moved to the new last pane row", |s| {
        s.row_text(28).contains("F10")
    });
    s.send("q");
    s.wait_until("shell back after htop", |s| !s.text().contains("PID"));
    s.send("echo after_htop_$((2+3))\r");
    s.wait_for("after_htop_5");
}

// ---- paste --------------------------------------------------------------

#[test]
fn paste_of_one_line_reaches_the_shell() {
    let mut s = Session::start(24, 80);
    s.send("\x1b[200~echo pasted_$((3+4))\x1b[201~");
    s.settle();
    s.send("\r");
    s.wait_for("pasted_7");
}

#[test]
fn paste_of_several_lines_runs_each_line() {
    let mut s = Session::start(24, 80);
    s.send("\x1b[200~echo m1_$((1+1))\necho m2_$((2+2))\n\x1b[201~");
    s.settle();
    s.send("\r"); // bracketed-paste-aware shells wait for Enter
    s.wait_for("m1_2");
    s.wait_for("m2_4");
}

#[test]
fn paste_into_a_bracketed_paste_shell_arrives_intact() {
    if !need("bash") {
        return;
    }
    let mut s = Session::start(24, 80);
    s.send("exec bash --norc --noprofile\r");
    s.settle();
    s.send("\x1b[200~echo bp_$((8+8))\x1b[201~");
    s.settle();
    s.send("\r");
    s.wait_for("bp_16");
}

// ---- Unicode and colors -------------------------------------------------

#[test]
fn unicode_text_is_drawn() {
    let mut s = Session::start(24, 80);
    // Euro sign, two wide CJK characters, and e + combining acute accent.
    s.send("printf 'u_\\342\\202\\254_\\346\\227\\245\\346\\234\\254_e\\314\\201_\\n'\r");
    s.wait_for("u_€_日本_e\u{301}_");
}

#[test]
fn colors_survive_the_trip() {
    let mut s = Session::start(24, 80);
    s.send("printf '\\033[31mRED\\033[0m \\033[48;2;1;2;3mBG\\033[0m\\n'\r");
    s.wait_until("colored output", |s| s.find("RED BG").is_some());
    let (row, col) = s.find("RED BG").unwrap();
    let (text, fg, _) = s.cell(row, col);
    assert_eq!(text, "R");
    assert_eq!(fg, vt100::Color::Idx(1), "red foreground");
    let (text, _, bg) = s.cell(row, col + 4);
    assert_eq!(text, "B");
    assert_eq!(bg, vt100::Color::Rgb(1, 2, 3), "24-bit background");
}

// ---- child exit, closing panes, cleanup ---------------------------------

#[test]
fn a_shell_that_exits_closes_only_its_pane() {
    let mut s = Session::start(24, 80);
    s.send("\x02c"); // Ctrl+B c: new pane
    s.wait_for("pane 2/2");
    s.send("exit\r");
    s.wait_for("pane 1/1");
    s.send("echo still_$((2+2))\r");
    s.wait_for("still_4");
    s.send("exit\r");
    assert!(s.exited_ok(), "the last pane exiting ends the app");
}

#[test]
fn a_killed_shell_closes_its_pane() {
    let mut s = Session::start(24, 80);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    s.send("kill -9 $$\r");
    s.wait_for("pane 1/1");
}

#[test]
fn quit_puts_the_outer_terminal_back() {
    let mut s = Session::start(24, 80);
    let (alt, _, paste) = s.outer_modes();
    assert!(alt, "uses the alternate screen while running");
    assert!(paste, "asks the terminal for bracketed paste while running");
    s.quit();
    assert!(s.exited_ok());
    // Let the last bytes reach our emulator.
    thread::sleep(Duration::from_millis(400));
    let (alt, hidden, paste) = s.outer_modes();
    assert!(!alt, "alternate screen left");
    assert!(!hidden, "cursor visible again");
    assert!(!paste, "bracketed paste turned off");
}

#[test]
fn quit_does_not_leave_processes_behind() {
    let mut s = Session::start(24, 80);
    s.send("echo SH_$$\r");
    s.send("sh -c 'echo SLP_$$; exec sleep 300'\r");
    s.wait_until("both pids printed", |s| {
        number_after(&s.text(), "SH_").is_some() && number_after(&s.text(), "SLP_").is_some()
    });
    let shell = number_after(&s.text(), "SH_").unwrap();
    let sleeper = number_after(&s.text(), "SLP_").unwrap();
    assert!(process_alive(shell) && process_alive(sleeper));
    s.quit();
    assert!(s.exited_ok());
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && (process_alive(shell) || process_alive(sleeper)) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!process_alive(shell), "shell {shell} still running");
    assert!(
        !process_alive(sleeper),
        "foreground program {sleeper} still running after quit"
    );
}

#[test]
fn closing_a_pane_stops_its_foreground_program() {
    let mut s = Session::start(24, 80);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    s.send("sh -c 'echo SLP_$$; exec sleep 300'\r");
    s.wait_until("pid printed", |s| number_after(&s.text(), "SLP_").is_some());
    let sleeper = number_after(&s.text(), "SLP_").unwrap();
    s.window_mode();
    s.send("x");
    s.wait_for("pane 1/1");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && process_alive(sleeper) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !process_alive(sleeper),
        "program {sleeper} outlived its pane"
    );
}

#[test]
fn many_panes_on_a_small_terminal_are_refused_politely_not_crashed() {
    // Regression: the emulator used to overflow on panes of 1 row or 1 column.
    let mut s = Session::start(24, 80);
    let mut opened = 1;
    for n in 2..=20 {
        s.send("\x02c");
        s.wait_until("a new pane or a refusal", |s| {
            s.text().contains(&format!("pane {n}/{n}")) || s.text().contains("no room")
        });
        if s.text().contains("no room") {
            break;
        }
        opened = n;
    }
    assert!(
        (6..20).contains(&opened),
        "expected a limit between 6 and 19 panes on 80x24, got {opened}"
    );
    // The focused pane is tiny, and a long shell prompt would scroll any
    // answer out of it. Check liveness in pane 1, the biggest one.
    s.window_mode();
    s.send("1");
    s.send("i");
    s.wait_for("TERMINAL");
    s.send("echo alive_$((7+7))\r");
    s.wait_for("alive_14");
    s.quit();
    assert!(s.exited_ok());
}

#[test]
fn shrinking_the_terminal_to_a_sliver_does_not_crash() {
    let mut s = Session::start(24, 80);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    for (rows, cols) in [(3, 5), (2, 6), (24, 80)] {
        s.resize(rows, cols);
        s.settle();
    }
    s.send("echo back_$((5+5))\r");
    s.wait_for("back_10");
}
