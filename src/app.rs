//! The multiplexer: owns the workspaces, panes and modal state, reacts to
//! events, and draws a frame.

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use crate::event::{AppEvent, InputEvent};
use crate::input::{encode, Key};
use crate::layout::{Rect, WindowId};
use crate::modal::{Command, Modal, Mode, Outcome};
use crate::pane::Pane;
use crate::render::{render_diff, Cell, Color, Frame, BOLD, INVERSE, ITALIC, UNDERLINE};
use crate::workspace::{Workspaces, WORKSPACE_COUNT};

/// Cells reserved between tiled panes for the separator line.
const GAP: i32 = 1;

/// Pasted text must not be able to end bracketed-paste mode early.
const PASTE_END: &str = "\x1b[201~";

fn to_u16(v: i32) -> u16 {
    u16::try_from(v.max(0)).unwrap_or(u16::MAX)
}

fn map_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Default,
        vt100::Color::Idx(i) => Color::Idx(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

pub struct App {
    ws: Workspaces,
    panes: HashMap<WindowId, Pane>,
    modal: Modal,
    /// Terminal size as (columns, rows).
    size: (u16, u16),
    tx: Sender<AppEvent>,
    shell: String,
    prev: Option<Frame>,
    notice: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new(cols: u16, rows: u16, shell: String, tx: Sender<AppEvent>) -> Self {
        Self {
            ws: Workspaces::new(),
            panes: HashMap::new(),
            modal: Modal::new(Key::ctrl('b')),
            size: (cols, rows),
            tx,
            shell,
            prev: None,
            notice: None,
            quit: false,
        }
    }

    /// Area for panes: everything except the bottom status row.
    fn bounds(&self) -> Rect {
        Rect::new(
            0,
            0,
            i32::from(self.size.0),
            i32::from(self.size.1.saturating_sub(1)),
        )
    }

    pub fn handle(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Input(InputEvent::Key(k)) => self.on_key(k),
            AppEvent::Input(InputEvent::Paste(text)) => self.on_paste(&text),
            AppEvent::Input(InputEvent::Resize(w, h)) => {
                self.size = (w, h);
                self.prev = None;
                self.relayout();
            }
            AppEvent::Input(InputEvent::Eof) => self.quit = true,
            AppEvent::Output(id, bytes) => {
                if let Some(p) = self.panes.get_mut(&id) {
                    p.process(&bytes);
                }
            }
            AppEvent::Exit(id) => {
                if self.panes.contains_key(&id) {
                    self.close_pane(id);
                }
            }
        }
    }

    fn on_key(&mut self, key: Key) {
        match self.modal.handle(key) {
            Outcome::Forward(k) => {
                if let Some(p) = self.focused_pane() {
                    let bytes = encode(k, p.app_cursor());
                    p.write(&bytes);
                }
            }
            Outcome::Run(cmd) => self.run(cmd),
            Outcome::Ignore => {}
        }
    }

    fn on_paste(&mut self, text: &str) {
        if self.modal.mode() != Mode::Terminal {
            return;
        }
        if let Some(p) = self.focused_pane() {
            if p.bracketed_paste() {
                p.write(b"\x1b[200~");
                p.write(text.replace(PASTE_END, "").as_bytes());
                p.write(PASTE_END.as_bytes());
            } else {
                p.write(text.replace("\r\n", "\r").replace('\n', "\r").as_bytes());
            }
        }
    }

    fn focused_pane(&mut self) -> Option<&mut Pane> {
        let id = self.ws.focused()?;
        self.panes.get_mut(&id)
    }

    fn run(&mut self, cmd: Command) {
        match cmd {
            Command::NewPane => self.open_pane(),
            Command::ClosePane => {
                if let Some(id) = self.ws.focused() {
                    self.close_pane(id);
                }
            }
            Command::FocusNext => {
                self.ws.cycle_focus(true);
            }
            Command::FocusPrev => {
                self.ws.cycle_focus(false);
            }
            Command::FocusIndex(n) => {
                let ids = self.ws.windows_in(self.ws.current());
                if let Some(&id) = n.checked_sub(1).and_then(|i| ids.get(i)) {
                    self.ws.focus(id);
                }
            }
            Command::Workspace(n) => {
                if self.ws.switch_to(n) {
                    self.relayout();
                    self.sync_mode();
                }
            }
            Command::RotateSplit => {
                self.ws.rotate_focused();
                self.relayout();
            }
            Command::Equalize => {
                self.ws.equalize_current();
                self.relayout();
            }
            Command::Quit => self.quit = true,
        }
    }

    /// Open a shell pane in the active workspace and focus it.
    pub fn open_pane(&mut self) {
        let bounds = self.bounds();
        let id = self.ws.new_window(bounds, GAP);
        let rect = self
            .ws
            .layout(bounds, GAP)
            .get(&id)
            .copied()
            .unwrap_or(bounds);
        match Pane::spawn(
            id,
            to_u16(rect.h),
            to_u16(rect.w),
            &self.shell,
            self.tx.clone(),
        ) {
            Ok(p) => {
                self.panes.insert(id, p);
                self.notice = None;
            }
            Err(e) => {
                self.ws.close_window(id);
                self.notice = Some(format!("cannot start {}: {e}", self.shell));
            }
        }
        self.relayout();
        self.sync_mode();
    }

    fn close_pane(&mut self, id: WindowId) {
        if self.ws.close_window(id) {
            self.panes.remove(&id);
        }
        self.relayout();
        self.sync_mode();
        if self.ws.window_count() == 0 {
            self.quit = true;
        }
    }

    /// Resize every pane of the active workspace to its current rectangle.
    fn relayout(&mut self) {
        let layout = self.ws.layout(self.bounds(), GAP);
        for (id, r) in layout {
            if let Some(p) = self.panes.get_mut(&id) {
                p.resize(to_u16(r.h), to_u16(r.w));
            }
        }
    }

    /// With nothing focused there is nowhere for keys to go, so use Window
    /// mode (where `n` makes a pane).
    fn sync_mode(&mut self) {
        if self.ws.focused().is_none() {
            self.modal.enter_window_mode();
        }
    }

    /// Draw the current state. Returns the terminal bytes to write, or
    /// `None` when nothing changed since the last frame.
    pub fn render(&mut self) -> Option<String> {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return None;
        }
        let mut frame = Frame::new(w, h);
        let layout = self.ws.layout(self.bounds(), GAP);
        let focused = self.ws.focused();
        let width = usize::from(w);
        let mut covered = vec![false; width * usize::from(h)];

        for (id, r) in &layout {
            let Some(pane) = self.panes.get(id) else {
                continue;
            };
            let screen = pane.screen();
            for row in 0..r.h {
                for col in 0..r.w {
                    let (x, y) = (to_u16(r.x + col), to_u16(r.y + row));
                    let cell =
                        screen
                            .cell(to_u16(row), to_u16(col))
                            .map_or_else(Cell::blank, |c| {
                                let text = if c.is_wide_continuation() {
                                    String::new()
                                } else if c.contents().is_empty() {
                                    " ".to_string()
                                } else {
                                    c.contents().to_string()
                                };
                                let mut attrs = 0;
                                if c.bold() {
                                    attrs |= BOLD;
                                }
                                if c.italic() {
                                    attrs |= ITALIC;
                                }
                                if c.underline() {
                                    attrs |= UNDERLINE;
                                }
                                if c.inverse() {
                                    attrs |= INVERSE;
                                }
                                Cell {
                                    text,
                                    fg: map_color(c.fgcolor()),
                                    bg: map_color(c.bgcolor()),
                                    attrs,
                                }
                            });
                    frame.set(x, y, cell);
                    if let Some(slot) = covered.get_mut(usize::from(y) * width + usize::from(x)) {
                        *slot = true;
                    }
                }
            }
            if Some(*id) == focused && self.modal.mode() == Mode::Terminal && !screen.hide_cursor()
            {
                let (cy, cx) = screen.cursor_position();
                if i32::from(cy) < r.h && i32::from(cx) < r.w {
                    frame.cursor = Some((to_u16(r.x + i32::from(cx)), to_u16(r.y + i32::from(cy))));
                }
            }
        }

        if layout.is_empty() {
            let msg = "No panes. Press n to open one.";
            let x = w.saturating_sub(u16::try_from(msg.len()).unwrap_or(0)) / 2;
            frame.put_str(
                x,
                h.saturating_sub(1) / 2,
                msg,
                Color::Idx(8),
                Color::Default,
                0,
            );
        } else {
            self.draw_separators(&mut frame, &covered, layout.get(&focused.unwrap_or(0)));
        }
        self.draw_status(&mut frame, &layout);

        if self.prev.as_ref() == Some(&frame) {
            return None;
        }
        let out = render_diff(self.prev.as_ref(), &frame);
        self.prev = Some(frame);
        Some(out)
    }

    fn draw_separators(&self, frame: &mut Frame, covered: &[bool], focus: Option<&Rect>) {
        let (w, h) = self.size;
        let width = usize::from(w);
        for y in 0..h.saturating_sub(1) {
            for x in 0..w {
                let i = usize::from(y) * width + usize::from(x);
                if covered[i] {
                    continue;
                }
                let beside = (x > 0 && covered[i - 1]) || (x + 1 < w && covered[i + 1]);
                let (xi, yi) = (i32::from(x), i32::from(y));
                let hot = focus.is_some_and(|r| {
                    xi + 1 >= r.x && xi <= r.x + r.w && yi + 1 >= r.y && yi <= r.y + r.h
                });
                frame.set(
                    x,
                    y,
                    Cell {
                        text: if beside { "│" } else { "─" }.to_string(),
                        fg: Color::Idx(if hot { 6 } else { 8 }),
                        bg: Color::Default,
                        attrs: 0,
                    },
                );
            }
        }
    }

    fn draw_status(&self, frame: &mut Frame, layout: &HashMap<WindowId, Rect>) {
        let (w, h) = self.size;
        let y = h - 1;
        for x in 0..w {
            frame.set(
                x,
                y,
                Cell {
                    text: " ".to_string(),
                    fg: Color::Default,
                    bg: Color::Default,
                    attrs: INVERSE,
                },
            );
        }
        let mode = if self.modal.prefix_armed() {
            "PREFIX"
        } else {
            match self.modal.mode() {
                Mode::Terminal => "TERMINAL",
                Mode::Window => "WINDOW",
            }
        };
        let ids = self.ws.windows_in(self.ws.current());
        let index = self
            .ws
            .focused()
            .and_then(|f| ids.iter().position(|i| *i == f))
            .map_or(0, |p| p + 1);
        let hint = match (&self.notice, self.modal.mode()) {
            (Some(n), _) => n.clone(),
            (None, Mode::Terminal) => "Ctrl+B then Esc: window mode".to_string(),
            (None, Mode::Window) => "n new  x close  tab next  i terminal  q quit".to_string(),
        };
        let line = format!(
            " {mode} | ws {}/{} | pane {index}/{} | {hint}",
            self.ws.current() + 1,
            WORKSPACE_COUNT,
            layout.len().max(ids.len()),
        );
        frame.put_str(0, y, &line, Color::Default, Color::Default, INVERSE);
    }
}
