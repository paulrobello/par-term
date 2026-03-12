//! Trigger management state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use par_term_emu_core_rust::terminal::ActionResult;
use regex::Regex;
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
pub(crate) struct TriggerState {
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: HashMap<u32, Instant>,
    /// Compiled regex cache for prettify trigger patterns (command_filter and block_end).
    /// Keyed by pattern string; avoids recompiling the same pattern every frame.
    pub(crate) trigger_regex_cache: HashMap<String, Regex>,
    /// Queue of dangerous actions waiting for user confirmation
    pub(crate) pending_trigger_actions: Vec<PendingTriggerAction>,
    /// Trigger IDs the user has approved for auto-execution this session
    pub(crate) always_allow_trigger_ids: HashSet<u64>,
    /// Whether the confirmation dialog is currently open (prevents stacking)
    pub(crate) trigger_prompt_dialog_open: bool,
    /// Frame number when the dialog opened (flicker guard). None = dialog not open.
    pub(crate) trigger_prompt_activated_frame: Option<u64>,
}

impl Default for TriggerState {
    fn default() -> Self {
        Self {
            trigger_spawned_processes: HashMap::new(),
            trigger_regex_cache: HashMap::new(),
            pending_trigger_actions: Vec::new(),
            always_allow_trigger_ids: HashSet::new(),
            trigger_prompt_dialog_open: false,
            trigger_prompt_activated_frame: None,
        }
    }
}
