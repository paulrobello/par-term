//! Pane manager for coordinating pane operations within a tab
//!
//! The PaneManager owns the pane tree and provides operations for:
//! - Splitting panes horizontally and vertically
//! - Closing panes
//! - Navigating between panes
//! - Resizing panes

use super::types::{
    DividerRect, NavigationDirection, Pane, PaneBounds, PaneId, PaneNode, SplitDirection,
};
use crate::config::{Config, PaneBackgroundConfig};
use crate::session::SessionPaneNode;
use crate::tmux::{LayoutNode, TmuxLayout, TmuxPaneId};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Manages the pane tree within a single tab
pub struct PaneManager {
    /// Root of the pane tree (None if no panes yet)
    root: Option<PaneNode>,
    /// ID of the currently focused pane
    focused_pane_id: Option<PaneId>,
    /// Counter for generating unique pane IDs
    next_pane_id: PaneId,
    /// Width of dividers between panes in pixels
    divider_width: f32,
    /// Width of the hit area for divider drag detection
    divider_hit_width: f32,
    /// Current total bounds available for panes
    total_bounds: PaneBounds,
}

impl PaneManager {
    /// Create a new empty pane manager
    pub fn new() -> Self {
        Self {
            root: None,
            focused_pane_id: None,
            next_pane_id: 1,
            divider_width: 1.0,     // Default 1 pixel divider
            divider_hit_width: 8.0, // Default 8 pixel hit area
            total_bounds: PaneBounds::default(),
        }
    }

