//! Cell frames and the ANSI needed to turn one frame into the next.
//!
//! Std-only: panes are copied into a [`Frame`], and [`render_diff`]
//! emits only the cells that changed since the previous frame.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const UNDERLINE: u8 = 4;
pub const INVERSE: u8 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Grapheme shown in the cell. Empty marks the right half of a wide
    /// character, which the terminal draws together with the left half.
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub attrs: u8,
}

impl Cell {
    pub fn blank() -> Self {
        Self::plain(" ")
    }

    pub fn plain(text: &str) -> Self {
        Self {
            text: text.to_string(),
            fg: Color::Default,
            bg: Color::Default,
            attrs: 0,
        }
    }

    fn style(&self) -> (Color, Color, u8) {
        (self.fg, self.bg, self.attrs)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub w: u16,
    pub h: u16,
    cells: Vec<Cell>,
    /// Where to leave the visible cursor, if any.
    pub cursor: Option<(u16, u16)>,
}

impl Frame {
    pub fn new(w: u16, h: u16) -> Self {
        Self {
            w,
            h,
            cells: vec![Cell::blank(); usize::from(w) * usize::from(h)],
            cursor: None,
        }
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.w) + usize::from(x)
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        (x < self.w && y < self.h).then(|| &self.cells[self.idx(x, y)])
    }

    /// Set a cell; coordinates outside the frame are ignored.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = cell;
        }
    }

    /// Write ASCII/Unicode text left to right with one style, one char per
    /// cell. Meant for single-width text such as the status line.
    pub fn put_str(&mut self, x: u16, y: u16, s: &str, fg: Color, bg: Color, attrs: u8) {
        for (i, ch) in s.chars().enumerate() {
            let Ok(dx) = u16::try_from(i) else { break };
            let mut buf = [0u8; 4];
            self.set(
                x.saturating_add(dx),
                y,
                Cell {
                    text: ch.encode_utf8(&mut buf).to_string(),
                    fg,
                    bg,
                    attrs,
                },
            );
        }
    }
}

fn sgr(out: &mut String, (fg, bg, attrs): (Color, Color, u8)) {
    out.push_str("\x1b[0");
    if attrs & BOLD != 0 {
        out.push_str(";1");
    }
    if attrs & ITALIC != 0 {
        out.push_str(";3");
    }
    if attrs & UNDERLINE != 0 {
        out.push_str(";4");
    }
    if attrs & INVERSE != 0 {
        out.push_str(";7");
    }
    match fg {
        Color::Default => {}
        Color::Idx(n) => {
            let _ = write!(out, ";38;5;{n}");
        }
        Color::Rgb(r, g, b) => {
            let _ = write!(out, ";38;2;{r};{g};{b}");
        }
    }
    match bg {
        Color::Default => {}
        Color::Idx(n) => {
            let _ = write!(out, ";48;5;{n}");
        }
        Color::Rgb(r, g, b) => {
            let _ = write!(out, ";48;2;{r};{g};{b}");
        }
    }
    out.push('m');
}

