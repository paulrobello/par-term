//! Split pane operations: split, navigate, resize, close panes.

use std::sync::Arc;

use super::super::window_state::WindowState;

impl WindowState {
    /// Shared implementation for trigger- and keyboard-initiated pane splits.
    ///
    /// Handles renderer bounds query, tmux delegation, and the split call.
    /// Returns the new pane ID on success, None on failure.
    ///
    /// `initial_command` — when `Some((cmd, args))` the new pane launches that
    /// process directly instead of the login shell. The pane closes when the
    /// process exits.
    pub(crate) fn split_pane_direction(
        &mut self,
        direction: crate::pane::SplitDirection,
        focus_new: bool,
        initial_command: Option<(String, Vec<String>)>,
        split_percent: u8,
    ) -> Option<crate::pane::PaneId> {
        // Calculate status bar height for proper content area
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            let scale = r.scale_factor();
            (
                size,
                padding,
                content_offset_y,
                cell_width,
                cell_height,
                scale,
            )
        });

        let dpi_scale = bounds_info.map(|b| b.5).unwrap_or(1.0);

        let tab = self.tab_manager.active_tab_mut()?;

        // Set pane bounds before split if we have renderer info
        if let Some((size, padding, content_offset_y, cell_width, cell_height, scale)) = bounds_info
        {
            // After split there will be multiple panes, so use 0 padding if configured
            let effective_padding = if self.config.hide_window_padding_on_split {
                0.0
            } else {
                padding
            };
            // Scale status_bar_height from logical to physical pixels
            let physical_status_bar_height = (status_bar_height + custom_status_bar_height) * scale;
            let content_width = size.width as f32 - effective_padding * 2.0;
            let content_height = size.height as f32
                - content_offset_y
                - effective_padding
                - physical_status_bar_height;
            let bounds = crate::pane::PaneBounds::new(
                effective_padding,
                content_offset_y,
                content_width,
                content_height,
            );
            tab.set_pane_bounds(bounds, cell_width, cell_height);
        }

        let result = match direction {
            crate::pane::SplitDirection::Horizontal => tab.split_horizontal(
                focus_new,
                &self.config,
                Arc::clone(&self.runtime),
                dpi_scale,
                initial_command,
                split_percent,
            ),
            crate::pane::SplitDirection::Vertical => tab.split_vertical(
                focus_new,
                &self.config,
                Arc::clone(&self.runtime),
                dpi_scale,
                initial_command,
                split_percent,
            ),
        };

        match result {
            Ok(Some(pane_id)) => {
                log::info!("Split pane {:?}, new pane {}", direction, pane_id);
                // Clear renderer cells to remove stale single-pane data
                if let Some(renderer) = &mut self.renderer {
                    renderer.clear_all_cells();
                }
                // Invalidate tab cache — must re-borrow since we moved tab above
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_cache_mut().cells = None;
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
                Some(pane_id)
            }
            Ok(None) => {
                log::info!(
                    "{:?} split not yet functional (renderer integration pending)",
                    direction
                );
                None
            }
            Err(e) => {
                log::error!("Failed to split pane {:?}: {}", direction, e);
                None
            }
        }
    }

    /// Split the current pane horizontally (panes stacked top/bottom)
    pub fn split_pane_horizontal(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(false) {
            crate::debug_info!("TMUX", "Sent horizontal split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected
        self.split_pane_direction(crate::pane::SplitDirection::Horizontal, true, None, 50);
    }

    /// Split the current pane vertically (panes side by side)
    pub fn split_pane_vertical(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(true) {
            crate::debug_info!("TMUX", "Sent vertical split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected
        self.split_pane_direction(crate::pane::SplitDirection::Vertical, true, None, 50);
    }

    /// Close the focused pane in the current tab
    ///
    /// If this is the last pane, the tab is closed.
    /// Returns true if the window should close (last tab was closed).
    pub fn close_focused_pane(&mut self) -> bool {
        // In tmux mode, send kill-pane command to tmux
        if self.is_tmux_connected() && self.close_pane_via_tmux() {
            crate::debug_info!("TMUX", "Sent kill-pane command to tmux");
            // Don't close the local pane - wait for tmux layout change
            return false;
        }
        // Fall through to local close if tmux command failed or not connected

        // Check if we need to show confirmation for running jobs
        if self.config.confirm_close_running_jobs
            && let Some(command_name) = self.check_current_pane_running_job()
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(pane_id) = tab.focused_pane_id()
        {
            let tab_id = tab.id;
            let tab_title = if tab.title.is_empty() {
                "Terminal".to_string()
            } else {
                tab.title.clone()
            };
            self.overlay_ui.close_confirmation_ui.show_for_pane(
                tab_id,
                pane_id,
                &tab_title,
                &command_name,
            );
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return false; // Don't close yet, waiting for confirmation
        }

        self.close_focused_pane_immediately()
    }

    /// Close the focused pane immediately without confirmation
    /// Returns true if the window should close (last tab was closed).
    pub(crate) fn close_focused_pane_immediately(&mut self) -> bool {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            let is_last_pane = tab.close_focused_pane();
            if is_last_pane {
                // Last pane closed, close the tab
                return self.close_current_tab_immediately();
            }
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return false;
        }
        // Single pane or no tab, close the tab
        self.close_current_tab_immediately()
    }

    /// Check if the current pane's terminal has a running job that should trigger confirmation
    ///
    /// Returns Some(command_name) if confirmation should be shown, None otherwise.
    pub(super) fn check_current_pane_running_job(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;

        // If the tab has split panes, check the focused pane
        if tab.has_multiple_panes() {
            let pane_manager = tab.pane_manager()?;
            let focused_id = pane_manager.focused_pane_id()?;
            let pane = pane_manager.get_pane(focused_id)?;
            // blocking_read: user-initiated close — must not silently skip confirmation.
            // should_confirm_close() only needs &self; read lock is correct.
            let term = pane.terminal.blocking_read();
            log::info!(
                "[CLOSE_CONFIRM] check_current_pane_running_job (split pane): marker={:?} command={:?}",
                term.shell_integration_marker(),
                term.shell_integration_command()
            );
            return term.should_confirm_close(&self.config.jobs_to_ignore);
        }

        // Single pane - use the tab's terminal.
        // blocking_read: user-initiated close — must not silently skip confirmation.
        let term = tab.terminal.blocking_read();
        log::info!(
            "[CLOSE_CONFIRM] check_current_pane_running_job (single pane): marker={:?} command={:?} is_running={}",
            term.shell_integration_marker(),
            term.shell_integration_command(),
            term.is_command_running()
        );
        term.should_confirm_close(&self.config.jobs_to_ignore)
    }

    /// Check if the current tab has multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.tab_manager
            .active_tab()
            .is_some_and(|tab| tab.has_multiple_panes())
    }

    /// Navigate to an adjacent pane in the given direction
    pub fn navigate_pane(&mut self, direction: crate::pane::NavigationDirection) {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            tab.navigate_pane(direction);
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Resize the focused pane in the given direction
    ///
    /// Growing left/up decreases the pane's ratio, growing right/down increases it
    pub fn resize_pane(&mut self, direction: crate::pane::NavigationDirection) {
        use crate::pane::NavigationDirection;

        // Resize step: 5% per keypress
        const RESIZE_DELTA: f32 = 0.05;

        // Determine delta based on direction
        // Right/Down: grow focused pane (positive delta)
        // Left/Up: shrink focused pane (negative delta)
        let delta = match direction {
            NavigationDirection::Right | NavigationDirection::Down => RESIZE_DELTA,
            NavigationDirection::Left | NavigationDirection::Up => -RESIZE_DELTA,
        };

        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(pm) = tab.pane_manager_mut()
            && let Some(focused_id) = pm.focused_pane_id()
        {
            pm.resize_split(focused_id, delta);
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }
}
