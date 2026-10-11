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
