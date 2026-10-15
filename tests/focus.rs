//! Pane focus switching in Window mode, driven like a person would: real
//! binary, real pseudo-terminal, keys typed in, screen and cursor read back.
//!
//! With two panes on an 80-column terminal the left pane covers columns
//! 0..40, the separator is column 40, and the right pane starts at 41.

mod common;

use common::Session;

const RIGHT_FROM: u16 = 41;
const LEFT_BELOW: u16 = 40;

fn cursor_in_right(s: &Session) -> bool {
    s.cursor().1 >= RIGHT_FROM
}

fn cursor_in_left(s: &Session) -> bool {
    s.cursor().1 < LEFT_BELOW
}

/// Two panes, focus on the right one, Terminal mode.
fn two_panes() -> Session {
    let mut s = Session::start(24, 80);
    s.send("\x02c"); // Ctrl+B c
    s.wait_for("pane 2/2");
    s.wait_until("cursor in the right pane", cursor_in_right);
    s
}

#[test]
fn tab_moves_focus_forward_and_back_and_input_follows() {
    let mut s = two_panes();

    // Ctrl+B, Esc, then Tab, as typed by hand with a pause between keys.
    s.window_mode();
    s.settle();
    s.send("\t");
    s.wait_for("pane 1/2");
    s.send("i");
    s.wait_for("TERMINAL");
    s.wait_until("cursor in the left pane", cursor_in_left);
    s.send("echo left_$((20+22))\r");
    s.wait_until("output in the left pane", |s| {
        s.find("left_42").is_some_and(|(_, c)| c < LEFT_BELOW)
    });

    // And back again.
    s.window_mode();
    s.settle();
    s.send("\t");
    s.wait_for("pane 2/2");
    s.send("i");
    s.wait_for("TERMINAL");
    s.wait_until("cursor in the right pane", cursor_in_right);
    s.send("echo right_$((30+12))\r");
    s.wait_until("output in the right pane", |s| {
        s.find("right_42").is_some_and(|(_, c)| c >= RIGHT_FROM)
    });
}

#[test]
fn tab_in_the_same_read_as_esc_still_switches_focus() {
    // On a slow or busy machine two keypresses can be delivered together. A
    // terminal then reports ESC followed by Tab as one Alt+Tab key.
    let mut s = two_panes();
    s.send("\x02\x1b\t"); // Ctrl+B, Esc, Tab in a single write
    s.wait_for("pane 1/2");
    s.wait_for("WINDOW");
}

#[test]
fn shift_tab_moves_focus_backwards() {
    let mut s = two_panes();
    s.window_mode();
    s.settle();
    s.send("\x1b[Z"); // Shift+Tab
    s.wait_for("pane 1/2");
    s.settle();
    s.send("\x1b[Z");
    s.wait_for("pane 2/2");
}

#[test]
fn three_panes_cycle_in_order_both_ways() {
    let mut s = two_panes();
    s.send("\x02c");
    s.wait_for("pane 3/3");
    s.window_mode();
    for want in ["pane 1/3", "pane 2/3", "pane 3/3"] {
        s.settle();
        s.send("\t");
        s.wait_for(want);
    }
    for want in ["pane 2/3", "pane 1/3", "pane 3/3"] {
        s.settle();
        s.send("\x1b[Z");
        s.wait_for(want);
    }
}

#[test]
fn tab_in_terminal_mode_goes_to_the_shell() {
    let mut s = two_panes();
    s.send("cat -A\r");
    s.settle();
    s.send("\t\r");
    s.wait_for("^I");
    s.send("\x04"); // Ctrl+D ends cat
    s.wait_for("pane 2/2"); // still two panes, still alive
}

#[test]
fn window_mode_keys_do_not_leak_into_the_shell() {
    let mut s = two_panes();
    s.window_mode();
    s.settle();
    s.send("\t");
    s.wait_for("pane 1/2");
    // Back in Terminal mode in pane 1, the shell has seen nothing from Tab.
    s.send("i");
    s.wait_for("TERMINAL");
    s.send("echo clean_$((1+1))\r");
    s.wait_for("clean_2");
}

