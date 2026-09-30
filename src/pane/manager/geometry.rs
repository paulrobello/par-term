//! Min-size-aware layout geometry for `PaneManager` (UX.md PN3, PN4, A3, A4).
//!
//! Every ratio change here respects `pane_min_size`: a split's ratio may
//! only move inside the range where both children, with their own nested
//! ratios as they are, keep every leaf at least the minimum size. The
//! invariant the operations keep is "no operation turns a feasible layout
//! infeasible" — a window shrunk below the minimum cannot be helped, but
//! nothing par-term does to the tree makes it worse.

use super::PaneManager;
use crate::pane::types::split_child_bounds;
use crate::pane::types::{NavigationDirection, Pane, PaneBounds, PaneId, PaneNode, SplitDirection};
use std::collections::HashMap;

/// Tolerance for float comparisons of pixel extents.
const EPS: f32 = 0.01;

/// The ratio clamp used before renderer metrics are known.
const LEGACY_RATIO_RANGE: (f32, f32) = (0.1, 0.9);

/// A layout preset (UX.md A4, tmux `select-layout` names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    /// Every pane side by side.
    EvenHorizontal,
    /// Every pane stacked.
    EvenVertical,
    /// The focused pane on the left, the rest stacked on the right.
    MainLeft,
    /// The focused pane on top, the rest side by side below.
    MainTop,
    /// A grid.
    Tiled,
}

impl LayoutPreset {
    /// Presets in `cycle_layout` order.
    pub const ALL: [LayoutPreset; 5] = [
        LayoutPreset::EvenHorizontal,
        LayoutPreset::EvenVertical,
        LayoutPreset::MainLeft,
        LayoutPreset::MainTop,
        LayoutPreset::Tiled,
    ];

    /// The `layout:<name>` action suffix.
    pub fn name(self) -> &'static str {
        match self {
            LayoutPreset::EvenHorizontal => "even-horizontal",
            LayoutPreset::EvenVertical => "even-vertical",
            LayoutPreset::MainLeft => "main-left",
            LayoutPreset::MainTop => "main-top",
            LayoutPreset::Tiled => "tiled",
        }
    }

    /// Parse a `layout:<name>` suffix.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }
}

/// How far one keyboard resize moves a divider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeStep {
    /// A fraction of the enclosing split (`pane_resize_step` / 100).
    Fraction(f32),
    /// Physical pixels (one cell for resize mode's Shift+arrow).
    Pixels(f32),
}

impl ResizeStep {
    fn signed(self, sign: f32) -> Self {
        match self {
            ResizeStep::Fraction(f) => ResizeStep::Fraction(f * sign),
            ResizeStep::Pixels(px) => ResizeStep::Pixels(px * sign),
        }
    }
}

/// Whether `direction` splits along the x axis (side by side).
fn is_x(direction: SplitDirection) -> bool {
    direction == SplitDirection::Vertical
}

/// Extent of `bounds` along a split direction's axis.
fn extent(bounds: PaneBounds, direction: SplitDirection) -> f32 {
    if is_x(direction) {
        bounds.width
    } else {
        bounds.height
    }
}

/// The smallest extent along `axis` that `node` can take while every leaf
/// stays at least `min` along that axis, with its nested ratios as they are.
fn required_extent(node: &PaneNode, axis: SplitDirection, min: f32, divider: f32) -> f32 {
    match node {
        PaneNode::Leaf(_) => min,
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let first_req = required_extent(first, axis, min, divider);
            let second_req = required_extent(second, axis, min, divider);
            if *direction == axis {
                let via_first = if *ratio > 0.0 {
                    first_req / ratio + divider
                } else {
                    f32::INFINITY
                };
                let via_second = if *ratio < 1.0 {
                    second_req / (1.0 - ratio) + divider
                } else {
                    f32::INFINITY
                };
                via_first.max(via_second)
            } else {
                first_req.max(second_req)
            }
        }
    }
}

/// Number of leaves in `node`.
fn leaf_count(node: &PaneNode) -> usize {
    node.pane_count()
}

impl PaneManager {
    /// Set the minimum pane size from `pane_min_size` (cells), the cell
    /// size, and the per-pane pixels that hold no cells (padding, scrollbar,
    /// title bar), all physical (PN4). A pane at the minimum then DISPLAYS
    /// at least `min_cells` columns and rows, the way the render gather
    /// computes its grid: `floor((extent - overhead) / cell)`.
    pub fn set_min_pane_size(&mut self, min_cells: usize, cell: (f32, f32), overhead: (f32, f32)) {
        let (cell_width, cell_height) = cell;
        // A ratio clamped to the minimum lands within float error of it;
        // the margin keeps `floor` from reading that as one cell short.
        const MARGIN: f32 = 0.5;
        self.min_pane_px = (min_cells > 0 && cell_width > 0.0 && cell_height > 0.0).then_some((
            min_cells as f32 * cell_width + overhead.0 + MARGIN,
            min_cells as f32 * cell_height + overhead.1 + MARGIN,
        ));
    }

