//! Script lifecycle management for the window manager.
//!
//! This module handles starting and stopping scripts on the active tab,
//! forwarding terminal events to scripts, reading script commands/output,
//! and syncing their running state to the settings window UI.

use super::WindowManager;

impl WindowManager {
    /// Start a script by config index on the focused window's active tab.
    pub fn start_script(&mut self, config_index: usize) {
        crate::debug_info!(
            "SCRIPT",
            "start_script called with config_index={}",
            config_index
        );
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            crate::debug_info!(
                "SCRIPT",
                "start_script: ws.config.scripts.len()={}, tab.script_ids.len()={}",
                ws.config.scripts.len(),
                tab.script_ids.len()
            );
            if config_index >= ws.config.scripts.len() {
                crate::debug_error!(
                    "SCRIPT",
                    "Script config index {} out of range (scripts.len={})",
                    config_index,
                    ws.config.scripts.len()
                );
                return;
            }
            let script_config = &ws.config.scripts[config_index];
            crate::debug_info!(
                "SCRIPT",
                "start_script: found config name='{}' path='{}' enabled={} args={:?}",
                script_config.name,
                script_config.script_path,
                script_config.enabled,
                script_config.args
            );
            if !script_config.enabled {
                crate::debug_info!(
                    "SCRIPT",
                    "Script '{}' is disabled, not starting",
                    script_config.name
                );
                return;
            }

            // Build subscription filter from config
            let subscription_filter = if script_config.subscriptions.is_empty() {
                None
            } else {
                Some(
                    script_config
                        .subscriptions
                        .iter()
                        .cloned()
                        .collect::<std::collections::HashSet<String>>(),
                )
            };

            // Create the event forwarder and register it as an observer
            let forwarder = std::sync::Arc::new(
                crate::scripting::observer::ScriptEventForwarder::new(subscription_filter),
            );

            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            let observer_id = {
                let term = tab.terminal.blocking_lock();
                term.add_observer(forwarder.clone())
            };

            // Start the script process
            crate::debug_info!("SCRIPT", "start_script: spawning process...");
            match tab.script_manager.start_script(script_config) {
                Ok(script_id) => {
                    crate::debug_info!(
                        "SCRIPT",
                        "start_script: SUCCESS script_id={} observer_id={:?}",
                        script_id,
                        observer_id
                    );

                    // Ensure vecs are large enough
                    while tab.script_ids.len() <= config_index {
                        tab.script_ids.push(None);
                    }
                    while tab.script_observer_ids.len() <= config_index {
                        tab.script_observer_ids.push(None);
                    }
                    while tab.script_forwarders.len() <= config_index {
                        tab.script_forwarders.push(None);
                    }

                    tab.script_ids[config_index] = Some(script_id);
                    tab.script_observer_ids[config_index] = Some(observer_id);
                    tab.script_forwarders[config_index] = Some(forwarder);
                }
                Err(e) => {
                    let err_msg = format!("Failed to start: {}", e);
                    crate::debug_error!(
                        "SCRIPT",
                        "start_script: FAILED to start '{}': {}",
                        script_config.name,
                        e
                    );

                    // Acceptable risk: blocking_lock() in error cleanup path.
                    // See docs/CONCURRENCY.md for mutex strategy.
                    let term = tab.terminal.blocking_lock();
                    term.remove_observer(observer_id);
                    drop(term);

                    // Show error in settings UI
                    if let Some(sw) = &mut self.settings_window {
                        let errors = &mut sw.settings_ui.script_errors;
                        while errors.len() <= config_index {
                            errors.push(String::new());
                        }
                        errors[config_index] = err_msg;
                        sw.request_redraw();
                    }
                    return;
                }
            }
            // Update running state in settings window
            self.sync_script_running_state();
        } else {
            crate::debug_error!(
                "SCRIPT",
                "start_script: no focused window or active tab found"
            );
        }
    }

    /// Stop a script by config index on the focused window's active tab.
    pub fn stop_script(&mut self, config_index: usize) {
        log::debug!("stop_script called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            // Stop the script process
            if let Some(Some(script_id)) = tab.script_ids.get(config_index).copied() {
                tab.script_manager.stop_script(script_id);
                log::info!(
                    "Stopped script at index {} (id={})",
                    config_index,
                    script_id
                );
            }

            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            if let Some(Some(observer_id)) = tab.script_observer_ids.get(config_index).copied() {
                let term = tab.terminal.blocking_lock();
                term.remove_observer(observer_id);
                drop(term);
            }

            // Clear tracking state
            if let Some(slot) = tab.script_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_observer_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_forwarders.get_mut(config_index) {
                *slot = None;
            }

            // Update running state in settings window
            self.sync_script_running_state();
        }
    }

    /// Maximum number of output lines kept per script in the UI.
    const SCRIPT_OUTPUT_MAX_LINES: usize = 200;

    /// Sync script running state to the settings window.
    ///
    /// Drains events from forwarders, sends them to scripts, reads commands
    /// and errors back, and updates the settings UI state.
    pub fn sync_script_running_state(&mut self) {
        let focused = self.get_focused_window_id();

        // Collect state from the active tab
        #[allow(clippy::type_complexity)]
        let (running_state, error_state, new_output, panel_state): (
            Vec<bool>,
            Vec<String>,
            Vec<Vec<String>>,
            Vec<Option<(String, String)>>,
        ) = if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            let script_count = ws.config.scripts.len();
            let mut running = Vec::with_capacity(script_count);
            let mut errors = Vec::with_capacity(script_count);
            let mut output = Vec::with_capacity(script_count);
            let mut panels = Vec::with_capacity(script_count);

            for i in 0..script_count {
                let has_script_id = tab.script_ids.get(i).and_then(|opt| *opt);
                let is_running = has_script_id.is_some_and(|id| tab.script_manager.is_running(id));

                // Drain events from forwarder and send to script
                if is_running && let Some(Some(forwarder)) = tab.script_forwarders.get(i) {
                    let events = forwarder.drain_events();
                    if let Some(script_id) = has_script_id {
                        for event in &events {
                            let _ = tab.script_manager.send_event(script_id, event);
                        }
                    }
                }

                // Read commands from script and process them
                let mut log_lines = Vec::new();
                let mut panel_val = tab
                    .script_manager
                    .get_panel(has_script_id.unwrap_or(0))
                    .cloned();

                if let Some(script_id) = has_script_id {
                    let commands = tab.script_manager.read_commands(script_id);
                    for cmd in commands {
                        match cmd {
                            crate::scripting::protocol::ScriptCommand::Log { level, message } => {
                                log_lines.push(format!("[{}] {}", level, message));
                            }
                            crate::scripting::protocol::ScriptCommand::SetPanel {
                                title,
                                content,
                            } => {
                                tab.script_manager.set_panel(
                                    script_id,
                                    title.clone(),
                                    content.clone(),
                                );
                                panel_val = Some((title, content));
                            }
                            crate::scripting::protocol::ScriptCommand::ClearPanel {} => {
                                tab.script_manager.clear_panel(script_id);
                                panel_val = None;
                            }
                            // TODO(#203): Implement WriteText, Notify, SetBadge, SetVariable,
                            // RunCommand, ChangeConfig — these require proper access to the
                            // terminal and config systems.
                            _ => {
                                log::debug!("Script command not yet implemented: {:?}", cmd);
                            }
                        }
                    }
                }

                // Read errors from script
                let err_text = if let Some(script_id) = has_script_id {
                    if is_running {
                        // Drain any stderr lines even while running
                        let err_lines = tab.script_manager.read_errors(script_id);
                        if !err_lines.is_empty() {
                            err_lines.join("\n")
                        } else {
                            String::new()
                        }
                    } else {
                        let err_lines = tab.script_manager.read_errors(script_id);
                        err_lines.join("\n")
                    }
                } else if let Some(sw) = &self.settings_window
                    && let Some(existing) = sw.settings_ui.script_errors.get(i)
                    && !existing.is_empty()
                {
                    existing.clone()
                } else {
                    String::new()
                };

                running.push(is_running);
                errors.push(err_text);
                output.push(log_lines);
                panels.push(panel_val);
            }

            (running, errors, output, panels)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        // Update settings window state
        if let Some(sw) = &mut self.settings_window {
            let running_changed = sw.settings_ui.script_running != running_state;
            let errors_changed = sw.settings_ui.script_errors != error_state;
            let has_new_output = new_output.iter().any(|lines| !lines.is_empty());
            let panels_changed = sw.settings_ui.script_panels != panel_state;

            if running_changed || errors_changed {
                crate::debug_info!(
                    "SCRIPT",
                    "sync: state change - running={:?} errors_changed={}",
                    running_state,
                    errors_changed
                );
            }

            let count = running_state.len();
            sw.settings_ui.script_output.resize_with(count, Vec::new);
            sw.settings_ui.script_output_expanded.resize(count, false);
            sw.settings_ui.script_panels.resize_with(count, || None);

            // Append new output lines, capping at max
            for (i, lines) in new_output.into_iter().enumerate() {
                if !lines.is_empty() {
                    let buf = &mut sw.settings_ui.script_output[i];
                    buf.extend(lines);
                    let overflow = buf.len().saturating_sub(Self::SCRIPT_OUTPUT_MAX_LINES);
                    if overflow > 0 {
                        buf.drain(..overflow);
                    }
                }
            }

            if running_changed || errors_changed || has_new_output || panels_changed {
                sw.settings_ui.script_running = running_state;
                sw.settings_ui.script_errors = error_state;
                sw.settings_ui.script_panels = panel_state;
                sw.request_redraw();
            }
        }
    }
}
