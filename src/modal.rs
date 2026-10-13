//! Modal input: Terminal mode forwards keys to the focused pane, Window
//! mode treats them as multiplexer commands.
//!
//! Defaults follow upstream's keybinding table (leader `ctrl+b`; window
//! mode `n` new, `w`/`x` close, `tab`/`shift+tab` cycle, `i`/`enter` enter
//! terminal mode, `q` quit, `alt+1..9` workspaces). Deviations are marked.

use crate::input::{Key, KeyCode, Mods};

/// Status-bar hints for each state. The bar starts with
/// ` MODE | ws N/9 | pane N/M | ` (at most 31 columns), so each hint stays
/// within 47 columns to fit an 80-column terminal.
pub const HINT_TERMINAL: &str = "Ctrl+B: c new, Tab next, Esc window";
pub const HINT_PREFIX: &str = "c new  Tab next  p prev  x close  Esc window";
pub const HINT_WINDOW: &str = "Tab next  n new  x close  i shell  q quit";
pub const HINT_MAX_COLS: usize = 47;

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
        // Terminals send Esc as a lone ESC byte, and when the next key
        // arrives in the same read (a quick Ctrl+B, Esc, Tab, or a loaded
        // machine) the input layer reports "Esc then Tab" as Alt+Tab.
        // Treat Alt+key after the prefix as Esc followed by that key:
        // enter Window mode and run the key there.
        if key.mods.alt && !key.mods.ctrl {
            self.mode = Mode::Window;
            return self.handle_window(Key::new(
                key.code,
                Mods {
                    alt: false,
                    ..key.mods
                },
            ));
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
        // In Window mode Esc is a no-op, so Esc+key arriving together
        // (reported as Alt+key) means the key.
        let key = if key.mods.alt && !key.mods.ctrl {
            Key::new(
                key.code,
                Mods {
                    alt: false,
                    ..key.mods
                },
            )
        } else {
            key
        };
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
    fn prefix_then_alt_tab_is_esc_then_tab() {
        // Ctrl+B, Esc, Tab read in one burst arrives as Ctrl+B, Alt+Tab.
        let mut m = modal();
        m.handle(PREFIX);
        let alt_tab = Key::new(
            KeyCode::Tab,
            Mods {
                alt: true,
                ..Mods::NONE
            },
        );
        assert_eq!(m.handle(alt_tab), Outcome::Run(Command::FocusNext));
        assert_eq!(m.mode(), Mode::Window);
        assert!(!m.prefix_armed());
    }

    #[test]
    fn prefix_then_alt_other_keys_enter_window_mode_and_act() {
        let mut m = modal();
        m.handle(PREFIX);
        assert_eq!(m.handle(Key::alt('n')), Outcome::Run(Command::NewPane));
        assert_eq!(m.mode(), Mode::Window);
        // Alt+BackTab arrives as Esc + Shift+Tab.
        let mut b = modal();
        b.handle(PREFIX);
        let alt_back = Key::new(
            KeyCode::BackTab,
            Mods {
                alt: true,
                ..Mods::NONE
            },
        );
        assert_eq!(b.handle(alt_back), Outcome::Run(Command::FocusPrev));
        // An unbound key still enters Window mode (the Esc was real).
        let mut u = modal();
        u.handle(PREFIX);
        assert_eq!(u.handle(Key::alt('z')), Outcome::Ignore);
        assert_eq!(u.mode(), Mode::Window);
    }

    #[test]
    fn alt_tab_in_window_mode_still_switches_focus() {
        let mut m = modal();
        m.enter_window_mode();
        let alt_tab = Key::new(
            KeyCode::Tab,
            Mods {
                alt: true,
                ..Mods::NONE
            },
        );
        assert_eq!(m.handle(alt_tab), Outcome::Run(Command::FocusNext));
    }

    #[test]
    fn alt_tab_without_prefix_goes_to_the_shell() {
        let mut m = modal();
        let alt_tab = Key::new(
            KeyCode::Tab,
            Mods {
                alt: true,
                ..Mods::NONE
            },
        );
        assert_eq!(m.handle(alt_tab), Outcome::Forward(alt_tab));
    }

    #[test]
    fn tab_cycles_both_ways_without_leaving_window_mode() {
        let mut m = modal();
        m.handle(PREFIX);
        m.handle(Key::plain(KeyCode::Esc));
        for _ in 0..3 {
            assert_eq!(
                m.handle(Key::plain(KeyCode::Tab)),
                Outcome::Run(Command::FocusNext)
            );
            assert_eq!(
                m.handle(Key::plain(KeyCode::BackTab)),
                Outcome::Run(Command::FocusPrev)
            );
            assert_eq!(m.mode(), Mode::Window);
        }
    }

    #[test]
    fn prefix_tab_switches_focus_without_entering_window_mode() {
        let mut m = modal();
        for _ in 0..4 {
            m.handle(PREFIX);
            assert_eq!(
                m.handle(Key::plain(KeyCode::Tab)),
                Outcome::Run(Command::FocusNext)
            );
            assert_eq!(m.mode(), Mode::Terminal);
            assert!(!m.prefix_armed());
            m.handle(PREFIX);
            assert_eq!(
                m.handle(Key::plain(KeyCode::BackTab)),
                Outcome::Run(Command::FocusPrev)
            );
            assert_eq!(m.mode(), Mode::Terminal);
        }
        // Plain keys afterwards still go to the pane.
        assert_eq!(m.handle(Key::ch('a')), Outcome::Forward(Key::ch('a')));
    }

    #[test]
    fn rapid_prefix_sequences_are_each_handled() {
        let mut m = modal();
        let seq = [
            (Key::plain(KeyCode::Tab), Outcome::Run(Command::FocusNext)),
            (Key::ch('c'), Outcome::Run(Command::NewPane)),
            (Key::ch('n'), Outcome::Run(Command::FocusNext)),
            (Key::plain(KeyCode::Tab), Outcome::Run(Command::FocusNext)),
        ];
        for (k, want) in seq {
            assert_eq!(m.handle(PREFIX), Outcome::Ignore);
            assert_eq!(m.handle(k), want);
        }
    }

    #[test]
    fn no_prefix_or_window_key_is_forwarded_to_the_shell() {
        // Terminal mode, prefix armed: nothing is forwarded except Ctrl+B Ctrl+B.
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Esc,
            KeyCode::Char('c'),
            KeyCode::Char('n'),
            KeyCode::Char('z'),
        ] {
            let mut m = modal();
            m.handle(PREFIX);
            assert!(
                !matches!(m.handle(Key::plain(code)), Outcome::Forward(_)),
                "{code:?}"
            );
        }
        // Window mode forwards nothing at all.
        let mut w = modal();
        w.enter_window_mode();
        for c in "nwxiqr=z123".chars() {
            assert!(!matches!(w.handle(Key::ch(c)), Outcome::Forward(_)), "{c}");
            w.enter_window_mode();
        }
    }

    #[test]
    fn hints_fit_in_80_columns_and_name_the_key_actions() {
        for h in [HINT_TERMINAL, HINT_PREFIX, HINT_WINDOW] {
            assert!(h.chars().count() <= HINT_MAX_COLS, "{h}");
        }
        assert!(HINT_TERMINAL.contains("Ctrl+B") && HINT_TERMINAL.contains("Tab"));
        assert!(HINT_PREFIX.contains("Esc window") && HINT_PREFIX.contains("Tab next"));
        assert!(HINT_WINDOW.contains("q quit") && HINT_WINDOW.contains("i shell"));
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
