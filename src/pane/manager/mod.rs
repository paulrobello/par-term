//! Pane manager for coordinating pane operations within a tab
//!
//! The PaneManager owns the pane tree and provides operations for:
//! - Splitting panes horizontally and vertically
//! - Closing panes
//! - Navigating between panes
//! - Resizing panes
//!
//! Sub-modules:
//! - [`super::tmux_helpers`]: Helper types and free functions for tmux layout operations.
//! - [`creation`]: Pane creation and tree manipulation (split, remove).
//! - [`focus`]: Focus management and directional navigation.
//! - [`layout`]: Bounds, resize, and divider operations.
//! - [`session`]: Session restore from saved layout.
//! - [`tmux_layout`]: Full tmux layout integration (set, rebuild, update).

mod creation;
mod focus;
mod layout;
mod session;
mod tmux_layout;

use crate::config::{Config, PaneBackgroundConfig};
use crate::pane::types::{Pane, PaneBounds, PaneId, PaneNode};
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Manages the pane tree within a single tab
pub struct PaneManager {
    /// Root of the pane tree (None if no panes yet)
    pub(super) root: Option<PaneNode>,
    /// ID of the currently focused pane
    pub(super) focused_pane_id: Option<PaneId>,
    /// Counter for generating unique pane IDs
    pub(super) next_pane_id: PaneId,
    /// Width of dividers between panes in pixels
    pub(super) divider_width: f32,
    /// Width of the hit area for divider drag detection
    pub(super) divider_hit_width: f32,
    /// Current total bounds available for panes
    pub(super) total_bounds: PaneBounds,
}

impl PaneManager {
    /// Create a new empty pane manager
    pub fn new() -> Self {
        Self {
            root: None,
            focused_pane_id: None,
            next_pane_id: 1,
            divider_width: 1.0,     // Default 1 pixel divider
            divider_hit_width: 8.0, // Default 8 pixel hit area
            total_bounds: PaneBounds::default(),
        }
    }

    /// Create a pane manager with an initial pane
    pub fn with_initial_pane(
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<Self> {
        let mut manager = Self::new();
        manager.divider_width = config.pane_divider_width.unwrap_or(1.0);
        manager.divider_hit_width = config.pane_divider_hit_width;
        manager.create_initial_pane(config, runtime, working_directory)?;
        Ok(manager)
    }

    /// Get the next pane ID that will be assigned
    pub fn next_pane_id(&self) -> PaneId {
        self.next_pane_id
    }

    /// Get a pane by ID
    pub fn get_pane(&self, id: PaneId) -> Option<&Pane> {
        self.root.as_ref()?.find_pane(id)
    }

    /// Get a mutable pane by ID
    pub fn get_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.root.as_mut()?.find_pane_mut(id)
    }

    /// Get all panes
    pub fn all_panes(&self) -> Vec<&Pane> {
        self.root
            .as_ref()
            .map(|r| r.all_panes())
            .unwrap_or_default()
    }

    /// Get all panes mutably
    pub fn all_panes_mut(&mut self) -> Vec<&mut Pane> {
        self.root
            .as_mut()
            .map(|r| r.all_panes_mut())
            .unwrap_or_default()
    }

    /// Collect current per-pane background settings for config persistence
    ///
    /// Returns a `Vec<PaneBackgroundConfig>` containing only panes that have
    /// a custom background image set. The `index` field corresponds to the
    /// pane's position in the tree traversal order.
    pub fn collect_pane_backgrounds(&self) -> Vec<PaneBackgroundConfig> {
        self.all_panes()
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                pane.background
                    .image_path
                    .as_ref()
                    .map(|path| PaneBackgroundConfig {
                        index,
                        image: path.clone(),
                        mode: pane.background.mode,
                        opacity: pane.background.opacity,
                        darken: pane.background.darken,
                    })
            })
            .collect()
    }

    /// Get the number of panes
    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map(|r| r.pane_count()).unwrap_or(0)
    }

    /// Check if there are multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.pane_count() > 1
    }

    /// Get access to the root node (for rendering)
    pub fn root(&self) -> Option<&PaneNode> {
        self.root.as_ref()
    }

    /// Get mutable access to the root node
    pub fn root_mut(&mut self) -> Option<&mut PaneNode> {
        self.root.as_mut()
    }
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests would require mocking TerminalManager
    // These are placeholder tests for the manager logic

    #[test]
    fn test_pane_manager_new() {
        let manager = PaneManager::new();
        assert!(manager.root.is_none());
        assert_eq!(manager.pane_count(), 0);
        assert!(!manager.has_multiple_panes());
    }
}
