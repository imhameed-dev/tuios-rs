# Dependencies and licenses

Facts below were read from the crates.io registry on 2026-10-10. Transitive dependencies have **not** been audited yet.

| Crate | Version | License | Last release | Use |
|---|---|---|---|---|
| portable-pty | 0.9.0 | MIT | 2025-02 | open PTYs, spawn shells (part of the wezterm project) |
| vt100 | 0.16.2 | MIT | 2025-07 | terminal emulation (escape-sequence parser, screen grid) |
| crossterm | 0.29.0 | MIT | 2025-04 | raw mode, key and resize events |
| toml | 1.1.8 | MIT OR Apache-2.0 | 2026-10 | planned: configuration |

Upstream TUIOS is MIT-licensed; this rewrite is MIT.

Open items: audit transitive dependencies (`cargo-deny` in CI), and note portable-pty's last release is eight months old.
