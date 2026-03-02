//! Scripting, coprocess, and trigger state for a terminal tab.
//!
//! Groups all fields related to script execution, coprocess management,
//! and trigger handling.

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
    /// Security metadata: maps trigger_id -> require_user_action flag.
    /// When true, dangerous actions (RunCommand, SendText) from that trigger
    /// are suppressed when fired from passive terminal output.
    pub(crate) trigger_security: std::collections::HashMap<u64, bool>,
    /// Rate limiter for output-triggered dangerous actions.
    pub(crate) trigger_rate_limiter: par_term_config::TriggerRateLimiter,
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
            trigger_security: std::collections::HashMap::new(),
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
        }
    }
}
