//! Trigger management state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use par_term_emu_core_rust::terminal::ActionResult;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::AutomationTarget;

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
    /// The tab this action must be resolved against, when it is not the active
    /// one.
    ///
    /// `None` — the case for output triggers and profile auto-switch commands —
    /// means the active tab at execution time, which is what `ActionResult`
    /// alone can express. `Some` is what lets a background tab's script write
    /// reach its own tab; see `automation_target`.
    pub(crate) target: Option<AutomationTarget>,
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
    /// `WriteText` and profile auto-switch commands — for which the default
    /// sentence would be false. Those producers register their own sentence
    /// here; an absent entry means the action did come from an output trigger.
    /// Entries are removed with the action they describe, so every id in this
    /// map must belong to at most one queued action.
    pub(crate) automation_action_notes: HashMap<u64, String>,
    /// Dialog-approved actions awaiting execution against the active tab on the
    /// next frame, drained by `check_trigger_actions`
    pub(crate) approved_pending_actions: Vec<ActionResult>,
    /// Dialog-approved actions awaiting execution against one specific tab,
    /// drained by `WindowState::execute_approved_targeted_actions`
    ///
    /// Separate from `approved_pending_actions` because that queue's sink
    /// resolves everything against the active tab. See `automation_target`.
    pub(crate) approved_targeted_actions: Vec<(AutomationTarget, ActionResult)>,
    /// Trigger IDs the user has approved for auto-execution this session
    pub(crate) always_allow_trigger_ids: HashSet<u64>,
    /// Whether the confirmation dialog is currently open (prevents stacking)
    pub(crate) trigger_prompt_dialog_open: bool,
    /// Frame number when the dialog opened (flicker guard). None = dialog not open.
    pub(crate) trigger_prompt_activated_frame: Option<u64>,
}
