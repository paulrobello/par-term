//! Pane/tab promotion: promote pane to tab, demote tab to pane.

use std::sync::Arc;

use par_term_config::TabId;

use super::super::window_state::WindowState;
use crate::pane::{ExtractResult, PaneId, SplitDirection};

/// State machine for the multi-step demote (tab → pane) pick mode.
#[derive(Default)]
pub(crate) enum PaneTransferState {
    #[default]
    Idle,
    DemotePickTab {
        source_tab_id: TabId,
    },
    DemotePickPane {
        source_tab_id: TabId,
        target_tab_id: TabId,
    },
    DemoteChooseDirection {
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
    },
}

impl PaneTransferState {
    pub fn is_active(&self) -> bool {
        !matches!(self, PaneTransferState::Idle)
    }
}

impl WindowState {
    /// Promote the focused pane in the current tab to its own tab.
    pub fn promote_pane_to_tab(&mut self) {
        let source_tab_id = match self.tab_manager.active_tab_id() {
            Some(id) => id,
            None => return,
        };

        let focused_pane_id = match self
            .tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id())
        {
            Some(id) => id,
            None => return,
        };

        // Extract the pane from the source tab's tree
        let pane = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => {
                let pm = match tab.pane_manager_mut() {
                    Some(pm) => pm,
                    None => return,
                };
                match pm.extract_pane(focused_pane_id) {
                    ExtractResult::Extracted { pane, remaining } => {
                        // Put the remaining tree back into the source tab
                        if let Some(remaining_node) = remaining
                            && let Some(tab) = self.tab_manager.get_tab_mut(source_tab_id)
                            && let Some(pm) = tab.pane_manager_mut()
                        {
                            pm.set_root(remaining_node);
                        }
                        pane
                    }
                    ExtractResult::OnlyPane(pane) => pane,
                    ExtractResult::NotFound => return,
                }
            }
            None => return,
        };

        // Check if source tab is now empty (pane_manager root is None)
        let source_is_empty = self
            .tab_manager
            .get_tab(source_tab_id)
            .is_none_or(|t| t.pane_count() == 0);

        // Create and insert the new tab after the source
        let new_tab_id = self.tab_manager.new_tab_from_pane(
            pane,
            &self.config.load(),
            Arc::clone(&self.runtime),
            if source_is_empty {
                None
            } else {
                Some(source_tab_id)
            },
        );

        // If source was empty, remove it without killing the terminal
        // (the promoted pane still holds an Arc to the same terminal)
        if source_is_empty {
            if let Some(source_tab) = self.tab_manager.get_tab_mut(source_tab_id) {
                source_tab.shutdown_fast = true;
                source_tab.stop_refresh_task();
            }
            let _ = self.tab_manager.remove_tab(source_tab_id);
        }

        // Start refresh tasks for the new tab
        if let Some(window) = &self.window
            && let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id)
        {
            tab.start_refresh_task(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().max_fps,
                self.config.load().inactive_tab_fps,
            );
            tab.start_pane_refresh_tasks(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().max_fps,
                self.config.load().inactive_tab_fps,
            );
        }

        // Resize the new tab's terminal to match renderer dimensions
        if let Some(renderer) = &self.renderer
            && let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id)
        {
            let (cols, rows) = renderer.grid_size();
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (cols as f32 * cell_width) as usize;
            let height_px = (rows as f32 * cell_height) as usize;
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                }
            }

        // Clear renderer and request redraw
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!(
            "PANE_PROMOTE",
            "Promoted pane {} to new tab {}",
            focused_pane_id,
            new_tab_id
        );
    }

    /// Start the demote (tab → pane) pick mode.
    pub fn start_demote_tab(&mut self) {
        if self.tab_manager.tab_count() < 2 {
            log::warn!("Cannot demote tab: need at least 2 tabs");
            return;
        }
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            self.pane_transfer_state = PaneTransferState::DemotePickTab {
                source_tab_id: tab_id,
            };
            self.show_toast("Demote: Click a tab to merge into");
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            crate::debug_info!("TAB_DEMOTE", "Started demote pick mode for tab {}", tab_id);
        }
    }

    /// Cancel the demote pick mode.
    pub fn cancel_pane_transfer(&mut self) {
        self.pane_transfer_state = PaneTransferState::Idle;
        self.show_toast("Demote cancelled");
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Execute the demote: merge source tab's pane tree into target tab.
    pub(crate) fn execute_demote(
        &mut self,
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
        direction: SplitDirection,
    ) {
        // Check max_panes on target tab
        let config = self.config.load();
        if config.max_panes > 0 {
            let target_count = self
                .tab_manager
                .get_tab(target_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            let source_count = self
                .tab_manager
                .get_tab(source_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            if target_count + source_count > config.max_panes {
                log::warn!(
                    "Cannot demote: would exceed max_panes ({})",
                    config.max_panes
                );
                self.cancel_pane_transfer();
                return;
            }
        }
        drop(config);

        // Extract the source tab's entire pane tree
        let source_tree = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => match tab.pane_manager_mut() {
                Some(pm) => pm.take_root(),
                None => {
                    self.cancel_pane_transfer();
                    return;
                }
            },
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        let source_tree = match source_tree {
            Some(tree) => tree,
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        // Insert the source tree into the target tab
        let inserted = match self.tab_manager.get_tab_mut(target_tab_id) {
            Some(tab) => match tab.pane_manager_mut() {
                Some(pm) => pm.insert_subtree_at(target_pane_id, source_tree, direction, 0.5),
                None => false,
            },
            None => false,
        };

        if !inserted {
            self.cancel_pane_transfer();
            return;
        }

        // Close the source tab without killing terminals (panes are now in target)
        // Setting shutdown_fast prevents Tab::Drop from killing self.terminal,
        // which is shared with the primary pane that was transplanted into the target.
        if let Some(source_tab) = self.tab_manager.get_tab_mut(source_tab_id) {
            source_tab.shutdown_fast = true;
            // Also stop the tab-level refresh task so it doesn't poll the
            // now-empty PaneManager after the tab is removed from the list.
            source_tab.stop_refresh_task();
        }
        let _ = self.tab_manager.remove_tab(source_tab_id);

        // Start refresh tasks for all panes in the target tab
        // (this also updates is_active on transplanted panes)
        if let Some(window) = &self.window
            && let Some(tab) = self.tab_manager.get_tab_mut(target_tab_id)
        {
            tab.start_pane_refresh_tasks(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().max_fps,
                self.config.load().inactive_tab_fps,
            );
        }

        self.pane_transfer_state = PaneTransferState::Idle;

        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!(
            "TAB_DEMOTE",
            "Demoted tab {} into tab {} at pane {}",
            source_tab_id,
            target_tab_id,
            target_pane_id
        );
    }
}
