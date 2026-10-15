//! Focus switching through the real `App` (real PTYs and shells, no terminal
//! around it): keys go in as events, and the focused pane, the shell that
//! receives typing, and the drawn frame are read back.

use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use tuios_rs::app::App;
use tuios_rs::event::{AppEvent, InputEvent};
use tuios_rs::input::{Key, KeyCode, Mods};
use tuios_rs::modal::Mode;
use tuios_rs::render::DIM;

struct Harness {
    app: App,
    rx: Receiver<AppEvent>,
}

impl Harness {
    fn new() -> Self {
        let (tx, rx) = channel();
        let mut app = App::new(80, 24, "/bin/sh".to_string(), tx);
        app.open_pane();
        app.open_pane();
        let mut h = Harness { app, rx };
        assert_eq!(h.app.pane_count(), 2);
        assert_eq!(h.app.focused_index(), Some(2));
        h.pump(Duration::from_millis(300));
        h
    }

    fn key(&mut self, k: Key) {
        self.app.handle(AppEvent::Input(InputEvent::Key(k)));
    }

    fn type_text(&mut self, s: &str) {
        for c in s.chars() {
            self.key(Key::ch(c));
        }
    }

    /// Process pane output for a while.
    fn pump(&mut self, d: Duration) {
        let end = Instant::now() + d;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            if let Ok(ev) = self.rx.recv_timeout(left.min(Duration::from_millis(50))) {
                self.app.handle(ev);
            }
        }
    }

    fn wait_pane(&mut self, index: usize, needle: &str) -> bool {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            self.pump(Duration::from_millis(50));
            if self
                .app
                .pane_text(index)
                .is_some_and(|t| t.contains(needle))
            {
                return true;
            }
        }
        false
    }

    fn window_mode(&mut self) {
        self.key(Key::ctrl('b'));
        self.key(Key::plain(KeyCode::Esc));
        assert_eq!(self.app.mode(), Mode::Window);
    }

    /// Whether the cell at the top-left of pane `index` (1 = left) is dimmed.
    fn pane_is_dim(&mut self, index: usize) -> bool {
        self.app.render();
        let x = if index == 1 { 0 } else { 41 };
        let frame = self.app.last_frame().expect("a frame was drawn");
        frame.get(x, 0).expect("cell").attrs & DIM != 0
    }
}

#[test]
fn tab_switches_focus_in_both_directions_and_typing_follows() {
    let mut h = Harness::new();
    h.window_mode();

    h.key(Key::plain(KeyCode::Tab));
    assert_eq!(h.app.focused_index(), Some(1), "Tab from pane 2 wraps to 1");
    h.key(Key::plain(KeyCode::Tab));
    assert_eq!(h.app.focused_index(), Some(2));
    h.key(Key::plain(KeyCode::BackTab));
    assert_eq!(h.app.focused_index(), Some(1), "Shift+Tab goes backwards");
    assert_eq!(h.app.mode(), Mode::Window);

    // Back to Terminal mode in pane 1: typing reaches pane 1 only.
    h.key(Key::ch('i'));
    h.type_text("echo one_$((20+22))");
    h.key(Key::plain(KeyCode::Enter));
    assert!(h.wait_pane(1, "one_42"), "pane 1: {:?}", h.app.pane_text(1));
    assert!(!h.app.pane_text(2).unwrap().contains("one_42"));

    // Window mode, Tab to pane 2: typing reaches pane 2 only.
    h.window_mode();
    h.key(Key::plain(KeyCode::Tab));
    assert_eq!(h.app.focused_index(), Some(2));
    h.key(Key::ch('i'));
    h.type_text("echo two_$((20+22))");
    h.key(Key::plain(KeyCode::Enter));
    assert!(h.wait_pane(2, "two_42"), "pane 2: {:?}", h.app.pane_text(2));
    assert!(!h.app.pane_text(1).unwrap().contains("two_42"));
}

#[test]
fn the_unfocused_pane_is_drawn_dim_and_the_mark_follows_focus() {
    let mut h = Harness::new();
    // Focus is on pane 2.
    assert!(h.pane_is_dim(1));
    assert!(!h.pane_is_dim(2));
    h.window_mode();
    h.key(Key::plain(KeyCode::Tab));
    assert!(!h.pane_is_dim(1), "pane 1 is active after Tab");
    assert!(h.pane_is_dim(2));
    h.key(Key::plain(KeyCode::BackTab));
    assert!(h.pane_is_dim(1));
    assert!(!h.pane_is_dim(2));
}

