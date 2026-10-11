# Dependencies and licenses

Audit of the build of commit e6f57f1 (Linux x86_64). The list comes from `cargo tree` in CI; release dates were read
from the crates.io registry on 2026-10-11.

## Summary

- Direct dependencies: 3 (`crossterm`, `portable-pty`, `vt100`). Total third-party crates in the Linux build: **45**.
- **No copyleft:** 45 of 45 are permissive (MIT, Apache-2.0, BSD-2-Clause, Unlicense, Unicode-3.0 for data tables).
  License expressions seen: MIT OR Apache-2.0 (26), MIT (11), MIT/Apache-2.0 (2), Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT (2), and one each of Unlicense OR MIT, BSD-2-Clause OR Apache-2.0, Apache-2.0/MIT,
  Apache-2.0 OR MIT, (MIT OR Apache-2.0) AND Unicode-3.0.
- Our own code has **no `unsafe`** (`#![forbid(unsafe_code)]` in the library and the binary). The dependencies do
  contain unsafe code (terminal, PTY and signal handling are system calls); that was not audited.
- tuios-rs is MIT; upstream TUIOS is MIT (Gaurav Gosain), credited in `LICENSE`.

## License obligations

MIT, BSD-2-Clause and Apache-2.0 all allow redistribution of the binary if the copyright notice and license text
travel with it. The release tarball does this: `LICENSE` plus a `licenses/` folder holding every dependency's license
and NOTICE files (53 folders, which includes some crates not used on Linux). Apache-2.0 additionally asks that NOTICE
files be kept, which the packaging script copies. No source-code disclosure duty applies.

## The three direct dependencies

| Crate | Version | License | Last release | Role |
|---|---|---|---|---|
| portable-pty | 0.9.0 | MIT | 2025-02-11 | open PTYs and spawn shells (part of the wezterm project) |
| vt100 | 0.16.2 | MIT | 2025-07-12 | terminal emulator: escape sequences to a screen grid |
| crossterm | 0.29.0 | MIT | 2025-04-05 | raw mode, key and resize events |

## Maintenance observations

| Crate | In build | Newest | Newest released | Note |
|---|---|---|---|---|
| portable-pty | 0.9.0 | 0.9.0 | 2025-02-11 | Current, but about 20 months old. If it stalls, the small PTY layer could be replaced. |
| filedescriptor | 0.8.3 | 0.8.3 | 2025-02-11 | Same project as portable-pty. |
| nix | 0.28.0 | 0.31.3 | 2026-05-11 | Three minor versions behind; only compiled in, not chosen by us. |
| thiserror | 1.0.69 | 2.0.21 | 2026-09-23 | Old major version, fine for now. |
| downcast-rs | 1.2.1 | 2.0.2 | 2025-09-01 | Old major version. |
| vte | 0.15.0 | 0.15.0 | 2025-02-02 | Current. Used by vt100. |
| shell-words | 1.1.1 | 1.1.1 | 2025-12-10 | Current. |
| serial2 | 0.2.38 | 0.2.38 | 2026-07-31 | Current. Serial-port support we do not use. |
| vt100 | 0.16.2 | 0.16.2 | 2025-07-12 | Current. **Has a bug we hit:** see below. |

Not traced: which crate pulls in which, and whether `serial2` / `shell-words` can be switched off in portable-pty.
They stay: removing a useful dependency to shorten the list is not worth the risk.

## Bug found in vt100 0.16.2

`Grid::col_wrap` (`src/grid.rs`, line 683) subtracts with `prev_pos.row -= scrolled`, which overflows when text wraps
on a 1-row screen; a wide character on a 1-column screen has the same problem (`self.size.cols - width`). In a debug
build that is a panic; in a release build it would silently wrap. tuios-rs never gives the emulator less than 2x2 and
catches any other emulator panic per pane, resetting that pane's screen. Worth reporting upstream (not done yet).

## Follow-ups

- Run `cargo-deny` (licenses, advisories, duplicate versions) in CI. Not done: advisories were not checked.
- Report the vt100 overflow upstream.
