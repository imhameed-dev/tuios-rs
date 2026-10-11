//! End-to-end tests of the multiplexer itself: run the real binary inside a
//! pseudo-terminal, type into it, and read the screen back.

mod common;

use std::thread;
use std::time::{Duration, Instant};

use common::{bin_path, Session, TIMEOUT};

#[test]
fn version_flag_prints_version() {
    let out = std::process::Command::new(bin_path())
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("tuios-rs "), "{text:?}");
}

#[test]
fn shell_runs_a_command_and_exit_closes_the_app() {
    let mut s = Session::start(24, 80);
    // The typed text contains `$((20+22))`; only a shell that really ran the
    // command prints `tuios_42`.
    s.send("echo tuios_$((20+22))\r");
    s.wait_for("tuios_42");
    s.send("exit\r");
    assert!(s.exited_ok(), "app should exit when its last shell exits");
}

#[test]
fn terminal_resize_reaches_the_shell() {
    let mut s = Session::start(24, 80);
    s.resize(30, 100);
    // One row is the status line, so the single pane is 29 x 100. Ask
    // repeatedly: the resize signal races with the first command.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        s.send("stty size\r");
        thread::sleep(Duration::from_millis(300));
        if s.text().contains("29 100") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "shell never saw 29 100; screen:\n{}",
            s.text()
        );
    }
}

#[test]
fn panes_modes_and_quit() {
    let mut s = Session::start(24, 80);
    s.window_mode();
    s.send("n"); // new pane
    s.wait_for("pane 2/2");
    s.wait_for("│"); // separator between the two panes
    s.send("i"); // back to Terminal mode, second pane focused
    s.wait_for("TERMINAL");
    s.send("echo second_$((1+1))\r");
    s.wait_for("second_2");
    s.window_mode();
    s.send("x"); // close the focused pane
    s.wait_for("pane 1/1");
    s.send("q");
    assert!(s.exited_ok(), "q in Window mode should quit");
}

#[test]
fn workspaces_switch_and_stay_isolated() {
    let mut s = Session::start(24, 80);
    s.send("\x022"); // Ctrl+B 2: workspace 2 (empty)
    s.wait_for("ws 2/9");
    s.wait_for("pane 0/0");
    s.wait_for("WINDOW"); // empty workspace falls back to Window mode
    s.send("n");
    s.wait_for("pane 1/1");
    s.send("\x1b1"); // Alt+1: back to workspace 1
    s.wait_for("ws 1/9");
    s.send("q");
    assert!(s.exited_ok());
}
