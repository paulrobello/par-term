//! Scripting, coprocess, and trigger state for a terminal tab.
//!
//! Groups all fields related to script execution, coprocess management,
//! and trigger handling.

use std::time::Instant;

use par_term_config::ScriptConfig;
use par_term_scripting::manager::ScriptId;
use par_term_scripting::{RestartAction, ScriptRestartState};
use par_term_terminal::TerminalManager;

/// Scripting, coprocess, and trigger state for a terminal tab.
pub(crate) struct TabScriptingState {
    /// Script manager for this tab
    pub(crate) script_manager: par_term_scripting::manager::ScriptManager,
    /// Maps config index to ScriptId for running scripts
    pub(crate) script_ids: Vec<Option<par_term_scripting::manager::ScriptId>>,
    /// Observer IDs registered with the terminal for script event forwarding
    pub(crate) script_observer_ids: Vec<Option<par_term_emu_core_rust::observer::ObserverId>>,
    /// Event forwarders (shared with observer registration)
    pub(crate) script_forwarders:
        Vec<Option<std::sync::Arc<par_term_scripting::observer::ScriptEventForwarder>>>,
    /// Per-config-index restart supervisor. Lazily populated; preserved across
    /// automatic restarts so the attempt cap accumulates, and reset by the grace
    /// window once a run survives long enough.
    pub(crate) restart_state: Vec<Option<ScriptRestartState>>,
    /// Mapping from config index to coprocess ID (for UI tracking)
    pub(crate) coprocess_ids: Vec<Option<par_term_emu_core_rust::coprocess::CoprocessId>>,
    /// Trigger-generated scrollbar marks (from MarkLine actions)
    pub(crate) trigger_marks: Vec<crate::config::ScrollbackMark>,
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

        let forwarder = std::sync::Arc::new(
            par_term_scripting::observer::ScriptEventForwarder::new(subscription_filter),
        );
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
                // Begin a fresh grace window for the (re)started process.
                self.restart_state_for(config_index, script_config)
                    .on_started(Instant::now());
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

    /// Drive restart supervision for one slot after its process exited.
    ///
    /// Call once per frame per slot whose [`ScriptStatus`] is `Exited`. The
    /// supervisor decides whether to stop, wait out the delay, or re-spawn;
    /// this method performs the side effects (clearing the slot or calling
    /// [`start_script_at`], which re-registers the observer/forwarder).
    ///
    /// `now` is taken as a parameter so every slot in a sweep shares one frame
    /// clock and the supervisor stays deterministic.
    pub(crate) fn handle_script_exit(
        &mut self,
        terminal: &TerminalManager,
        config_index: usize,
        script_config: &ScriptConfig,
        success: bool,
        now: Instant,
    ) {
        let action = {
            let rs = self.restart_state_for(config_index, script_config);
            if rs.pending() {
                rs.poll(now)
            } else {
                rs.on_exit(now, success)
            }
        };
        match action {
            RestartAction::Idle | RestartAction::Wait => {}
            RestartAction::Stop => {
                log::info!(
                    "Script at index {} giving up after {} consecutive restart failures",
                    config_index,
                    self.restart_state_for(config_index, script_config)
                        .consecutive_failures()
                );
                self.clear_script_at(terminal, config_index);
            }
            RestartAction::Restart => {
                match self.start_script_at(terminal, config_index, script_config) {
                    Ok(_) => log::info!("Restarted script at index {}", config_index),
                    Err(e) => {
                        log::warn!(
                            "Restart of script at index {} failed: {}; will retry after delay",
                            config_index,
                            e
                        );
                        self.restart_state_for(config_index, script_config)
                            .reschedule(now);
                    }
                }
            }
        }
    }

    /// Get or create the restart supervisor for `config_index`, syncing it to
    /// the current config so edits to policy/delay take effect.
    fn restart_state_for(
        &mut self,
        config_index: usize,
        script_config: &ScriptConfig,
    ) -> &mut ScriptRestartState {
        if self.restart_state.len() <= config_index {
            self.restart_state.resize(config_index + 1, None);
        }
        let slot = self.restart_state[config_index].get_or_insert_with(|| {
            ScriptRestartState::new(script_config.restart_policy, script_config.restart_delay_ms)
        });
        slot.reconfigure(script_config.restart_policy, script_config.restart_delay_ms);
        slot
    }
}

impl Default for TabScriptingState {
    fn default() -> Self {
        Self {
            script_manager: par_term_scripting::manager::ScriptManager::new(),
            script_ids: Vec::new(),
            script_observer_ids: Vec::new(),
            script_forwarders: Vec::new(),
            restart_state: Vec::new(),
            coprocess_ids: Vec::new(),
            trigger_marks: Vec::new(),
            trigger_prompt_before_run: std::collections::HashMap::new(),
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
        }
    }
}
