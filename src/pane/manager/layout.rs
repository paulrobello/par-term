//! Layout management operations for PaneManager
//!
//! Handles bounds calculation, terminal resizing, divider management,
//! split ratio adjustment, and drag-to-resize functionality.

use super::PaneManager;
use crate::pane::tmux_helpers::DividerUpdateContext;
use crate::pane::types::{DividerRect, PaneBounds, PaneId, PaneNode, SplitDirection};

impl PaneManager {
    /// Set the total bounds available for panes and recalculate layout
    pub fn set_bounds(&mut self, bounds: PaneBounds) {
        self.total_bounds = bounds;
        self.recalculate_bounds();
    }

    /// Recalculate bounds for all panes
    pub fn recalculate_bounds(&mut self) {
        if let Some(ref mut root) = self.root {
            root.calculate_bounds(self.total_bounds, self.divider_width);
        }
    }

    /// Resize all pane terminals to match their current bounds
    ///
    /// This should be called after bounds are updated (split, resize, window resize)
    /// to ensure each PTY is sized correctly for its pane area.
    pub fn resize_all_terminals(&self, cell_width: f32, cell_height: f32) {
        self.resize_all_terminals_with_padding(cell_width, cell_height, 0.0, 0.0);
    }

    /// Resize all terminal PTYs to match their pane bounds, accounting for padding.
    ///
    /// The padding reduces the content area where text is rendered, so terminals
    /// should be sized for the padded (smaller) area to avoid content being cut off.
    ///
    /// `height_offset` is an additional height reduction (e.g., pane title bar height)
    /// subtracted once from each pane's content height.
    pub fn resize_all_terminals_with_padding(
        &self,
        cell_width: f32,
        cell_height: f32,
        padding: f32,
        height_offset: f32,
    ) {
        if let Some(ref root) = self.root {
            for pane in root.all_panes() {
                // Calculate content size (bounds minus padding on each side, minus title bar)
                let content_width = (pane.bounds.width - padding * 2.0).max(cell_width);
                let content_height =
                    (pane.bounds.height - padding * 2.0 - height_offset).max(cell_height);

                let cols = (content_width / cell_width).floor() as usize;
                let rows = (content_height / cell_height).floor() as usize;

                pane.resize_terminal_with_cell_dims(
                    cols.max(1),
                    rows.max(1),
                    cell_width as u32,
                    cell_height as u32,
                );
            }
        }
    }

    /// Set the divider width
    pub fn set_divider_width(&mut self, width: f32) {
        self.divider_width = width;
        self.recalculate_bounds();
    }

    /// Get the divider width
    pub fn divider_width(&self) -> f32 {
        self.divider_width
    }

    /// Get the hit detection padding (extra area around divider for easier grabbing)
    pub fn divider_hit_padding(&self) -> f32 {
        (self.divider_hit_width - self.divider_width).max(0.0) / 2.0
    }

    /// Resize a split by adjusting its ratio
    ///
    /// `pane_id`: The pane whose adjacent split should be resized
    /// `delta`: Amount to adjust the ratio (-1.0 to 1.0)
    pub fn resize_split(&mut self, pane_id: PaneId, delta: f32) {
        if let Some(ref mut root) = self.root {
            Self::adjust_split_ratio(root, pane_id, delta);
            self.recalculate_bounds();
        }
    }

