//! `--check-keys`: the guided check that tells a user whether Ctrl+B then Esc
//! enters Window mode in their terminal. Run against the real binary in a
//! pseudo-terminal, fed the key encodings terminals use.

mod common;

use common::Session;

fn start() -> Session {
    Session::start_args(
        24,
        80,
        &["--check-keys"],
        &[("TUIOS_KEYTEST_TIMEOUT_MS", "4000")],
        "Step 1",
    )
}

#[test]
fn check_keys_passes_for_every_esc_encoding_we_support() {
    for (name, esc) in [
        ("plain ESC", "\x1b"),
        ("Ctrl+Esc as CSI 27;5u", "\x1b[27;5u"),
        ("Shift+Esc as CSI 27;2u", "\x1b[27;2u"),
        ("Alt+Esc as ESC ESC", "\x1b\x1b"),
    ] {
        let mut s = start();
        s.send("\x02");
        s.wait_for("prefix key recognised");
        s.settle();
        s.send(esc);
        s.wait_for("RESULT:");
        let t = s.text();
        assert!(t.contains("RESULT: PASS"), "{name}:\n{t}");
        // The verdict comes from the real app: its mode and its status bar.
        assert!(t.contains("mode: Terminal -> Window"), "{name}:\n{t}");
        assert!(
            t.contains("PREFIX |") && t.contains("WINDOW |"),
            "{name}:\n{t}"
        );
    }
}

#[test]
fn check_keys_reports_a_missing_esc_instead_of_hanging() {
    let mut s = start();
    s.send("\x02");
    s.wait_for("prefix key recognised");
    // The terminal sends nothing for Esc: the check must say so.
    s.wait_for("never received the Esc key");
    assert!(s.text().contains("RESULT: FAIL"), "{}", s.text());
}

#[test]
fn check_keys_reports_an_unreadable_esc_encoding() {
    // xterm's modifyOtherKeys form of Ctrl+Esc cannot be read by the input
    // library: the program sees nothing, and the check says so.
    let mut s = start();
    s.send("\x02");
    s.wait_for("prefix key recognised");
    s.settle();
    s.send("\x1b[27;5;27~");
    s.wait_for("RESULT: FAIL");
}

#[test]
fn check_keys_rejects_a_wrong_first_key() {
    let mut s = start();
    s.send("x");
    s.wait_for("did not recognise");
}