    /// The minimum pane extent along a split direction's axis, or `None`
    /// before renderer metrics are known.
    fn min_along(&self, direction: SplitDirection) -> Option<f32> {
        self.min_pane_px
            .map(|(w, h)| if is_x(direction) { w } else { h })
    }

    /// The ratios a split may take: both children keep their minimum
    /// extents. `None` when no ratio satisfies both (already infeasible).
    fn ratio_range(
        &self,
        direction: SplitDirection,
        bounds: PaneBounds,
        first: &PaneNode,
        second: &PaneNode,
    ) -> Option<(f32, f32)> {
        let Some(min) = self.min_along(direction) else {
            return Some(LEGACY_RATIO_RANGE);
        };
        let d = self.divider_width;
        let available = extent(bounds, direction) - d;
        if available <= 0.0 {
            return None;
        }
        let lo = required_extent(first, direction, min, d) / available;
        let hi = 1.0 - required_extent(second, direction, min, d) / available;
        if lo > hi + EPS / available {
            return None;
        }
        // At the exact boundary float error can leave lo a hair above hi;
        // collapse the range to its midpoint rather than invert it.
        let (lo, hi) = (lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0));
        Some(if lo <= hi {
            (lo, hi)
        } else {
            let m = (lo + hi) / 2.0;
            (m, m)
        })
    }

    /// Panes smaller than the minimum size, if any (tests and guards).
    pub fn min_size_violations(&self) -> Vec<PaneId> {
        let Some((min_w, min_h)) = self.min_pane_px else {
            return Vec::new();
        };
        self.all_panes()
            .into_iter()
            .filter(|p| p.bounds.width + EPS < min_w || p.bounds.height + EPS < min_h)
            .map(|p| p.id)
            .collect()
    }

    /// Resize with an arrow (PN3): move the divider nearest the pane on the
    /// arrow's axis — its innermost enclosing split of that orientation —
    /// by `step` (a fraction of that split) in the arrow's direction, and
    /// never past the minimum size. Returns whether the divider moved.
    pub fn resize_toward(
        &mut self,
        pane: PaneId,
        direction: NavigationDirection,
        step: f32,
    ) -> bool {
        self.resize_toward_by(pane, direction, ResizeStep::Fraction(step))
    }

    /// [`Self::resize_toward`] with the step as a fraction of the split or
    /// as pixels (resize mode's Shift+arrow moves one cell, A6).
    pub fn resize_toward_by(
        &mut self,
        pane: PaneId,
        direction: NavigationDirection,
        step: ResizeStep,
    ) -> bool {
        self.unzoom();
        let (axis, sign) = match direction {
            NavigationDirection::Left => (SplitDirection::Vertical, -1.0),
            NavigationDirection::Right => (SplitDirection::Vertical, 1.0),
            NavigationDirection::Up => (SplitDirection::Horizontal, -1.0),
            NavigationDirection::Down => (SplitDirection::Horizontal, 1.0),
        };
        let Some(mut root) = self.root.take() else {
            return false;
        };
        // tmux semantics: the divider on the arrow's side of the pane
        // (its right border for Right), else the one on the other side.
        let arrow_side_first = sign > 0.0;
        let step = step.signed(sign);
        let moved = match self.resize_in(
            &mut root,
            self.total_bounds,
            pane,
            axis,
            step,
            Some(arrow_side_first),
        ) {
            Some(moved) => moved,
            None => self
                .resize_in(&mut root, self.total_bounds, pane, axis, step, None)
                .unwrap_or(false),
        };
        self.root = Some(root);
        self.recalculate_bounds();
        moved
    }

    /// `Some(moved)` once the innermost split of `axis` above `pane` is
    /// found, with `pane` on its `first` side when `side_first` is
    /// `Some(true)`, its `second` side for `Some(false)`, either for `None`.
    fn resize_in(
        &self,
        node: &mut PaneNode,
        bounds: PaneBounds,
        pane: PaneId,
        axis: SplitDirection,
        step: ResizeStep,
        side_first: Option<bool>,
    ) -> Option<bool> {
        let PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } = node
        else {
            return None;
        };
        let (first_bounds, second_bounds) =
            split_child_bounds(*direction, *ratio, bounds, self.divider_width);
        let in_first = first.find_pane(pane).is_some();
        if !in_first && second.find_pane(pane).is_none() {
            return None;
        }
        let nested = if in_first {
            self.resize_in(first, first_bounds, pane, axis, step, side_first)
        } else {
            self.resize_in(second, second_bounds, pane, axis, step, side_first)
        };
        if nested.is_some() {
            return nested;
        }
        if *direction != axis || side_first.is_some_and(|want| want != in_first) {
            return None;
        }
        let Some((lo, hi)) = self.ratio_range(*direction, bounds, first, second) else {
            return Some(false);
        };
        let delta = match step {
            ResizeStep::Fraction(f) => f,
            ResizeStep::Pixels(px) => {
                let available = extent(bounds, *direction) - self.divider_width;
                if available <= 0.0 {
                    return Some(false);
                }
                px / available
            }
        };
        // Move toward the arrow only: a ratio already outside the range
        // stays put rather than jumping the other way.
        let old = *ratio;
        let new = if delta > 0.0 {
            old.max((old + delta).min(hi))
        } else {
            old.min((old + delta).max(lo))
        };
        *ratio = new;
        Some((new - old).abs() > f32::EPSILON)
    }

    /// Clamp a dragged divider's ratio into its min-size range.
    pub(super) fn clamp_drag_ratio(
        &self,
        direction: SplitDirection,
        bounds: PaneBounds,
        first: &PaneNode,
        second: &PaneNode,
        wanted: f32,
        current: f32,
    ) -> f32 {
        match self.ratio_range(direction, bounds, first, second) {
            Some((lo, hi)) => wanted.clamp(lo, hi),
            None => current,
        }
    }

    /// Whether the focused pane can be split in `direction` without either
    /// half falling below the minimum size (PN4).
    pub fn can_split(&self, direction: SplitDirection) -> bool {
        let (Some(min), Some(pane)) = (self.min_along(direction), self.focused_pane()) else {
            return true;
        };
        // The zoomed pane's bounds are the whole tab; the split lands in
        // the tree position (the split unzooms first).
        let bounds = match self.zoomed_tree_bounds {
            Some(b) if self.zoomed_pane_id == Some(pane.id) => b,
            _ => pane.bounds,
        };
        extent(bounds, direction) - self.divider_width + EPS >= 2.0 * min
    }

    /// The ratio a new split of the focused pane gets: `wanted`, moved into
    /// the range where both halves keep the minimum size.
    pub(super) fn split_ratio_for(&self, direction: SplitDirection, wanted: f32) -> f32 {
        let (Some(min), Some(pane)) = (self.min_along(direction), self.focused_pane()) else {
            return wanted;
        };
        let available = extent(pane.bounds, direction) - self.divider_width;
        let lo = min / available;
        if lo >= 0.5 {
            return 0.5;
        }
        wanted.clamp(lo, 1.0 - lo)
    }

    /// Give every leaf an equal area (A3): each split's ratio becomes its
    /// first child's share of the leaves. Refused (tree unchanged) when the
    /// result would push a pane below the minimum size of a layout that
    /// was within it. Returns whether the layout was equalized.
    pub fn equalize(&mut self) -> bool {
        self.unzoom();
        self.guard_ratios(|pm| {
            if let Some(root) = pm.root.as_mut() {
                equalize_node(root);
            }
        })
    }

    /// Equalize only the split whose divider is `divider_index` (a
    /// double-click on the divider, PN8).
    pub fn equalize_divider(&mut self, divider_index: usize) -> bool {
        self.guard_ratios(|pm| {
            let mut index = 0;
            if let Some(root) = pm.root.as_mut() {
                equalize_nth_split(root, divider_index, &mut index);
            }
        })
    }

    /// Run a ratio-only change; restore the previous ratios if it made a
    /// feasible layout infeasible. Returns whether the change was kept.
    pub(super) fn guard_ratios(&mut self, change: impl FnOnce(&mut Self)) -> bool {
        let was_feasible = self.min_size_violations().is_empty();
        let before = self.root.as_ref().map(collect_ratios).unwrap_or_default();
        change(self);
        self.recalculate_bounds();
        if was_feasible && !self.min_size_violations().is_empty() {
            if let Some(root) = self.root.as_mut() {
                restore_ratios(root, &mut before.iter().copied());
            }
            self.recalculate_bounds();
            return false;
        }
        true
    }

    /// Rearrange the panes into `preset` (A4) with equal leaf areas. The
    /// focused pane is the main pane of `main-*`. Refused (tree unchanged)
    /// when the preset would push a pane below the minimum size.
    pub fn apply_preset(&mut self, preset: LayoutPreset) -> bool {
        self.unzoom();
        let was_feasible = self.min_size_violations().is_empty();
        let Some(root) = self.root.take() else {
            return false;
        };
        let old_shape = Shape::of(&root);
        let mut order = root.all_pane_ids();
        let mut panes = into_panes(root);
        if let Some(main) = self.focused_pane_id
            && matches!(preset, LayoutPreset::MainLeft | LayoutPreset::MainTop)
            && let Some(pos) = order.iter().position(|id| *id == main)
        {
            let id = order.remove(pos);
            order.insert(0, id);
        }
        let new_shape = Shape::preset(preset, &order);
        self.root = Some(new_shape.build(&mut panes));
        let mut root = self.root.take().expect("just built");
        equalize_node(&mut root);
        self.root = Some(root);
        self.recalculate_bounds();
        if was_feasible && !self.min_size_violations().is_empty() {
            let current = self.root.take().expect("present");
            let mut panes = into_panes(current);
            self.root = Some(old_shape.build(&mut panes));
            self.recalculate_bounds();
            return false;
        }
        self.layout_preset = Some(preset);
        true
    }

    /// The preset last applied, for `cycle_layout`.
    pub fn layout_preset(&self) -> Option<LayoutPreset> {
        self.layout_preset
    }

    /// Insert an already-built pane beside the focused one, the tree half
    /// of [`Self::split`]. `before` places it left of / above the focused
    /// pane (A5). Hands the pane back when the split would violate the
    /// minimum size or nothing is focused.
    pub(crate) fn split_with_pane(
        &mut self,
        new_pane: Pane,
        direction: SplitDirection,
        before: bool,
        focus_new: bool,
        ratio: f32,
    ) -> Result<PaneId, Box<Pane>> {
        let Some(focused_id) = self.focused_pane_id else {
            return Err(Box::new(new_pane));
        };
        self.unzoom();
        if !self.can_split(direction) {
            return Err(Box::new(new_pane));
        }
        // `ratio` is the focused pane's share; `before` mirrors it.
        let focused_share = self.split_ratio_for(direction, ratio);
        let new_id = new_pane.id;
        let Some(root) = self.root.take() else {
            return Err(Box::new(new_pane));
        };
        let (new_root, left_over) = Self::split_node(
            root,
            focused_id,
            direction,
            Some(new_pane),
            if before {
                1.0 - focused_share
            } else {
                focused_share
            },
        );
        self.root = Some(new_root);
        if let Some(pane) = left_over {
            return Err(Box::new(pane));
        }
        if before && let Some(root) = self.root.as_mut() {
            put_new_leaf_first(root, new_id);
        }
        self.recalculate_bounds();
        if focus_new {
            self.set_focus(new_id);
        }
        Ok(new_id)
    }
}

