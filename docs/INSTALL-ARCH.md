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

The status bar shows the current mode, workspace, active pane and the keys
that apply right now. The active pane is bright; the other panes are faint.

**Terminal mode** (status bar says TERMINAL; the default). Everything you type
goes to the shell in the active pane, except the prefix key Ctrl+B:

| Keys | What it does |
|---|---|
| Ctrl+B then c | new pane |
| Ctrl+B then Tab | next pane, stays in Terminal mode |
| Ctrl+B then Shift+Tab or p | previous pane |
| Ctrl+B then n | next pane (same as Tab) |
| Ctrl+B then x | close the active pane |
| Ctrl+B then Esc | switch to Window mode |
| Ctrl+B then 1-9 | switch workspace |
| Alt+1-9 | switch workspace (may clash with dwm shortcuts if Alt is your dwm key) |
| Ctrl+B then Ctrl+B | send a literal Ctrl+B to the shell |

After Ctrl+B the bar says PREFIX until you press the next key.

**Window mode** (status bar says WINDOW). Keys are commands and nothing reaches
the shell:

| Keys | What it does |
|---|---|
| Tab / Shift+Tab | next / previous pane |
| 1-9 | focus the Nth pane of this workspace |
| n | new pane |
| x or w | close the active pane |
| r / = | rotate the split / equalize sizes |
| i or Enter | back to Terminal mode |
| q | quit |

To quit: Ctrl+B, Esc, then q (or close every shell with `exit`).

## Troubleshooting

- **"cannot execute" or "Permission denied"**: run `chmod +x tuios-rs-linux-x86_64/tuios-rs` first.
- **Strange characters or colors**: start it inside a terminal that supports 256 colors
  (alacritty, st, xterm, kitty). `echo $TERM` should print something like `xterm-256color`.
- **Alt+number does nothing or switches a dwm tag**: dwm takes the key first. Use Ctrl+B then the number.
- **The terminal is left garbled after a crash**: type `reset` and press Enter.
- **Nothing happens when you press `q`**: `q` only quits in Window mode. Press Ctrl+B then Esc first.
- **Tab does not switch panes**: the active pane is the bright one; the other panes are drawn faint, and the status bar shows `pane N/M`. Press Ctrl+B and then Tab (no need to enter Window mode); Shift+Tab or `p` goes back. In Window mode, plain Tab / Shift+Tab switch. If it still does nothing, record what the program received:
  `TUIOS_KEYLOG=/tmp/keys.txt ./tuios-rs`, press Ctrl+B, Esc, Tab, quit, then send me `/tmp/keys.txt`.
  The log shows each key and what it did. Text typed into a shell is never written to it.
- Report problems at <https://github.com/imhameed-dev/tuios-rs/issues> with what you pressed and what you saw.
