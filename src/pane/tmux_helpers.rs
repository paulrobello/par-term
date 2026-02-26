//! Tmux layout and divider update helper types for pane management.
//!
//! Contains lightweight context structs and helper types used during
//! tmux layout rebuild operations and divider ratio updates in `PaneManager`.

use super::types::{Pane, PaneBounds, PaneId, PaneNode};
use crate::config::Config;
use crate::tmux::TmuxPaneId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Context for divider ratio update operations.
/// Groups the immutable parameters passed through recursive calls in `update_divider_ratio`.
#[derive(Clone, Copy)]
pub(super) struct DividerUpdateContext {
    pub(super) target_index: usize,
    pub(super) new_x: f32,
    pub(super) new_y: f32,
    pub(super) bounds: PaneBounds,
    pub(super) divider_width: f32,
}

/// Context for tmux layout rebuild operations.
/// Groups the shared parameters passed through `rebuild_layout_node`,
/// `rebuild_multi_split_to_binary`, and `rebuild_remaining_children`.
pub(super) struct TmuxLayoutRebuildContext<'a> {
    pub(super) existing_mappings: &'a HashMap<TmuxPaneId, PaneId>,
    pub(super) new_tmux_panes: &'a [TmuxPaneId],
    pub(super) existing_panes: &'a mut HashMap<PaneId, Pane>,
    pub(super) config: &'a Config,
    pub(super) runtime: Arc<Runtime>,
    pub(super) new_mappings: &'a mut HashMap<TmuxPaneId, PaneId>,
}

/// Result of attempting to remove a pane from the tree
pub(super) enum RemoveResult {
    /// Pane was removed, returning the new subtree (or None if empty)
    Removed(Option<PaneNode>),
    /// Pane was not found, returning the original tree
    NotFound(PaneNode),
}

/// Extract all panes from a node into a map
pub(super) fn extract_panes_from_node(node: PaneNode, panes: &mut HashMap<PaneId, Pane>) {
    match node {
        PaneNode::Leaf(pane) => {
            let pane = *pane; // Unbox the pane
            panes.insert(pane.id, pane);
        }
        PaneNode::Split { first, second, .. } => {
            extract_panes_from_node(*first, panes);
            extract_panes_from_node(*second, panes);
        }
    }
}
