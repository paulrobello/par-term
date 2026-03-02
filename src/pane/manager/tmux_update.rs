//! In-place tmux layout ratio and direction updates.
//!
//! Provides `update_layout_from_tmux`, `update_from_tmux_layout`, and their
//! recursive helpers. These methods update the split ratios of an existing pane
//! tree to match a new tmux layout without recreating terminal sessions.
//!
//! For full-replace and rebuild operations, see `tmux_layout.rs`.
//! For creating new pane trees from tmux layouts, see `tmux_convert.rs`.

use super::PaneManager;
use crate::config::Config;
use crate::pane::types::{PaneId, PaneNode, SplitDirection};
use crate::tmux::{LayoutNode, TmuxLayout, TmuxPaneId};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl PaneManager {
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
