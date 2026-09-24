//! Per-tab state for the Automation tab.

/// Inline trigger and coprocess editor form state.
///
/// `Default` is written out rather than derived: several fields start at a
/// non-zero value, and deriving would silently reset them.
///
/// Runtime coprocess status (`coprocess_running`, `coprocess_output`, …) stays
/// on [`crate::SettingsUI`]: the main window owns and writes it.
#[derive(Debug)]
pub struct AutomationTabState {
    /// Index of trigger currently being edited (None = not editing)
    pub editing_trigger_index: Option<usize>,
    /// Temporary trigger name for edit form
    pub temp_trigger_name: String,
    /// Temporary trigger regex pattern for edit form
    pub temp_trigger_pattern: String,
    /// Temporary trigger actions for edit form
    pub temp_trigger_actions: Vec<par_term_config::automation::TriggerActionConfig>,
    /// Temporary prompt_before_run flag for trigger edit form
    pub temp_trigger_prompt_before_run: bool,
    /// Whether the add-new-trigger form is active
    pub adding_new_trigger: bool,
    /// Regex validation error for trigger pattern
    pub trigger_pattern_error: Option<String>,
    /// Index of coprocess currently being edited (None = not editing)
    pub editing_coprocess_index: Option<usize>,
    /// Temporary coprocess name for edit form
    pub temp_coprocess_name: String,
    /// Temporary coprocess command for edit form
    pub temp_coprocess_command: String,
    /// Temporary coprocess args for edit form
    pub temp_coprocess_args: String,
    /// Temporary coprocess auto_start for edit form
    pub temp_coprocess_auto_start: bool,
    /// Temporary coprocess copy_terminal_output for edit form
    pub temp_coprocess_copy_output: bool,
    /// Temporary coprocess restart policy for edit form
    pub temp_coprocess_restart_policy: par_term_config::automation::RestartPolicy,
    /// Temporary coprocess restart delay for edit form
    pub temp_coprocess_restart_delay_ms: u64,
    /// Whether the add-new-coprocess form is active
    pub adding_new_coprocess: bool,
    /// Flag to request trigger resync after save
    pub trigger_resync_requested: bool,
    /// Plugin discovery cache for the Plugins section: `None` until the
    /// section first renders, refreshed by its Rescan button. Holds the
    /// valid plugins plus the skip warnings so both survive the frame
    /// that ran the scan.
    pub plugin_scan: Option<(
        Vec<par_term_scripting::manifest::DiscoveredPlugin>,
        Vec<par_term_scripting::manifest::DiscoveryWarning>,
    )>,
    /// Git URL text input for the Plugins section's Add field.
    pub plugin_git_url: String,
    /// True while a plugin add/update/remove runs on its background thread.
    pub plugin_git_busy: bool,
    /// Last status or error line from a plugin git operation.
    pub plugin_git_message: Option<String>,
    /// Receiver for the in-flight plugin git operation, if any.
    pub plugin_git_receiver:
        Option<std::sync::mpsc::Receiver<Result<crate::PluginGitOutcome, String>>>,
    /// A fetched update awaiting the user's Apply, with its plugin id.
    pub plugin_update_preview: Option<(String, par_term_scripting::plugin_git::UpdatePreview)>,
    /// Plugin id whose Remove button is armed for a second-click confirm.
    pub plugin_remove_pending: Option<String>,
    /// Missing-plugin id whose Forget button is armed for a second-click confirm.
    pub plugin_forget_pending: Option<String>,
    /// Cached ids of git-installed plugins, refreshed together with
    /// [`Self::plugin_scan`] — the `.git` + origin probe spawns a git
    /// process per plugin, so it runs on rescan, never per frame.
    pub plugin_git_ids: Option<std::collections::HashSet<String>>,
}

impl Default for AutomationTabState {
    fn default() -> Self {
        Self {
            editing_trigger_index: None,
            temp_trigger_name: String::new(),
            temp_trigger_pattern: String::new(),
            temp_trigger_actions: Vec::new(),
            temp_trigger_prompt_before_run: true,
            adding_new_trigger: false,
            trigger_pattern_error: None,
            editing_coprocess_index: None,
            temp_coprocess_name: String::new(),
            temp_coprocess_command: String::new(),
            temp_coprocess_args: String::new(),
            temp_coprocess_auto_start: false,
            temp_coprocess_copy_output: true,
            temp_coprocess_restart_policy: par_term_config::automation::RestartPolicy::Never,
            temp_coprocess_restart_delay_ms: 0,
            adding_new_coprocess: false,
            trigger_resync_requested: false,
            plugin_scan: None,
            plugin_git_url: String::new(),
            plugin_git_busy: false,
            plugin_git_message: None,
            plugin_git_receiver: None,
            plugin_update_preview: None,
            plugin_remove_pending: None,
            plugin_forget_pending: None,
            plugin_git_ids: None,
        }
    }
}
