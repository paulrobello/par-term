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
    /// Returns the ID of the new pane, or None if no pane is focused or the
    /// split would leave a pane below `pane_min_size` (PN4).
    ///
    /// `initial_command` — when `Some((cmd, args))` the new pane launches that
    /// process directly instead of the login shell. The pane closes when the
    /// process exits.
    pub fn split(
        &mut self,
        direction: SplitDirection,
        focus_new: bool,
        config: &Config,
        runtime: Arc<Runtime>,
        initial_command: Option<(String, Vec<String>)>,
        ratio: f32,
    ) -> Result<Option<PaneId>> {
        self.split_placed(
            direction,
            false,
            focus_new,
            config,
            runtime,
            initial_command.map(|(cmd, args)| crate::pane::LaunchCommand::new(cmd, args)),
            ratio,
        )
    }

    /// [`Self::split`] with the new pane placed before (left of / above)
    /// the focused one when `before` is set (UX.md A5). `launch` is the new
    /// pane's program (a split command, or the tab's inherited profile,
    /// D5); `None` runs the configured shell.
    #[allow(clippy::too_many_arguments)] // split() plus the placement flag
    pub fn split_placed(
        &mut self,
        direction: SplitDirection,
        before: bool,
        focus_new: bool,
        config: &Config,
        runtime: Arc<Runtime>,
        launch: Option<crate::pane::LaunchCommand>,
        ratio: f32,
    ) -> Result<Option<PaneId>> {
        self.split_placed_in(
            direction, before, focus_new, config, runtime, launch, ratio, None,
        )
    }

    /// [`Self::split_placed`] with the new pane started in `cwd` instead of
    /// the focused pane's directory — a profile split carries the profile's
    /// working directory (UX.md PR1).
    #[allow(clippy::too_many_arguments)] // split_placed() plus the directory
    pub fn split_placed_in(
        &mut self,
        direction: SplitDirection,
        before: bool,
        focus_new: bool,
        config: &Config,
        runtime: Arc<Runtime>,
        launch: Option<crate::pane::LaunchCommand>,
        ratio: f32,
        cwd: Option<String>,
    ) -> Result<Option<PaneId>> {
        let focused_id = match self.focused_pane_id {
            Some(id) => id,
            None => return Ok(None),
        };
        // A split changes the layout the zoom was hiding (A1).
        self.unzoom();
        // Refuse before spawning a shell that would only be dropped.
        if !self.can_split(direction) {
            log::info!("Split of pane {focused_id} refused: below pane_min_size");
            return Ok(None);
        }

        // Get the working directory and bounds from the focused pane
        let (working_dir, focused_bounds) = if let Some(pane) = self.focused_pane() {
            (pane.get_cwd(), pane.bounds)
        } else {
            (None, self.total_bounds)
        };
        let working_dir = cwd.or(working_dir);

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

        let mut new_pane = if let Some(launch) = launch {
            Pane::new_with_launch(new_id, &pane_config, runtime, working_dir, launch)?
        } else {
            Pane::new(new_id, &pane_config, runtime, working_dir)?
        };

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

        if self
            .split_with_pane(new_pane, direction, before, focus_new, ratio)
            .is_err()
        {
            return Ok(None);
        }

        crate::debug_info!(
            "PANE_SPLIT",
            "Split pane {} {:?} (before={}), created new pane {}",
            focused_id,
            direction,
            before,
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
        ratio: f32,
    ) -> (PaneNode, Option<Pane>) {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    if let Some(new) = new_pane {
                        // This is the pane to split - create a new split node
                        (
                            PaneNode::split(
                                direction,
                                ratio,
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
                ratio: existing_ratio,
                first,
                second,
            } => {
                // Try to insert in first child
                let (new_first, remaining) =
                    Self::split_node(*first, target_id, direction, new_pane, ratio);

                if remaining.is_none() {
                    // Target was found in first child
                    (
                        PaneNode::Split {
                            direction: split_dir,
                            ratio: existing_ratio,
                            first: Box::new(new_first),
                            second,
                        },
                        None,
                    )
                } else {
                    // Target not in first, try second
                    let (new_second, remaining) =
                        Self::split_node(*second, target_id, direction, remaining, ratio);
                    (
                        PaneNode::Split {
                            direction: split_dir,
                            ratio: existing_ratio,
                            first: Box::new(new_first),
                            second: Box::new(new_second),
                        },
                        remaining,
                    )
                }
            }
        }
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

    /// Recursive helper: find a target leaf and replace it with a Split
    /// containing the leaf and the subtree to insert.
    ///
    /// Returns `Ok(new_node)` on success, or `Err(original_node)` if the
    /// target was not found (so the caller can restore the tree).
    pub(super) fn insert_subtree_at_node(
        node: PaneNode,
        target_id: PaneId,
        subtree: PaneNode,
        direction: crate::pane::types::SplitDirection,
        ratio: f32,
    ) -> Result<PaneNode, (PaneNode, PaneNode)> {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    Ok(PaneNode::Split {
                        direction,
                        ratio,
                        first: Box::new(PaneNode::Leaf(pane)),
                        second: Box::new(subtree),
                    })
                } else {
                    Err((PaneNode::Leaf(pane), subtree))
                }
            }
            PaneNode::Split {
                direction: split_dir,
                ratio: existing_ratio,
                first,
                second,
            } => match Self::insert_subtree_at_node(*first, target_id, subtree, direction, ratio) {
                Ok(new_first) => Ok(PaneNode::Split {
                    direction: split_dir,
                    ratio: existing_ratio,
                    first: Box::new(new_first),
                    second,
                }),
                Err((first_node, subtree)) => {
                    match Self::insert_subtree_at_node(
                        *second, target_id, subtree, direction, ratio,
                    ) {
                        Ok(new_second) => Ok(PaneNode::Split {
                            direction: split_dir,
                            ratio: existing_ratio,
                            first: Box::new(first_node),
                            second: Box::new(new_second),
                        }),
                        Err((second_node, subtree)) => Err((
                            PaneNode::Split {
                                direction: split_dir,
                                ratio: existing_ratio,
                                first: Box::new(first_node),
                                second: Box::new(second_node),
                            },
                            subtree,
                        )),
                    }
                }
            },
        }
    }

    /// Recursive helper: extract a pane from the tree, returning the live Pane.
    pub(super) fn extract_pane_from_node(
        node: PaneNode,
        target_id: PaneId,
    ) -> super::ExtractInternal {
        match node {
            PaneNode::Leaf(pane) => {
                if pane.id == target_id {
                    super::ExtractInternal::OnlyPane(*pane)
                } else {
                    super::ExtractInternal::NotFound(PaneNode::Leaf(pane))
                }
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => match Self::extract_pane_from_node(*first, target_id) {
                super::ExtractInternal::OnlyPane(pane) => super::ExtractInternal::Extracted {
                    pane,
                    remaining: *second,
                },
                super::ExtractInternal::Extracted { pane, remaining } => {
                    super::ExtractInternal::Extracted {
                        pane,
                        remaining: PaneNode::Split {
                            direction,
                            ratio,
                            first: Box::new(remaining),
                            second,
                        },
                    }
                }
                super::ExtractInternal::NotFound(first_node) => {
                    match Self::extract_pane_from_node(*second, target_id) {
                        super::ExtractInternal::OnlyPane(pane) => {
                            super::ExtractInternal::Extracted {
                                pane,
                                remaining: first_node,
                            }
                        }
                        super::ExtractInternal::Extracted { pane, remaining } => {
                            super::ExtractInternal::Extracted {
                                pane,
                                remaining: PaneNode::Split {
                                    direction,
                                    ratio,
                                    first: Box::new(first_node),
                                    second: Box::new(remaining),
                                },
                            }
                        }
                        super::ExtractInternal::NotFound(second_node) => {
                            super::ExtractInternal::NotFound(PaneNode::Split {
                                direction,
                                ratio,
                                first: Box::new(first_node),
                                second: Box::new(second_node),
                            })
                        }
                    }
                }
            },
        }
    }
}
