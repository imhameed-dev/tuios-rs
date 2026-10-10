//! Modal input: Terminal mode forwards keys to the focused pane, Window
//! mode treats them as multiplexer commands.
//!
//! Defaults follow upstream's keybinding table (leader `ctrl+b`; window
//! mode `n` new, `w`/`x` close, `tab`/`shift+tab` cycle, `i`/`enter` enter
//! terminal mode, `q` quit, `alt+1..9` workspaces). Deviations are marked.

use crate::input::{Key, KeyCode, Mods};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Keys go to the focused pane.
    Terminal,
    /// Keys are multiplexer commands.
    Window,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NewPane,
    ClosePane,
    FocusNext,
    FocusPrev,
    /// Focus the Nth pane (1-based) of the current workspace.
    FocusIndex(usize),
    /// Switch to workspace (0-based).
    Workspace(usize),
    RotateSplit,
    Equalize,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Send this key to the focused pane.
    Forward(Key),
    Run(Command),
    /// Handled internally (mode change, prefix armed) or unbound.
    Ignore,
}

#[derive(Debug)]
pub struct Modal {
    mode: Mode,
    prefix: Key,
    prefix_armed: bool,
}

impl Modal {
    pub fn new(prefix: Key) -> Self {
        Self {
            mode: Mode::Terminal,
            prefix,
            prefix_armed: false,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn prefix_armed(&self) -> bool {
        self.prefix_armed
    }

    /// Force Window mode (used when no pane has focus).
    pub fn enter_window_mode(&mut self) {
        self.mode = Mode::Window;
        self.prefix_armed = false;
    }

    pub fn handle(&mut self, key: Key) -> Outcome {
        // Alt+1..9 switches workspace in both modes, like upstream.
        if let Some(ws) = alt_digit(key) {
            self.prefix_armed = false;
            return Outcome::Run(Command::Workspace(ws));
        }
        match self.mode {
            Mode::Terminal => self.handle_terminal(key),
            Mode::Window => self.handle_window(key),
        }
    }

    fn handle_terminal(&mut self, key: Key) -> Outcome {
        if !self.prefix_armed {
            if key == self.prefix {
                self.prefix_armed = true;
                return Outcome::Ignore;
            }
            return Outcome::Forward(key);
        }
        self.prefix_armed = false;
        if key == self.prefix {
            // Prefix twice sends the prefix key itself.
            return Outcome::Forward(key);
        }
        if key.mods != Mods::NONE {
            return Outcome::Ignore;
        }
        match key.code {
            // Deviation: upstream's Esc alone leaves terminal mode, which
            // would steal Esc from vim; here it needs the prefix first.
            KeyCode::Esc => {
                self.mode = Mode::Window;
                Outcome::Ignore
            }
            KeyCode::Char('c') => Outcome::Run(Command::NewPane),
            KeyCode::Char('x') => Outcome::Run(Command::ClosePane),
            KeyCode::Char('n') | KeyCode::Tab => Outcome::Run(Command::FocusNext),
            KeyCode::Char('p') | KeyCode::BackTab => Outcome::Run(Command::FocusPrev),
            // Deviation: prefix+digit switches workspace, for desktops where
            // Alt+digit belongs to the window manager (dwm tags).
            KeyCode::Char(d @ '1'..='9') => Outcome::Run(Command::Workspace(digit(d) - 1)),
            _ => Outcome::Ignore,
        }
    }

    fn handle_window(&mut self, key: Key) -> Outcome {
        if key.mods == Mods::NONE {
            match key.code {
                KeyCode::Char('n') => return Outcome::Run(Command::NewPane),
                KeyCode::Char('w') | KeyCode::Char('x') => return Outcome::Run(Command::ClosePane),
                KeyCode::Tab => return Outcome::Run(Command::FocusNext),
                KeyCode::Char('i') | KeyCode::Enter => {
                    self.mode = Mode::Terminal;
                    return Outcome::Ignore;
                }
                KeyCode::Char('q') => return Outcome::Run(Command::Quit),
                KeyCode::Char(d @ '1'..='9') => {
                    return Outcome::Run(Command::FocusIndex(digit(d)));
                }
                // Not upstream defaults (upstream has no plain-letter
                // equivalents for these): minimal rotate/equalize bindings.
                KeyCode::Char('r') => return Outcome::Run(Command::RotateSplit),
                KeyCode::Char('=') => return Outcome::Run(Command::Equalize),
                _ => {}
            }
        }
        if key == Key::plain(KeyCode::BackTab)
            || key
                == Key::new(
                    KeyCode::Tab,
                    Mods {
                        shift: true,
                        ..Mods::NONE
                    },
                )
        {
            return Outcome::Run(Command::FocusPrev);
        }
        Outcome::Ignore
    }
}

fn digit(c: char) -> usize {
    c.to_digit(10).map_or(0, |d| d as usize)
}

fn alt_digit(key: Key) -> Option<usize> {
    match key {
        Key {
            code: KeyCode::Char(d @ '1'..='9'),
            mods:
                Mods {
                    alt: true,
                    ctrl: false,
                    shift: false,
                },
        } => Some(digit(d) - 1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: Key = Key::ctrl('b');

    fn modal() -> Modal {
        Modal::new(PREFIX)
    }

    #[test]
    fn terminal_mode_forwards_everything_but_the_prefix() {
        let mut m = modal();
        assert_eq!(m.handle(Key::ch('a')), Outcome::Forward(Key::ch('a')));
        assert_eq!(
            m.handle(Key::plain(KeyCode::Esc)),
            Outcome::Forward(Key::plain(KeyCode::Esc))
        );
        assert_eq!(m.handle(PREFIX), Outcome::Ignore);
        assert!(m.prefix_armed());
    }

    #[test]
    fn double_prefix_sends_literal_prefix() {
        let mut m = modal();
        m.handle(PREFIX);
        assert_eq!(m.handle(PREFIX), Outcome::Forward(PREFIX));
        assert!(!m.prefix_armed());
    }

    #[test]
    fn prefix_esc_enters_window_mode_and_i_returns() {
        let mut m = modal();
        m.handle(PREFIX);
        assert_eq!(m.handle(Key::plain(KeyCode::Esc)), Outcome::Ignore);
        assert_eq!(m.mode(), Mode::Window);
        assert_eq!(m.handle(Key::ch('i')), Outcome::Ignore);
        assert_eq!(m.mode(), Mode::Terminal);
    }

    #[test]
    fn prefix_commands_in_terminal_mode() {
        let mut m = modal();
        for (k, c) in [
            (Key::ch('c'), Command::NewPane),
            (Key::ch('x'), Command::ClosePane),
            (Key::ch('n'), Command::FocusNext),
            (Key::ch('p'), Command::FocusPrev),
            (Key::ch('3'), Command::Workspace(2)),
        ] {
            m.handle(PREFIX);
            assert_eq!(m.handle(k), Outcome::Run(c), "{k:?}");
            assert_eq!(m.mode(), Mode::Terminal);
        }
    }

    #[test]
    fn unbound_key_after_prefix_is_swallowed_and_disarms() {
        let mut m = modal();
        m.handle(PREFIX);
        assert_eq!(m.handle(Key::ch('~')), Outcome::Ignore);
        assert!(!m.prefix_armed());
        assert_eq!(m.handle(Key::ch('a')), Outcome::Forward(Key::ch('a')));
    }

    #[test]
    fn window_mode_commands() {
        let mut m = modal();
        m.enter_window_mode();
        for (k, c) in [
            (Key::ch('n'), Command::NewPane),
            (Key::ch('w'), Command::ClosePane),
            (Key::ch('x'), Command::ClosePane),
            (Key::plain(KeyCode::Tab), Command::FocusNext),
            (Key::plain(KeyCode::BackTab), Command::FocusPrev),
            (Key::ch('2'), Command::FocusIndex(2)),
            (Key::ch('r'), Command::RotateSplit),
            (Key::ch('='), Command::Equalize),
            (Key::ch('q'), Command::Quit),
        ] {
            assert_eq!(m.handle(k), Outcome::Run(c), "{k:?}");
            assert_eq!(m.mode(), Mode::Window);
        }
        assert_eq!(m.handle(Key::ch('z')), Outcome::Ignore);
    }

    #[test]
    fn window_mode_enter_returns_to_terminal() {
        let mut m = modal();
        m.enter_window_mode();
        assert_eq!(m.handle(Key::plain(KeyCode::Enter)), Outcome::Ignore);
        assert_eq!(m.mode(), Mode::Terminal);
    }

    #[test]
    fn quit_is_not_reachable_from_terminal_mode_by_plain_q() {
        let mut m = modal();
        assert_eq!(m.handle(Key::ch('q')), Outcome::Forward(Key::ch('q')));
    }

    #[test]
    fn alt_digits_switch_workspace_in_both_modes() {
        let mut m = modal();
        assert_eq!(m.handle(Key::alt('1')), Outcome::Run(Command::Workspace(0)));
        m.enter_window_mode();
        assert_eq!(m.handle(Key::alt('9')), Outcome::Run(Command::Workspace(8)));
        // Alt+0 is not a workspace key.
        let mut t = modal();
        assert_eq!(t.handle(Key::alt('0')), Outcome::Forward(Key::alt('0')));
    }
}
