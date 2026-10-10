# Feature inventory

Source of truth for upstream: `Gaurav-Gosain/tuios` v0.9.2 (Go, MIT), README feature list.
Status values: **done** (code + passing tests), **partial**, **planned**, **deferred** (outside agreed scope).
"done" means the logic is tested; it does not mean the user-facing feature is wired up end to end.

## Release 1: core multiplexer (agreed scope)

| Feature | Status | Evidence / note |
|---|---|---|
| BSP tiling layout (spiral, longest-side, alternate, smart-split, preselect, rotate, swap, equalize) | done (algorithm only) | `src/layout/bsp.rs`, 21 tests, CI green |
| 9 workspaces (isolated trees, focus, move window) | done (model only) | `src/workspace.rs`, 7 tests |
| Pane process spawning (PTY) | planned | needs a PTY crate; license check pending |
| Terminal emulation (VT parser, screen grid) | planned | upstream `internal/vt` ~20k lines; evaluate a maintained Rust VT crate |
| Rendering to the real terminal | planned | crossterm or similar; license check pending |
| Modal keybindings (window-management vs terminal mode, prefix key) | planned | |
| Copy mode (scrollback, search, yank) | planned | |
| Configuration (TOML file, rebindable keys) | planned | |
| Daemon attach/detach (persistent sessions) | planned | |
| Mouse support | planned | |

## Later releases (deferred, not in the 10-day scope)

Master-stack and scrolling layouts, command palette, launcher, session rail/sidebar, settings page, popups,
agent state / Inbox / fleets / grants / MCP server, tmux shim, program-status protocol (OSC 7501),
hosts and remote sessions, SSH server mode, web terminal (`tuios-web`), tape scripting and recording,
layout templates, kitty graphics / sixel / kitty keyboard protocol passthrough, themes, hooks,
aggregate view, multifocus, session resurrection.
