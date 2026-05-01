//! Trigger synchronisation methods for [`TerminalManager`].
//!
//! Bridges the frontend `TriggerConfig` (from `par-term-config`) into the
//! core `TriggerRegistry` inside the PTY session.  All existing triggers are
//! replaced on each call so that the registry stays in sync with the live
//! config.

use super::TerminalManager;
use crate::conversion::to_core_trigger_action;

impl TerminalManager {
    /// Sync trigger configs from Config into the core TriggerRegistry.
    ///
    /// Returns a map of `trigger_id -> prompt_before_run` for each
    /// successfully registered trigger, so the frontend can decide whether
    /// to show a confirmation dialog for dangerous actions.
    pub fn sync_triggers(
        &self,
        triggers: &[par_term_config::TriggerConfig],
    ) -> std::collections::HashMap<u64, bool> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        // Clear existing trigger registrations before applying the new config.
        let existing: Vec<u64> = term.list_triggers().iter().map(|t| t.id).collect();
        for id in existing {
            term.remove_trigger(id);
        }

        let mut security_map = std::collections::HashMap::new();

        for trigger_config in triggers {
            let actions: Vec<par_term_emu_core_rust::terminal::TriggerAction> = trigger_config
                .actions
                .iter()
                .map(|a| to_core_trigger_action(a.clone()))
                .collect();

            match term.add_trigger(
                trigger_config.name.clone(),
                trigger_config.pattern.clone(),
                actions,
            ) {
                Ok(id) => {
                    if !trigger_config.enabled {
                        term.set_trigger_enabled(id, false);
                    }
                    security_map.insert(id, trigger_config.prompt_before_run);
                    log::info!(
                        "Trigger '{}' registered (id={}, prompt_before_run={})",
                        trigger_config.name,
                        id,
                        trigger_config.prompt_before_run,
                    );
                }
                Err(e) => {
                    log::error!(
                        "Failed to register trigger '{}': {}",
                        trigger_config.name,
                        e
                    );
                }
            }
        }

        security_map
    }

    /// Returns a snapshot of all registered trigger names keyed by trigger ID.
    ///
    /// Used by the frontend to display human-readable trigger names in confirmation
    /// dialogs for dangerous actions (`RunCommand`, `SendText`).
    pub fn trigger_names(&self) -> std::collections::HashMap<u64, String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.list_triggers()
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect()
    }
}