#[test]
fn key_log_records_the_tab_decision_and_hides_shell_input() {
    let path = std::env::temp_dir().join(format!("tuios-keylog-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let path_str = path.to_str().unwrap().to_string();
    let mut s = Session::start_with_env(24, 80, &[("TUIOS_KEYLOG", &path_str)]);
    s.send("\x02c");
    s.wait_for("pane 2/2");
    s.wait_until("cursor in the right pane", cursor_in_right);
    s.send("echo secret_$((20+22))\r");
    s.wait_for("secret_42");
    s.window_mode();
    s.settle();
    s.send("\t");
    s.wait_for("pane 1/2");
    let log = std::fs::read_to_string(&path).expect("key log written");
    let _ = std::fs::remove_file(&path);
    assert!(log.contains("action=FocusNext"), "{log}");
    assert!(log.contains("window/pane2of2 -> window/pane1of2"), "{log}");
    assert!(log.contains("key=(hidden)"), "{log}");
    assert!(
        !log.contains("secret") && !log.contains("'e'"),
        "typed shell input must not be logged: {log}"
    );
}

#[test]
fn prefix_tab_switches_focus_without_entering_window_mode() {
    let mut s = two_panes();
    s.send("\x02\t"); // Ctrl+B, Tab
    s.wait_for("pane 1/2");
    s.wait_until("cursor in the left pane", cursor_in_left);
    assert!(s.text().contains("TERMINAL"), "still Terminal mode");
    s.send("echo left_$((20+22))\r");
    s.wait_for("left_42");
    assert!(s.cursor().1 < LEFT_BELOW, "typing stayed in the left pane");
    s.send("\x02\t");
    s.wait_for("pane 2/2");
    s.wait_until("cursor in the right pane", cursor_in_right);
    s.send("echo right_$((20+22))\r");
    s.wait_for("right_42");
}

#[test]
fn rapid_prefix_tab_burst_ends_on_the_right_pane() {
    let mut s = two_panes();
    // Five Ctrl+B Tab pairs in one write: 2 -> 1 -> 2 -> 1 -> 2 -> 1.
    s.send("\x02\t\x02\t\x02\t\x02\t\x02\t");
    s.wait_for("pane 1/2");
    s.wait_until("cursor in the left pane", cursor_in_left);
    // Six more: back to where it started, after an even number of switches.
    s.send("\x02\t\x02\t\x02\t\x02\t\x02\t\x02\t");
    s.wait_for("pane 1/2");
    s.settle();
    s.send("\x02\t");
    s.wait_for("pane 2/2");
    s.wait_until("cursor in the right pane", cursor_in_right);
    s.send("echo burst_$((20+22))\r");
    s.wait_for("burst_42");
}

#[test]
fn rapid_window_mode_tab_presses_each_switch_once() {
    let mut s = two_panes();
    s.window_mode();
    s.settle();
    s.send("\t\t\t"); // three switches: 2 -> 1 -> 2 -> 1
    s.wait_for("pane 1/2");
    s.settle();
    assert!(s.text().contains("WINDOW"));
    s.send("\x1b[Z\x1b[Z"); // two back: 1 -> 2 -> 1
    s.settle();
    s.wait_for("pane 1/2");
    s.send("\x1b[Z");
    s.wait_for("pane 2/2");
}

#[test]
fn prefix_keys_do_not_leak_into_the_shell() {
    let mut s = two_panes();
    s.send("cat -A\r");
    s.settle();
    // Away and back with the prefix, plus a new pane and a close.
    s.send("\x02\t\x02\t");
    s.wait_for("pane 2/2");
    s.send("\x02c");
    s.wait_for("pane 3/3");
    s.send("\x02x");
    s.wait_for("pane 2/2");
    s.settle();
    if cursor_in_left(&s) {
        s.send("\x02\t"); // focus fell back to the left pane; cat runs on the right
        s.wait_until("cursor in the right pane", cursor_in_right);
    }
    s.send("Z\r");
    s.wait_for("Z$");
    let text = s.text();
    assert!(!text.contains("^B"), "Ctrl+B reached the shell:\n{text}");
    assert!(!text.contains("^I"), "Tab reached the shell:\n{text}");
    s.send("\x04");
}

#[test]
fn status_bar_explains_each_mode() {
    let mut s = Session::start(24, 80);
    s.wait_for("Ctrl+B: c new, Tab next, Esc window");
    s.send("\x02");
    s.wait_for("PREFIX");
    s.wait_for("c new  Tab next  p prev  x close  Esc window");
    s.send("\x1b");
    s.wait_for("WINDOW");
    s.wait_for("Tab next  n new  x close  i shell  q quit");
}

/// Esc encodings a terminal may send for "Esc" while Ctrl or Shift is held
/// (terminals that report modifiers), plus the plain byte and Alt+Esc.
const ESC_ENCODINGS: [(&str, &str); 5] = [
    ("plain ESC", "\x1b"),
    ("Ctrl+Esc as CSI 27;5u", "\x1b[27;5u"),
    ("Shift+Esc as CSI 27;2u", "\x1b[27;2u"),
    ("Esc as CSI 27u", "\x1b[27u"),
    ("Alt+Esc as ESC ESC", "\x1b\x1b"),
];

#[test]
fn ctrl_b_esc_enters_window_mode_and_window_keys_need_no_prefix() {
    for (name, esc) in ESC_ENCODINGS {
        let mut s = Session::start(24, 80);
        s.send("\x02");
        s.settle();
        s.send(esc);
        s.wait_for("WINDOW");
        assert!(!s.text().contains("PREFIX"), "{name}: {}", s.text());
        // `n` makes a pane with no prefix, and the status says so.
        s.send("n");
        s.wait_for("pane 2/2");
        assert!(s.text().contains("WINDOW"), "{name}: still Window mode");
        // `i` returns to Terminal mode, and typing reaches the shell again.
        s.send("i");
        s.wait_for("TERMINAL");
        s.send("echo back_$((20+22))\r");
        s.wait_for("back_42");
    }
}

#[test]
fn ctrl_b_and_esc_in_one_write_enters_window_mode() {
    let mut s = Session::start(24, 80);
    s.send("\x02\x1b");
    s.wait_for("WINDOW");
    s.send("n");
    s.wait_for("pane 2/2");
}

#[test]
fn esc_in_terminal_mode_reaches_the_shell_and_focus_still_switches() {
    let mut s = two_panes();
    s.send("cat -A\r");
    s.settle();
    s.send("\x1b"); // a lone Esc, no prefix
    s.settle();
    s.send("Q\r");
    s.wait_for("^[Q$");
    assert!(s.text().contains("TERMINAL"), "Esc must not change mode");
    s.send("\x04");
    s.send("\x02\t");
    s.wait_for("pane 1/2");
    s.wait_until("cursor in the left pane", cursor_in_left);
}
