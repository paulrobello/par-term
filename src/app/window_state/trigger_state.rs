//! Trigger management state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use par_term_emu_core_rust::terminal::ActionResult;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// A dangerous trigger action awaiting user confirmation in the prompt dialog.
pub(crate) struct PendingTriggerAction {
    /// Trigger ID (assigned at config-load time)
    pub(crate) trigger_id: u64,
    /// Human-readable trigger name (for dialog title)
    pub(crate) trigger_name: String,
    /// The action to execute if approved
    pub(crate) action: ActionResult,
    /// Pre-formatted description of the action (for dialog body)
    pub(crate) description: String,
}

/// State for managing terminal triggers and their spawned processes.
#[derive(Default)]
pub(crate) struct TriggerState {
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: HashMap<u32, Instant>,
    /// Queue of dangerous actions waiting for user confirmation
    pub(crate) pending_trigger_actions: Vec<PendingTriggerAction>,
    /// Per-action replacement for the dialog's "where this came from" sentence,
    /// keyed by action id.
    ///
    /// The queue above carries actions no output trigger produced — script
    /// `WriteText` — for which the default sentence would be false.
    /// `PendingTriggerAction` cannot grow a field to say so: its producers in
    /// `src/app/triggers/mod.rs` build it with exhaustive struct literals. A
    /// producer that is not an output trigger registers its own sentence here;
    /// an absent entry means the action did come from one. Entries are removed
    /// with the action they describe.
    pub(crate) automation_action_notes: HashMap<u64, String>,
    /// Dialog-approved actions awaiting execution on the next frame
    pub(crate) approved_pending_actions: Vec<ActionResult>,
    /// Trigger IDs the user has approved for auto-execution this session
    pub(crate) always_allow_trigger_ids: HashSet<u64>,
    /// Whether the confirmation dialog is currently open (prevents stacking)
    pub(crate) trigger_prompt_dialog_open: bool,
    /// Frame number when the dialog opened (flicker guard). None = dialog not open.
    pub(crate) trigger_prompt_activated_frame: Option<u64>,
}
