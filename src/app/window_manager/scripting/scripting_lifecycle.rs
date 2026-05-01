//! Script start/stop lifecycle for the window manager.
//!
//! Handles starting and stopping scripts on the active tab, including
//! spawning the script process, registering the event-forwarder observer,
//! and cleaning up all tracking state on stop.

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
                "start_script: ws.config.load().scripts.len()={}, tab.scripting.script_ids.len()={}",
                ws.config.load().scripts.len(),
                tab.scripting.script_ids.len()
            );
            if config_index >= ws.config.load().scripts.len() {
                crate::debug_error!(
                    "SCRIPT",
                    "Script config index {} out of range (scripts.len={})",
                    config_index,
                    ws.config.load().scripts.len()
                );
                return;
            }
            let script_config = &ws.config.load().scripts[config_index];
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
                let term = tab.terminal.blocking_write();
                term.add_observer(forwarder.clone())
            };

            // Start the script process
            crate::debug_info!("SCRIPT", "start_script: spawning process...");
            match tab.scripting.script_manager.start_script(script_config) {
                Ok(script_id) => {
                    crate::debug_info!(
                        "SCRIPT",
                        "start_script: SUCCESS script_id={} observer_id={:?}",
                        script_id,
                        observer_id
                    );

                    // Ensure vecs are large enough
                    while tab.scripting.script_ids.len() <= config_index {
                        tab.scripting.script_ids.push(None);
                    }
                    while tab.scripting.script_observer_ids.len() <= config_index {
                        tab.scripting.script_observer_ids.push(None);
                    }
                    while tab.scripting.script_forwarders.len() <= config_index {
                        tab.scripting.script_forwarders.push(None);
                    }

                    tab.scripting.script_ids[config_index] = Some(script_id);
                    tab.scripting.script_observer_ids[config_index] = Some(observer_id);
                    tab.scripting.script_forwarders[config_index] = Some(forwarder);
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
                    let term = tab.terminal.blocking_write();
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
            if let Some(Some(script_id)) = tab.scripting.script_ids.get(config_index).copied() {
                tab.scripting.script_manager.stop_script(script_id);
                log::info!(
                    "Stopped script at index {} (id={})",
                    config_index,
                    script_id
                );
            }

            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            if let Some(Some(observer_id)) =
                tab.scripting.script_observer_ids.get(config_index).copied()
            {
                let term = tab.terminal.blocking_write();
                term.remove_observer(observer_id);
                drop(term);
            }

            // Clear tracking state
            if let Some(slot) = tab.scripting.script_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.scripting.script_observer_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.scripting.script_forwarders.get_mut(config_index) {
                *slot = None;
            }

            // Update running state in settings window
            self.sync_script_running_state();
        }
    }
}
