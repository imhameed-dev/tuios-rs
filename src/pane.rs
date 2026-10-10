//! One pane: a shell running in a PTY plus the terminal emulator state
//! (`vt100`) that tracks what it has drawn.

use std::io::{self, Read, Write};
use std::sync::mpsc::Sender;
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::event::AppEvent;
use crate::layout::WindowId;

/// Lines of scrollback kept per pane.
const SCROLLBACK: usize = 1000;

fn pty_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub struct Pane {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    parser: vt100::Parser,
    size: (u16, u16),
}

impl Pane {
    /// Start `shell` in a new PTY of `rows` x `cols`. A reader thread sends
    /// the process output, then an exit notice, to `tx`.
    pub fn spawn(
        id: WindowId,
        rows: u16,
        cols: u16,
        shell: &str,
        tx: Sender<AppEvent>,
    ) -> io::Result<Pane> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let pair = native_pty_system()
            .openpty(pty_size(rows, cols))
            .map_err(pty_err)?;
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        if let Ok(dir) = std::env::current_dir() {
            cmd.cwd(dir);
        }
        let child = pair.slave.spawn_command(cmd).map_err(pty_err)?;
        // The child holds the slave end; keeping ours open would stop the
        // reader from ever seeing end-of-file.
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(pty_err)?;
        let writer = pair.master.take_writer().map_err(pty_err)?;
        thread::Builder::new()
            .name(format!("pane-{id}-reader"))
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(AppEvent::Output(id, buf[..n].to_vec())).is_err() {
                                return;
                            }
                        }
                    }
                }
                let _ = tx.send(AppEvent::Exit(id));
            })?;
        Ok(Pane {
            master: pair.master,
            writer,
            child,
            parser: vt100::Parser::new(rows, cols, SCROLLBACK),
            size: (rows, cols),
        })
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn write(&mut self, bytes: &[u8]) {
        // A failed write means the shell is gone; the reader thread will
        // report that as an exit, so there is nothing to do here.
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        if (rows, cols) == self.size {
            return;
        }
        let _ = self.master.resize(pty_size(rows, cols));
        self.parser.screen_mut().set_size(rows, cols);
        self.size = (rows, cols);
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn app_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    pub fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
