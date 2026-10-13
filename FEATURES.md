# Feature inventory

Upstream reference: `Gaurav-Gosain/tuios` v0.9.2 (Go, MIT), README feature list.
Status: **done** = implemented and covered by passing tests in CI on Linux; **partial**; **planned**; **deferred** (outside the agreed scope).
"done" never means run by hand on the target laptop: that has not happened yet.

## Release 1: core multiplexer (agreed scope)

| Feature | Status | Evidence / note |
|---|---|---|
| BSP tiling (spiral default, longest-side, alternate, smart-split, preselect, rotate, swap, equalize) | done | `src/layout/bsp.rs`, 21 unit tests. In the app: spiral for new panes; `r` rotates, `=` equalizes. Preselect and swap exist in the library but have no key. |
| 9 workspaces | done | `src/workspace.rs`, 8 unit tests; e2e `workspaces_switch_and_stay_isolated`. Moving a window to another workspace has no key yet. |
| Panes running real shells (PTY) | done | `src/pane.rs`; e2e `shell_runs_a_command_and_exit_closes_the_app` |
| Terminal emulation | partial | `vt100` crate. Verified with vim, less, htop, bash (see compatibility table). Not supported: mouse reporting, kitty/sixel graphics, kitty keyboard protocol. |
| Drawing, resize | done | diff renderer (10 unit tests); e2e resize; compat `htop_draws_in_color_follows_resize_and_quits`, `shrinking_the_terminal_to_a_sliver_does_not_crash` |
| Modal keys (Terminal / Window mode, Ctrl+B prefix) | done, fixed keys | `src/modal.rs`, 9 unit tests; e2e `panes_modes_and_quit`. Keys follow upstream defaults; deviations are marked in the source. Not rebindable until config exists. |
| Create / focus / close panes | done | e2e + compat. Focus is next / previous / Nth; inactive panes are drawn faint. Directional focus is not implemented. |
| Minimum pane size | done | New pane refused if any pane would drop under 4x2 (status bar says so). The emulator is never given less than 2x2. Compat `many_panes_on_a_small_terminal_are_refused_politely_not_crashed`. |
| Paste (bracketed and plain) | done | `input::paste_bytes` (5 unit tests incl. end-marker injection); compat: one line, several lines, into bash with bracketed paste |
| Cleanup (processes, terminal state) | done | compat: shell exit, killed shell, pane close stops foreground program, quit leaves no processes, quit restores alternate screen / cursor / bracketed paste |
| Panic safety | done, untested path | terminal is restored before a panic message prints; an emulator panic is caught per pane. The catch path has no test (no known input triggers it now). |
| Scrollback storage | partial | 1,000 lines stored per pane; no way to scroll or browse them yet |
| Copy mode (scrollback browse, search, yank) | planned | next |
| TOML configuration, rebindable keys | planned | after copy mode |
| Daemon attach/detach | planned | after configuration |
| Mouse support | planned | |
| Text selection | untested | The program does not capture the mouse, so the outer terminal's own selection should work. Needs a real terminal to confirm. |

## Compatibility results (Arch Linux container, release binary, run 38150651160)

Environment: Arch Linux rolling, glibc 2.44, vim 9.2.1164, less 710, htop 3.5.3, bash 5.3.20, TERM=xterm-256color.
The same 21 tests (5 end-to-end, 16 compatibility) also pass on Ubuntu in the normal CI run.

| Program / behavior | Result | What the test does |
|---|---|---|
| vim | pass | insert text, Left arrow, Esc, `:wq`; file content checked; shell screen restored afterwards |
| vim | pass | `G`, `gg`, Page Down through a 200-line file |
| less | pass | page forward, jump to end, `q`; shell usable afterwards |
| htop | pass | draws with colored backgrounds, function-key bar follows a resize, `q` returns to the shell |
| bash with bracketed paste | pass | pasted text arrives intact |
| Paste, one line and several lines | pass | shell runs the pasted commands |
| Unicode | pass | euro sign, wide CJK characters, combining accent |
| Colors | pass | 16-color foreground and 24-bit background reach the screen |
| Alternate screen, cursor, paste mode on quit | pass | all restored |
| Shell exits / is killed / pane closed | pass | only that pane closes; foreground programs do not outlive it |
| 8 panes on 80x24, 9th refused | pass | caused a crash before the fix below |
| Terminal shrunk to 3x5 and 2x6, then restored | pass | |

### Failures found and fixed this session
1. **Crash with 11 panes on 80x24** (`vt100` overflow on 1-row panes). Fixed: minimum emulator size 2x2, pane refusal, per-pane panic recovery.
2. **Panic messages were invisible** (printed on the alternate screen, then discarded). Fixed: terminal restored before the message.
3. Four test-design mistakes (markers wrapping in narrow panes, a long bash prompt scrolling output away) found by running on Arch. These were test bugs, not product bugs.

### Cannot be tested in CI
Real terminal programs on your machine (alacritty, st, xterm), dwm key conflicts (Alt+digit), mouse and outer-terminal selection,
other shells (zsh, fish), neovim, tmux inside a pane, kitty-protocol programs, slow-disk startup, and anything on the A6-9225.

## Later releases (deferred, outside the 10-day scope)

Master-stack and scrolling layouts, command palette, launcher, session rail, settings page, popups, agent state / Inbox /
fleets / MCP server, tmux shim, program-status protocol, hosts and remote sessions, SSH server mode, web terminal,
tape scripting, layout templates, kitty graphics / sixel passthrough, themes, hooks, aggregate view, multifocus.
