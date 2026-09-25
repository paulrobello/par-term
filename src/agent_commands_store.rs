//! Runtime store for agent-authored commands: loads the commands directory,
//! watches it, feeds the command palette, and owns the first-run confirmation
//! flow for script commands.
//!
//! The file format and provenance rules live in
//! `par_term_config::agent_commands`; this module is the app-side half —
//! snapshot keeping, directory watching, palette rows, and the pending-
//! confirmation queue the egui dialog drains.
//!
//! Watching uses `ConfigWatcher::new_yaml_dir`, polled from `about_to_wait`:
//! it watches the directory itself (one level) and reacts to create/modify/
//! remove of non-hidden `*.yaml` children, so the MCP tool's atomic-rename
//! writes, hand edits, Settings edits, and deletes all trigger a rescan while
//! `.confirmations.json` ledger writes do not. (The plain file-mode
//! `ConfigWatcher::new` watches a path's parent for that exact filename and
//! would never see a child of the directory.)

use crate::config::watcher::ConfigWatcher;
use par_term_config::agent_commands::{
    AgentCommandFile, ConfirmationLedger, LoadedCommand, load_all_commands,
    load_confirmation_ledger, save_confirmation_ledger,
};
use std::collections::BTreeMap;

/// Drain + execute agent commands approved by the confirmation dialog.
/// Called from `about_to_wait` where `&mut self` is free of the egui
/// render-pass borrows.
pub(crate) fn run_approved_agent_commands(state: &mut crate::app::window_state::WindowState) {
    for file in state.agent_commands.take_approved() {
        if let par_term_config::CustomActionConfig::ShellCommand {
            command,
            args,
            notify_on_success,
            timeout_secs,
            title,
            capture_output,
            ..
        } = file.action.clone()
        {
            let env = crate::app::input_events::keybinding_actions::agent_command_env(&file);
            state.execute_shell_command_action_with_env(
                command,
                args,
                notify_on_success,
                timeout_secs,
                title,
                capture_output,
                env,
            );
        }
    }
}

/// A script command waiting for the user's first-run confirmation.
pub(crate) struct PendingCommandConfirmation {
    /// The full command file, for re-execution on Run.
    pub(crate) file: AgentCommandFile,
    /// The body hash awaiting approval.
    pub(crate) body_hash: String,
}

/// Palette + dispatch state for agent commands.
pub(crate) struct AgentCommandStore {
    /// Validated command files, keyed by id. Rebuilt on watcher events.
    snapshot: BTreeMap<String, LoadedCommand>,
    /// Directory watcher on `<config_dir>/commands/`.
    watcher: Option<ConfigWatcher>,
    /// id → approved body hash. Persisted to `.confirmations.json`.
    ledger: ConfirmationLedger,
    /// Script commands awaiting first-run confirmation, in arrival order.
    /// The dialog shows the head; Run executes it, Cancel drops it.
    pub(crate) pending_confirmations: Vec<PendingCommandConfirmation>,
    /// Commands the dialog approved this frame, drained by `about_to_wait`
    /// where `&mut self` is free of the egui borrow.
    pub(crate) approved_for_execution: Vec<AgentCommandFile>,
    /// Flicker guard: the frame the confirmation dialog opened, so the
    /// keypress that triggered the command cannot also click a button.
    pub(crate) confirm_dialog_activated_frame: Option<u64>,
}

impl AgentCommandStore {
    pub(crate) fn new() -> Self {
        let dir = par_term_config::agent_commands::commands_dir();
        let _ = std::fs::create_dir_all(&dir);
        let mut store = Self {
            snapshot: BTreeMap::new(),
            watcher: None,
            ledger: load_confirmation_ledger(&dir),
            pending_confirmations: Vec::new(),
            approved_for_execution: Vec::new(),
            confirm_dialog_activated_frame: None,
        };
        store.attach_watcher();
        store.rescan();
        store
    }

    /// Start the directory watcher if the directory exists.
    fn attach_watcher(&mut self) {
        if self.watcher.is_some() {
            return;
        }
        let dir = par_term_config::agent_commands::commands_dir();
        match ConfigWatcher::new_yaml_dir(&dir, 300) {
            Ok(w) => self.watcher = Some(w),
            Err(e) => {
                log::warn!("agent-commands watcher unavailable ({e}); palette rows are static")
            }
        }
    }

    /// Reload every valid command file and the ledger.
    pub(crate) fn rescan(&mut self) {
        let dir = par_term_config::agent_commands::commands_dir();
        self.snapshot = load_all_commands(&dir)
            .into_iter()
            .map(|c| (c.file.id().to_string(), c))
            .collect();
        self.ledger = load_confirmation_ledger(&dir);
    }

