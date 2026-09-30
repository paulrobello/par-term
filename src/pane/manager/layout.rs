//! Layout management operations for PaneManager
//!
//! Handles bounds calculation, terminal resizing, divider management,
//! split ratio adjustment, and drag-to-resize functionality.

use super::PaneManager;
use crate::pane::tmux_helpers::DividerUpdateContext;
use crate::pane::types::{DividerRect, PaneBounds, PaneNode, SplitDirection, split_child_bounds};

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
        self.apply_zoom_bounds();
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

    /// Get all divider rectangles in the pane tree
    pub fn get_dividers(&self) -> Vec<DividerRect> {
        // A zoomed pane covers every divider: none is drawn or draggable.
        if self.zoomed_pane_id.is_some() {
            return Vec::new();
        }
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

    /// Resize by dragging a divider to a new position, never past the
    /// minimum pane size (PN4).
    ///
    /// `divider_index`: Which divider is being dragged
    /// `new_position`: New mouse position (x for vertical, y for horizontal dividers)
    pub fn drag_divider(&mut self, divider_index: usize, new_x: f32, new_y: f32) {
        if self.zoomed_pane_id.is_some() {
            return;
        }
        let Some(mut root) = self.root.take() else {
            return;
        };
        let mut divider_count = 0;
        let ctx = DividerUpdateContext {
            target_index: divider_index,
            new_x,
            new_y,
            bounds: self.total_bounds,
            divider_width: self.divider_width,
        };
        self.update_divider_ratio(&mut root, &mut divider_count, &ctx);
        self.root = Some(root);
        self.recalculate_bounds();
    }

    /// Recursively find and update the split ratio for a divider
    fn update_divider_ratio(
        &self,
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
                if *current_index == ctx.target_index {
                    let wanted = match direction {
                        SplitDirection::Horizontal => {
                            (ctx.new_y - ctx.bounds.y) / ctx.bounds.height
                        }
                        SplitDirection::Vertical => (ctx.new_x - ctx.bounds.x) / ctx.bounds.width,
                    };
                    *ratio = self
                        .clamp_drag_ratio(*direction, ctx.bounds, first, second, wanted, *ratio);
                    return true;
                }
                *current_index += 1;
                let (first_bounds, second_bounds) =
                    split_child_bounds(*direction, *ratio, ctx.bounds, ctx.divider_width);
                let first_ctx = DividerUpdateContext {
                    bounds: first_bounds,
                    ..*ctx
                };
                if self.update_divider_ratio(first, current_index, &first_ctx) {
                    return true;
                }
                let second_ctx = DividerUpdateContext {
                    bounds: second_bounds,
                    ..*ctx
                };
                self.update_divider_ratio(second, current_index, &second_ctx)
            }
        }
    }
}
