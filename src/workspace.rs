//! Workspaces: nine independent tiling trees, each with its own focus.
//!
//! Mirrors upstream's nine-workspace model. A window lives in exactly one
//! workspace; switching workspaces never touches another workspace's layout.

use std::collections::HashMap;

use crate::layout::{BspTree, Rect, WindowId};

/// Number of workspaces, as in upstream.
pub const WORKSPACE_COUNT: usize = 9;

#[derive(Debug, Default)]
struct Workspace {
    tree: BspTree,
    focused: Option<WindowId>,
}

/// All workspaces plus the window-to-workspace index.
#[derive(Debug)]
pub struct Workspaces {
    spaces: Vec<Workspace>,
    current: usize,
    home: HashMap<WindowId, usize>,
    next_id: WindowId,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspaces {
    pub fn new() -> Self {
        Self {
            spaces: (0..WORKSPACE_COUNT).map(|_| Workspace::default()).collect(),
            current: 0,
            home: HashMap::new(),
            next_id: 1,
        }
    }

    /// Zero-based index of the active workspace.
    pub fn current(&self) -> usize {
        self.current
    }

    pub fn window_count(&self) -> usize {
        self.home.len()
    }

    /// Windows in `ws` in tree order, or empty for an invalid index.
    pub fn windows_in(&self, ws: usize) -> Vec<WindowId> {
        self.spaces
            .get(ws)
            .map(|w| w.tree.window_ids())
            .unwrap_or_default()
    }

    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        self.home.get(&id).copied()
    }

    /// Focused window of the active workspace.
    pub fn focused(&self) -> Option<WindowId> {
        self.spaces[self.current].focused
    }

    /// Switch to `ws`. Returns false (and changes nothing) if out of range.
    pub fn switch_to(&mut self, ws: usize) -> bool {
        if ws >= WORKSPACE_COUNT {
            return false;
        }
        self.current = ws;
        true
    }

    /// Open a new window in the active workspace and focus it.
    pub fn new_window(&mut self, bounds: Rect, gap: i32) -> WindowId {
        let id = self.next_id;
        self.next_id += 1;
        let ws = &mut self.spaces[self.current];
        ws.tree
            .insert(id, ws.focused.unwrap_or(0), None, 0.5, bounds, gap);
        ws.focused = Some(id);
        self.home.insert(id, self.current);
        id
    }

    /// Close a window. Focus moves to the window now sharing its slot, or
    /// the first remaining one. Returns false if the id is unknown.
    pub fn close_window(&mut self, id: WindowId) -> bool {
        let Some(ws_idx) = self.home.remove(&id) else {
            return false;
        };
        let ws = &mut self.spaces[ws_idx];
        let before = ws.tree.window_ids();
        ws.tree.remove(id);
        if ws.focused == Some(id) {
            let pos = before.iter().position(|w| *w == id).unwrap_or(0);
            let after = ws.tree.window_ids();
            ws.focused = after.get(pos.saturating_sub(1)).or(after.first()).copied();
        }
        true
    }

    /// Focus a specific window, switching workspace if it lives elsewhere.
    pub fn focus(&mut self, id: WindowId) -> bool {
        let Some(&ws) = self.home.get(&id) else {
            return false;
        };
        self.current = ws;
        self.spaces[ws].focused = Some(id);
        true
    }

    /// Focus the next (`forward`) or previous window in the active
    /// workspace, wrapping around.
    pub fn cycle_focus(&mut self, forward: bool) -> Option<WindowId> {
        let ws = &mut self.spaces[self.current];
        let ids = ws.tree.window_ids();
        if ids.is_empty() {
            return None;
        }
        let pos = ws
            .focused
            .and_then(|f| ids.iter().position(|w| *w == f))
            .unwrap_or(0);
        let n = ids.len();
        let next = if forward {
            (pos + 1) % n
        } else {
            (pos + n - 1) % n
        };
        ws.focused = Some(ids[next]);
        ws.focused
    }

