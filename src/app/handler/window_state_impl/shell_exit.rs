//! Shell exit action handling for WindowState (called from the RedrawRequested event arm).
//!
//! Contains:
//! - `handle_shell_exit`: dispatches shell exit actions (Keep, Close, Restart variants)
//!   for all tabs and panes. Returns true if the window should close.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Handle shell exit based on the configured `shell_exit_action`.
    ///
    /// Returns `true` if the window should close (last tab exited and action is Close).
    pub(crate) fn handle_shell_exit(&mut self) -> bool {
        use crate::config::ShellExitAction;
        use crate::pane::RestartState;

        match self.config.shell_exit_action {
            ShellExitAction::Keep => {
                // Do nothing - keep dead shells showing
            }

            ShellExitAction::Close => {
                // Original behavior: close exited panes and their tabs
                let mut tabs_needing_resize: Vec<crate::tab::TabId> = Vec::new();

                let tabs_to_close: Vec<crate::tab::TabId> = self
                    .tab_manager
                    .tabs_mut()
                    .iter_mut()
                    .filter_map(|tab| {
                        if tab.tmux_gateway_active || tab.tmux_pane_id.is_some() {
                            return None;
                        }
                        if tab.pane_manager.is_some() {
                            let (closed_panes, tab_should_close) = tab.close_exited_panes();
                            if !closed_panes.is_empty() {
                                log::info!(
                                    "Tab {}: closed {} exited pane(s)",
                                    tab.id,
                                    closed_panes.len()
                                );
                                if !tab_should_close {
                                    tabs_needing_resize.push(tab.id);
                                }
                            }
                            if tab_should_close {
                                return Some(tab.id);
                            }
                        }
                        None
                    })
                    .collect();

                if !tabs_needing_resize.is_empty()
                    && let Some(renderer) = &self.renderer
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let padding = self.config.pane_padding;
                    let title_offset = if self.config.show_pane_titles {
                        self.config.pane_title_height
                    } else {
                        0.0
                    };
                    for tab_id in tabs_needing_resize {
                        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                            && let Some(pm) = tab.pane_manager_mut()
                        {
                            pm.resize_all_terminals_with_padding(
                                cell_width,
                                cell_height,
                                padding,
                                title_offset,
                            );
                        }
                    }
                }

                for tab_id in &tabs_to_close {
                    log::info!("Closing tab {} - all panes exited", tab_id);
                    if self.tab_manager.tab_count() <= 1 {
                        log::info!("Last tab, closing window");
                        self.is_shutting_down = true;
                        for tab in self.tab_manager.tabs_mut() {
                            tab.stop_refresh_task();
                        }
                        return true;
                    } else {
                        let _ = self.tab_manager.close_tab(*tab_id);
                    }
                }

                // Also check legacy single-pane tabs
                let (shell_exited, active_tab_id, tab_count, tab_title, exit_notified) = {
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let exited = tab.pane_manager.is_none()
                            // try_lock: intentional — shell exit check during RedrawRequested
                            // in the sync event loop. On miss: treat as still running
                            // (is_some_and returns false on None), so the tab stays open
                            // until the next frame resolves the exit.
                            && tab
                                .terminal
                                .try_write()
                                .ok()
                                .is_some_and(|term| !term.is_running());
                        (
                            exited,
                            Some(tab.id),
                            self.tab_manager.tab_count(),
                            tab.title.clone(),
                            tab.exit_notified,
                        )
                    } else {
                        (false, None, 0, String::new(), false)
                    }
                };

                if shell_exited {
                    log::info!("Shell in active tab has exited");
                    if self.config.notification_session_ended && !exit_notified {
                        if let Some(tab) = self.tab_manager.active_tab_mut() {
                            tab.exit_notified = true;
                        }
                        let title = format!("Session Ended: {}", tab_title);
                        let message = "The shell process has exited".to_string();
                        self.deliver_notification(&title, &message);
                    }

                    if tab_count <= 1 {
                        log::info!("Last tab, closing window");
                        self.is_shutting_down = true;
                        for tab in self.tab_manager.tabs_mut() {
                            tab.stop_refresh_task();
                        }
                        return true;
                    } else if let Some(tab_id) = active_tab_id {
                        let _ = self.tab_manager.close_tab(tab_id);
                    }
                }
            }

            ShellExitAction::RestartImmediately
            | ShellExitAction::RestartWithPrompt
            | ShellExitAction::RestartAfterDelay => {
                // Handle restart variants
                let config_clone = self.config.clone();

                for tab in self.tab_manager.tabs_mut() {
                    if tab.tmux_gateway_active || tab.tmux_pane_id.is_some() {
                        continue;
                    }

                    if let Some(pm) = tab.pane_manager_mut() {
                        for pane in pm.all_panes_mut() {
                            let is_running = pane.is_running();

                            // Check if pane needs restart action
                            if !is_running && pane.restart_state.is_none() {
                                // Shell just exited, handle based on action
                                match self.config.shell_exit_action {
                                    ShellExitAction::RestartImmediately => {
                                        log::info!(
                                            "Pane {} shell exited, restarting immediately",
                                            pane.id
                                        );
                                        if let Err(e) = pane.respawn_shell(&config_clone) {
                                            log::error!(
                                                "Failed to respawn shell in pane {}: {}",
                                                pane.id,
                                                e
                                            );
                                        }
                                    }
                                    ShellExitAction::RestartWithPrompt => {
                                        log::info!(
                                            "Pane {} shell exited, showing restart prompt",
                                            pane.id
                                        );
                                        pane.write_restart_prompt();
                                        pane.restart_state = Some(RestartState::AwaitingInput);
                                    }
                                    ShellExitAction::RestartAfterDelay => {
                                        log::info!(
                                            "Pane {} shell exited, will restart after 1s",
                                            pane.id
                                        );
                                        pane.restart_state = Some(RestartState::AwaitingDelay(
                                            std::time::Instant::now(),
                                        ));
                                    }
                                    _ => {}
                                }
                            }

                            // Check if waiting for delay and time has elapsed
                            if let Some(RestartState::AwaitingDelay(exit_time)) =
                                &pane.restart_state
                                && exit_time.elapsed() >= std::time::Duration::from_secs(1)
                            {
                                log::info!("Pane {} delay elapsed, restarting shell", pane.id);
                                if let Err(e) = pane.respawn_shell(&config_clone) {
                                    log::error!(
                                        "Failed to respawn shell in pane {}: {}",
                                        pane.id,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        false // Window stays open
    }
}
