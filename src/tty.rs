//! The real terminal: raw mode, alternate screen, and key/resize events.

use std::io::{self, Write};
use std::sync::mpsc::Sender;
use std::thread;

use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode as CKey, KeyEvent,
    KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use crate::event::{AppEvent, InputEvent};
use crate::input::{Key, KeyCode, Mods};

/// Puts the terminal in raw + alternate-screen mode; restores it on drop,
/// including when the program panics.
pub struct TtyGuard;

impl TtyGuard {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste) {
            let _ = disable_raw_mode();
            return Err(e);
        }
        Ok(TtyGuard)
    }
}

impl Drop for TtyGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = execute!(out, DisableBracketedPaste, Show, LeaveAlternateScreen);
        let _ = out.flush();
        let _ = disable_raw_mode();
    }
}

fn convert_key(k: KeyEvent) -> Option<Key> {
    if k.kind == KeyEventKind::Release {
        return None;
    }
    let mut mods = Mods {
        ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
        alt: k.modifiers.contains(KeyModifiers::ALT),
        shift: k.modifiers.contains(KeyModifiers::SHIFT),
    };
    let code = match k.code {
        // The character already carries its shift (A vs a).
        CKey::Char(c) => {
            mods.shift = false;
            KeyCode::Char(c)
        }
        CKey::Enter => KeyCode::Enter,
        CKey::Tab => KeyCode::Tab,
        CKey::BackTab => {
            mods.shift = false;
            KeyCode::BackTab
        }
        CKey::Backspace => KeyCode::Backspace,
        CKey::Esc => KeyCode::Esc,
        CKey::Up => KeyCode::Up,
        CKey::Down => KeyCode::Down,
        CKey::Left => KeyCode::Left,
        CKey::Right => KeyCode::Right,
        CKey::Home => KeyCode::Home,
        CKey::End => KeyCode::End,
        CKey::PageUp => KeyCode::PageUp,
        CKey::PageDown => KeyCode::PageDown,
        CKey::Insert => KeyCode::Insert,
        CKey::Delete => KeyCode::Delete,
        CKey::F(n) => KeyCode::F(n),
        _ => return None,
    };
    Some(Key::new(code, mods))
}

/// Read terminal events on a background thread and forward them to `tx`.
pub fn spawn_input_thread(tx: Sender<AppEvent>) {
    thread::spawn(move || loop {
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(_) => {
                let _ = tx.send(AppEvent::Input(InputEvent::Eof));
                break;
            }
        };
        let mapped = match ev {
            Event::Key(k) => convert_key(k).map(InputEvent::Key),
            Event::Paste(s) => Some(InputEvent::Paste(s)),
            Event::Resize(w, h) => Some(InputEvent::Resize(w, h)),
            _ => None,
        };
        if let Some(m) = mapped {
            if tx.send(AppEvent::Input(m)).is_err() {
                break;
            }
        }
    });
}