    /// Move a window to workspace `to`. The active workspace and focus in
    /// the destination are unchanged unless the destination was empty.
    pub fn move_window(&mut self, id: WindowId, to: usize, bounds: Rect, gap: i32) -> bool {
        if to >= WORKSPACE_COUNT {
            return false;
        }
        let Some(&from) = self.home.get(&id) else {
            return false;
        };
        if from == to {
            return true;
        }
        self.close_window(id);
        let dest = &mut self.spaces[to];
        dest.tree
            .insert(id, dest.focused.unwrap_or(0), None, 0.5, bounds, gap);
        if dest.focused.is_none() {
            dest.focused = Some(id);
        }
        self.home.insert(id, to);
        true
    }

    /// Flip the split holding the focused window of the active workspace.
    pub fn rotate_focused(&mut self) {
        let ws = &mut self.spaces[self.current];
        if let Some(f) = ws.focused {
            ws.tree.rotate_split(f);
        }
    }

    /// Reset every split of the active workspace to an even ratio.
    pub fn equalize_current(&mut self) {
        self.spaces[self.current].tree.equalize();
    }

    /// Rectangles for the active workspace.
    pub fn layout(&self, bounds: Rect, gap: i32) -> HashMap<WindowId, Rect> {
        self.spaces[self.current].tree.layout(bounds, gap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Rect = Rect::new(0, 0, 120, 40);

    #[test]
    fn new_windows_get_unique_ids_and_focus() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        let b = w.new_window(B, 0);
        assert_ne!(a, b);
        assert_eq!(w.focused(), Some(b));
        assert_eq!(w.window_count(), 2);
    }

    #[test]
    fn workspaces_are_isolated() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        assert!(w.switch_to(3));
        assert_eq!(w.focused(), None);
        let b = w.new_window(B, 0);
        assert_eq!(w.windows_in(0), vec![a]);
        assert_eq!(w.windows_in(3), vec![b]);
        assert_eq!(w.layout(B, 0).len(), 1);
    }

    #[test]
    fn switch_out_of_range_is_rejected() {
        let mut w = Workspaces::new();
        assert!(!w.switch_to(WORKSPACE_COUNT));
        assert_eq!(w.current(), 0);
    }

    #[test]
    fn closing_focused_window_moves_focus() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        let b = w.new_window(B, 0);
        assert!(w.close_window(b));
        assert_eq!(w.focused(), Some(a));
        assert!(w.close_window(a));
        assert_eq!(w.focused(), None);
        assert!(!w.close_window(a));
    }

    #[test]
    fn cycle_focus_wraps_both_ways() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        let b = w.new_window(B, 0);
        let c = w.new_window(B, 0);
        assert_eq!(w.cycle_focus(true), Some(a));
        assert_eq!(w.cycle_focus(false), Some(c));
        assert_eq!(w.cycle_focus(false), Some(b));
        assert_eq!(Workspaces::new().cycle_focus(true), None);
    }

    #[test]
    fn move_window_changes_home_and_keeps_layouts_valid() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        let b = w.new_window(B, 0);
        assert!(w.move_window(a, 4, B, 0));
        assert_eq!(w.workspace_of(a), Some(4));
        assert_eq!(w.windows_in(0), vec![b]);
        assert_eq!(w.windows_in(4), vec![a]);
        assert_eq!(w.layout(B, 0)[&b], B);
        assert!(!w.move_window(a, 99, B, 0));
        assert!(!w.move_window(777, 1, B, 0));
    }

    #[test]
    fn rotate_and_equalize_change_the_active_layout() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        let b = w.new_window(B, 0);
        let before = w.layout(B, 0);
        assert_eq!(before[&a].y, before[&b].y, "side by side at first");
        w.rotate_focused();
        let after = w.layout(B, 0);
        assert_eq!(after[&a].x, after[&b].x, "stacked after rotate");
        w.equalize_current();
        assert_eq!(w.layout(B, 0).len(), 2);
        // No focus, no panic.
        Workspaces::new().rotate_focused();
    }

    #[test]
    fn focus_on_other_workspace_switches_to_it() {
        let mut w = Workspaces::new();
        let a = w.new_window(B, 0);
        w.switch_to(2);
        w.new_window(B, 0);
        assert!(w.focus(a));
        assert_eq!(w.current(), 0);
        assert!(!w.focus(555));
    }
}