    /// Poll the watcher; rescans when an event arrived.
    /// Called from `about_to_wait`, like the config-update check.
    pub(crate) fn poll(&mut self) -> bool {
        self.attach_watcher();
        let changed = self
            .watcher
            .as_ref()
            .is_some_and(|w| w.try_recv().is_some());
        if changed {
            let before: Vec<String> = self.snapshot.keys().cloned().collect();
            self.rescan();
            // Prune ledger entries for commands that no longer exist, so a
            // re-created command of the same id asks for confirmation again.
            for id in before {
                if !self.snapshot.contains_key(&id) {
                    self.prune_ledger_entry(&id);
                }
            }
        }
        changed
    }

    /// Look up a command by id.
    pub(crate) fn get(&self, id: &str) -> Option<&LoadedCommand> {
        self.snapshot.get(id)
    }

    /// Palette rows for every loaded command (design: priority 0, label from
    /// provenance).
    pub(crate) fn palette_rows(&self) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        self.snapshot
            .values()
            .map(|c| crate::command_palette::catalog::PaletteEntry {
                action_id: format!("agent-cmd:{}", c.file.id()),
                label: c.file.palette_label(),
                chord: None,
                priority: 0,
            })
            .collect()
    }

    /// Whether a script command's current body has been confirmed.
    pub(crate) fn is_confirmed(&self, file: &AgentCommandFile) -> bool {
        self.ledger.get(file.id()) == Some(&file.body_hash())
    }

    /// Queue a script command for first-run confirmation.
    pub(crate) fn request_confirmation(&mut self, file: AgentCommandFile) {
        let body_hash = file.body_hash();
        self.pending_confirmations
            .push(PendingCommandConfirmation { file, body_hash });
    }

    /// Record an approval from the dialog: persist the hash and queue the
    /// approved file for execution (drained by `about_to_wait`).
    pub(crate) fn approve_head(&mut self) -> Option<AgentCommandFile> {
        let pending = self.pending_confirmations.pop()?;
        self.ledger
            .insert(pending.file.id().to_string(), pending.body_hash);
        let dir = par_term_config::agent_commands::commands_dir();
        if let Err(e) = save_confirmation_ledger(&self.ledger, &dir) {
            log::warn!("failed to persist confirmation ledger: {e:#}");
        }
        self.approved_for_execution.push(pending.file.clone());
        Some(pending.file)
    }

    /// Drop the head confirmation (Cancel).
    pub(crate) fn cancel_head(&mut self) -> Option<PendingCommandConfirmation> {
        self.pending_confirmations.pop()
    }

    /// Take everything the dialog approved, for execution outside the egui
    /// render pass.
    pub(crate) fn take_approved(&mut self) -> Vec<AgentCommandFile> {
        std::mem::take(&mut self.approved_for_execution)
    }

    /// Remove ledger entries for a deleted command (id no longer on disk).
    pub(crate) fn prune_ledger_entry(&mut self, id: &str) {
        if self.ledger.remove(id).is_some() {
            let dir = par_term_config::agent_commands::commands_dir();
            if let Err(e) = save_confirmation_ledger(&self.ledger, &dir) {
                log::warn!("failed to persist confirmation ledger: {e:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script_file(id: &str) -> AgentCommandFile {
        let yaml = format!(
            "created_by: agent\nsource_agent: claude-code\naction:\n  type: shell_command\n  \
             id: {id}\n  title: Test\n  command: echo\n  args: [\"hi\"]\n"
        );
        serde_yaml_ng::from_str(&yaml).unwrap()
    }

    // Store construction reads the real commands dir; for unit tests we
    // exercise only the confirmation queue mechanics, which are dir-independent.
    #[test]
    fn confirmation_queue_roundtrip() {
        let mut store = AgentCommandStore {
            snapshot: BTreeMap::new(),
            watcher: None,
            ledger: BTreeMap::new(),
            pending_confirmations: Vec::new(),
            approved_for_execution: Vec::new(),
            confirm_dialog_activated_frame: None,
        };
        let file = script_file("queue-test");
        let hash = file.body_hash();

        assert!(!store.is_confirmed(&file));
        store.request_confirmation(file.clone());
        assert_eq!(store.pending_confirmations.len(), 1);

        // Cancel drops without approving.
        store.cancel_head();
        assert!(store.pending_confirmations.is_empty());

        // Approve persists in the in-memory ledger (disk write targets the
        // real dir; failure is warned, not fatal — verified by is_confirmed).
        store.request_confirmation(file.clone());
        let approved = store.approve_head().unwrap();
        assert_eq!(approved.id(), "queue-test");
        assert_eq!(store.ledger.get("queue-test"), Some(&hash));
    }
}