/// Set every split's ratio to its first child's share of the leaves.
fn equalize_node(node: &mut PaneNode) {
    if let PaneNode::Split {
        ratio,
        first,
        second,
        ..
    } = node
    {
        let a = leaf_count(first) as f32;
        let b = leaf_count(second) as f32;
        *ratio = a / (a + b);
        equalize_node(first);
        equalize_node(second);
    }
}

/// Equalize the split whose pre-order divider index is `target`.
fn equalize_nth_split(node: &mut PaneNode, target: usize, index: &mut usize) -> bool {
    if let PaneNode::Split {
        ratio,
        first,
        second,
        ..
    } = node
    {
        if *index == target {
            let a = leaf_count(first) as f32;
            let b = leaf_count(second) as f32;
            *ratio = a / (a + b);
            return true;
        }
        *index += 1;
        return equalize_nth_split(first, target, index)
            || equalize_nth_split(second, target, index);
    }
    false
}

/// Every split ratio in pre-order.
fn collect_ratios(node: &PaneNode) -> Vec<f32> {
    let mut out = Vec::new();
    fn walk(node: &PaneNode, out: &mut Vec<f32>) {
        if let PaneNode::Split {
            ratio,
            first,
            second,
            ..
        } = node
        {
            out.push(*ratio);
            walk(first, out);
            walk(second, out);
        }
    }
    walk(node, &mut out);
    out
}

