//! Binary space partition tiling.
//!
//! Port of upstream `internal/layout/bsp.go`. The tree lives in an arena
//! (`Vec` of nodes addressed by index) so it needs no `unsafe`, no `Rc`, and
//! no parent pointers that can dangle.

use std::collections::HashMap;
use std::str::FromStr;

/// Identifier of a pane/window inside a layout.
pub type WindowId = u32;

/// How many times taller than wide a character cell is. Rect sizes are in
/// cells but a reader judges shape in pixels, so heights are scaled by this
/// before being compared with widths (same constant as upstream).
const CELL_ASPECT: i32 = 2;

/// A rectangle in cell coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// True when the two rectangles share at least one cell.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

/// Direction a node divides its space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    /// Children sit left | right, divided by a vertical line.
    Vertical,
    /// Children sit top / bottom, divided by a horizontal line.
    Horizontal,
}

impl Split {
    fn flipped(self) -> Self {
        match self {
            Split::Vertical => Split::Horizontal,
            Split::Horizontal => Split::Vertical,
        }
    }
}

/// Which side of the focused pane the next pane should appear on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preselect {
    Left,
    Right,
    Up,
    Down,
}

/// How a new pane picks its split direction when none is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AutoScheme {
    /// Split across the longest side of the target pane.
    LongestSide,
    /// Alternate vertical/horizontal by total split count.
    Alternate,
    /// bspwm-style spiral: alternate by the depth of the pane being split.
    #[default]
    Spiral,
    /// Choose from the focused pane's aspect ratio, falling back to depth.
    SmartSplit,
}

impl AutoScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            AutoScheme::LongestSide => "longest_side",
            AutoScheme::Alternate => "alternate",
            AutoScheme::Spiral => "spiral",
            AutoScheme::SmartSplit => "smart_split",
        }
    }
}

impl FromStr for AutoScheme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "longest_side" => Ok(AutoScheme::LongestSide),
            "alternate" => Ok(AutoScheme::Alternate),
            "spiral" => Ok(AutoScheme::Spiral),
            "smart_split" => Ok(AutoScheme::SmartSplit),
            _ => Err(()),
        }
    }
}

type NodeId = usize;

#[derive(Clone, Debug)]
enum Kind {
    Leaf(WindowId),
    Split {
        dir: Split,
        /// Position of the divider, 0.0..1.0 of the node's extent.
        ratio: f64,
        first: NodeId,
        second: NodeId,
    },
}

#[derive(Clone, Debug)]
struct Node {
    parent: Option<NodeId>,
    kind: Kind,
}

/// A BSP tiling tree for one workspace.
#[derive(Clone, Debug)]
pub struct BspTree {
    nodes: Vec<Option<Node>>,
    free: Vec<NodeId>,
    root: Option<NodeId>,
    index: HashMap<WindowId, NodeId>,
    /// Scheme used when an insert does not name a direction.
    pub scheme: AutoScheme,
    /// Ratio used for new splits when the caller passes an invalid one.
    pub default_ratio: f64,
}

impl Default for BspTree {
    fn default() -> Self {
        Self::new()
    }
}

