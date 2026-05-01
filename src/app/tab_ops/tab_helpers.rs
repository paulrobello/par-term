//! Tab query and duplication helpers for WindowState.
//!
//! Contains:
//! - `duplicate_tab`, `duplicate_tab_by_id` — duplicate an existing tab
//! - `has_multiple_tabs` — query predicate
//! - `active_terminal` — accessor for the active tab's terminal
//! - `check_current_tab_running_job` — running-job confirmation gate

//! - Debug logging for close confirmation flow

use std::sync::Arc;

use super::super::window_state::WindowState;

impl WindowState {
    /// Duplicate current tab
    pub fn duplicate_tab(&mut self) {
        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.duplicate_active_tab(
            &self.config.load(),
            Arc::clone(&self.runtime),
            grid_size,
        ) {
            Ok(Some(tab_id)) => {
                // Start refresh task for the new tab
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.load().max_fps,
                        self.config.load().inactive_tab_fps,
                    );
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Ok(None) => {
                log::debug!("No active tab to duplicate");
            }
            Err(e) => {
                log::error!("Failed to duplicate tab: {}", e);
            }
        }
    }

    /// Duplicate a specific tab by ID
    pub fn duplicate_tab_by_id(&mut self, source_tab_id: crate::tab::TabId) {
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.duplicate_tab_by_id(
            source_tab_id,
            &self.config.load(),
            Arc::clone(&self.runtime),
            grid_size,
        ) {
            Ok(Some(tab_id)) => {
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.load().max_fps,
                        self.config.load().inactive_tab_fps,
                    );
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Ok(None) => {
                log::debug!("Tab {} not found for duplication", source_tab_id);
            }
            Err(e) => {
                log::error!("Failed to duplicate tab {}: {}", source_tab_id, e);
            }
        }
    }

    /// Check if there are multiple tabs
    pub fn has_multiple_tabs(&self) -> bool {
        self.tab_manager.has_multiple_tabs()
    }

    /// Get the active tab's terminal
    pub fn active_terminal(
        &self,
    ) -> Option<&Arc<tokio::sync::RwLock<crate::terminal::TerminalManager>>> {
        self.tab_manager.active_tab().map(|tab| &tab.terminal)
    }

    /// Check if the current tab's terminal has a running job that should trigger confirmation
    ///
    /// Returns Some(command_name) if confirmation should be shown, None otherwise.
    pub(super) fn check_current_tab_running_job(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        // blocking_read: user-initiated close — we must not silently skip confirmation.
        // should_confirm_close() only needs &self so a shared read lock is correct.
        // blocking_read() waits for any active writer (e.g. async key/mouse task) to
        // finish rather than returning None and letting the tab close without prompting.
        let term = tab.terminal.blocking_read();
        log::info!(
            "[CLOSE_CONFIRM] checking: confirm_close_running_jobs={} jobs_to_ignore_len={}",
            self.config.load().confirm_close_running_jobs,
            self.config.load().jobs_to_ignore.len()
        );
        let marker = term.shell_integration_marker();
        let command_name = term.shell_integration_command();
        let is_command_running = term.is_command_running();
        log::info!(
            "[CLOSE_CONFIRM] shell_integration: marker={:?} command_name={:?} is_command_running={}",
            marker,
            command_name,
            is_command_running
        );
        let result = term.should_confirm_close(&self.config.load().jobs_to_ignore);
        log::info!("[CLOSE_CONFIRM] should_confirm_close result={:?}", result);
        result
    }
}
