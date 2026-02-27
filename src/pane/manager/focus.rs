//! Focus and navigation operations for PaneManager
//!
//! Handles pane focus state, directional navigation, click-to-focus,
//! and closing panes with automatic focus transfer.

use super::PaneManager;
use crate::pane::types::{NavigationDirection, Pane, PaneId};

impl PaneManager {
    /// Close a pane by ID
    ///
    /// Returns true if this was the last pane (tab should close)
    pub fn close_pane(&mut self, id: PaneId) -> bool {
        crate::debug_info!("PANE_CLOSE", "close_pane called for pane {}", id);

        if let Some(root) = self.root.take() {
            match Self::remove_pane(root, id) {
                crate::pane::tmux_helpers::RemoveResult::Removed(new_root) => {
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
                crate::pane::tmux_helpers::RemoveResult::NotFound(root) => {
                    crate::debug_info!("PANE_CLOSE", "Pane {} not found in tree", id);
                    self.root = Some(root);
                }
            }
        }

        self.root.is_none()
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
}
