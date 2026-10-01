//! Tab query and duplication helpers for WindowState.
//!
//! Contains:
//! - `duplicate_tab`, `duplicate_tab_by_id` — duplicate an existing tab
//! - `has_multiple_tabs` — query predicate
//! - `active_terminal` — accessor for the active tab's terminal
//! - `confirm_current_tab_close` — running-job confirmation gate for a tab
//!   close (local tabs read the tab's terminal; attached par-mux tabs ask
//!   the daemon about every pane in the window)
//! - `check_current_tab_running_job` — the local tab's running-job check

//! - Debug logging for close confirmation flow

use std::sync::Arc;

use super::super::window_state::WindowState;
#[cfg(feature = "mux")]
use super::pane_ops::MuxJobCheck;

impl WindowState {
    /// Duplicate current tab
    pub fn duplicate_tab(&mut self) {
        match self.tab_manager.active_tab_id() {
            Some(id) => self.duplicate_tab_by_id(id),
            None => log::debug!("No active tab to duplicate"),
        }
    }

    /// Duplicate a specific tab by ID: same cwd, color, and icon, and the
    /// same pane layout with each pane in its source pane's directory
    /// (PN11). A tmux or par-mux display tab's panes belong to the server,
    /// so only its cwd is copied.
    pub fn duplicate_tab_by_id(&mut self, source_tab_id: crate::tab::TabId) {
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());
        let layout = self
            .tab_manager
            .get_tab(source_tab_id)
            .filter(|t| t.tmux.tmux_pane_id.is_none() && !t.tmux.tmux_gateway_active)
            .and_then(|t| crate::session::capture::capture_tab_snapshot(t).pane_layout);

        match self.tab_manager.duplicate_tab_by_id(
            source_tab_id,
            &self.config.load(),
            Arc::clone(&self.runtime),
            grid_size,
        ) {
            Ok(Some(tab_id)) => {
                let config = self.config.load_full();
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    if let Some(layout) = &layout {
                        tab.restore_pane_layout(layout, &config, Arc::clone(&self.runtime));
                    }
                    if let Some(window) = &self.window {
                        tab.start_refresh_task(
                            Arc::clone(&self.runtime),
                            Arc::clone(window),
                            config.rendering.max_fps,
                            config.power.inactive_tab_fps,
                        );
                        tab.start_pane_refresh_tasks(
                            Arc::clone(&self.runtime),
                            Arc::clone(window),
                            config.rendering.max_fps,
                            config.power.inactive_tab_fps,
                        );
                    }
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
    ) -> Option<&Arc<tokio::sync::RwLock<par_term_terminal::TerminalManager>>> {
        self.tab_manager.active_tab().map(|tab| &tab.terminal)
    }

    /// Hold the active tab's close behind the running-job confirmation when
    /// closing it would end a job. Returns true when the dialog is now up
    /// and the close must wait for its answer.
    ///
    /// An attached par-mux tab's own terminal is a hidden local shell that
    /// never runs the job, and the close kills the whole daemon window, so
    /// the daemon is asked about every live pane in it — the same bounded,
    /// separate-connection check as a pane close, failing closed when the
    /// daemon cannot answer in time.
    pub(super) fn confirm_current_tab_close(&mut self) -> bool {
        if !self.config.load().shell.confirm_close_running_jobs {
            return false;
        }
        #[cfg(feature = "mux")]
        if let Some(check) = self.check_mux_tab_running_job() {
            return match check {
                MuxJobCheck::Idle => false,
                MuxJobCheck::Job(name) => self.show_tab_close_confirmation(Some(&name)),
                MuxJobCheck::Unanswered => self.show_tab_close_confirmation(None),
            };
        }
        match self.check_current_tab_running_job() {
            Some(name) => self.show_tab_close_confirmation(Some(&name)),
            None => false,
        }
    }

    /// The running-job check for the active tab when it mirrors a par-mux
    /// daemon window, over every daemon pane in it that has not exited (a
    /// held pane has no job, and an all-held tab's close must stay the
    /// plain `kill-window`). `None` for a tab that mirrors no daemon window.
    #[cfg(feature = "mux")]
    fn check_mux_tab_running_job(&self) -> Option<MuxJobCheck> {
        let tab_id = self.tab_manager.active_tab_id()?;
        self.mux_window_for_tab(tab_id)?;
        let mut panes: Vec<u64> = self
            .tmux_state
            .tab_tmux_pane_ids(tab_id)
            .into_iter()
            .filter(|pane| !self.tmux_state.mux_exited_panes.contains_key(pane))
            .collect();
        panes.sort_unstable();
        Some(self.check_mux_panes_running_job(&panes))
    }

    /// Open the running-job confirmation for the active tab, naming
    /// `command_name` (`None`: the job could not be checked). Returns true:
    /// the close waits for the answer.
    fn show_tab_close_confirmation(&mut self, command_name: Option<&str>) -> bool {
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };
        let tab_id = tab.id;
        let tab_title = if tab.title.is_empty() {
            "Terminal".to_string()
        } else {
            tab.title.clone()
        };
        log::info!(
            "[CLOSE_TAB] Showing close confirmation for tab {} with running command: {:?}",
            tab_id,
            command_name
        );
        let dialog = &mut self.overlay_ui.close_confirmation_ui;
        match command_name {
            Some(name) => dialog.show_for_tab(tab_id, &tab_title, name),
            None => dialog.show_for_tab_unverified(tab_id, &tab_title),
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        true
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
            self.config.load().shell.confirm_close_running_jobs,
            self.config.load().shell.jobs_to_ignore.len()
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
        let result = term.should_confirm_close(&self.config.load().shell.jobs_to_ignore);
        log::info!("[CLOSE_CONFIRM] should_confirm_close result={:?}", result);
        result
    }
}
