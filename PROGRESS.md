# Progress log

## Clock
- **Start:** 2026-10-09 17:33 IST (12:03 UTC), the moment CI was first verified green.
- **Deadline (10 days):** 2026-10-19 17:33 IST.
- Scope: core multiplexer only (panes, workspaces, BSP tiling, modal keys, copy mode, config, daemon attach/detach).

## Verified checkpoints
| When (IST) | Checkpoint | Evidence |
|---|---|---|
| 2026-10-09 17:30 | Push to `dev` works | `git ls-remote` shows `dev` at f49fb47 |
| 2026-10-09 17:33 | CI runs and passes (fmt, clippy, build, test) | Actions run 37927580083, commit dc9720a, conclusion success |

## Day 1 (Oct 9)
- Upstream audited: Gaurav-Gosain/tuios v0.9.2, Go, MIT, ~378k non-test LOC.
- Local sandbox cannot reach crates.io or the Go proxy, so all dependency builds happen in CI.

## Day 2 (Oct 10)
- Resumed from verified checkpoint 6515fa2 (remote `dev` matched local; two green CI runs confirmed via `gh run list`).
- Found uncommitted BSP tiling port (`src/layout/bsp.rs`, 21 tests) that did not compile: closure-type error in a test, then clippy MSRV mismatch (`is_multiple_of` needs 1.87). Fixed both; MSRV now 1.87.
- Local (no-dependency crate, so offline cargo works): fmt clean, clippy `-D warnings` clean, 22 tests pass. CI result for this commit recorded below once it finishes.
- Added `src/workspace.rs` (9 workspaces, focus, cycle, move, close) with 7 tests; local total 29 tests pass, clippy `-D warnings` clean.
- Added `FEATURES.md` (Release 1 vs deferred).
- Next: dependency research (PTY, VT parser, terminal I/O, TOML) with license checks, then modal input state machine.
- Researched crates (see DEPENDENCIES.md). Chose portable-pty + vt100 + crossterm over writing a terminal emulator.
- Built the vertical slice: `input.rs` (key encoding), `modal.rs` (modes), `render.rs` (frame diff), `pane.rs` (PTY + emulator), `tty.rs`, `app.rs`, `main.rs`; BSP and workspace modules are now used by the app.
- **CI result, commit f1d1885 (run 38052837448): fmt, clippy `-D warnings`, build, test all pass. 56 unit tests + 5 end-to-end tests pass.** The e2e tests start the real binary in a pseudo-terminal and check: a shell command runs, `exit` closes the app, resize reaches the shell (29x100), Ctrl+B Esc / n / i / x / q, the pane separator appears, workspace switch with Ctrl+B 2 and Alt+1.
- CI job logs cannot be downloaded from this workspace, so CI now publishes test names and results as annotations (`scripts/ci-report.sh`).
- Not verified: behavior on the user's laptop, RAM/CPU/binary size (not measured yet), vim/htop/less inside a pane, long-running output, paste.
- Next: release-binary build in CI plus RSS/binary-size measurement; copy mode and scrollback; config; detach.

## Day 3 (Oct 11)
- Resumed from verified `50f9e4f`. Found unpushed work in the tree (release workflow, compat and perf tests, packaging, install guide, paste hardening); read all of it, ran the offline checks, and pushed it.
- Annotation size: GitHub cuts annotations at about 4 KB, which hid test failures. `scripts/ci-report.sh` now emits failures first as errors, compresses passing test names, and splits long logs.
- **Bug found by the new compatibility tests:** 11 panes on 80x24 crashed the app. Cause: `vt100` 0.16.2 overflows in `Grid::col_wrap` on 1-row screens (and on wide characters in 1-column screens). Fix: emulator and PTY never below 2x2; new pane refused if any pane would drop under 4x2; emulator panics caught per pane; panic messages now print on the restored terminal.
- **Release pipeline works:** the release workflow builds, tests the exact binary inside `archlinux:latest`, then publishes the rolling pre-release `dev-latest`. Verified by downloading it anonymously (HTTP 200), checking the SHA-256, unpacking, and running `--version`.
- Arch results: 5 end-to-end + 16 compatibility tests pass (vim, less, htop, bash, paste, Unicode, colors, resize, cleanup).
- Measured (CI runner, not the laptop): binary 771,432 bytes; RSS 4.1 MB (1 pane), 5.0 MB (4), 5.2 MB (9); idle CPU 0.00 %. See `docs/PERFORMANCE.md`.
- Dependencies audited: 45 third-party crates, all permissive, none copyleft (`DEPENDENCIES.md`).
- Not verified: anything on the user's laptop; real terminal emulators; dwm key conflicts; neovim; zsh/fish.
- Next: copy mode and scrollback browsing, then TOML config with rebindable keys, then attach/detach.