impl BspTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            free: Vec::new(),
            root: None,
            index: HashMap::new(),
            scheme: AutoScheme::default(),
            default_ratio: 0.5,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn window_count(&self) -> usize {
        self.index.len()
    }

    pub fn has_window(&self, id: WindowId) -> bool {
        self.index.contains_key(&id)
    }

    /// Window ids in tree order (left/top subtree first).
    pub fn window_ids(&self) -> Vec<WindowId> {
        let mut out = Vec::with_capacity(self.index.len());
        if let Some(root) = self.root {
            self.collect_ids(root, &mut out);
        }
        out
    }

    fn collect_ids(&self, id: NodeId, out: &mut Vec<WindowId>) {
        match &self.node(id).kind {
            Kind::Leaf(w) => out.push(*w),
            Kind::Split { first, second, .. } => {
                self.collect_ids(*first, out);
                self.collect_ids(*second, out);
            }
        }
    }

    fn node(&self, id: NodeId) -> &Node {
        self.nodes[id].as_ref().expect("live node id")
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes[id].as_mut().expect("live node id")
    }

    fn alloc(&mut self, node: Node) -> NodeId {
        if let Some(id) = self.free.pop() {
            self.nodes[id] = Some(node);
            id
        } else {
            self.nodes.push(Some(node));
            self.nodes.len() - 1
        }
    }

    fn release(&mut self, id: NodeId) {
        self.nodes[id] = None;
        self.free.push(id);
    }

    /// Number of ancestors above `id` (the root has depth 0).
    fn depth(&self, id: NodeId) -> usize {
        let mut d = 0;
        let mut cur = id;
        while let Some(p) = self.node(cur).parent {
            d += 1;
            cur = p;
        }
        d
    }

    fn any_leaf(&self) -> Option<NodeId> {
        let mut cur = self.root?;
        loop {
            match &self.node(cur).kind {
                Kind::Leaf(_) => return Some(cur),
                Kind::Split { first, .. } => cur = *first,
            }
        }
    }

    fn count_splits(&self) -> usize {
        self.nodes
            .iter()
            .flatten()
            .filter(|n| matches!(n.kind, Kind::Split { .. }))
            .count()
    }

    /// Divide `bounds` between the two children of a split.
    ///
    /// `gap` cells between the children are reserved for a drawn separator.
    /// This is the single definition of the split model.
    pub fn child_bounds(dir: Split, ratio: f64, bounds: Rect, gap: i32) -> (Rect, Rect) {
        match dir {
            Split::Vertical => {
                let mut split_x = bounds.x + (f64::from(bounds.w) * ratio) as i32;
                if gap > 0 {
                    split_x = (bounds.x + 1).max(split_x.min(bounds.x + bounds.w - 1 - gap));
                }
                (
                    Rect::new(bounds.x, bounds.y, split_x - bounds.x, bounds.h),
                    Rect::new(
                        split_x + gap,
                        bounds.y,
                        bounds.x + bounds.w - split_x - gap,
                        bounds.h,
                    ),
                )
            }
            Split::Horizontal => {
                let mut split_y = bounds.y + (f64::from(bounds.h) * ratio) as i32;
                if gap > 0 {
                    split_y = (bounds.y + 1).max(split_y.min(bounds.y + bounds.h - 1 - gap));
                }
                (
                    Rect::new(bounds.x, bounds.y, bounds.w, split_y - bounds.y),
                    Rect::new(
                        bounds.x,
                        split_y + gap,
                        bounds.w,
                        bounds.y + bounds.h - split_y - gap,
                    ),
                )
            }
        }
    }

    /// The rectangle `target` occupies when the tree is laid out in `bounds`.
    fn node_bounds(&self, target: NodeId, bounds: Rect, gap: i32) -> Option<Rect> {
        // Path from target up to (but excluding) the root, then walk down.
        let mut path = Vec::new();
        let mut cur = target;
        while let Some(p) = self.node(cur).parent {
            path.push(cur);
            cur = p;
        }
        if Some(cur) != self.root {
            return None;
        }
        let mut node = cur;
        let mut rect = bounds;
        while let Some(step) = path.pop() {
            if let Kind::Split {
                dir,
                ratio,
                first,
                second,
            } = self.node(node).kind
            {
                let (a, b) = Self::child_bounds(dir, ratio, rect, gap);
                if step == first {
                    rect = a;
                } else if step == second {
                    rect = b;
                } else {
                    return None;
                }
                node = step;
            } else {
                return None;
            }
        }
        Some(rect)
    }

    fn auto_split(&self, target: NodeId, bounds: Rect, gap: i32) -> Split {
        match self.scheme {
            AutoScheme::LongestSide => {
                let r = self.node_bounds(target, bounds, gap).unwrap_or(bounds);
                if r.w >= r.h * CELL_ASPECT {
                    Split::Vertical
                } else {
                    Split::Horizontal
                }
            }
            AutoScheme::Alternate => {
                if self.count_splits().is_multiple_of(2) {
                    Split::Vertical
                } else {
                    Split::Horizontal
                }
            }
            AutoScheme::Spiral => {
                let mut vertical = self.depth(target).is_multiple_of(2);
                if bounds.w < bounds.h * CELL_ASPECT {
                    vertical = !vertical;
                }
                if vertical {
                    Split::Vertical
                } else {
                    Split::Horizontal
                }
            }
            AutoScheme::SmartSplit => {
                let r = self.node_bounds(target, bounds, gap).unwrap_or(bounds);
                let (w, h) = (r.w, r.h * CELL_ASPECT);
                if w > h * 2 {
                    Split::Vertical
                } else if h > w {
                    Split::Horizontal
                } else if self.depth(target).is_multiple_of(2) {
                    Split::Vertical
                } else {
                    Split::Horizontal
                }
            }
        }
    }

    /// Turn the leaf `target` into a split holding the old window and `new`.
    fn split_leaf(
        &mut self,
        target: NodeId,
        new: WindowId,
        dir: Split,
        ratio: f64,
        new_first: bool,
    ) {
        let old = match self.node(target).kind {
            Kind::Leaf(w) => w,
            Kind::Split { .. } => return,
        };
        let old_leaf = self.alloc(Node {
            parent: Some(target),
            kind: Kind::Leaf(old),
        });
        let new_leaf = self.alloc(Node {
            parent: Some(target),
            kind: Kind::Leaf(new),
        });
        let (first, second) = if new_first {
            (new_leaf, old_leaf)
        } else {
            (old_leaf, new_leaf)
        };
        self.node_mut(target).kind = Kind::Split {
            dir,
            ratio,
            first,
            second,
        };
        self.index.insert(old, old_leaf);
        self.index.insert(new, new_leaf);
    }

    /// Add `new` next to `focused`. The old pane stays first, the new one
    /// second. `direction` of `None` picks a split with the tree's scheme; a
    /// `ratio` outside (0, 1) uses `default_ratio`. Duplicates are ignored.
    pub fn insert(
        &mut self,
        new: WindowId,
        focused: WindowId,
        direction: Option<Split>,
        ratio: f64,
        bounds: Rect,
        gap: i32,
    ) {
        if self.has_window(new) {
            return;
        }
        let Some(target) = self
            .index
            .get(&focused)
            .copied()
            .or_else(|| self.any_leaf())
        else {
            // Empty tree: the first window becomes the root.
            let id = self.alloc(Node {
                parent: None,
                kind: Kind::Leaf(new),
            });
            self.root = Some(id);
            self.index.insert(new, id);
            return;
        };
        let dir = direction.unwrap_or_else(|| self.auto_split(target, bounds, gap));
        let ratio = if ratio > 0.0 && ratio < 1.0 {
            ratio
        } else {
            self.default_ratio
        };
        self.split_leaf(target, new, dir, ratio, false);
    }

    /// Add `new` on the `side` of `focused`, using the default ratio.
    /// With no preselection this behaves like [`BspTree::insert`] with auto split.
    pub fn insert_preselected(
        &mut self,
        new: WindowId,
        focused: WindowId,
        side: Option<Preselect>,
        bounds: Rect,
        gap: i32,
    ) {
        let Some(side) = side else {
            let ratio = self.default_ratio;
            self.insert(new, focused, None, ratio, bounds, gap);
            return;
        };
        if self.has_window(new) {
            return;
        }
        let (dir, new_first) = match side {
            Preselect::Left => (Split::Vertical, true),
            Preselect::Right => (Split::Vertical, false),
            Preselect::Up => (Split::Horizontal, true),
            Preselect::Down => (Split::Horizontal, false),
        };
        let Some(target) = self
            .index
            .get(&focused)
            .copied()
            .or_else(|| self.any_leaf())
        else {
            let id = self.alloc(Node {
                parent: None,
                kind: Kind::Leaf(new),
            });
            self.root = Some(id);
            self.index.insert(new, id);
            return;
        };
        let ratio = self.default_ratio;
        self.split_leaf(target, new, dir, ratio, new_first);
    }

    /// Remove a window; its sibling takes over the parent's space.
    pub fn remove(&mut self, id: WindowId) {
        let Some(node) = self.index.remove(&id) else {
            return;
        };
        let Some(parent) = self.node(node).parent else {
            self.root = None;
            self.release(node);
            return;
        };
        let sibling = match self.node(parent).kind {
            Kind::Split { first, second, .. } => {
                if first == node {
                    second
                } else {
                    first
                }
            }
            Kind::Leaf(_) => return,
        };
        let grand = self.node(parent).parent;
        self.node_mut(sibling).parent = grand;
        match grand {
            None => self.root = Some(sibling),
            Some(g) => {
                if let Kind::Split { first, second, .. } = &mut self.node_mut(g).kind {
                    if *first == parent {
                        *first = sibling;
                    } else if *second == parent {
                        *second = sibling;
                    }
                }
            }
        }
        self.release(node);
        self.release(parent);
    }

    /// Compute every window's rectangle inside `bounds`.
    ///
    /// Tiles never overlap: when space runs short they shrink, with a floor
    /// of one cell.
    pub fn layout(&self, bounds: Rect, gap: i32) -> HashMap<WindowId, Rect> {
        let mut out = HashMap::with_capacity(self.index.len());
        if let Some(root) = self.root {
            self.layout_node(root, bounds, gap, &mut out);
        }
        for r in out.values_mut() {
            r.x = bounds.x.max(r.x.min(bounds.x + bounds.w - r.w));
            r.y = bounds.y.max(r.y.min(bounds.y + bounds.h - r.h));
        }
        out
    }

    fn layout_node(&self, id: NodeId, bounds: Rect, gap: i32, out: &mut HashMap<WindowId, Rect>) {
        match self.node(id).kind {
            Kind::Leaf(w) => {
                out.insert(
                    w,
                    Rect::new(bounds.x, bounds.y, bounds.w.max(1), bounds.h.max(1)),
                );
            }
            Kind::Split {
                dir,
                ratio,
                first,
                second,
            } => {
                let (a, b) = Self::child_bounds(dir, ratio, bounds, gap);
                self.layout_node(first, a, gap, out);
                self.layout_node(second, b, gap, out);
            }
        }
    }

    /// Flip the split that directly contains `id` between vertical and horizontal.
    pub fn rotate_split(&mut self, id: WindowId) {
        let Some(&node) = self.index.get(&id) else {
            return;
        };
        let Some(parent) = self.node(node).parent else {
            return;
        };
        if let Kind::Split { dir, .. } = &mut self.node_mut(parent).kind {
            *dir = dir.flipped();
        }
    }

    /// Exchange the positions of two windows.
    pub fn swap(&mut self, a: WindowId, b: WindowId) {
        let (Some(&na), Some(&nb)) = (self.index.get(&a), self.index.get(&b)) else {
            return;
        };
        self.node_mut(na).kind = Kind::Leaf(b);
        self.node_mut(nb).kind = Kind::Leaf(a);
        self.index.insert(a, nb);
        self.index.insert(b, na);
    }

    /// Reset every split to an even 0.5 ratio.
    pub fn equalize(&mut self) {
        for node in self.nodes.iter_mut().flatten() {
            if let Kind::Split { ratio, .. } = &mut node.kind {
                *ratio = 0.5;
            }
        }
    }

    /// Ratio of the split that directly contains `id`, if any.
    pub fn parent_ratio(&self, id: WindowId) -> Option<f64> {
        let node = *self.index.get(&id)?;
        let parent = self.node(node).parent?;
        match self.node(parent).kind {
            Kind::Split { ratio, .. } => Some(ratio),
            Kind::Leaf(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Rect = Rect::new(0, 0, 120, 40);

    fn build(n: u32, bounds: Rect, gap: i32) -> BspTree {
        let mut t = BspTree::new();
        t.scheme = AutoScheme::Spiral;
        let mut last = 0;
        for i in 1..=n {
            t.insert(i, last, None, 0.5, bounds, gap);
            last = i;
        }
        t
    }

    fn assert_no_overlap(t: &BspTree, bounds: Rect, gap: i32, n: u32) {
        let rects = t.layout(bounds, gap);
        assert_eq!(rects.len(), n as usize, "n={n}");
        let list: Vec<_> = rects.iter().collect();
        for (id, r) in &list {
            assert!(r.w >= 1 && r.h >= 1, "n={n} window {id} size {r:?}");
            assert!(
                r.x >= bounds.x
                    && r.y >= bounds.y
                    && r.x + r.w <= bounds.x + bounds.w
                    && r.y + r.h <= bounds.y + bounds.h,
                "n={n} window {id} {r:?} escapes {bounds:?}"
            );
        }
        for (i, (ia, ra)) in list.iter().enumerate() {
            for (ib, rb) in &list[i + 1..] {
                assert!(!ra.overlaps(rb), "n={n}: {ia} {ra:?} overlaps {ib} {rb:?}");
            }
        }
    }

    #[test]
    fn layout_never_overlaps() {
        for n in [2, 3, 4, 6, 8, 12] {
            assert_no_overlap(&build(n, B, 0), B, 0, n);
        }
    }

    #[test]
    fn layout_never_overlaps_with_separator_gap() {
        let b = Rect::new(0, 0, 80, 24);
        for n in [2, 3, 4, 5, 6] {
            assert_no_overlap(&build(n, b, 1), b, 1, n);
        }
    }

    #[test]
    fn first_window_fills_bounds() {
        let t = build(1, B, 0);
        assert_eq!(t.layout(B, 0)[&1], B);
    }

    #[test]
    fn spiral_second_window_splits_vertically() {
        let t = build(2, B, 0);
        let r = t.layout(B, 0);
        assert_eq!(r[&1], Rect::new(0, 0, 60, 40));
        assert_eq!(r[&2], Rect::new(60, 0, 60, 40));
    }

    #[test]
    fn spiral_third_window_splits_newest_horizontally() {
        let t = build(3, B, 0);
        let r = t.layout(B, 0);
        assert_eq!(r[&1], Rect::new(0, 0, 60, 40));
        assert_eq!(r[&2], Rect::new(60, 0, 60, 20));
        assert_eq!(r[&3], Rect::new(60, 20, 60, 20));
    }

    #[test]
    fn spiral_on_tall_terminal_starts_horizontal() {
        let tall = Rect::new(0, 0, 51, 37);
        let t = build(2, tall, 0);
        let r = t.layout(tall, 0);
        assert_eq!(r[&1].w, 51);
        assert_eq!(r[&2].w, 51);
        assert_eq!(r[&1].h + r[&2].h, 37);
    }

    #[test]
    fn gap_reserves_separator_cells() {
        let b = Rect::new(0, 0, 81, 24);
        let t = build(2, b, 1);
        let r = t.layout(b, 1);
        assert_eq!(r[&1], Rect::new(0, 0, 40, 24));
        assert_eq!(r[&2], Rect::new(41, 0, 40, 24));
    }

    #[test]
    fn longest_side_follows_shape() {
        let wide = Rect::new(0, 0, 100, 10);
        let mut t = BspTree::new();
        t.scheme = AutoScheme::LongestSide;
        t.insert(1, 0, None, 0.5, wide, 0);
        t.insert(2, 1, None, 0.5, wide, 0);
        let r = t.layout(wide, 0);
        assert_eq!(r[&1].h, 10, "wide area splits side by side");
        assert_eq!(r[&2].h, 10);

        let tall = Rect::new(0, 0, 20, 40);
        let mut t = BspTree::new();
        t.scheme = AutoScheme::LongestSide;
        t.insert(1, 0, None, 0.5, tall, 0);
        t.insert(2, 1, None, 0.5, tall, 0);
        let r = t.layout(tall, 0);
        assert_eq!(r[&1].w, 20, "tall area stacks");
    }

    #[test]
    fn alternate_uses_total_split_count() {
        let mut t = BspTree::new();
        t.scheme = AutoScheme::Alternate;
        t.insert(1, 0, None, 0.5, B, 0);
        t.insert(2, 1, None, 0.5, B, 0); // 0 splits -> vertical
        t.insert(3, 1, None, 0.5, B, 0); // 1 split -> horizontal, splitting window 1
        let r = t.layout(B, 0);
        assert_eq!(r[&1], Rect::new(0, 0, 60, 20));
        assert_eq!(r[&3], Rect::new(0, 20, 60, 20));
        assert_eq!(r[&2], Rect::new(60, 0, 60, 40));
    }

    #[test]
    fn explicit_direction_and_ratio_are_used() {
        let mut t = BspTree::new();
        t.insert(1, 0, None, 0.5, B, 0);
        t.insert(2, 1, Some(Split::Horizontal), 0.25, B, 0);
        let r = t.layout(B, 0);
        assert_eq!(r[&1], Rect::new(0, 0, 120, 10));
        assert_eq!(r[&2], Rect::new(0, 10, 120, 30));
    }

    #[test]
    fn invalid_ratio_falls_back_to_default() {
        let mut t = BspTree::new();
        t.insert(1, 0, None, 0.5, B, 0);
        t.insert(2, 1, Some(Split::Vertical), 7.0, B, 0);
        assert_eq!(t.parent_ratio(2), Some(0.5));
    }

    #[test]
    fn remove_gives_space_to_sibling() {
        let mut t = build(3, B, 0);
        t.remove(3);
        let r = t.layout(B, 0);
        assert_eq!(r.len(), 2);
        assert_eq!(r[&2], Rect::new(60, 0, 60, 40));
        t.remove(2);
        assert_eq!(t.layout(B, 0)[&1], B);
        t.remove(1);
        assert!(t.is_empty());
        assert!(t.layout(B, 0).is_empty());
    }

    #[test]
    fn remove_unknown_and_duplicate_insert_are_noops() {
        let mut t = build(2, B, 0);
        t.remove(99);
        t.insert(2, 1, None, 0.5, B, 0);
        assert_eq!(t.window_count(), 2);
        assert_eq!(t.window_ids(), vec![1, 2]);
    }

    #[test]
    fn insert_with_unknown_focus_still_places_window() {
        let mut t = build(1, B, 0);
        t.insert(2, 42, None, 0.5, B, 0);
        assert_eq!(t.window_count(), 2);
        assert_no_overlap(&t, B, 0, 2);
    }

    #[test]
    fn preselection_places_new_window_on_chosen_side() {
        type Check = fn(Rect, Rect) -> bool;
        let cases: [(Preselect, Check); 4] = [
            (Preselect::Left, |n, o| n.x < o.x),
            (Preselect::Right, |n, o| n.x > o.x),
            (Preselect::Up, |n, o| n.y < o.y),
            (Preselect::Down, |n, o| n.y > o.y),
        ];
        for (side, check) in cases {
            let mut t = BspTree::new();
            t.insert(1, 0, None, 0.5, B, 0);
            t.insert_preselected(2, 1, Some(side), B, 0);
            let r = t.layout(B, 0);
            assert!(check(r[&2], r[&1]), "{side:?}: {r:?}");
        }
    }

    #[test]
    fn rotate_split_flips_axis() {
        let mut t = build(2, B, 0);
        t.rotate_split(2);
        let r = t.layout(B, 0);
        assert_eq!(r[&1], Rect::new(0, 0, 120, 20));
        assert_eq!(r[&2], Rect::new(0, 20, 120, 20));
    }

    #[test]
    fn swap_exchanges_positions() {
        let mut t = build(2, B, 0);
        t.swap(1, 2);
        let r = t.layout(B, 0);
        assert_eq!(r[&2], Rect::new(0, 0, 60, 40));
        assert_eq!(r[&1], Rect::new(60, 0, 60, 40));
        assert_eq!(t.window_ids(), vec![2, 1]);
    }

    #[test]
    fn equalize_resets_ratios() {
        let mut t = BspTree::new();
        t.insert(1, 0, None, 0.5, B, 0);
        t.insert(2, 1, Some(Split::Vertical), 0.8, B, 0);
        assert_eq!(t.parent_ratio(1), Some(0.8));
        t.equalize();
        assert_eq!(t.parent_ratio(1), Some(0.5));
    }

    #[test]
    fn arena_slots_are_reused_after_removal() {
        let mut t = build(4, B, 0);
        let before = t.nodes.len();
        t.remove(4);
        t.insert(5, 3, None, 0.5, B, 0);
        assert_eq!(t.nodes.len(), before);
        assert_no_overlap(&t, B, 0, 4);
    }

    #[test]
    fn many_windows_stay_consistent() {
        let mut t = BspTree::new();
        let mut last = 0;
        for i in 1..=30 {
            t.insert(i, last, None, 0.5, B, 0);
            last = i;
        }
        for i in (1..=30).step_by(3) {
            t.remove(i);
        }
        assert_eq!(t.window_count(), 20);
        let r = t.layout(B, 0);
        assert_eq!(r.len(), 20);
        for id in t.window_ids() {
            assert!(r.contains_key(&id));
        }
    }

    #[test]
    fn scheme_names_round_trip() {
        for s in [
            AutoScheme::LongestSide,
            AutoScheme::Alternate,
            AutoScheme::Spiral,
            AutoScheme::SmartSplit,
        ] {
            assert_eq!(s.as_str().parse::<AutoScheme>(), Ok(s));
        }
        assert!("bogus".parse::<AutoScheme>().is_err());
    }
}
