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
