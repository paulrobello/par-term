//! Pane creation operations for PaneManager
//!
//! Handles creating new panes (initial, split, tmux-driven) and adding
//! them to the pane tree.

use super::PaneManager;
use crate::config::Config;
use crate::pane::tmux_helpers::RemoveResult;
use crate::pane::types::{Pane, PaneBounds, PaneId, PaneNode, SplitDirection};
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl PaneManager {
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
    pub(super) fn create_initial_pane_internal(
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
    pub(super) fn split_node(
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

    /// Remove a pane from the tree, returning the new tree structure
    pub(super) fn remove_pane(node: PaneNode, target_id: PaneId) -> RemoveResult {
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
}