#[test]
fn esc_and_tab_read_together_as_alt_tab_still_switch_focus() {
    let mut h = Harness::new();
    h.key(Key::ctrl('b'));
    h.key(Key::new(
        KeyCode::Tab,
        Mods {
            alt: true,
            ..Mods::NONE
        },
    ));
    assert_eq!(h.app.mode(), Mode::Window);
    assert_eq!(h.app.focused_index(), Some(1));
}

#[test]
fn a_single_pane_is_not_dimmed() {
    let (tx, _rx) = channel();
    let mut app = App::new(80, 24, "/bin/sh".to_string(), tx);
    app.open_pane();
    app.render();
    let frame = app.last_frame().unwrap();
    assert_eq!(frame.get(0, 0).unwrap().attrs & DIM, 0);
}

#[test]
fn prefix_tab_repeated_cycles_focus_and_keeps_terminal_mode() {
    let mut h = Harness::new();
    for want in [1, 2, 1, 2, 1, 2] {
        h.key(Key::ctrl('b'));
        h.key(Key::plain(KeyCode::Tab));
        assert_eq!(h.app.focused_index(), Some(want));
        assert_eq!(h.app.mode(), Mode::Terminal);
    }
    h.key(Key::ctrl('b'));
    h.key(Key::plain(KeyCode::BackTab));
    assert_eq!(h.app.focused_index(), Some(1));
    // Typing now reaches pane 1 only.
    h.type_text("echo p1_$((20+22))");
    h.key(Key::plain(KeyCode::Enter));
    assert!(h.wait_pane(1, "p1_42"), "{:?}", h.app.pane_text(1));
    assert!(!h.app.pane_text(2).unwrap().contains("p1_42"));
}

#[test]
fn prefix_and_window_keys_never_appear_in_either_shell() {
    let mut h = Harness::new();
    h.key(Key::ctrl('b'));
    h.key(Key::plain(KeyCode::Tab));
    h.window_mode();
    for k in [KeyCode::Tab, KeyCode::BackTab, KeyCode::Tab] {
        h.key(Key::plain(k));
    }
    h.type_text("nxr=z"); // window-mode commands (q would quit the app)
                          // `n` opened a pane and `x` closed it; no shell may have received letters.
    h.pump(Duration::from_millis(300));
    for i in 1..=h.app.pane_count() {
        let t = h.app.pane_text(i).unwrap();
        assert!(!t.contains("nxr"), "pane {i}: {t:?}");
    }
}

#[test]
fn status_line_text_follows_the_mode() {
    let mut h = Harness::new();
    h.app.render();
    let t = h.app.status_text().unwrap();
    assert!(
        t.contains("TERMINAL") && t.contains("Ctrl+B: c new, Tab next, Esc window"),
        "{t}"
    );
    h.key(Key::ctrl('b'));
    h.app.render();
    let t = h.app.status_text().unwrap();
    assert!(t.contains("PREFIX") && t.contains("Esc window"), "{t}");
    h.key(Key::plain(KeyCode::Esc));
    h.app.render();
    let t = h.app.status_text().unwrap();
    assert!(
        t.contains("WINDOW") && t.contains("q quit") && t.contains("pane 2/2"),
        "{t}"
    );
}

#[test]
fn a_paste_after_the_prefix_ends_the_prefix() {
    let mut h = Harness::new();
    h.key(Key::ctrl('b'));
    h.app.handle(AppEvent::Input(InputEvent::Paste(
        "echo pp_$((20+22))\n".to_string(),
    )));
    assert!(h.wait_pane(2, "pp_42"), "{:?}", h.app.pane_text(2));
    // 'c' is now plain text for the shell, not "new pane".
    h.key(Key::ch('c'));
    assert_eq!(h.app.pane_count(), 2);
    assert_eq!(h.app.mode(), Mode::Terminal);
    h.app.render();
    let t = h.app.status_text().unwrap();
    assert!(t.contains("TERMINAL") && !t.contains("PREFIX"), "{t}");
}

#[test]
fn prefix_then_ctrl_esc_shows_window_in_the_status_bar() {
    let mut h = Harness::new();
    h.key(Key::ctrl('b'));
    h.key(Key::new(
        KeyCode::Esc,
        Mods {
            ctrl: true,
            ..Mods::NONE
        },
    ));
    assert_eq!(h.app.mode(), Mode::Window);
    h.app.render();
    let t = h.app.status_text().unwrap();
    assert!(t.contains("WINDOW") && !t.contains("PREFIX"), "{t}");
}