/// Put back ratios captured by [`collect_ratios`] on the same structure.
fn restore_ratios(node: &mut PaneNode, ratios: &mut impl Iterator<Item = f32>) {
    if let PaneNode::Split {
        ratio,
        first,
        second,
        ..
    } = node
    {
        if let Some(r) = ratios.next() {
            *ratio = r;
        }
        restore_ratios(first, ratios);
        restore_ratios(second, ratios);
    }
}

/// After a split, the new leaf is the split's `second`; a "before" split
/// (A5) swaps it to `first`. `split_node` passed `1 - share` as the ratio
/// so the swap leaves the focused pane its requested share.
fn put_new_leaf_first(node: &mut PaneNode, new_id: PaneId) -> bool {
    if let PaneNode::Split { first, second, .. } = node {
        if matches!(second.as_ref(), PaneNode::Leaf(p) if p.id == new_id) && first.is_leaf() {
            std::mem::swap(first, second);
            return true;
        }
        return put_new_leaf_first(first, new_id) || put_new_leaf_first(second, new_id);
    }
    false
}

/// Take a tree apart into its panes, keyed by id.
fn into_panes(node: PaneNode) -> HashMap<PaneId, Pane> {
    let mut out = HashMap::new();
    fn walk(node: PaneNode, out: &mut HashMap<PaneId, Pane>) {
        match node {
            PaneNode::Leaf(pane) => {
                out.insert(pane.id, *pane);
            }
            PaneNode::Split { first, second, .. } => {
                walk(*first, out);
                walk(*second, out);
            }
        }
    }
    walk(node, &mut out);
    out
}

