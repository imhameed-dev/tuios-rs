use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::mpsc;

use tuios_rs::app::App;
use tuios_rs::tty::{spawn_input_thread, TtyGuard};

const HELP: &str = "\
tuios-rs: terminal multiplexer (Rust rewrite of TUIOS, early version)

Usage: tuios-rs [--version | --help]

Keys (Terminal mode, the default):
  Ctrl+B Esc     switch to Window mode
  Ctrl+B c       new pane        Ctrl+B x   close pane
  Ctrl+B n / p   next / previous pane
  Ctrl+B 1-9     switch workspace (Alt+1-9 also works)
  Ctrl+B Ctrl+B  send a literal Ctrl+B

Window mode:
  n new pane   x or w close   Tab / Shift+Tab focus
  1-9 focus Nth pane   r rotate split   = equalize
  i or Enter back to Terminal mode   q quit
";

fn flush(app: &mut App) -> io::Result<()> {
    if let Some(frame) = app.render() {
        let mut out = io::stdout().lock();
        out.write_all(frame.as_bytes())?;
        out.flush()?;
    }
    Ok(())
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tuios-rs {}", tuios_rs::version());
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }

    let _guard = TtyGuard::enter()?;
    let (cols, rows) = crossterm::terminal::size()?;
    let (tx, rx) = mpsc::channel();
    spawn_input_thread(tx.clone());
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());

    let mut app = App::new(cols, rows, shell, tx);
    app.open_pane();
    flush(&mut app)?;
    while !app.quit {
        let Ok(ev) = rx.recv() else { break };
        app.handle(ev);
        // Batch whatever else is already waiting so one burst of output
        // costs one redraw.
        while !app.quit {
            match rx.try_recv() {
                Ok(ev) => app.handle(ev),
                Err(_) => break,
            }
        }
        flush(&mut app)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tuios-rs: {e}");
            ExitCode::FAILURE
        }
    }
}