    /// Recursively find and adjust the split ratio for a pane
    pub(super) fn adjust_split_ratio(node: &mut PaneNode, target_id: PaneId, delta: f32) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split {
                ratio,
                first,
                second,
                ..
            } => {
                // Check if target is in first child
                if first.all_pane_ids().contains(&target_id) {
                    // Try to find in nested splits first
                    if Self::adjust_split_ratio(first, target_id, delta) {
                        return true;
                    }
                    // Adjust this split's ratio (making first child larger/smaller)
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                    return true;
                }

                // Check if target is in second child
                if second.all_pane_ids().contains(&target_id) {
                    // Try to find in nested splits first
                    if Self::adjust_split_ratio(second, target_id, delta) {
                        return true;
                    }
                    // Adjust this split's ratio (making second child larger/smaller)
                    *ratio = (*ratio - delta).clamp(0.1, 0.9);
                    return true;
                }

                false
            }
        }
    }

    /// Get all divider rectangles in the pane tree
    pub fn get_dividers(&self) -> Vec<DividerRect> {
        self.root
            .as_ref()
            .map(|r| r.collect_dividers(self.total_bounds, self.divider_width))
            .unwrap_or_default()
    }

    /// Find a divider at the given position
    ///
    /// Returns the index of the divider if found, with optional padding for easier grabbing
    pub fn find_divider_at(&self, x: f32, y: f32, padding: f32) -> Option<usize> {
        let dividers = self.get_dividers();
        for (i, divider) in dividers.iter().enumerate() {
            if divider.contains(x, y, padding) {
                return Some(i);
            }
        }
        None
    }

    /// Check if a position is on a divider
    pub fn is_on_divider(&self, x: f32, y: f32) -> bool {
        let padding = (self.divider_hit_width - self.divider_width).max(0.0) / 2.0;
        self.find_divider_at(x, y, padding).is_some()
    }

    /// Set the divider hit width
    pub fn set_divider_hit_width(&mut self, width: f32) {
        self.divider_hit_width = width;
    }

    /// Get the divider at an index
    pub fn get_divider(&self, index: usize) -> Option<DividerRect> {
        self.get_dividers().get(index).copied()
    }

    /// Resize by dragging a divider to a new position
    ///
    /// `divider_index`: Which divider is being dragged
    /// `new_position`: New mouse position (x for vertical, y for horizontal dividers)
    pub fn drag_divider(&mut self, divider_index: usize, new_x: f32, new_y: f32) {
        // Get the divider info first
        let dividers = self.get_dividers();
        if dividers.get(divider_index).is_some() {
            // Find the split node that owns this divider and update its ratio
            if let Some(ref mut root) = self.root {
                let mut divider_count = 0;
                let ctx = DividerUpdateContext {
                    target_index: divider_index,
                    new_x,
                    new_y,
                    bounds: self.total_bounds,
                    divider_width: self.divider_width,
                };
                Self::update_divider_ratio(root, &mut divider_count, &ctx);
                self.recalculate_bounds();
            }
        }
    }

    /// Recursively find and update the split ratio for a divider
    pub(super) fn update_divider_ratio(
        node: &mut PaneNode,
        current_index: &mut usize,
        ctx: &DividerUpdateContext,
    ) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Check if this is the target divider
                if *current_index == ctx.target_index {
                    // Calculate new ratio based on mouse position
                    let new_ratio = match direction {
                        SplitDirection::Horizontal => {
                            // Horizontal split: mouse Y position determines ratio
                            ((ctx.new_y - ctx.bounds.y) / ctx.bounds.height).clamp(0.1, 0.9)
                        }
                        SplitDirection::Vertical => {
                            // Vertical split: mouse X position determines ratio
                            ((ctx.new_x - ctx.bounds.x) / ctx.bounds.width).clamp(0.1, 0.9)
                        }
                    };
                    *ratio = new_ratio;
                    return true;
                }
                *current_index += 1;

                // Calculate child bounds to recurse
                let (first_bounds, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        let first_height = (ctx.bounds.height - ctx.divider_width) * *ratio;
                        let second_height = ctx.bounds.height - first_height - ctx.divider_width;
                        (
                            PaneBounds::new(
                                ctx.bounds.x,
                                ctx.bounds.y,
                                ctx.bounds.width,
                                first_height,
                            ),
                            PaneBounds::new(
                                ctx.bounds.x,
                                ctx.bounds.y + first_height + ctx.divider_width,
                                ctx.bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        let first_width = (ctx.bounds.width - ctx.divider_width) * *ratio;
                        let second_width = ctx.bounds.width - first_width - ctx.divider_width;
                        (
                            PaneBounds::new(
                                ctx.bounds.x,
                                ctx.bounds.y,
                                first_width,
                                ctx.bounds.height,
                            ),
                            PaneBounds::new(
                                ctx.bounds.x + first_width + ctx.divider_width,
                                ctx.bounds.y,
                                second_width,
                                ctx.bounds.height,
                            ),
                        )
                    }
                };

                // Try children
                let first_ctx = DividerUpdateContext {
                    bounds: first_bounds,
                    ..*ctx
                };
                if Self::update_divider_ratio(first, current_index, &first_ctx) {
                    return true;
                }
                let second_ctx = DividerUpdateContext {
                    bounds: second_bounds,
                    ..*ctx
                };
                Self::update_divider_ratio(second, current_index, &second_ctx)
            }
        }
    }
}
