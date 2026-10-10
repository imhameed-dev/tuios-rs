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
