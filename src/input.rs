//! Key model and the bytes a terminal program expects for each key.
//!
//! Std-only on purpose: the terminal backend converts its own events into
//! [`Key`], so this logic is unit-testable without a terminal.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    BackTab,
    Backspace,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
    };

    fn is_none(self) -> bool {
        !(self.ctrl || self.alt || self.shift)
    }

    /// xterm modifier parameter: 1 + shift + 2*alt + 4*ctrl.
    fn param(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

impl Key {
    pub const fn new(code: KeyCode, mods: Mods) -> Self {
        Self { code, mods }
    }

    pub const fn plain(code: KeyCode) -> Self {
        Self::new(code, Mods::NONE)
    }

    pub const fn ch(c: char) -> Self {
        Self::plain(KeyCode::Char(c))
    }

    pub const fn ctrl(c: char) -> Self {
        Self::new(
            KeyCode::Char(c),
            Mods {
                ctrl: true,
                alt: false,
                shift: false,
            },
        )
    }

    pub const fn alt(c: char) -> Self {
        Self::new(
            KeyCode::Char(c),
            Mods {
                ctrl: false,
                alt: true,
                shift: false,
            },
        )
    }
}

/// Bytes to write to a pane's PTY for `key`.
///
/// `app_cursor` is the pane's DECCKM state (cursor keys send `ESC O x`
/// instead of `ESC [ x` when set).
pub fn encode(key: Key, app_cursor: bool) -> Vec<u8> {
    let m = key.mods;
    let mut out = Vec::new();
    match key.code {
        KeyCode::Char(c) => {
            if m.alt {
                out.push(0x1b);
            }
            match (m.ctrl, ctrl_byte(c)) {
                (true, Some(b)) => out.push(b),
                _ => {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
            }
        }
        KeyCode::Enter => alt_prefixed(&mut out, m, b"\r"),
        KeyCode::Tab => alt_prefixed(&mut out, m, b"\t"),
        KeyCode::Backspace => alt_prefixed(&mut out, m, b"\x7f"),
        KeyCode::Esc => alt_prefixed(&mut out, m, b"\x1b"),
        KeyCode::BackTab => out.extend_from_slice(b"\x1b[Z"),
        KeyCode::Up => cursor_key(&mut out, m, app_cursor, b'A'),
        KeyCode::Down => cursor_key(&mut out, m, app_cursor, b'B'),
        KeyCode::Right => cursor_key(&mut out, m, app_cursor, b'C'),
        KeyCode::Left => cursor_key(&mut out, m, app_cursor, b'D'),
        KeyCode::Home => cursor_key(&mut out, m, app_cursor, b'H'),
        KeyCode::End => cursor_key(&mut out, m, app_cursor, b'F'),
        KeyCode::Insert => tilde_key(&mut out, m, 2),
        KeyCode::Delete => tilde_key(&mut out, m, 3),
        KeyCode::PageUp => tilde_key(&mut out, m, 5),
        KeyCode::PageDown => tilde_key(&mut out, m, 6),
        KeyCode::F(n) => function_key(&mut out, m, n),
    }
    out
}

fn alt_prefixed(out: &mut Vec<u8>, m: Mods, bytes: &[u8]) {
    if m.alt {
        out.push(0x1b);
    }
    out.extend_from_slice(bytes);
}

/// Control-character byte for `ctrl+c`, if one exists.
fn ctrl_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '/' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

fn cursor_key(out: &mut Vec<u8>, m: Mods, app_cursor: bool, final_byte: u8) {
    if m.is_none() {
        out.extend_from_slice(if app_cursor { b"\x1bO" } else { b"\x1b[" });
    } else {
        out.extend_from_slice(format!("\x1b[1;{}", m.param()).as_bytes());
    }
    out.push(final_byte);
}

fn tilde_key(out: &mut Vec<u8>, m: Mods, n: u8) {
    if m.is_none() {
        out.extend_from_slice(format!("\x1b[{n}~").as_bytes());
    } else {
        out.extend_from_slice(format!("\x1b[{n};{}~", m.param()).as_bytes());
    }
}

fn function_key(out: &mut Vec<u8>, m: Mods, n: u8) {
    match n {
        1..=4 => {
            let f = b"PQRS"[usize::from(n) - 1];
            if m.is_none() {
                out.extend_from_slice(b"\x1bO");
            } else {
                out.extend_from_slice(format!("\x1b[1;{}", m.param()).as_bytes());
            }
            out.push(f);
        }
        5..=12 => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][usize::from(n) - 5];
            tilde_key(out, m, code);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(k: Key) -> Vec<u8> {
        encode(k, false)
    }

    #[test]
    fn plain_and_utf8_chars() {
        assert_eq!(e(Key::ch('a')), b"a");
        assert_eq!(e(Key::ch('é')), "é".as_bytes());
        assert_eq!(e(Key::ch('日')), "日".as_bytes());
    }

    #[test]
    fn control_chars() {
        assert_eq!(e(Key::ctrl('c')), [3]);
        assert_eq!(e(Key::ctrl('d')), [4]);
        assert_eq!(e(Key::ctrl('z')), [26]);
        assert_eq!(e(Key::ctrl(' ')), [0]);
        assert_eq!(e(Key::ctrl('[')), [0x1b]);
        assert_eq!(e(Key::ctrl('/')), [0x1f]);
        // No control byte for '!': falls back to the character itself.
        assert_eq!(e(Key::ctrl('!')), b"!");
    }

    #[test]
    fn alt_prefixes_escape() {
        assert_eq!(e(Key::alt('x')), b"\x1bx");
        assert_eq!(
            e(Key::new(
                KeyCode::Char('c'),
                Mods {
                    ctrl: true,
                    alt: true,
                    shift: false
                }
            )),
            [0x1b, 3]
        );
    }

    #[test]
    fn editing_keys() {
        assert_eq!(e(Key::plain(KeyCode::Enter)), b"\r");
        assert_eq!(e(Key::plain(KeyCode::Tab)), b"\t");
        assert_eq!(e(Key::plain(KeyCode::BackTab)), b"\x1b[Z");
        assert_eq!(e(Key::plain(KeyCode::Backspace)), [0x7f]);
        assert_eq!(e(Key::plain(KeyCode::Esc)), [0x1b]);
        assert_eq!(e(Key::plain(KeyCode::Delete)), b"\x1b[3~");
        assert_eq!(e(Key::plain(KeyCode::PageUp)), b"\x1b[5~");
        assert_eq!(e(Key::plain(KeyCode::PageDown)), b"\x1b[6~");
        assert_eq!(e(Key::plain(KeyCode::Insert)), b"\x1b[2~");
    }

    #[test]
    fn arrows_respect_application_cursor_mode() {
        assert_eq!(encode(Key::plain(KeyCode::Up), false), b"\x1b[A");
        assert_eq!(encode(Key::plain(KeyCode::Up), true), b"\x1bOA");
        assert_eq!(encode(Key::plain(KeyCode::Home), true), b"\x1bOH");
        assert_eq!(encode(Key::plain(KeyCode::End), false), b"\x1b[F");
    }

    #[test]
    fn modified_cursor_keys_use_csi_params() {
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        let ctrl = Mods {
            ctrl: true,
            ..Mods::NONE
        };
        // Modified keys ignore application-cursor mode, as xterm does.
        assert_eq!(encode(Key::new(KeyCode::Up, shift), true), b"\x1b[1;2A");
        assert_eq!(encode(Key::new(KeyCode::Right, ctrl), false), b"\x1b[1;5C");
        assert_eq!(encode(Key::new(KeyCode::Delete, ctrl), false), b"\x1b[3;5~");
    }

    #[test]
    fn function_keys() {
        assert_eq!(e(Key::plain(KeyCode::F(1))), b"\x1bOP");
        assert_eq!(e(Key::plain(KeyCode::F(4))), b"\x1bOS");
        assert_eq!(e(Key::plain(KeyCode::F(5))), b"\x1b[15~");
        assert_eq!(e(Key::plain(KeyCode::F(12))), b"\x1b[24~");
        assert!(e(Key::plain(KeyCode::F(13))).is_empty());
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        assert_eq!(e(Key::new(KeyCode::F(1), shift)), b"\x1b[1;2P");
    }
}
