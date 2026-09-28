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
            // A daemon (or tmux) swap-pane reorders the layout's leaves
            // without changing any size, so the ratio pass above cannot see
            // it. Realign the leaves: every tree position must hold the
            // native pane whose tmux id the layout puts at that position.
            Self::realign_leaves_from_tmux(root, &layout.root, pane_mappings);
        }

        log::debug!(
            "Updated pane layout ratios from tmux layout ({} panes)",
            pane_mappings.len()
        );
    }

    /// Permute the tree's leaves so each position holds the native pane
    /// whose tmux id the layout string puts there.
    ///
    /// Only runs when every leaf is identified in BOTH directions (forward
    /// `tmux -> native` mapping covers every layout leaf, inverse covers
    /// every tree leaf); otherwise identities are unknowable and the tree
    /// is left alone. Skips the no-op permutation silently.
    fn realign_leaves_from_tmux(
        root: &mut PaneNode,
        tmux_node: &LayoutNode,
        pane_mappings: &HashMap<TmuxPaneId, PaneId>,
    ) {
        fn collect_layout_leaves(node: &LayoutNode, leaves: &mut Vec<TmuxPaneId>) {
            match node {
                LayoutNode::Pane { id, .. } => leaves.push(*id),
                LayoutNode::HorizontalSplit { children, .. }
                | LayoutNode::VerticalSplit { children, .. } => {
                    for child in children {
                        collect_layout_leaves(child, leaves);
                    }
                }
            }
        }

        let mut desired: Vec<TmuxPaneId> = Vec::new();
        collect_layout_leaves(tmux_node, &mut desired);

        let native_to_tmux: HashMap<PaneId, TmuxPaneId> = pane_mappings
            .iter()
            .map(|(&tmux, &native)| (native, tmux))
            .collect();
        let mut current = root.all_pane_ids();
        if desired.len() != current.len()
            || !current.iter().all(|id| native_to_tmux.contains_key(id))
        {
            return;
        }

        // Selection-sort the permutation: while position i holds the wrong
        // pane, swap it with the position that holds the pane `desired[i]`
        // wants. With one mismatched pair this is a single swap.
        for i in 0..desired.len() {
            let at_i = native_to_tmux[&current[i]];
            if at_i == desired[i] {
                continue;
            }
            let Some(j) =
                (i + 1..desired.len()).find(|&j| native_to_tmux[&current[j]] == desired[i])
            else {
                return; // identity missing mid-permutation — leave the rest
            };
            root.swap_panes(current[i], current[j]);
            current.swap(i, j);
        }
    }

    /// Re-fit each mapped pane's terminal to its daemon-side cell geometry.
    ///
    /// The layout string's leaf `WxH` is the authority for a mirror
    /// terminal's grid: after a resize driven by another client
    /// (`refresh-client -C`, `resize-pane`), the local pane must adopt it
    /// or the desktop keeps rendering at its own window's size with
    /// wrapping drift. Runs AFTER `update_layout_from_tmux` (ratios) and
    /// `resize_all_terminals` (local metrics) so the daemon geometry wins.
    pub fn resize_pane_terminals_from_layout(
        &self,
        layout: &TmuxLayout,
        pane_mappings: &HashMap<TmuxPaneId, PaneId>,
    ) {
        fn collect_leaves(node: &LayoutNode, leaves: &mut Vec<(TmuxPaneId, usize, usize)>) {
            match node {
                LayoutNode::Pane {
                    id, width, height, ..
                } => {
                    leaves.push((*id, *width, *height));
                }
                LayoutNode::HorizontalSplit { children, .. }
                | LayoutNode::VerticalSplit { children, .. } => {
                    for child in children {
                        collect_leaves(child, leaves);
                    }
                }
            }
        }

        let mut leaves = Vec::new();
        collect_leaves(&layout.root, &mut leaves);
        let mut adopted = 0;
        for (tmux_id, cols, rows) in leaves {
            if let Some(pane) = pane_mappings
                .get(&tmux_id)
                .and_then(|pid| self.get_pane(*pid))
            {
                // `resize_terminal` is a no-op when the grid already
                // matches, so the attach-fit echo re-adopts for free.
                pane.resize_terminal(cols.max(1), rows.max(1));
                adopted += 1;
            }
        }
        if adopted > 0 {
            log::debug!("Adopted daemon pane geometry for {adopted} mirror terminal(s)");
        }
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
