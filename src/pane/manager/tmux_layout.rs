//! Tmux layout integration for PaneManager
//!
//! Handles building, rebuilding, and updating the pane tree from tmux layouts.
//! Provides both full-replace (`set_from_tmux_layout`) and incremental
//! (`rebuild_from_tmux_layout`) operations along with ratio-only updates
//! (`update_layout_from_tmux` / `update_from_tmux_layout`).

use super::PaneManager;
use crate::config::Config;
use crate::pane::tmux_helpers::{TmuxLayoutRebuildContext, extract_panes_from_node};
use crate::pane::types::{Pane, PaneId, PaneNode, SplitDirection};
use crate::tmux::{LayoutNode, TmuxLayout, TmuxPaneId};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl PaneManager {
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
            extract_panes_from_node(root, &mut existing_panes);
        }

        log::debug!(
            "Rebuilding layout: extracted {} existing panes, expecting {} new tmux panes",
            existing_panes.len(),
            new_tmux_panes.len()
        );

        // Build the new tree structure from the tmux layout
        let mut new_mappings = HashMap::new();
        let mut rebuild_ctx = TmuxLayoutRebuildContext {
            existing_mappings,
            new_tmux_panes,
            existing_panes: &mut existing_panes,
            config,
            runtime: runtime.clone(),
            new_mappings: &mut new_mappings,
        };
        let new_root = self.rebuild_layout_node(&layout.root, &mut rebuild_ctx)?;

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

    /// Rebuild a layout node, reusing existing panes where possible
    fn rebuild_layout_node(
        &mut self,
        node: &LayoutNode,
        ctx: &mut TmuxLayoutRebuildContext<'_>,
    ) -> Result<PaneNode> {
        match node {
            LayoutNode::Pane { id: tmux_id, .. } => {
                // Check if this is an existing pane we can reuse
                if let Some(&native_id) = ctx.existing_mappings.get(tmux_id)
                    && let Some(pane) = ctx.existing_panes.remove(&native_id)
                {
                    log::debug!(
                        "Reusing existing pane {} for tmux pane %{}",
                        native_id,
                        tmux_id
                    );
                    ctx.new_mappings.insert(*tmux_id, native_id);
                    return Ok(PaneNode::leaf(pane));
                }

                // This is a new pane - create it
                if ctx.new_tmux_panes.contains(tmux_id) {
                    let native_id = self.next_pane_id;
                    self.next_pane_id += 1;

                    let pane = Pane::new_for_tmux(native_id, ctx.config, ctx.runtime.clone())?;
                    log::debug!("Created new pane {} for tmux pane %{}", native_id, tmux_id);
                    ctx.new_mappings.insert(*tmux_id, native_id);
                    return Ok(PaneNode::leaf(pane));
                }

                // Fallback - create a new pane (shouldn't happen normally)
                log::warn!("Unexpected tmux pane %{} - creating new pane", tmux_id);
                let native_id = self.next_pane_id;
                self.next_pane_id += 1;
                let pane = Pane::new_for_tmux(native_id, ctx.config, ctx.runtime.clone())?;
                ctx.new_mappings.insert(*tmux_id, native_id);
                Ok(PaneNode::leaf(pane))
            }

            LayoutNode::VerticalSplit {
                width, children, ..
            } => {
                // Vertical split = panes side by side
                self.rebuild_multi_split_to_binary(children, SplitDirection::Vertical, *width, ctx)
            }

            LayoutNode::HorizontalSplit {
                height, children, ..
            } => {
                // Horizontal split = panes stacked
                self.rebuild_multi_split_to_binary(
                    children,
                    SplitDirection::Horizontal,
                    *height,
                    ctx,
                )
            }
        }
    }

    /// Rebuild multi-child split to binary, reusing existing panes
    fn rebuild_multi_split_to_binary(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        ctx: &mut TmuxLayoutRebuildContext<'_>,
    ) -> Result<PaneNode> {
        if children.is_empty() {
            anyhow::bail!("Empty children list in tmux layout");
        }

        if children.len() == 1 {
            return self.rebuild_layout_node(&children[0], ctx);
        }

        // Calculate the size of the first child for the ratio
        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        // Rebuild the first child
        let first = self.rebuild_layout_node(&children[0], ctx)?;

        // Calculate remaining size
        let remaining_size = total_size.saturating_sub(first_size + 1);

        // Rebuild the rest recursively
        let second = if children.len() == 2 {
            self.rebuild_layout_node(&children[1], ctx)?
        } else {
            self.rebuild_remaining_children(&children[1..], direction, remaining_size, ctx)?
        };

        Ok(PaneNode::split(direction, ratio, first, second))
    }

    /// Rebuild remaining children into nested binary splits
    fn rebuild_remaining_children(
        &mut self,
        children: &[LayoutNode],
        direction: SplitDirection,
        total_size: usize,
        ctx: &mut TmuxLayoutRebuildContext<'_>,
    ) -> Result<PaneNode> {
        if children.len() == 1 {
            return self.rebuild_layout_node(&children[0], ctx);
        }

        let first_size = Self::get_node_size(&children[0], direction);
        let ratio = (first_size as f32) / (total_size as f32);

        let first = self.rebuild_layout_node(&children[0], ctx)?;

        let remaining_size = total_size.saturating_sub(first_size + 1);
        let second =
            self.rebuild_remaining_children(&children[1..], direction, remaining_size, ctx)?;

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
    pub(super) fn get_node_size(node: &LayoutNode, direction: SplitDirection) -> usize {
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
