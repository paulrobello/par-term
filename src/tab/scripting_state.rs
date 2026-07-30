//! Scripting, coprocess, and trigger state for a terminal tab.
//!
//! Groups all fields related to script execution, coprocess management,
//! and trigger handling.

use crate::scripting::manager::ScriptId;
use par_term_config::ScriptConfig;
use par_term_terminal::TerminalManager;

/// Scripting, coprocess, and trigger state for a terminal tab.
pub(crate) struct TabScriptingState {
    /// Script manager for this tab
    pub(crate) script_manager: crate::scripting::manager::ScriptManager,
    /// Maps config index to ScriptId for running scripts
    pub(crate) script_ids: Vec<Option<crate::scripting::manager::ScriptId>>,
    /// Observer IDs registered with the terminal for script event forwarding
    pub(crate) script_observer_ids: Vec<Option<par_term_emu_core_rust::observer::ObserverId>>,
    /// Event forwarders (shared with observer registration)
    pub(crate) script_forwarders:
        Vec<Option<std::sync::Arc<crate::scripting::observer::ScriptEventForwarder>>>,
    /// Mapping from config index to coprocess ID (for UI tracking)
    pub(crate) coprocess_ids: Vec<Option<par_term_emu_core_rust::coprocess::CoprocessId>>,
    /// Trigger-generated scrollbar marks (from MarkLine actions)
    pub(crate) trigger_marks: Vec<crate::scrollback_metadata::ScrollbackMark>,
    /// Security metadata: maps trigger_id -> prompt_before_run flag.
    /// When true, dangerous actions show a confirmation dialog instead of executing automatically.
    pub(crate) trigger_prompt_before_run: std::collections::HashMap<u64, bool>,
    /// Rate limiter for output-triggered dangerous actions.
    pub(crate) trigger_rate_limiter: par_term_config::TriggerRateLimiter,
}

impl TabScriptingState {
    /// Spawn the script at `config_index` and register its event forwarder as a
    /// terminal observer, recording all per-index tracking state on success.
    ///
    /// Shared by the tab-creation auto-start loop and the Settings UI start
    /// button so both paths populate `script_ids` / `script_observer_ids` /
    /// `script_forwarders` identically. Callers are responsible for checking
    /// [`ScriptConfig::enabled`] — this method starts whatever it is given.
    ///
    /// On spawn failure the observer is unregistered again so a failed start
    /// leaves no dangling observer on the terminal.
    ///
    /// Any previous script at `config_index` is torn down first. A script that
    /// exits on its own leaves its observer registered (only an explicit stop
    /// removes it), so restarting after a crash would otherwise orphan the old
    /// forwarder — still attached to the terminal, still being pushed events
    /// into an unbounded buffer that nobody drains.
    ///
    /// # Errors
    /// Returns the underlying spawn error string if the process cannot start.
    pub(crate) fn start_script_at(
        &mut self,
        terminal: &TerminalManager,
        config_index: usize,
        script_config: &ScriptConfig,
    ) -> Result<ScriptId, String> {
        self.clear_script_at(terminal, config_index);

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

        let forwarder = std::sync::Arc::new(crate::scripting::observer::ScriptEventForwarder::new(
            subscription_filter,
        ));
        let observer_id = terminal.add_observer(forwarder.clone());

        match self.script_manager.start_script(script_config) {
            Ok(script_id) => {
                let slots_needed = config_index + 1;
                if self.script_ids.len() < slots_needed {
                    self.script_ids.resize(slots_needed, None);
                }
                if self.script_observer_ids.len() < slots_needed {
                    self.script_observer_ids.resize(slots_needed, None);
                }
                if self.script_forwarders.len() < slots_needed {
                    self.script_forwarders.resize(slots_needed, None);
                }

                self.script_ids[config_index] = Some(script_id);
                self.script_observer_ids[config_index] = Some(observer_id);
                self.script_forwarders[config_index] = Some(forwarder);
                Ok(script_id)
            }
            Err(e) => {
                terminal.remove_observer(observer_id);
                Err(e)
            }
        }
    }

    /// Stop the script at `config_index` (if any), unregister its observer, and
    /// clear its tracking slots.
    ///
    /// Safe to call for an index that was never started or has already exited.
    pub(crate) fn clear_script_at(&mut self, terminal: &TerminalManager, config_index: usize) {
        if let Some(slot) = self.script_ids.get_mut(config_index)
            && let Some(script_id) = slot.take()
        {
            self.script_manager.stop_script(script_id);
            log::info!(
                "Stopped script at index {} (id={})",
                config_index,
                script_id
            );
        }
        if let Some(slot) = self.script_observer_ids.get_mut(config_index)
            && let Some(observer_id) = slot.take()
        {
            terminal.remove_observer(observer_id);
        }
        if let Some(slot) = self.script_forwarders.get_mut(config_index) {
            *slot = None;
        }
    }
}

impl Default for TabScriptingState {
    fn default() -> Self {
        Self {
            script_manager: crate::scripting::manager::ScriptManager::new(),
            script_ids: Vec::new(),
            script_observer_ids: Vec::new(),
            script_forwarders: Vec::new(),
            coprocess_ids: Vec::new(),
            trigger_marks: Vec::new(),
            trigger_prompt_before_run: std::collections::HashMap::new(),
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
        }
    }
}