    /// Create a pane manager with an initial pane
    pub fn with_initial_pane(
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<Self> {
        let mut manager = Self::new();
        manager.divider_width = config.pane_divider_width.unwrap_or(1.0);
        manager.divider_hit_width = config.pane_divider_hit_width;
        manager.create_initial_pane(config, runtime, working_directory)?;
        Ok(manager)
    }

    /// Create the initial pane (when tab is first created)
    ///
    /// If bounds have been set on the PaneManager via `set_bounds()`, the pane
    /// will be created with dimensions calculated from those bounds. Otherwise,
    /// the default config dimensions are used.
    pub fn create_initial_pane(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<PaneId> {
        self.create_initial_pane_internal(None, config, runtime, working_directory)
    }

    /// Create the initial pane sized for an upcoming split
    ///
    /// This calculates dimensions based on what the pane size will be AFTER
    /// the split, preventing the shell from seeing a resize.
    pub fn create_initial_pane_for_split(
        &mut self,
        direction: SplitDirection,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<PaneId> {
        self.create_initial_pane_internal(Some(direction), config, runtime, working_directory)
    }

    /// Internal method to create initial pane with optional split direction
    fn create_initial_pane_internal(
        &mut self,
        split_direction: Option<SplitDirection>,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<PaneId> {
        let id = self.next_pane_id;
        self.next_pane_id += 1;

        // Calculate dimensions from bounds if available
        let pane_config = if self.total_bounds.width > 0.0 && self.total_bounds.height > 0.0 {
            // Approximate cell dimensions from font size
            let cell_width = config.font_size * 0.6; // Approximate monospace char width
            let cell_height = config.font_size * 1.2; // Approximate line height

            // Calculate bounds accounting for upcoming split
            let effective_bounds = match split_direction {
                Some(SplitDirection::Vertical) => {
                    // After vertical split, this pane will have half the width
                    PaneBounds::new(
                        self.total_bounds.x,
                        self.total_bounds.y,
                        (self.total_bounds.width - self.divider_width) / 2.0,
                        self.total_bounds.height,
                    )
                }
                Some(SplitDirection::Horizontal) => {
                    // After horizontal split, this pane will have half the height
                    PaneBounds::new(
                        self.total_bounds.x,
                        self.total_bounds.y,
                        self.total_bounds.width,
                        (self.total_bounds.height - self.divider_width) / 2.0,
                    )
                }
                None => self.total_bounds,
            };

            let (cols, rows) = effective_bounds.grid_size(cell_width, cell_height);

            let mut cfg = config.clone();
            cfg.cols = cols.max(10);
            cfg.rows = rows.max(5);
            log::info!(
                "Initial pane {} using bounds-based dimensions: {}x{} (split={:?})",
                id,
                cfg.cols,
                cfg.rows,
                split_direction
            );
            cfg
        } else {
            log::info!(
                "Initial pane {} using config dimensions: {}x{}",
                id,
                config.cols,
                config.rows
            );
            config.clone()
        };

        let mut pane = Pane::new(id, &pane_config, runtime, working_directory)?;

        // Apply per-pane background from config if available (index 0 for initial pane)
        if let Some((image_path, mode, opacity, darken)) = config.get_pane_background(0) {
            pane.set_background(crate::pane::PaneBackground {
                image_path: Some(image_path),
                mode,
                opacity,
                darken,
            });
        }

        self.root = Some(PaneNode::leaf(pane));
        self.focused_pane_id = Some(id);

        Ok(id)
    }

    /// Split the focused pane in the given direction
    ///
    /// Returns the ID of the new pane, or None if no pane is focused
    pub fn split(
        &mut self,
        direction: SplitDirection,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<Option<PaneId>> {
        let focused_id = match self.focused_pane_id {
            Some(id) => id,
            None => return Ok(None),
        };

        // Get the working directory and bounds from the focused pane
        let (working_dir, focused_bounds) = if let Some(pane) = self.focused_pane() {
            (pane.get_cwd(), pane.bounds)
        } else {
            (None, self.total_bounds)
        };

        // Calculate approximate dimensions for the new pane (half of focused pane)
        let (new_cols, new_rows) = match direction {
            SplitDirection::Vertical => {
                // New pane gets half the width
                let half_width = (focused_bounds.width - self.divider_width) / 2.0;
                let cols = (half_width / config.font_size * 1.8).floor() as usize; // Approximate
                (cols.max(10), config.rows)
            }
            SplitDirection::Horizontal => {
                // New pane gets half the height
                let half_height = (focused_bounds.height - self.divider_width) / 2.0;
                let rows = (half_height / (config.font_size * 1.2)).floor() as usize; // Approximate
                (config.cols, rows.max(5))
            }
        };

        // Create a modified config with the approximate dimensions
        let mut pane_config = config.clone();
        pane_config.cols = new_cols;
        pane_config.rows = new_rows;

        // Create the new pane with approximate dimensions
        let new_id = self.next_pane_id;
        self.next_pane_id += 1;

        let mut new_pane = Pane::new(new_id, &pane_config, runtime, working_dir)?;

        // Apply per-pane background from config if available
        // The new pane will be at the end of the pane list, so its index is the current count
        let new_pane_index = self.pane_count(); // current count = index of new pane after insertion
        if let Some((image_path, mode, opacity, darken)) =
            config.get_pane_background(new_pane_index)
        {
            new_pane.set_background(crate::pane::PaneBackground {
                image_path: Some(image_path),
                mode,
                opacity,
                darken,
            });
        }

        // Find and split the focused pane
        if let Some(root) = self.root.take() {
            let (new_root, _) = Self::split_node(root, focused_id, direction, Some(new_pane));
            self.root = Some(new_root);
        }

        // Recalculate bounds
        self.recalculate_bounds();

        // Focus the new pane
        self.focused_pane_id = Some(new_id);

        crate::debug_info!(
            "PANE_SPLIT",
            "Split pane {} {:?}, created new pane {}. First(left/top)={} Second(right/bottom)={} (focused)",
            focused_id,
            direction,
            new_id,
            focused_id,
            new_id
        );

        Ok(Some(new_id))
    }

    /// Split a node, finding the target pane and replacing it with a split
    ///
    /// Returns (new_node, remaining_pane) where remaining_pane is Some if
    /// the target was not found in this subtree.
    fn split_node(
        node: PaneNode,
        target_id: PaneId,
        direction: SplitDirection,
        new_pane: Option<Pane>,
    ) -> (PaneNode, Option<Pane>) {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    if let Some(new) = new_pane {
                        // This is the pane to split - create a new split node
                        (
                            PaneNode::split(
                                direction,
                                0.5, // 50/50 split
                                PaneNode::leaf(*pane),
                                PaneNode::leaf(new),
                            ),
                            None,
                        )
                    } else {
                        // No pane to insert (shouldn't happen)
                        (PaneNode::Leaf(pane), None)
                    }
                } else {
                    // Not the target, keep as-is and pass the new_pane through
                    (PaneNode::Leaf(pane), new_pane)
                }
            }
            PaneNode::Split {
                direction: split_dir,
                ratio,
                first,
                second,
            } => {
                // Try to insert in first child
                let (new_first, remaining) =
                    Self::split_node(*first, target_id, direction, new_pane);

                if remaining.is_none() {
                    // Target was found in first child
                    (
                        PaneNode::Split {
                            direction: split_dir,
                            ratio,
                            first: Box::new(new_first),
                            second,
                        },
                        None,
                    )
                } else {
                    // Target not in first, try second
                    let (new_second, remaining) =
                        Self::split_node(*second, target_id, direction, remaining);
                    (
                        PaneNode::Split {
                            direction: split_dir,
                            ratio,
                            first: Box::new(new_first),
                            second: Box::new(new_second),
                        },
                        remaining,
                    )
                }
            }
        }
    }

    /// Close a pane by ID
    ///
    /// Returns true if this was the last pane (tab should close)
    pub fn close_pane(&mut self, id: PaneId) -> bool {
        crate::debug_info!("PANE_CLOSE", "close_pane called for pane {}", id);

        if let Some(root) = self.root.take() {
            match Self::remove_pane(root, id) {
                RemoveResult::Removed(new_root) => {
                    self.root = new_root;

                    // If we closed the focused pane, focus another
                    if self.focused_pane_id == Some(id) {
                        let new_focus = self
                            .root
                            .as_ref()
                            .and_then(|r| r.all_pane_ids().first().copied());
                        crate::debug_info!(
                            "PANE_CLOSE",
                            "Closed focused pane {}, new focus: {:?}",
                            id,
                            new_focus
                        );
                        self.focused_pane_id = new_focus;
                    }

                    // Recalculate bounds
                    self.recalculate_bounds();

                    // Log remaining panes after closure
                    if let Some(ref root) = self.root {
                        for pane_id in root.all_pane_ids() {
                            if let Some(pane) = self.get_pane(pane_id) {
                                crate::debug_info!(
                                    "PANE_CLOSE",
                                    "Remaining pane {} bounds=({:.0},{:.0} {:.0}x{:.0})",
                                    pane.id,
                                    pane.bounds.x,
                                    pane.bounds.y,
                                    pane.bounds.width,
                                    pane.bounds.height
                                );
                            }
                        }
                    }

                    crate::debug_info!("PANE_CLOSE", "Successfully closed pane {}", id);
                }
                RemoveResult::NotFound(root) => {
                    crate::debug_info!("PANE_CLOSE", "Pane {} not found in tree", id);
                    self.root = Some(root);
                }
            }
        }

        self.root.is_none()
    }

    /// Remove a pane from the tree, returning the new tree structure
    fn remove_pane(node: PaneNode, target_id: PaneId) -> RemoveResult {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    // This pane should be removed
                    RemoveResult::Removed(None)
                } else {
                    RemoveResult::NotFound(PaneNode::Leaf(pane))
                }
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Try to remove from first child
                match Self::remove_pane(*first, target_id) {
                    RemoveResult::Removed(None) => {
                        // First child was the target and is now gone
                        // Replace this split with the second child
                        RemoveResult::Removed(Some(*second))
                    }
                    RemoveResult::Removed(Some(new_first)) => {
                        // First child was modified
                        RemoveResult::Removed(Some(PaneNode::Split {
                            direction,
                            ratio,
                            first: Box::new(new_first),
                            second,
                        }))
                    }
                    RemoveResult::NotFound(first_node) => {
                        // Target not in first child, try second
                        match Self::remove_pane(*second, target_id) {
                            RemoveResult::Removed(None) => {
                                // Second child was the target and is now gone
                                // Replace this split with the first child
                                RemoveResult::Removed(Some(first_node))
                            }
                            RemoveResult::Removed(Some(new_second)) => {
                                // Second child was modified
                                RemoveResult::Removed(Some(PaneNode::Split {
                                    direction,
                                    ratio,
                                    first: Box::new(first_node),
                                    second: Box::new(new_second),
                                }))
                            }
                            RemoveResult::NotFound(second_node) => {
                                // Target not found in either child
                                RemoveResult::NotFound(PaneNode::Split {
                                    direction,
                                    ratio,
                                    first: Box::new(first_node),
                                    second: Box::new(second_node),
                                })
                            }
                        }
                    }
                }
            }
        }
    }

    /// Navigate to a pane in the given direction
    pub fn navigate(&mut self, direction: NavigationDirection) {
        if let Some(focused_id) = self.focused_pane_id
            && let Some(ref root) = self.root
            && let Some(new_id) = root.find_pane_in_direction(focused_id, direction)
        {
            self.focused_pane_id = Some(new_id);
            log::debug!(
                "Navigated {:?} from pane {} to pane {}",
                direction,
                focused_id,
                new_id
            );
        }
    }

    /// Focus a specific pane by ID
    pub fn focus_pane(&mut self, id: PaneId) {
        if self
            .root
            .as_ref()
            .is_some_and(|r| r.find_pane(id).is_some())
        {
            self.focused_pane_id = Some(id);
        }
    }

    /// Focus the pane at a given pixel position
    pub fn focus_pane_at(&mut self, x: f32, y: f32) -> Option<PaneId> {
        if let Some(ref root) = self.root
            && let Some(pane) = root.find_pane_at(x, y)
        {
            let id = pane.id;
            self.focused_pane_id = Some(id);
            return Some(id);
        }
        None
    }

    /// Get the currently focused pane
    pub fn focused_pane(&self) -> Option<&Pane> {
        self.focused_pane_id
            .and_then(|id| self.root.as_ref()?.find_pane(id))
    }

    /// Get the currently focused pane mutably
    pub fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.focused_pane_id?;
        self.root.as_mut()?.find_pane_mut(id)
    }

    /// Get the focused pane ID
    pub fn focused_pane_id(&self) -> Option<PaneId> {
        self.focused_pane_id
    }

    /// Get the next pane ID that will be assigned
    pub fn next_pane_id(&self) -> PaneId {
        self.next_pane_id
    }

    /// Add a pane for tmux integration (doesn't create split, just adds to flat structure)
    ///
    /// This is used when tmux splits a pane - we need to add a new native pane
    /// without restructuring our tree (tmux layout update will handle that).
    pub fn add_pane_for_tmux(&mut self, pane: Pane) {
        let pane_id = pane.id;

        // Update next_pane_id if needed
        if pane_id >= self.next_pane_id {
            self.next_pane_id = pane_id + 1;
        }

        // If no root, this becomes the root
        if self.root.is_none() {
            self.root = Some(PaneNode::leaf(pane));
            self.focused_pane_id = Some(pane_id);
            return;
        }

        // Otherwise, we need to add it to the tree structure
        // For now, we'll create a simple vertical split with the new pane
        // The actual layout will be corrected by update_layout_from_tmux
        if let Some(existing_root) = self.root.take() {
            self.root = Some(PaneNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(existing_root),
                second: Box::new(PaneNode::leaf(pane)),
            });
        }

        // Focus the new pane
        self.focused_pane_id = Some(pane_id);
    }

    /// Get a pane by ID
    pub fn get_pane(&self, id: PaneId) -> Option<&Pane> {
        self.root.as_ref()?.find_pane(id)
    }

    /// Get a mutable pane by ID
    pub fn get_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.root.as_mut()?.find_pane_mut(id)
    }

    /// Get all panes
    pub fn all_panes(&self) -> Vec<&Pane> {
        self.root
            .as_ref()
            .map(|r| r.all_panes())
            .unwrap_or_default()
    }

    /// Get all panes mutably
    pub fn all_panes_mut(&mut self) -> Vec<&mut Pane> {
        self.root
            .as_mut()
            .map(|r| r.all_panes_mut())
            .unwrap_or_default()
    }

    /// Collect current per-pane background settings for config persistence
    ///
    /// Returns a `Vec<PaneBackgroundConfig>` containing only panes that have
    /// a custom background image set. The `index` field corresponds to the
    /// pane's position in the tree traversal order.
    pub fn collect_pane_backgrounds(&self) -> Vec<PaneBackgroundConfig> {
        self.all_panes()
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                pane.background
                    .image_path
                    .as_ref()
                    .map(|path| PaneBackgroundConfig {
                        index,
                        image: path.clone(),
                        mode: pane.background.mode,
                        opacity: pane.background.opacity,
                        darken: pane.background.darken,
                    })
            })
            .collect()
    }

    /// Get the number of panes
    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map(|r| r.pane_count()).unwrap_or(0)
    }

    /// Check if there are multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.pane_count() > 1
    }

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
    fn adjust_split_ratio(node: &mut PaneNode, target_id: PaneId, delta: f32) -> bool {
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

    /// Get access to the root node (for rendering)
    pub fn root(&self) -> Option<&PaneNode> {
        self.root.as_ref()
    }

    /// Get mutable access to the root node
    pub fn root_mut(&mut self) -> Option<&mut PaneNode> {
        self.root.as_mut()
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
        if let Some(divider) = dividers.get(divider_index) {
            // Find the split node that owns this divider and update its ratio
            if let Some(ref mut root) = self.root {
                let mut divider_count = 0;
                Self::update_divider_ratio(
                    root,
                    divider_index,
                    &mut divider_count,
                    divider.is_horizontal,
                    new_x,
                    new_y,
                    self.total_bounds,
                    self.divider_width,
                );
                self.recalculate_bounds();
            }
        }
    }

    /// Recursively find and update the split ratio for a divider
    #[allow(clippy::only_used_in_recursion, clippy::too_many_arguments)]
    fn update_divider_ratio(
        node: &mut PaneNode,
        target_index: usize,
        current_index: &mut usize,
        is_horizontal: bool,
        new_x: f32,
        new_y: f32,
        bounds: PaneBounds,
        divider_width: f32,
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
                if *current_index == target_index {
                    // Calculate new ratio based on mouse position
                    let new_ratio = match direction {
                        SplitDirection::Horizontal => {
                            // Horizontal split: mouse Y position determines ratio
                            ((new_y - bounds.y) / bounds.height).clamp(0.1, 0.9)
                        }
                        SplitDirection::Vertical => {
                            // Vertical split: mouse X position determines ratio
                            ((new_x - bounds.x) / bounds.width).clamp(0.1, 0.9)
                        }
                    };
                    *ratio = new_ratio;
                    return true;
                }
                *current_index += 1;

                // Calculate child bounds to recurse
                let (first_bounds, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        let first_height = (bounds.height - divider_width) * *ratio;
                        let second_height = bounds.height - first_height - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, bounds.width, first_height),
                            PaneBounds::new(
                                bounds.x,
                                bounds.y + first_height + divider_width,
                                bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        let first_width = (bounds.width - divider_width) * *ratio;
                        let second_width = bounds.width - first_width - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, first_width, bounds.height),
                            PaneBounds::new(
                                bounds.x + first_width + divider_width,
                                bounds.y,
                                second_width,
                                bounds.height,
                            ),
                        )
                    }
                };

                // Try children
                if Self::update_divider_ratio(
                    first,
                    target_index,
                    current_index,
                    is_horizontal,
                    new_x,
                    new_y,
                    first_bounds,
                    divider_width,
                ) {
                    return true;
                }
                Self::update_divider_ratio(
                    second,
                    target_index,
                    current_index,
                    is_horizontal,
                    new_x,
                    new_y,
                    second_bounds,
                    divider_width,
                )
            }
        }
    }

    // =========================================================================
    // Session Restore
    // =========================================================================

    /// Build a pane tree from a saved session layout
    ///
    /// Recursively constructs live `PaneNode` tree from a `SessionPaneNode`,
    /// creating new terminal panes for each leaf. If a leaf's CWD no longer
    /// exists, falls back to `$HOME`.
    pub fn build_from_layout(
        &mut self,
        layout: &SessionPaneNode,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<()> {
        let root = self.build_node_from_layout(layout, config, runtime)?;
        let first_id = root.all_pane_ids().first().copied();
        self.root = Some(root);
        self.focused_pane_id = first_id;
        self.recalculate_bounds();

        // Apply per-pane backgrounds from config to restored panes
        let panes = self.all_panes_mut();
        for (index, pane) in panes.into_iter().enumerate() {
            if let Some((image_path, mode, opacity, darken)) = config.get_pane_background(index) {
                pane.set_background(crate::pane::PaneBackground {
                    image_path: Some(image_path),
                    mode,
                    opacity,
                    darken,
                });
            }
        }

        Ok(())
    }

    /// Recursively build a PaneNode from a SessionPaneNode
    fn build_node_from_layout(
        &mut self,
        layout: &SessionPaneNode,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<PaneNode> {
        match layout {
            SessionPaneNode::Leaf { cwd } => {
                let id = self.next_pane_id;
                self.next_pane_id += 1;

                let validated_cwd = crate::session::restore::validate_cwd(cwd);
                let pane = Pane::new(id, config, runtime, validated_cwd)?;
                Ok(PaneNode::leaf(pane))
            }
            SessionPaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first_node = self.build_node_from_layout(first, config, runtime.clone())?;
                let second_node = self.build_node_from_layout(second, config, runtime)?;
                Ok(PaneNode::split(*direction, *ratio, first_node, second_node))
            }
        }
    }

    // =========================================================================
    // tmux Layout Integration
    // =========================================================================

    /// Set the pane tree from a tmux layout
    ///
    /// This replaces the entire pane tree with one constructed from the tmux layout.
    /// Returns a mapping of tmux pane IDs to native pane IDs.
    ///
    /// # Arguments
    /// * `layout` - The parsed tmux layout
    /// * `config` - Configuration for creating panes
    /// * `runtime` - Async runtime for pane tasks
    pub fn set_from_tmux_layout(
        &mut self,
        layout: &TmuxLayout,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<HashMap<TmuxPaneId, PaneId>> {
        let mut pane_mappings = HashMap::new();

        // Convert the tmux layout to our pane tree
        let new_root =
            self.convert_layout_node(&layout.root, config, runtime.clone(), &mut pane_mappings)?;

        // Replace the root
        self.root = Some(new_root);

        // Set focus to the first pane in the mapping
        if let Some(first_native_id) = pane_mappings.values().next() {
            self.focused_pane_id = Some(*first_native_id);
        }

        // Update next_pane_id to avoid conflicts
        if let Some(max_id) = pane_mappings.values().max() {
            self.next_pane_id = max_id + 1;
        }

        // Recalculate bounds
        self.recalculate_bounds();

        log::info!(
            "Set pane tree from tmux layout: {} panes",
            pane_mappings.len()
        );

        Ok(pane_mappings)
    }

    /// Rebuild the pane tree from a tmux layout, preserving existing pane terminals
    ///
    /// This is called when panes are added or the layout structure changes.
    /// It rebuilds the entire tree structure to match the tmux layout while
    /// reusing existing Pane objects to preserve their terminal state.
    ///
    /// # Arguments
    /// * `layout` - The parsed tmux layout
    /// * `existing_mappings` - Map from tmux pane ID to native pane ID for panes to preserve
    /// * `new_tmux_panes` - List of new tmux pane IDs that need new Pane objects
    /// * `config` - Configuration for creating new panes
    /// * `runtime` - Async runtime for new pane tasks
    ///
    /// # Returns
    /// Updated mapping of tmux pane IDs to native pane IDs
    pub fn rebuild_from_tmux_layout(
        &mut self,
        layout: &TmuxLayout,
        existing_mappings: &HashMap<TmuxPaneId, PaneId>,
        new_tmux_panes: &[TmuxPaneId],
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<HashMap<TmuxPaneId, PaneId>> {
        // Extract all existing panes from the current tree
        let mut existing_panes: HashMap<PaneId, Pane> = HashMap::new();
        if let Some(root) = self.root.take() {
            Self::extract_panes_from_node(root, &mut existing_panes);
        }

        log::debug!(
            "Rebuilding layout: extracted {} existing panes, expecting {} new tmux panes",
            existing_panes.len(),
            new_tmux_panes.len()
        );

        // Build the new tree structure from the tmux layout
        let mut new_mappings = HashMap::new();
        let new_root = self.rebuild_layout_node(
            &layout.root,
            existing_mappings,
            new_tmux_panes,
            &mut existing_panes,
            config,
            runtime.clone(),
            &mut new_mappings,
        )?;

        // Replace the root
        self.root = Some(new_root);

        // Set focus to the first pane if not already set
        if self.focused_pane_id.is_none()
            && let Some(first_native_id) = new_mappings.values().next()
        {
            self.focused_pane_id = Some(*first_native_id);
        }

        // Update next_pane_id to avoid conflicts
        if let Some(max_id) = new_mappings.values().max()
            && *max_id >= self.next_pane_id
        {
            self.next_pane_id = max_id + 1;
        }

        // Recalculate bounds
        self.recalculate_bounds();

        log::info!(
            "Rebuilt pane tree from tmux layout: {} panes",
            new_mappings.len()
        );

        Ok(new_mappings)
    }

    /// Extract all panes from a node into a map
    fn extract_panes_from_node(node: PaneNode, panes: &mut HashMap<PaneId, Pane>) {
        match node {
            PaneNode::Leaf(pane) => {
                let pane = *pane; // Unbox the pane
                panes.insert(pane.id, pane);
            }
            PaneNode::Split { first, second, .. } => {
                Self::extract_panes_from_node(*first, panes);
                Self::extract_panes_from_node(*second, panes);
            }
        }
    }

    /// Rebuild a layout node, reusing existing panes where possible
    #[allow(clippy::too_many_arguments)]
    fn rebuild_layout_node(
        &mut self,
        node: &LayoutNode,
        existing_mappings: &HashMap<TmuxPaneId, PaneId>,
        new_tmux_panes: &[TmuxPaneId],
        existing_panes: &mut HashMap<PaneId, Pane>,
        config: &Config,
        runtime: Arc<Runtime>,
        new_mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        match node {
            LayoutNode::Pane { id: tmux_id, .. } => {
                // Check if this is an existing pane we can reuse
                if let Some(&native_id) = existing_mappings.get(tmux_id)
                    && let Some(pane) = existing_panes.remove(&native_id)
                {
                    log::debug!(
                        "Reusing existing pane {} for tmux pane %{}",
                        native_id,
                        tmux_id
                    );
                    new_mappings.insert(*tmux_id, native_id);
                    return Ok(PaneNode::leaf(pane));
                }

                // This is a new pane - create it
                if new_tmux_panes.contains(tmux_id) {
                    let native_id = self.next_pane_id;
                    self.next_pane_id += 1;

                    let pane = Pane::new_for_tmux(native_id, config, runtime)?;
                    log::debug!("Created new pane {} for tmux pane %{}", native_id, tmux_id);
                    new_mappings.insert(*tmux_id, native_id);
                    return Ok(PaneNode::leaf(pane));
                }

                // Fallback - create a new pane (shouldn't happen normally)
                log::warn!("Unexpected tmux pane %{} - creating new pane", tmux_id);
                let native_id = self.next_pane_id;
                self.next_pane_id += 1;
                let pane = Pane::new_for_tmux(native_id, config, runtime)?;
                new_mappings.insert(*tmux_id, native_id);
                Ok(PaneNode::leaf(pane))
            }

            LayoutNode::VerticalSplit {
                width, children, ..
            } => {
                // Vertical split = panes side by side
                self.rebuild_multi_split_to_binary(
                    children,
                    SplitDirection::Vertical,
                    *width,
                    existing_mappings,
                    new_tmux_panes,
                    existing_panes,
                    config,
                    runtime,
                    new_mappings,
                )
            }

            LayoutNode::HorizontalSplit {
                height, children, ..
            } => {
                // Horizontal split = panes stacked
                self.rebuild_multi_split_to_binary(
                    children,
                    SplitDirection::Horizontal,
                    *height,
                    existing_mappings,
                    new_tmux_panes,
                    existing_panes,
                    config,
                    runtime,
                    new_mappings,
                )
            }
        }
    }

    /// Rebuild multi-child split to binary, reusing existing panes
    #[allow(clippy::too_many_arguments)]
    fn rebuild_multi_split_to_binary(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        existing_mappings: &HashMap<TmuxPaneId, PaneId>,
        new_tmux_panes: &[TmuxPaneId],
        existing_panes: &mut HashMap<PaneId, Pane>,
        config: &Config,
        runtime: Arc<Runtime>,
        new_mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        if children.is_empty() {
            anyhow::bail!("Empty children list in tmux layout");
        }

        if children.len() == 1 {
            return self.rebuild_layout_node(
                &children[0],
                existing_mappings,
                new_tmux_panes,
                existing_panes,
                config,
                runtime,
                new_mappings,
            );
        }

        // Calculate the size of the first child for the ratio
        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        // Rebuild the first child
        let first = self.rebuild_layout_node(
            &children[0],
            existing_mappings,
            new_tmux_panes,
            existing_panes,
            config,
            runtime.clone(),
            new_mappings,
        )?;

        // Calculate remaining size
        let remaining_size = total_size.saturating_sub(first_size + 1);

        // Rebuild the rest recursively
        let second = if children.len() == 2 {
            self.rebuild_layout_node(
                &children[1],
                existing_mappings,
                new_tmux_panes,
                existing_panes,
                config,
                runtime,
                new_mappings,
            )?
        } else {
            self.rebuild_remaining_children(
                &children[1..],
                direction,
                remaining_size,
                existing_mappings,
                new_tmux_panes,
                existing_panes,
                config,
                runtime,
                new_mappings,
            )?
        };

        Ok(PaneNode::split(direction, ratio, first, second))
    }

    /// Rebuild remaining children into nested binary splits
    #[allow(clippy::too_many_arguments)]
    fn rebuild_remaining_children(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        existing_mappings: &HashMap<TmuxPaneId, PaneId>,
        new_tmux_panes: &[TmuxPaneId],
        existing_panes: &mut HashMap<PaneId, Pane>,
        config: &Config,
        runtime: Arc<Runtime>,
        new_mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        if children.len() == 1 {
            return self.rebuild_layout_node(
                &children[0],
                existing_mappings,
                new_tmux_panes,
                existing_panes,
                config,
                runtime,
                new_mappings,
            );
        }

        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        let first = self.rebuild_layout_node(
            &children[0],
            existing_mappings,
            new_tmux_panes,
            existing_panes,
            config,
            runtime.clone(),
            new_mappings,
        )?;

        let remaining_size = total_size.saturating_sub(first_size + 1);
        let second = self.rebuild_remaining_children(
            &children[1..],
            direction,
            remaining_size,
            existing_mappings,
            new_tmux_panes,
            existing_panes,
            config,
            runtime,
            new_mappings,
        )?;

        Ok(PaneNode::split(direction, ratio, first, second))
    }

    /// Convert a tmux LayoutNode to a PaneNode
    fn convert_layout_node(
        &mut self,
        node: &LayoutNode,
        config: &Config,
        runtime: Arc<Runtime>,
        mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        match node {
            LayoutNode::Pane {
                id: tmux_id,
                width: _,
                height: _,
                x: _,
                y: _,
            } => {
                // Create a native pane for this tmux pane
                let native_id = self.next_pane_id;
                self.next_pane_id += 1;

                let pane = Pane::new_for_tmux(native_id, config, runtime)?;

                // Record the mapping
                mappings.insert(*tmux_id, native_id);

                log::debug!(
                    "Created native pane {} for tmux pane %{}",
                    native_id,
                    tmux_id
                );

                Ok(PaneNode::leaf(pane))
            }

            LayoutNode::VerticalSplit {
                width,
                height: _,
                x: _,
                y: _,
                children,
            } => {
                // Vertical split = panes side by side
                self.convert_multi_split_to_binary(
                    children,
                    SplitDirection::Vertical,
                    *width,
                    config,
                    runtime,
                    mappings,
                )
            }

            LayoutNode::HorizontalSplit {
                width: _,
                height,
                x: _,
                y: _,
                children,
            } => {
                // Horizontal split = panes stacked
                self.convert_multi_split_to_binary(
                    children,
                    SplitDirection::Horizontal,
                    *height,
                    config,
                    runtime,
                    mappings,
                )
            }
        }
    }

    /// Convert a multi-child split to nested binary splits
    ///
    /// tmux layouts can have multiple children in a single split,
    /// but our pane tree uses binary splits. We convert like this:
    /// [A, B, C] -> Split(A, Split(B, C))
    fn convert_multi_split_to_binary(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        config: &Config,
        runtime: Arc<Runtime>,
        mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        if children.is_empty() {
            anyhow::bail!("Empty children list in tmux layout");
        }

        if children.len() == 1 {
            // Single child - just convert it directly
            return self.convert_layout_node(&children[0], config, runtime, mappings);
        }

        // Calculate the size of the first child for the ratio
        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        // Convert the first child
        let first = self.convert_layout_node(&children[0], config, runtime.clone(), mappings)?;

        // Calculate remaining size for the rest
        let remaining_size = total_size.saturating_sub(first_size + 1); // -1 for divider

        // Convert the rest recursively
        let second = if children.len() == 2 {
            self.convert_layout_node(&children[1], config, runtime, mappings)?
        } else {
            // Create a synthetic split node for the remaining children
            let remaining = &children[1..];
            self.convert_remaining_children(
                remaining,
                direction,
                remaining_size,
                config,
                runtime,
                mappings,
            )?
        };

        Ok(PaneNode::split(direction, ratio, first, second))
    }

    /// Convert remaining children into nested binary splits
    fn convert_remaining_children(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        config: &Config,
        runtime: Arc<Runtime>,
        mappings: &mut HashMap<TmuxPaneId, PaneId>,
    ) -> Result<PaneNode> {
        if children.len() == 1 {
            return self.convert_layout_node(&children[0], config, runtime, mappings);
        }

        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        let first = self.convert_layout_node(&children[0], config, runtime.clone(), mappings)?;

        let remaining_size = total_size.saturating_sub(first_size + 1);
        let second = self.convert_remaining_children(
            &children[1..],
            direction,
            remaining_size,
            config,
            runtime,
            mappings,
        )?;

        Ok(PaneNode::split(direction, ratio, first, second))
    }

    /// Get the size of a node in the given direction
    fn get_node_size(node: &LayoutNode, direction: SplitDirection) -> usize {
        match node {
            LayoutNode::Pane { width, height, .. } => match direction {
                SplitDirection::Vertical => *width,
                SplitDirection::Horizontal => *height,
            },
            LayoutNode::VerticalSplit { width, height, .. }
            | LayoutNode::HorizontalSplit { width, height, .. } => match direction {
                SplitDirection::Vertical => *width,
                SplitDirection::Horizontal => *height,
            },
        }
    }

    /// Update the layout structure (ratios) from a tmux layout without recreating terminals
    ///
    /// This is called when the tmux pane IDs haven't changed but the layout
    /// dimensions have (e.g., due to resize or another client connecting).
    /// It updates the split ratios in our pane tree to match the tmux layout.
    pub fn update_layout_from_tmux(
        &mut self,
        layout: &TmuxLayout,
        pane_mappings: &HashMap<TmuxPaneId, PaneId>,
    ) {
        // Calculate ratios from the tmux layout and update our tree
        if let Some(ref mut root) = self.root {
            Self::update_node_from_tmux_layout(root, &layout.root, pane_mappings);
        }

        log::debug!(
            "Updated pane layout ratios from tmux layout ({} panes)",
            pane_mappings.len()
        );
    }

    /// Recursively update a pane node's ratios and directions from tmux layout
    fn update_node_from_tmux_layout(
        node: &mut PaneNode,
        tmux_node: &LayoutNode,
        pane_mappings: &HashMap<TmuxPaneId, PaneId>,
    ) {
        match (node, tmux_node) {
            // Leaf nodes - nothing to update for ratios
            (PaneNode::Leaf(_), LayoutNode::Pane { .. }) => {}

            // Split node with VerticalSplit layout (panes side by side)
            (
                PaneNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                },
                LayoutNode::VerticalSplit {
                    width, children, ..
                },
            ) if !children.is_empty() => {
                // Update direction to match tmux layout
                if *direction != SplitDirection::Vertical {
                    log::debug!(
                        "Updating split direction from {:?} to Vertical to match tmux layout",
                        direction
                    );
                    *direction = SplitDirection::Vertical;
                }

                // Calculate ratio from first child's width vs total
                let first_size = Self::get_node_size(&children[0], SplitDirection::Vertical);
                let total_size = *width;
                if total_size > 0 {
                    *ratio = (first_size as f32) / (total_size as f32);
                    log::debug!(
                        "Updated vertical split ratio: {} / {} = {}",
                        first_size,
                        total_size,
                        *ratio
                    );
                }

                // Recursively update first child
                Self::update_node_from_tmux_layout(first, &children[0], pane_mappings);

                // For the second child, handle multi-child case
                if children.len() == 2 {
                    Self::update_node_from_tmux_layout(second, &children[1], pane_mappings);
                } else if children.len() > 2 {
                    // Our tree is binary but tmux has N children
                    // The second child is a nested split containing children[1..]
                    // Recursively update with remaining children treated as a nested split
                    Self::update_nested_split(
                        second,
                        &children[1..],
                        SplitDirection::Vertical,
                        pane_mappings,
                    );
                }
            }

            // Split node with HorizontalSplit layout (panes stacked)
            (
                PaneNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                },
                LayoutNode::HorizontalSplit {
                    height, children, ..
                },
            ) if !children.is_empty() => {
                // Update direction to match tmux layout
                if *direction != SplitDirection::Horizontal {
                    log::debug!(
                        "Updating split direction from {:?} to Horizontal to match tmux layout",
                        direction
                    );
                    *direction = SplitDirection::Horizontal;
                }

                // Calculate ratio from first child's height vs total
                let first_size = Self::get_node_size(&children[0], SplitDirection::Horizontal);
                let total_size = *height;
                if total_size > 0 {
                    *ratio = (first_size as f32) / (total_size as f32);
                    log::debug!(
                        "Updated horizontal split ratio: {} / {} = {}",
                        first_size,
                        total_size,
                        *ratio
                    );
                }

                // Recursively update first child
                Self::update_node_from_tmux_layout(first, &children[0], pane_mappings);

                // For the second child, handle multi-child case
                if children.len() == 2 {
                    Self::update_node_from_tmux_layout(second, &children[1], pane_mappings);
                } else if children.len() > 2 {
                    // Our tree is binary but tmux has N children
                    Self::update_nested_split(
                        second,
                        &children[1..],
                        SplitDirection::Horizontal,
                        pane_mappings,
                    );
                }
            }

            // Mismatched structure - log and skip
            _ => {
                log::debug!("Layout structure mismatch during update - skipping ratio update");
            }
        }
    }

    /// Update a nested binary split from a flat list of tmux children
    fn update_nested_split(
        node: &mut PaneNode,
        children: &[LayoutNode],
        direction: SplitDirection,
        pane_mappings: &HashMap<TmuxPaneId, PaneId>,
    ) {
        if children.is_empty() {
            return;
        }

        if children.len() == 1 {
            // Single child - update directly
            Self::update_node_from_tmux_layout(node, &children[0], pane_mappings);
            return;
        }

        // Multiple children - node should be a split
        if let PaneNode::Split {
            ratio,
            first,
            second,
            ..
        } = node
        {
            // Calculate ratio: first child size vs remaining total
            let first_size = Self::get_node_size(&children[0], direction);
            let remaining_size: usize = children
                .iter()
                .map(|c| Self::get_node_size(c, direction))
                .sum();

            if remaining_size > 0 {
                *ratio = (first_size as f32) / (remaining_size as f32);
                log::debug!(
                    "Updated nested split ratio: {} / {} = {}",
                    first_size,
                    remaining_size,
                    *ratio
                );
            }

            // Update first child
            Self::update_node_from_tmux_layout(first, &children[0], pane_mappings);

            // Recurse for remaining children
            Self::update_nested_split(second, &children[1..], direction, pane_mappings);
        } else {
            // Node isn't a split but we expected one - update as single
            Self::update_node_from_tmux_layout(node, &children[0], pane_mappings);
        }
    }

    /// Update an existing pane tree to match a new tmux layout
    ///
    /// This tries to preserve existing panes where possible and only
    /// creates/destroys panes as needed.
    ///
    /// Returns updated mappings (Some = new mappings, None = no changes needed)
    pub fn update_from_tmux_layout(
        &mut self,
        layout: &TmuxLayout,
        existing_mappings: &HashMap<TmuxPaneId, PaneId>,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<Option<HashMap<TmuxPaneId, PaneId>>> {
        // Get the pane IDs from the new layout
        let new_pane_ids: std::collections::HashSet<_> = layout.pane_ids().into_iter().collect();

        // Check if the pane set has changed
        let existing_tmux_ids: std::collections::HashSet<_> =
            existing_mappings.keys().copied().collect();

        if new_pane_ids == existing_tmux_ids {
            // Same panes, just need to update the layout structure
            // For now, we rebuild completely since layout changes are complex
            // A future optimization could preserve terminals and just restructure
            log::debug!("tmux layout changed but same panes - rebuilding structure");
        }

        // For now, always rebuild the tree completely
        // A more sophisticated implementation would try to preserve terminals
        let new_mappings = self.set_from_tmux_layout(layout, config, runtime)?;
        Ok(Some(new_mappings))
    }
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attempting to remove a pane from the tree
enum RemoveResult {
    /// Pane was removed, returning the new subtree (or None if empty)
    Removed(Option<PaneNode>),
    /// Pane was not found, returning the original tree
    NotFound(PaneNode),
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests would require mocking TerminalManager
    // These are placeholder tests for the manager logic

    #[test]
    fn test_pane_manager_new() {
        let manager = PaneManager::new();
        assert!(manager.root.is_none());
        assert_eq!(manager.pane_count(), 0);
        assert!(!manager.has_multiple_panes());
    }
}
