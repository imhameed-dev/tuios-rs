//! End-to-end tests: run the real binary inside a pseudo-terminal, type into
//! it, and read the screen back through a terminal emulator.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

const TIMEOUT: Duration = Duration::from_secs(20);

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    screen: Arc<Mutex<vt100::Parser>>,
}

impl Session {
    fn start(rows: u16, cols: u16) -> Session {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_tuios-rs"));
        cmd.env("TERM", "xterm-256color");
        cmd.env("SHELL", "/bin/sh");
        let child = pair.slave.spawn_command(cmd).expect("spawn tuios-rs");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("clone reader");
        let writer = pair.master.take_writer().expect("take writer");
        let screen = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        let sink = Arc::clone(&screen);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().process(&buf[..n]);
            }
        });
        let s = Session {
            master: pair.master,
            writer,
            child,
            screen,
        };
        // Typing before the app has switched the tty to raw mode would be
        // handled by the line discipline instead, so wait for the first frame.
        s.wait_for("TERMINAL");
        s
    }

    fn send(&mut self, s: &str) {
        self.writer.write_all(s.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    fn text(&self) -> String {
        self.screen.lock().unwrap().screen().contents()
    }

    fn wait_for(&self, needle: &str) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if self.text().contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {needle:?}; screen was:\n{}",
                self.text()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// True if the process exited successfully within the timeout.
    fn exited_ok(&mut self) -> bool {
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn version_flag_prints_version() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_tuios-rs"))
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
    s.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    s.screen.lock().unwrap().screen_mut().set_size(30, 100);
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
    s.send("\x02\x1b"); // Ctrl+B, Esc: Window mode
    s.wait_for("WINDOW");
    s.send("n"); // new pane
    s.wait_for("pane 2/2");
    s.wait_for("│"); // separator between the two panes
    s.send("i"); // back to Terminal mode, second pane focused
    s.wait_for("TERMINAL");
    s.send("echo second_$((1+1))\r");
    s.wait_for("second_2");
    s.send("\x02\x1b");
    s.wait_for("WINDOW");
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
