//! Session restore operations for PaneManager
//!
//! Handles rebuilding a live pane tree from a saved session layout,
//! including fallback CWD validation for missing directories.

use super::PaneManager;
use crate::config::Config;
use crate::pane::types::{Pane, PaneNode};
use crate::session::SessionPaneNode;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl PaneManager {
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
}
