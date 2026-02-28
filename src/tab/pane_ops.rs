//! Tab split pane operations.
//!
//! Provides methods for creating, closing, navigating, and resizing split panes
//! within a tab. Pane operations use `PaneManager` to manage the binary tree
//! layout of panes.

use crate::config::Config;
use crate::pane::{NavigationDirection, PaneManager, SplitDirection};
use crate::tab::Tab;
use std::sync::Arc;
use tokio::runtime::Runtime;

impl Tab {
    /// Check if this tab has multiple panes (split)
    pub fn has_multiple_panes(&self) -> bool {
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.has_multiple_panes())
    }

    /// Get the number of panes in this tab
    pub fn pane_count(&self) -> usize {
        self.pane_manager
            .as_ref()
            .map(|pm| pm.pane_count())
            .unwrap_or(1)
    }

    /// Split the current pane horizontally (panes stacked vertically)
    ///
    /// Returns the new pane ID if successful.
    /// `dpi_scale` converts logical pixel config values to physical pixels.
    pub fn split_horizontal(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Horizontal, config, runtime, dpi_scale)
    }

    /// Split the current pane vertically (panes side by side)
    ///
    /// Returns the new pane ID if successful.
    /// `dpi_scale` converts logical pixel config values to physical pixels.
    pub fn split_vertical(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Vertical, config, runtime, dpi_scale)
    }

    /// Split the focused pane in the given direction.
    /// `dpi_scale` is used to convert logical pixel config values to physical pixels.
    fn split(
        &mut self,
        direction: SplitDirection,
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        // Check max panes limit
        if config.max_panes > 0 && self.pane_count() >= config.max_panes {
            log::warn!(
                "Cannot split: max panes limit ({}) reached",
                config.max_panes
            );
            return Ok(None);
        }

        // Initialize pane manager and create initial pane if needed
        let needs_initial_pane = self
            .pane_manager
            .as_ref()
            .map(|pm| pm.pane_count() == 0)
            .unwrap_or(true);

        if needs_initial_pane {
            // Create pane manager if it doesn't exist
            if self.pane_manager.is_none() {
                let mut pm = PaneManager::new();
                // Scale from logical pixels (config) to physical pixels for layout
                pm.set_divider_width(config.pane_divider_width.unwrap_or(2.0) * dpi_scale);
                pm.set_divider_hit_width(config.pane_divider_hit_width * dpi_scale);
                self.pane_manager = Some(pm);
            }

            // Create initial pane with size calculated for AFTER the split
            // (since we know a split is about to happen)
            if let Some(ref mut pm) = self.pane_manager {
                pm.create_initial_pane_for_split(
                    direction,
                    config,
                    Arc::clone(&runtime),
                    self.working_directory.clone(),
                )?;
                log::info!(
                    "Created PaneManager for tab {} with initial pane on first split",
                    self.id
                );
            }
        }

        // Perform the split
        if let Some(ref mut pm) = self.pane_manager {
            let new_pane_id = pm.split(direction, config, Arc::clone(&runtime))?;
            if let Some(id) = new_pane_id {
                log::info!("Split tab {} {:?}, new pane {}", self.id, direction, id);
            }
            Ok(new_pane_id)
        } else {
            Ok(None)
        }
    }

    /// Close the focused pane
    ///
    /// Returns true if this was the last pane (tab should close)
    pub fn close_focused_pane(&mut self) -> bool {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(focused_id) = pm.focused_pane_id()
        {
            let is_last = pm.close_pane(focused_id);
            if is_last {
                // Last pane closed, clear the pane manager
                self.pane_manager = None;
            }
            return is_last;
        }
        // No pane manager or no focused pane means single pane tab
        true
    }

    /// Check for exited panes and close them
    ///
    /// Returns (closed_pane_ids, tab_should_close) where:
    /// - `closed_pane_ids`: Vec of pane IDs that were closed
    /// - `tab_should_close`: true if all panes have exited (tab should close)
    pub fn close_exited_panes(&mut self) -> (Vec<crate::pane::PaneId>, bool) {
        let mut closed_panes = Vec::new();

        // Get IDs of panes whose shells have exited
        let exited_pane_ids: Vec<crate::pane::PaneId> = if let Some(ref pm) = self.pane_manager {
            let focused_id = pm.focused_pane_id();
            pm.all_panes()
                .iter()
                .filter_map(|pane| {
                    let is_running = pane.is_running();
                    crate::debug_info!(
                        "PANE_CHECK",
                        "Pane {} running={} focused={} bounds=({:.0},{:.0} {:.0}x{:.0})",
                        pane.id,
                        is_running,
                        focused_id == Some(pane.id),
                        pane.bounds.x,
                        pane.bounds.y,
                        pane.bounds.width,
                        pane.bounds.height
                    );
                    if !is_running { Some(pane.id) } else { None }
                })
                .collect()
        } else {
            Vec::new()
        };

        // Close each exited pane
        if let Some(ref mut pm) = self.pane_manager {
            for pane_id in exited_pane_ids {
                crate::debug_info!("PANE_CLOSE", "Closing pane {} - shell exited", pane_id);
                let is_last = pm.close_pane(pane_id);
                closed_panes.push(pane_id);

                if is_last {
                    // Last pane closed, clear the pane manager
                    self.pane_manager = None;
                    return (closed_panes, true);
                }
            }
        }

        (closed_panes, false)
    }

    /// Get the pane manager if split panes are enabled
    pub fn pane_manager(&self) -> Option<&PaneManager> {
        self.pane_manager.as_ref()
    }

    /// Get mutable access to the pane manager
    pub fn pane_manager_mut(&mut self) -> Option<&mut PaneManager> {
        self.pane_manager.as_mut()
    }

    /// Initialize the pane manager if not already present
    ///
    /// This is used for tmux integration where we need to create the pane manager
    /// before applying a layout.
    pub fn init_pane_manager(&mut self) {
        if self.pane_manager.is_none() {
            self.pane_manager = Some(PaneManager::new());
        }
    }

    /// Set the pane bounds and resize terminals
    ///
    /// This should be called before creating splits to ensure panes are sized correctly.
    /// If the pane manager doesn't exist yet, this creates it with the bounds set.
    pub fn set_pane_bounds(
        &mut self,
        bounds: crate::pane::PaneBounds,
        cell_width: f32,
        cell_height: f32,
    ) {
        self.set_pane_bounds_with_padding(bounds, cell_width, cell_height, 0.0);
    }

    /// Set the pane bounds and resize terminals with padding
    ///
    /// This should be called before creating splits to ensure panes are sized correctly.
    /// The padding parameter accounts for content inset from pane edges.
    pub fn set_pane_bounds_with_padding(
        &mut self,
        bounds: crate::pane::PaneBounds,
        cell_width: f32,
        cell_height: f32,
        padding: f32,
    ) {
        if self.pane_manager.is_none() {
            let mut pm = PaneManager::new();
            pm.set_bounds(bounds);
            self.pane_manager = Some(pm);
        } else if let Some(ref mut pm) = self.pane_manager {
            pm.set_bounds(bounds);
            pm.resize_all_terminals_with_padding(cell_width, cell_height, padding, 0.0);
        }
    }

    /// Focus the pane at the given pixel coordinates
    ///
    /// Returns the ID of the newly focused pane, or None if no pane at that position
    pub fn focus_pane_at(&mut self, x: f32, y: f32) -> Option<crate::pane::PaneId> {
        if let Some(ref mut pm) = self.pane_manager {
            pm.focus_pane_at(x, y)
        } else {
            None
        }
    }

    /// Get the ID of the currently focused pane
    pub fn focused_pane_id(&self) -> Option<crate::pane::PaneId> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane_id())
    }

    /// Check if a specific pane is focused
    pub fn is_pane_focused(&self, pane_id: crate::pane::PaneId) -> bool {
        self.focused_pane_id() == Some(pane_id)
    }

    /// Navigate to an adjacent pane
    pub fn navigate_pane(&mut self, direction: NavigationDirection) {
        if let Some(ref mut pm) = self.pane_manager {
            pm.navigate(direction);
        }
    }

    /// Check if a position is on a divider
    pub fn is_on_divider(&self, x: f32, y: f32) -> bool {
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.is_on_divider(x, y))
    }

    /// Find divider at position
    ///
    /// Returns the divider index if found
    pub fn find_divider_at(&self, x: f32, y: f32) -> Option<usize> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.find_divider_at(x, y, pm.divider_hit_padding()))
    }

    /// Get divider info by index
    pub fn get_divider(&self, index: usize) -> Option<crate::pane::DividerRect> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.get_divider(index))
    }

    /// Drag a divider to a new position
    pub fn drag_divider(&mut self, divider_index: usize, x: f32, y: f32) {
        if let Some(ref mut pm) = self.pane_manager {
            pm.drag_divider(divider_index, x, y);
        }
    }

    /// Restore a pane layout from a saved session
    ///
    /// Replaces the current single-pane layout with a saved pane tree.
    /// Each leaf in the tree gets a new terminal session with the saved CWD.
    /// If the build fails, the tab keeps its existing single pane.
    pub fn restore_pane_layout(
        &mut self,
        layout: &crate::session::SessionPaneNode,
        config: &Config,
        runtime: Arc<Runtime>,
    ) {
        let mut pm = PaneManager::new();
        pm.set_divider_width(config.pane_divider_width.unwrap_or(1.0));
        pm.set_divider_hit_width(config.pane_divider_hit_width);

        match pm.build_from_layout(layout, config, runtime) {
            Ok(()) => {
                log::info!(
                    "Restored pane layout for tab {} ({} panes)",
                    self.id,
                    pm.pane_count()
                );
                self.pane_manager = Some(pm);
            }
            Err(e) => {
                log::warn!(
                    "Failed to restore pane layout for tab {}: {}, keeping single pane",
                    self.id,
                    e
                );
            }
        }
    }
}
