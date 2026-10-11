# tuios-rs on Arch Linux

An early build of a Rust rewrite of TUIOS, a terminal multiplexer (several
shells in tiled panes inside one terminal). **It is not ready for daily use
yet**: there is no scrollback browsing, no copy mode, no configuration file
and no detach/reattach. See FEATURES.md in the project for the exact status.

This build is made on Ubuntu and tested on every release inside an
`archlinux:latest` container (the CI log says which Arch package versions).
Arch's glibc is newer than the build machine's, so it runs there. It has not
been run on your hardware by the project.

## Download and run

1. Download `tuios-rs-linux-x86_64.tar.gz` from the Assets list of
   <https://github.com/imhameed-dev/tuios-rs/releases/tag/dev-latest>
2. In a terminal (for example alacritty), run these three lines:

       cd ~/Downloads
       tar xzf tuios-rs-linux-x86_64.tar.gz
       ./tuios-rs-linux-x86_64/tuios-rs

3. To start it from a launcher such as dmenu, install it once, then launch
   it inside your terminal program:

       sudo install -m 755 ~/Downloads/tuios-rs-linux-x86_64/tuios-rs /usr/local/bin/
       alacritty -e tuios-rs

Optional integrity check: put `tuios-rs-linux-x86_64.tar.gz.sha256` next to the
tarball and run `sha256sum -c tuios-rs-linux-x86_64.tar.gz.sha256`.

Third-party license texts are in the `licenses/` folder of the tarball.

## Keys

You start in Terminal mode: everything you type goes to the shell, except the
prefix key Ctrl+B.

| Keys | What it does |
|---|---|
| Ctrl+B then Esc | switch to Window mode |
| Ctrl+B then c | new pane |
| Ctrl+B then x | close the pane |
| Ctrl+B then n / p | next / previous pane |
| Ctrl+B then 1-9 | switch workspace |
| Alt+1-9 | switch workspace (may clash with dwm shortcuts if Alt is your dwm key) |
| Ctrl+B then Ctrl+B | send a literal Ctrl+B |

In Window mode (status bar says WINDOW): `n` new pane, `x` or `w` close,
`Tab` / `Shift+Tab` focus, `1`-`9` focus the Nth pane, `r` rotate the split,
`=` equalize, `i` or `Enter` back to Terminal mode, `q` quit.

To leave: Ctrl+B, Esc, then q (or close every shell with `exit`).

## Troubleshooting

- **"cannot execute" or "Permission denied"**: run `chmod +x tuios-rs-linux-x86_64/tuios-rs` first.
- **Strange characters or colors**: start it inside a terminal that supports 256 colors
  (alacritty, st, xterm, kitty). `echo $TERM` should print something like `xterm-256color`.
- **Alt+number does nothing or switches a dwm tag**: dwm takes the key first. Use Ctrl+B then the number.
- **The terminal is left garbled after a crash**: type `reset` and press Enter.
- **Nothing happens when you press `q`**: `q` only quits in Window mode. Press Ctrl+B then Esc first.
- Report problems at <https://github.com/imhameed-dev/tuios-rs/issues> with what you pressed and what you saw.
