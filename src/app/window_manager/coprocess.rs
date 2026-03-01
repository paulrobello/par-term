//! Coprocess lifecycle management for the window manager.
//!
//! This module handles starting and stopping coprocesses on the active tab,
//! and syncing their running state and output to the settings window UI.

use super::WindowManager;

impl WindowManager {
    /// Start a coprocess by config index on the focused window's active tab.
    pub fn start_coprocess(&mut self, config_index: usize) {
        log::debug!("start_coprocess called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            if config_index >= ws.config.coprocesses.len() {
                log::warn!("Coprocess config index {} out of range", config_index);
                return;
            }
            let coproc_config = &ws.config.coprocesses[config_index];
            let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                command: coproc_config.command.clone(),
                args: coproc_config.args.clone(),
                cwd: None,
                env: crate::terminal::coprocess_env(),
                copy_terminal_output: coproc_config.copy_terminal_output,
                restart_policy: coproc_config.restart_policy.to_core(),
                restart_delay_ms: coproc_config.restart_delay_ms,
            };
            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            let term = tab.terminal.blocking_write();
            match term.start_coprocess(core_config) {
                Ok(id) => {
                    log::info!("Started coprocess '{}' (id={})", coproc_config.name, id);
                    // Ensure coprocess_ids vec is large enough
                    while tab.scripting.coprocess_ids.len() <= config_index {
                        tab.scripting.coprocess_ids.push(None);
                    }
                    tab.scripting.coprocess_ids[config_index] = Some(id);
                }
                Err(e) => {
                    let err_msg = format!("Failed to start: {}", e);
                    log::error!("Failed to start coprocess '{}': {}", coproc_config.name, e);
                    // Show error in settings UI
                    if let Some(sw) = &mut self.settings_window {
                        let errors = &mut sw.settings_ui.coprocess_errors;
                        while errors.len() <= config_index {
                            errors.push(String::new());
                        }
                        errors[config_index] = err_msg;
                        sw.request_redraw();
                    }
                    return;
                }
            }
            drop(term);
            // Update running state in settings window
            self.sync_coprocess_running_state();
        } else {
            log::warn!("start_coprocess: no focused window or active tab found");
        }
    }

    /// Stop a coprocess by config index on the focused window's active tab.
    pub fn stop_coprocess(&mut self, config_index: usize) {
        log::debug!("stop_coprocess called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            if let Some(Some(id)) = tab.scripting.coprocess_ids.get(config_index).copied() {
                // Acceptable risk: blocking_lock() from sync event loop for infrequent
                // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
                let term = tab.terminal.blocking_write();
                if let Err(e) = term.stop_coprocess(id) {
                    log::error!("Failed to stop coprocess at index {}: {}", config_index, e);
                } else {
                    log::info!("Stopped coprocess at index {} (id={})", config_index, id);
                }
                drop(term);
                tab.scripting.coprocess_ids[config_index] = None;
            }
            // Update running state in settings window
            self.sync_coprocess_running_state();
        }
    }

    /// Maximum number of output lines kept per coprocess in the UI.
    const COPROCESS_OUTPUT_MAX_LINES: usize = 200;

    /// Sync coprocess running state to the settings window.
    pub fn sync_coprocess_running_state(&mut self) {
        let focused = self.get_focused_window_id();
        let (running_state, error_state, new_output): (Vec<bool>, Vec<String>, Vec<Vec<String>>) =
            if let Some(window_id) = focused
                && let Some(ws) = self.windows.get(&window_id)
                && let Some(tab) = ws.tab_manager.active_tab()
            {
                if let Ok(term) = tab.terminal.try_write() {
                    let mut running = Vec::new();
                    let mut errors = Vec::new();
                    let mut output = Vec::new();
                    for (i, _) in ws.config.coprocesses.iter().enumerate() {
                        let has_id = tab
                            .scripting
                            .coprocess_ids
                            .get(i)
                            .and_then(|opt| opt.as_ref());
                        let is_running =
                            has_id.is_some_and(|id| term.coprocess_status(*id).unwrap_or(false));
                        // If coprocess has an id but is not running, check stderr.
                        let err_text = if let Some(id) = has_id {
                            if is_running {
                                String::new()
                            } else {
                                term.read_coprocess_errors(*id)
                                    .unwrap_or_default()
                                    .join("\n")
                            }
                        } else if let Some(sw) = &self.settings_window
                            && let Some(existing) = sw.settings_ui.coprocess_errors.get(i)
                            && !existing.is_empty()
                        {
                            existing.clone()
                        } else {
                            String::new()
                        };
                        // Drain stdout buffer from the core
                        let lines = if let Some(id) = has_id {
                            term.read_from_coprocess(*id).unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        running.push(is_running);
                        errors.push(err_text);
                        output.push(lines);
                    }
                    (running, errors, output)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        if let Some(sw) = &mut self.settings_window {
            let running_changed = sw.settings_ui.coprocess_running != running_state;
            let errors_changed = sw.settings_ui.coprocess_errors != error_state;
            let has_new_output = new_output.iter().any(|lines| !lines.is_empty());

            // Ensure output/expanded vecs are the right size
            let count = running_state.len();
            sw.settings_ui.coprocess_output.resize_with(count, Vec::new);
            sw.settings_ui
                .coprocess_output_expanded
                .resize(count, false);

            // Append new output lines, capping at max
            for (i, lines) in new_output.into_iter().enumerate() {
                if !lines.is_empty() {
                    let buf = &mut sw.settings_ui.coprocess_output[i];
                    buf.extend(lines);
                    let overflow = buf.len().saturating_sub(Self::COPROCESS_OUTPUT_MAX_LINES);
                    if overflow > 0 {
                        buf.drain(..overflow);
                    }
                }
            }

            if running_changed || errors_changed || has_new_output {
                sw.settings_ui.coprocess_running = running_state;
                sw.settings_ui.coprocess_errors = error_state;
                sw.request_redraw();
            }
        }
    }
}
