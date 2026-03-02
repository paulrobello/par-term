//! Tmux layout integration for PaneManager
//!
//! Handles building and rebuilding the pane tree from tmux layouts.
//! Provides both full-replace (`set_from_tmux_layout`) and incremental
//! (`rebuild_from_tmux_layout`) operations.
//!
//! Related sub-modules:
//! - `tmux_convert`: Converting new tmux layouts to pane trees (fresh creation).
//! - `tmux_update`: In-place ratio/direction updates without recreating terminals.

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
}
