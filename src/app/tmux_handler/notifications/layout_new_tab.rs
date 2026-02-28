//! Helper for creating a new tab when a tmux layout arrives without an existing tab mapping.
//!
//! Extracted from `layout.rs` to keep that file under the 800-line target.

use crate::app::window_state::WindowState;
use crate::tmux::{TmuxLayout, TmuxWindowId};

use super::layout::BoundsInfo;

impl WindowState {
    /// Create a brand-new tab and apply the tmux layout to it.
    ///
    /// Called when `handle_tmux_layout_change` receives a layout for a window
    /// that has no existing tab mapping.
    pub(super) fn create_tab_for_layout(
        &mut self,
        window_id: TmuxWindowId,
        parsed_layout: &TmuxLayout,
        pane_ids: &[crate::tmux::TmuxPaneId],
        bounds_info: BoundsInfo,
    ) {
        crate::debug_info!(
            "TMUX",
            "No tab mapping for window @{}, creating new tab for layout",
            window_id
        );

        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            return;
        }

        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());
        match self.tab_manager.new_tab(
            &self.config,
            std::sync::Arc::clone(&self.runtime),
            false,
            grid_size,
        ) {
            Ok(new_tab_id) => {
                crate::debug_info!(
                    "TMUX",
                    "Created tab {} for tmux window @{}",
                    new_tab_id,
                    window_id
                );

                self.tmux_state.tmux_sync.map_window(window_id, new_tab_id);

                if let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id) {
                    tab.init_pane_manager();
                    tab.set_title(&format!("tmux @{}", window_id));

                    if let Some(window) = &self.window {
                        tab.start_refresh_task(
                            std::sync::Arc::clone(&self.runtime),
                            std::sync::Arc::clone(window),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );
                    }

                    if let Some((
                        size,
                        padding,
                        content_offset_y,
                        content_inset_right,
                        _cell_width,
                        _cell_height,
                        status_bar_height,
                    )) = bounds_info
                        && let Some(pm) = tab.pane_manager_mut()
                    {
                        let content_width = size.width as f32 - padding * 2.0 - content_inset_right;
                        let content_height =
                            size.height as f32 - content_offset_y - padding - status_bar_height;
                        let bounds = crate::pane::PaneBounds::new(
                            padding,
                            content_offset_y,
                            content_width,
                            content_height,
                        );
                        pm.set_bounds(bounds);
                    }

                    if let Some(pm) = tab.pane_manager_mut() {
                        match pm.set_from_tmux_layout(
                            parsed_layout,
                            &self.config,
                            std::sync::Arc::clone(&self.runtime),
                        ) {
                            Ok(pane_mappings) => {
                                crate::debug_info!(
                                    "TMUX",
                                    "Storing pane mappings for new tab: {:?}",
                                    pane_mappings
                                );
                                self.tmux_state.native_pane_to_tmux_pane = pane_mappings
                                    .iter()
                                    .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                                    .collect();
                                self.tmux_state.tmux_pane_to_native_pane = pane_mappings;

                                if !pane_ids.is_empty() {
                                    tab.tmux_pane_id = Some(pane_ids[0]);
                                }

                                self.request_pane_refresh(pane_ids);
                                self.focus_state.needs_redraw = true;
                            }
                            Err(e) => {
                                crate::debug_error!(
                                    "TMUX",
                                    "Failed to apply tmux layout to new tab: {}",
                                    e
                                );
                            }
                        }
                    }
                }

                self.tab_manager.switch_to(new_tab_id);
            }
            Err(e) => {
                crate::debug_error!(
                    "TMUX",
                    "Failed to create tab for tmux window @{}: {}",
                    window_id,
                    e
                );
            }
        }
    }
}
