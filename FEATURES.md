# Feature inventory

Source of truth for upstream: `Gaurav-Gosain/tuios` v0.9.2 (Go, MIT), README feature list.
Status values: **done** (code + passing tests), **partial**, **planned**, **deferred** (outside agreed scope).
"done" means the logic is tested; it does not mean the user-facing feature is wired up end to end.

## Release 1: core multiplexer (agreed scope)

| Feature | Status | Evidence / note |
|---|---|---|
| BSP tiling layout (spiral, longest-side, alternate, smart-split, preselect, rotate, swap, equalize) | done | `src/layout/bsp.rs`, 21 unit tests. In the app: new panes use the default spiral scheme; rotate (`r`) and equalize (`=`) are bound. Preselect and swap exist in the library but have no key yet. |
| 9 workspaces (isolated trees, focus, move window) | done (move-window has no key yet) | `src/workspace.rs` 8 tests; e2e `workspaces_switch_and_stay_isolated` |
| Pane process spawning (PTY) | done | `src/pane.rs` (portable-pty); e2e `shell_runs_a_command_and_exit_closes_the_app` |
| Terminal emulation | partial | `vt100` crate: colors, attributes, cursor, wide chars, alternate screen, application cursor, bracketed paste. No mouse reporting, no kitty/sixel graphics, scrollback is stored (1000 lines) but cannot be browsed yet. |
| Rendering to the real terminal | done (basic) | `src/render.rs` diff renderer (10 tests) + `src/app.rs`; e2e confirms output reaches a terminal emulator. Borders are plain line characters; titles are not drawn. |
| Resize handling | done | e2e `terminal_resize_reaches_the_shell` (shell reports 29x100 after resize) |
| Modal keybindings (Terminal / Window mode, Ctrl+B prefix) | done (fixed keys) | `src/modal.rs` 9 tests; e2e `panes_modes_and_quit`. Keys follow upstream defaults except where marked in the source (prefix+Esc, prefix+digit). Not rebindable until config exists. |
| Create / focus / close panes | done | e2e `panes_modes_and_quit`. Focus is next/previous/Nth; directional (h/j/k/l-style) focus is not implemented. |
| Bracketed paste (with end-marker stripping) | done, untested | code in `src/app.rs`; no test yet |
| Copy mode (scrollback, search, yank) | planned | |
| Configuration (TOML file, rebindable keys) | planned | `toml` 1.x is MIT OR Apache-2.0 |
| Daemon attach/detach (persistent sessions) | planned | needs a design for a background process and a socket; not started |
| Mouse support | planned | |

"done" means the behavior is covered by passing tests in CI on Linux. None of it has been run by hand on the target laptop.

## Later releases (deferred, not in the 10-day scope)

Master-stack and scrolling layouts, command palette, launcher, session rail/sidebar, settings page, popups,
agent state / Inbox / fleets / grants / MCP server, tmux shim, program-status protocol (OSC 7501),
hosts and remote sessions, SSH server mode, web terminal (`tuios-web`), tape scripting and recording,
layout templates, kitty graphics / sixel / kitty keyboard protocol passthrough, themes, hooks,
aggregate view, multifocus, session resurrection.
