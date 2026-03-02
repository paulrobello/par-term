//! Tmux layout conversion to native pane tree.
//!
//! Provides `convert_layout_node` and its helpers, which build a fresh pane
//! tree from a tmux layout by creating new `Pane` objects for every node.
//!
//! For rebuilds that reuse existing panes, see `tmux_layout.rs`.
//! For ratio-only updates, see `tmux_update.rs`.

use super::PaneManager;
use crate::config::Config;
use crate::pane::types::{Pane, PaneId, PaneNode, SplitDirection};
use crate::tmux::{LayoutNode, TmuxPaneId};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl PaneManager {
    /// Convert a tmux LayoutNode to a PaneNode
    pub(super) fn convert_layout_node(
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
    pub(super) fn convert_multi_split_to_binary(
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
    pub(super) fn convert_remaining_children(
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
}