/// ANSI that turns what `prev` drew into `next`. With no `prev`, or a
/// different size, the screen is cleared and fully redrawn.
pub fn render_diff(prev: Option<&Frame>, next: &Frame) -> String {
    let mut out = String::new();
    // Synchronized output keeps partial frames from showing; hide the
    // cursor while drawing.
    out.push_str("\x1b[?2026h\x1b[?25l");
    let prev = prev.filter(|p| p.w == next.w && p.h == next.h);
    if prev.is_none() {
        out.push_str("\x1b[0m\x1b[2J");
    }
    let mut style: Option<(Color, Color, u8)> = None;
    let mut at: Option<(u16, u16)> = None;
    for y in 0..next.h {
        let mut x = 0;
        while x < next.w {
            let cell = &next.cells[next.idx(x, y)];
            if cell.text.is_empty() {
                x += 1;
                continue;
            }
            let changed = prev.is_none_or(|p| p.cells[p.idx(x, y)] != *cell);
            if changed {
                if at != Some((x, y)) {
                    let _ = write!(out, "\x1b[{};{}H", y + 1, x + 1);
                }
                if style != Some(cell.style()) {
                    sgr(&mut out, cell.style());
                    style = Some(cell.style());
                }
                out.push_str(&cell.text);
                let wide = x + 1 < next.w && next.cells[next.idx(x + 1, y)].text.is_empty();
                let adv = if wide { 2 } else { 1 };
                at = Some((x + adv, y));
            }
            x += 1;
        }
    }
    out.push_str("\x1b[0m");
    if let Some((cx, cy)) = next.cursor {
        let _ = write!(out, "\x1b[{};{}H\x1b[?25h", cy + 1, cx + 1);
    }
    out.push_str("\x1b[?2026l");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_frame(w: u16, h: u16, s: &str) -> Frame {
        let mut f = Frame::new(w, h);
        f.put_str(0, 0, s, Color::Default, Color::Default, 0);
        f
    }

    #[test]
    fn first_render_clears_and_draws_everything() {
        let f = text_frame(5, 2, "hi");
        let out = render_diff(None, &f);
        assert!(out.contains("\x1b[2J"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn identical_frames_emit_no_cell_output() {
        let f = text_frame(5, 2, "hi");
        let out = render_diff(Some(&f), &f);
        // Only the fixed wrapper: begin sync, hide cursor, reset, end sync.
        assert_eq!(out, "\x1b[?2026h\x1b[?25l\x1b[0m\x1b[?2026l");
    }

    #[test]
    fn only_changed_cell_is_redrawn_at_its_position() {
        let a = text_frame(5, 2, "hello");
        let mut b = a.clone();
        b.set(3, 1, Cell::plain("Z"));
        let out = render_diff(Some(&a), &b);
        assert!(out.contains("\x1b[2;4H\x1b[0mZ"), "{out:?}");
        assert!(!out.contains("hello"));
    }

    #[test]
    fn adjacent_changes_share_one_cursor_move() {
        let a = Frame::new(6, 1);
        let b = text_frame(6, 1, "abc");
        let out = render_diff(Some(&a), &b);
        assert_eq!(out.matches("\x1b[1;1H").count(), 1);
        assert!(out.contains("abc"));
        assert_eq!(out.matches('H').count(), 1);
    }

    #[test]
    fn resize_forces_full_redraw() {
        let a = text_frame(3, 1, "abc");
        let b = text_frame(4, 1, "abc");
        assert!(render_diff(Some(&a), &b).contains("\x1b[2J"));
    }

    #[test]
    fn colors_and_attributes_become_sgr() {
        let mut f = Frame::new(3, 1);
        f.set(
            0,
            0,
            Cell {
                text: "x".into(),
                fg: Color::Idx(1),
                bg: Color::Rgb(1, 2, 3),
                attrs: BOLD | INVERSE,
            },
        );
        let out = render_diff(None, &f);
        assert!(out.contains("\x1b[0;1;7;38;5;1;48;2;1;2;3mx"), "{out:?}");
    }

    #[test]
    fn style_is_not_repeated_for_same_style_neighbours() {
        let f = text_frame(4, 1, "abcd");
        let out = render_diff(None, &f);
        // One SGR for the run (the trailing reset is "\x1b[0m").
        assert_eq!(out.matches("\x1b[0m").count(), 3, "{out:?}");
    }

    #[test]
    fn wide_character_continuation_cell_is_skipped() {
        let mut f = Frame::new(4, 1);
        f.set(0, 0, Cell::plain("日"));
        f.set(1, 0, Cell::plain(""));
        f.set(2, 0, Cell::plain("x"));
        let out = render_diff(None, &f);
        assert!(out.contains("日x"), "{out:?}");
        assert_eq!(out.matches("\x1b[1;").count(), 1, "{out:?}");
    }

    #[test]
    fn cursor_is_shown_only_when_requested() {
        let mut f = Frame::new(3, 2);
        assert!(!render_diff(None, &f).contains("\x1b[?25h"));
        f.cursor = Some((2, 1));
        let out = render_diff(None, &f);
        assert!(out.contains("\x1b[2;3H\x1b[?25h"), "{out:?}");
    }

    #[test]
    fn out_of_range_writes_are_ignored() {
        let mut f = Frame::new(2, 1);
        f.set(5, 5, Cell::plain("x"));
        f.put_str(1, 0, "abc", Color::Default, Color::Default, 0);
        assert_eq!(f.get(1, 0).map(|c| c.text.as_str()), Some("a"));
        assert!(f.get(2, 0).is_none());
    }
}