/// A pane tree's structure without its panes.
enum Shape {
    Leaf(PaneId),
    Split(SplitDirection, f32, Box<Shape>, Box<Shape>),
}

impl Shape {
    fn of(node: &PaneNode) -> Shape {
        match node {
            PaneNode::Leaf(p) => Shape::Leaf(p.id),
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => Shape::Split(
                *direction,
                *ratio,
                Box::new(Shape::of(first)),
                Box::new(Shape::of(second)),
            ),
        }
    }

    /// `ids` in a chain along `direction`, right-leaning.
    fn chain(direction: SplitDirection, ids: &[PaneId]) -> Shape {
        match ids {
            [only] => Shape::Leaf(*only),
            [head, rest @ ..] => Shape::Split(
                direction,
                0.5,
                Box::new(Shape::Leaf(*head)),
                Box::new(Shape::chain(direction, rest)),
            ),
            [] => unreachable!("a chain has at least one pane"),
        }
    }

    /// Rows (or columns) of shapes chained along `direction`.
    fn chain_of(direction: SplitDirection, mut parts: Vec<Shape>) -> Shape {
        let head = parts.remove(0);
        if parts.is_empty() {
            head
        } else {
            Shape::Split(
                direction,
                0.5,
                Box::new(head),
                Box::new(Shape::chain_of(direction, parts)),
            )
        }
    }

    fn preset(preset: LayoutPreset, ids: &[PaneId]) -> Shape {
        use SplitDirection::{Horizontal as Stack, Vertical as Side};
        if ids.len() < 2 {
            return Shape::chain(Side, ids);
        }
        match preset {
            LayoutPreset::EvenHorizontal => Shape::chain(Side, ids),
            LayoutPreset::EvenVertical => Shape::chain(Stack, ids),
            LayoutPreset::MainLeft => Shape::Split(
                Side,
                0.5,
                Box::new(Shape::Leaf(ids[0])),
                Box::new(Shape::chain(Stack, &ids[1..])),
            ),
            LayoutPreset::MainTop => Shape::Split(
                Stack,
                0.5,
                Box::new(Shape::Leaf(ids[0])),
                Box::new(Shape::chain(Side, &ids[1..])),
            ),
            LayoutPreset::Tiled => {
                let cols = (ids.len() as f64).sqrt().ceil() as usize;
                let rows: Vec<Shape> = ids
                    .chunks(cols)
                    .map(|row| Shape::chain(Side, row))
                    .collect();
                Shape::chain_of(Stack, rows)
            }
        }
    }

    /// Build a tree, taking each leaf's pane out of `panes`. Splits are
    /// built directly: `PaneNode::split` clamps ratios to 10–90%, which
    /// cannot express an equal share of more than ten panes.
    fn build(self, panes: &mut HashMap<PaneId, Pane>) -> PaneNode {
        match self {
            Shape::Leaf(id) => {
                PaneNode::leaf(panes.remove(&id).expect("every shape leaf has a pane"))
            }
            Shape::Split(direction, ratio, first, second) => PaneNode::Split {
                direction,
                ratio,
                first: Box::new(first.build(panes)),
                second: Box::new(second.build(panes)),
            },
        }
    }
}
