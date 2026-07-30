//! Per-tab state for the Actions tab.

/// Inline custom-action editor form state.
///
/// `Default` is written out rather than derived: several fields start at a
/// non-zero value, and deriving would silently reset them.
#[derive(Debug)]
pub struct ActionsTabState {
    /// Index of action currently being edited (None = not editing)
    pub editing_action_index: Option<usize>,
    /// Temporary action type for edit form (0=ShellCommand, 1=NewTab, 2=InsertText, 3=KeySequence, 4=SplitPane, 5=Sequence, 6=Condition, 7=Repeat)
    pub temp_action_type: usize,
    /// Temporary action ID for edit form
    pub temp_action_id: String,
    /// Temporary action title for edit form
    pub temp_action_title: String,
    /// Temporary action command (for ShellCommand type)
    pub temp_action_command: String,
    /// Temporary action args (for ShellCommand type)
    pub temp_action_args: String,
    /// Temporary command text for NewTab type (empty = open an empty tab)
    pub temp_action_new_tab_command: String,
    /// Temporary action text (for InsertText type)
    pub temp_action_text: String,
    /// Temporary action keys (for KeySequence type)
    pub temp_action_keys: String,
    /// Temporary action keybinding for edit form
    pub temp_action_keybinding: String,
    /// Temporary single-character suffix used after the global custom action prefix key
    pub temp_action_prefix_char: String,
    /// Split direction index for SplitPane type (0=Horizontal, 1=Vertical)
    pub temp_action_split_direction: usize,
    /// Optional command text for SplitPane type (empty = no command)
    pub temp_action_split_command: String,
    /// Whether to focus new pane for SplitPane type
    pub temp_action_split_focus_new: bool,
    /// Delay ms before sending command for SplitPane type
    pub temp_action_split_delay_ms: u64,
    /// When true, command is the pane's initial process (not sent to shell)
    pub temp_action_split_command_is_direct: bool,
    /// Split percent for SplitPane type (10–90, default 66)
    pub temp_action_split_percent: u8,
    /// Whether the add-new-action form is active
    pub adding_new_action: bool,
    /// Whether currently recording a keybinding for an action
    pub recording_action_keybinding: bool,
    /// Recorded keybinding combo for action (displayed during recording)
    pub action_recorded_combo: Option<String>,
    /// Whether currently recording the section-level custom action prefix key
    pub recording_custom_action_prefix_key: bool,
    /// Recorded combo for the section-level custom action prefix key
    pub custom_action_prefix_key_recorded_combo: Option<String>,
    /// Steps in a Sequence action (action_id, delay_ms, on_failure behavior)
    pub temp_action_steps: Vec<(String, u64, par_term_config::snippets::SequenceStepBehavior)>,
    /// Check type index: 0=exit_code, 1=output_contains, 2=env_var, 3=dir_matches, 4=git_branch
    pub temp_action_check_type: usize,
    /// Check value (exit code as string, or glob/regex pattern)
    pub temp_action_check_value: String,
    /// Case sensitive flag (for output_contains check type)
    pub temp_action_case_sensitive: bool,
    /// Environment variable name (for env_var check type)
    pub temp_action_env_name: String,
    /// Environment variable expected value (for env_var check type, empty = existence-only)
    pub temp_action_env_value: String,
    /// Whether to check env var existence only (for env_var check type)
    pub temp_action_env_check_existence: bool,
    /// Action ID to execute when condition is true
    pub temp_action_on_true_id: String,
    /// Action ID to execute when condition is false
    pub temp_action_on_false_id: String,
    /// Action ID to repeat
    pub temp_action_repeat_action_id: String,
    /// Number of repetitions (1–100)
    pub temp_action_repeat_count: u32,
    /// Delay in milliseconds between repetitions
    pub temp_action_repeat_delay_ms: u64,
    /// Stop repeating when the action succeeds
    pub temp_action_stop_on_success: bool,
    /// Stop repeating when the action fails
    pub temp_action_stop_on_failure: bool,
    /// Whether to capture stdout/stderr for use in Condition checks (for ShellCommand type)
    pub temp_action_capture_output: bool,
    /// Temporary keybinding_enabled flag for action edit form
    pub temp_action_keybinding_enabled: bool,
}

impl Default for ActionsTabState {
    fn default() -> Self {
        Self {
            editing_action_index: None,
            temp_action_type: 0,
            temp_action_id: String::new(),
            temp_action_title: String::new(),
            temp_action_command: String::new(),
            temp_action_args: String::new(),
            temp_action_new_tab_command: String::new(),
            temp_action_text: String::new(),
            temp_action_keys: String::new(),
            temp_action_keybinding: String::new(),
            temp_action_prefix_char: String::new(),
            temp_action_split_direction: 0,
            temp_action_split_command: String::new(),
            temp_action_split_focus_new: true,
            temp_action_split_delay_ms: 200,
            temp_action_split_command_is_direct: false,
            temp_action_split_percent: 66,
            adding_new_action: false,
            recording_action_keybinding: false,
            action_recorded_combo: None,
            recording_custom_action_prefix_key: false,
            custom_action_prefix_key_recorded_combo: None,
            temp_action_steps: Vec::new(),
            temp_action_check_type: 0,
            temp_action_check_value: String::new(),
            temp_action_case_sensitive: false,
            temp_action_env_name: String::new(),
            temp_action_env_value: String::new(),
            temp_action_env_check_existence: false,
            temp_action_on_true_id: String::new(),
            temp_action_on_false_id: String::new(),
            temp_action_repeat_action_id: String::new(),
            temp_action_repeat_count: 3,
            temp_action_repeat_delay_ms: 0,
            temp_action_stop_on_success: false,
            temp_action_stop_on_failure: false,
            temp_action_capture_output: false,
            temp_action_keybinding_enabled: true,
        }
    }
}