## Day 4 (Oct 13): pane focus bug
- Report: on the real laptop, Ctrl+B Esc works but Tab does not switch the active pane.
- Investigated before changing code: key conversion (`tty.rs`), modal state machine, `Workspaces::cycle_focus`, and rendering. Added `tests/focus.rs` driving the real binary in a PTY. Result in CI before the fix: Tab, Shift+Tab and three-pane cycling all worked when keys arrive one at a time.
- **Defect reproduced:** when Esc and Tab arrive in one read (fast typing, loaded machine, or some terminals), crossterm reports Alt+Tab. After the prefix the modal layer swallowed any modified key, so Window mode was never entered and focus never changed. Fix in `src/modal.rs`: after the prefix, Alt+key means Esc then key; in Window mode Alt+key is read as the key.
- **Weak indicator:** in Window mode the cursor is hidden and the separator highlight is the same for two panes, so a working switch was hard to see. Inactive panes are now drawn faint (`DIM`), and the status bar shows `pane N/M`.
- **New diagnostics:** `TUIOS_KEYLOG=<file>` records each key and the decision taken, never text sent to a shell.
- Tests added: 5 modal unit tests, 1 render unit test, 4 in-process tests (`tests/app_focus.rs`: both directions, typing reaches the focused shell only, dim follows focus, Esc+Tab coalescing), 7 PTY tests (`tests/focus.rs`). `focus` now also runs against the packaged binary inside `archlinux:latest` in the release workflow.
- CI for commit 4c0f837 (runs 38389419908 and 38389419980): fmt, clippy `-D warnings`, build and tests pass; 67 unit, 4 app_focus, 16 compat, 5 e2e, 7 focus tests pass on the runner and on Arch.
- Not verified: the user's own laptop, a real terminal emulator or dwm. The laptop's exact cause is unconfirmed; the Esc+Tab coalescing defect is the one that was reproduced.

## Day 5 (Oct 14): keybinding audit
- Audited the committed keybindings (`modal.rs`, `app.rs`, `main.rs` help, `docs/INSTALL-ARCH.md`) against the required behavior: Ctrl+B c new pane, Ctrl+B Tab next pane without entering Window mode, Ctrl+B Esc Window mode. All three were already implemented and covered by tests in 6e70662.
- Bug found by reading the code: a paste after Ctrl+B left the prefix armed, so the next typed key ran as a command (for example `c` opened a pane) instead of reaching the shell. Fixed: a paste now cancels the pending prefix. Unit and in-process regression tests added.
- Design note (unchanged): Window mode uses plain keys; the Ctrl+B prefix applies in Terminal mode only. Documented.
- The user's `/tmp/keys.txt` is on the user's laptop and was not available in this workspace, so it was not part of this audit.

## Day 5/6 (Oct 14-15): Ctrl+B Esc report
- Report: on the laptop, Ctrl+B then Esc did not enter Window mode; Ctrl+B c, Ctrl+B Tab and per-pane typing passed.
- Reproduction against the downloaded release (dev-latest, f5e3aef) in a PTY, with `TUIOS_KEYLOG` for the app's decisions: plain Esc worked at every timing tried (300 ms, 40 ms, 5 ms gaps, one write, Ctrl+[), with real bash and sh, in the full manual sequence (create pane, switch, type, Esc, `n`, `i`). **The reported failure did not reproduce with a plain Esc byte.**
- **Defect reproduced:** after the prefix, Esc reported with Ctrl or Shift (kitty-style `CSI 27;5u` / `27;2u`, sent by terminals that report modifiers) was treated as an unbound modified key: the prefix was dropped and Window mode was not entered. Fix in `src/modal.rs`: after the prefix, Esc means Window mode whatever modifiers are reported.
- Not fixable here: xterm modifyOtherKeys Esc (`CSI 27;5;27~`) is dropped by the input library (crossterm), so the app never sees a key; documented in INSTALL-ARCH.md.
- Whether this is the cause on the laptop is unconfirmed; the terminal emulator is not known.

## Day 6 (Oct 15): Alacritty + dwm report
- Laptop report: basic pane tests pass; Ctrl+B then Esc still reported as failing in Alacritty under dwm.
- Code and encodings re-checked: plain Esc, Ctrl/Shift/Alt+Esc, `Ctrl+[`, one-write bursts and held-key repeat all behave correctly against the released binary in a PTY. No Alacritty-specific cause was found, and Alacritty's real key output could not be observed from this workspace.
- Cause **not identified**. Added `tuios-rs --check-keys`: a two-keypress check that runs the real key conversion and mode logic in the user's own terminal and prints PASS/FAIL with the reason. Tests in `tests/keycheck.rs`.
