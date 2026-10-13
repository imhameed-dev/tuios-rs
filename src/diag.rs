//! Opt-in key log for diagnosing "my key does nothing" reports.
//!
//! Off unless `TUIOS_KEYLOG=<file>` is set. It records what the multiplexer
//! received and decided, one line per key. It never records text that is
//! forwarded to a shell, so passwords typed into a pane do not end up in the
//! file: forwarded keys are written as `(hidden)`.

use std::fs::{File, OpenOptions};
use std::io::Write;

/// Environment variable naming the log file.
pub const ENV: &str = "TUIOS_KEYLOG";

pub struct KeyLog {
    file: File,
    seq: u64,
}

impl KeyLog {
    /// Open the log named by `TUIOS_KEYLOG`, if set and writable.
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os(ENV).filter(|p| !p.is_empty())?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let mut log = Self { file, seq: 0 };
        log.line("--- tuios-rs key log started (forwarded keys are not recorded) ---");
        Some(log)
    }

    fn line(&mut self, text: &str) {
        let _ = writeln!(self.file, "{text}");
        let _ = self.file.flush();
    }

    /// One received key and what became of it. `key` is `None` for keys that
    /// were forwarded to a shell.
    pub fn key(&mut self, key: Option<&str>, before: &str, after: &str, outcome: &str) {
        self.seq += 1;
        let n = self.seq;
        let key = key.unwrap_or("(hidden)");
        self.line(&format!(
            "#{n} key={key} state={before} -> {after} action={outcome}"
        ));
    }
}
