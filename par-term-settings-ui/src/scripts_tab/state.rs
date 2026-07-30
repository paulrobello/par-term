//! Per-tab state for the Scripts tab.

/// Inline script editor form state.
///
/// `Default` is written out rather than derived: several fields start at a
/// non-zero value, and deriving would silently reset them.
///
/// Runtime script status (`script_running`, `script_output`, …) stays on
/// [`crate::SettingsUI`]: the main window owns and writes it.
#[derive(Debug)]
pub struct ScriptsTabState {
    /// Index of script currently being edited (None = not editing)
    pub editing_script_index: Option<usize>,
    /// Temporary script name for edit form
    pub temp_script_name: String,
    /// Temporary script path for edit form
    pub temp_script_path: String,
    /// Temporary script args for edit form
    pub temp_script_args: String,
    /// Temporary script auto_start for edit form
    pub temp_script_auto_start: bool,
    /// Temporary script enabled for edit form
    pub temp_script_enabled: bool,
    /// Temporary script restart policy for edit form
    pub temp_script_restart_policy: par_term_config::automation::RestartPolicy,
    /// Temporary script restart delay for edit form
    pub temp_script_restart_delay_ms: u64,
    /// Temporary script subscriptions for edit form (comma-separated)
    pub temp_script_subscriptions: String,
    /// Temporary: allow WriteText commands
    pub temp_script_allow_write_text: bool,
    /// Temporary: confirm each WriteText injection before it reaches the PTY
    pub temp_script_prompt_before_write_text: bool,
    /// Temporary: allow RunCommand commands
    pub temp_script_allow_run_command: bool,
    /// Temporary: allow ChangeConfig commands
    pub temp_script_allow_change_config: bool,
    /// Temporary: WriteText rate limit (writes/sec, 0 = default)
    pub temp_script_write_text_rate_limit: u32,
    /// Temporary: RunCommand rate limit (runs/sec, 0 = default)
    pub temp_script_run_command_rate_limit: u32,
    /// Whether the add-new-script form is active
    pub adding_new_script: bool,
}

impl Default for ScriptsTabState {
    fn default() -> Self {
        Self {
            editing_script_index: None,
            temp_script_name: String::new(),
            temp_script_path: String::new(),
            temp_script_args: String::new(),
            temp_script_auto_start: false,
            temp_script_enabled: true,
            temp_script_restart_policy: par_term_config::automation::RestartPolicy::Never,
            temp_script_restart_delay_ms: 0,
            temp_script_subscriptions: String::new(),
            temp_script_allow_write_text: false,
            temp_script_prompt_before_write_text: true,
            temp_script_allow_run_command: false,
            temp_script_allow_change_config: false,
            temp_script_write_text_rate_limit: 0,
            temp_script_run_command_rate_limit: 0,
            adding_new_script: false,
        }
    }
}
