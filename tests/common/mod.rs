//! Helpers shared by the integration tests: run the real binary inside a
//! pseudo-terminal and read its screen back through a terminal emulator.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub const TIMEOUT: Duration = Duration::from_secs(25);

/// The binary under test. `TUIOS_BIN` overrides it so the same test
/// executables can run against a released binary on another machine.
pub fn bin_path() -> String {
    std::env::var("TUIOS_BIN").unwrap_or_else(|_| env!("CARGO_BIN_EXE_tuios-rs").to_string())
}

/// Whether `tool` is installed. With `TUIOS_REQUIRE_TOOLS` set (as in CI) a
/// missing tool fails the test instead of silently skipping it.
pub fn need(tool: &str) -> bool {
    let found = std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()));
    if !found {
        assert!(
            std::env::var_os("TUIOS_REQUIRE_TOOLS").is_none(),
            "{tool} is required but not installed"
        );
        eprintln!("SKIPPED: {tool} is not installed");
    }
    found
}

pub fn tmp_path(name: &str) -> String {
    format!(
        "{}/tuios_{name}_{}",
        std::env::temp_dir().display(),
        std::process::id()
    )
}

/// True while `pid` exists and is not a zombie.
pub fn process_alive(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => false,
        Ok(s) => s
            .rsplit(") ")
            .next()
            .and_then(|rest| rest.chars().next())
            .is_some_and(|state| state != 'Z' && state != 'X'),
    }
}

/// The digits right after `prefix` on any line of `text`.
pub fn number_after(text: &str, prefix: &str) -> Option<u32> {
    for line in text.lines() {
        if let Some(i) = line.find(prefix) {
            let digits: String = line[i + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = digits.parse() {
                return Some(n);
            }
        }
    }
    None
}

pub struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    screen: Arc<Mutex<vt100::Parser>>,
    pub rows: u16,
    pub cols: u16,
}

impl Session {
    pub fn start(rows: u16, cols: u16) -> Session {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let mut cmd = CommandBuilder::new(bin_path());
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
            rows,
            cols,
        };
        // Typing before the app has switched the tty to raw mode would be
        // handled by the line discipline instead, so wait for the first frame.
        s.wait_for("TERMINAL");
        s
    }

    pub fn pid(&self) -> u32 {
        self.child.process_id().expect("child pid")
    }

    pub fn send(&mut self, s: &str) {
        self.writer.write_all(s.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    /// Pause so the app and the program in the pane can react. Used where a
    /// lone Esc must not be read as the start of an escape sequence.
    pub fn settle(&self) {
        thread::sleep(Duration::from_millis(250));
    }

    pub fn text(&self) -> String {
        self.screen.lock().unwrap().screen().contents()
    }

    pub fn lines(&self) -> Vec<String> {
        self.text().lines().map(|l| l.trim().to_string()).collect()
    }

    pub fn has_line(&self, line: &str) -> bool {
        self.lines().iter().any(|l| l == line)
    }

    /// One screen row, one character per column (blank cells as spaces).
    pub fn row_text(&self, row: u16) -> String {
        let g = self.screen.lock().unwrap();
        (0..self.cols)
            .map(|c| {
                g.screen()
                    .cell(row, c)
                    .and_then(|x| x.contents().chars().next())
                    .unwrap_or(' ')
            })
            .collect()
    }

    /// Row and column of the first occurrence of an ASCII `needle`.
    pub fn find(&self, needle: &str) -> Option<(u16, u16)> {
        (0..self.rows).find_map(|r| {
            self.row_text(r)
                .find(needle)
                .and_then(|c| u16::try_from(c).ok())
                .map(|c| (r, c))
        })
    }

    pub fn cell(&self, row: u16, col: u16) -> (String, vt100::Color, vt100::Color) {
        let g = self.screen.lock().unwrap();
        let c = g.screen().cell(row, col).expect("cell in range");
        (c.contents().to_string(), c.fgcolor(), c.bgcolor())
    }

    pub fn any_cell_with_background(&self) -> bool {
        let g = self.screen.lock().unwrap();
        (0..self.rows).any(|r| {
            (0..self.cols).any(|c| {
                g.screen()
                    .cell(r, c)
                    .is_some_and(|x| x.bgcolor() != vt100::Color::Default)
            })
        })
    }

    /// Cursor position as (row, column) on the outer screen. In Terminal mode
    /// it sits inside the focused pane, so it shows which pane has focus.
    pub fn cursor(&self) -> (u16, u16) {
        self.screen.lock().unwrap().screen().cursor_position()
    }

    /// (alternate screen on, cursor hidden, bracketed paste on) as the
    /// outer terminal currently sees them.
    pub fn outer_modes(&self) -> (bool, bool, bool) {
        let g = self.screen.lock().unwrap();
        let s = g.screen();
        (s.alternate_screen(), s.hide_cursor(), s.bracketed_paste())
    }

    pub fn wait_until(&self, what: &str, cond: impl Fn(&Session) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if cond(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; screen was:\n{}",
                self.text()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn wait_for(&self, needle: &str) {
        self.wait_until(&format!("text {needle:?}"), |s| s.text().contains(needle));
    }

    pub fn wait_for_line(&self, line: &str) {
        self.wait_until(&format!("line {line:?}"), |s| s.has_line(line));
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        self.screen
            .lock()
            .unwrap()
            .screen_mut()
            .set_size(rows, cols);
        self.rows = rows;
        self.cols = cols;
    }

    /// True if the process exited successfully within the timeout.
    pub fn exited_ok(&mut self) -> bool {
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Switch to Window mode (Ctrl+B, Esc).
    pub fn window_mode(&mut self) {
        self.send("\x02\x1b");
        self.wait_for("WINDOW");
    }

    /// Leave via Window mode `q`.
    pub fn quit(&mut self) {
        self.window_mode();
        self.send("q");
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
