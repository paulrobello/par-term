//! SettingsUI struct and implementation.
//!
//! This module contains the main SettingsUI manager struct and all its
//! methods for displaying and managing the settings window.

use par_term_config::{
    BackgroundImageMode, Config, CursorShaderMetadataCache, ProfileId, ShaderMetadataCache,
};
use std::collections::HashSet;

use crate::profile_modal_ui::ProfileModalUI;
use crate::sidebar::SettingsTab;
use crate::{
    ArrangementId, ArrangementManager, InstallationType, SettingsWindowAction,
    ShaderDetectModifiedFn, ShaderInstallResult, ShaderUninstallResult,
    ShellIntegrationInstallResult, ShellIntegrationUninstallResult, UpdateCheckResult,
    UpdateResult,
};

/// Settings UI manager using egui
pub struct SettingsUI {
    /// Whether the settings window is currently visible
    pub visible: bool,

    /// Working copy of config being edited
    pub config: Config,

    /// Last opacity value that was forwarded for live updates
    pub last_live_opacity: f32,

    /// Whether config has unsaved changes
    pub has_changes: bool,

    /// Temp strings for optional fields (for UI editing)
    pub temp_font_bold: String,
    pub temp_font_italic: String,
    pub temp_font_bold_italic: String,
    pub temp_font_family: String,
    pub temp_font_size: f32,
    pub temp_line_spacing: f32,
    pub temp_char_spacing: f32,
    pub temp_enable_text_shaping: bool,
    pub temp_enable_ligatures: bool,
    pub temp_enable_kerning: bool,
    pub font_pending_changes: bool,
    pub temp_custom_shell: String,
    pub temp_shell_args: String,
    pub temp_working_directory: String,
    pub temp_startup_directory: String,
    pub temp_initial_text: String,
    pub temp_background_image: String,
    pub temp_custom_shader: String,
    pub temp_cursor_shader: String,

    /// Temporary strings for shader channel texture paths (iChannel0-3)
    pub temp_shader_channel0: String,
    pub temp_shader_channel1: String,
    pub temp_shader_channel2: String,
    pub temp_shader_channel3: String,

    /// Temporary string for cubemap path prefix (iCubemap)
    pub temp_cubemap_path: String,

    /// Temporary color for solid background color editing
    pub temp_background_color: [u8; 3],

    /// Temporary per-pane background image path for editing
    pub temp_pane_bg_path: String,
    /// Temporary per-pane background mode
    pub temp_pane_bg_mode: BackgroundImageMode,
    /// Temporary per-pane background opacity
    pub temp_pane_bg_opacity: f32,
    /// Temporary per-pane background darken amount
    pub temp_pane_bg_darken: f32,
    /// Index of the pane currently being configured (None = no pane selected)
    pub temp_pane_bg_index: Option<usize>,

    /// Search query used to filter settings sections
    pub search_query: String,
    /// Whether to focus the search input on next frame
    pub focus_search: bool,

    // Background shader editor state
    /// Whether the shader editor window is visible
    pub shader_editor_visible: bool,
    /// The shader source code being edited
    pub shader_editor_source: String,
    /// Shader compilation error message (if any)
    pub shader_editor_error: Option<String>,
    /// Original source when editor was opened (for cancel)
    pub shader_editor_original: String,

    // Cursor shader editor state
    /// Whether the cursor shader editor window is visible
    pub cursor_shader_editor_visible: bool,
    /// The cursor shader source code being edited
    pub cursor_shader_editor_source: String,
    /// Cursor shader compilation error message (if any)
    pub cursor_shader_editor_error: Option<String>,
    /// Original cursor shader source when editor was opened (for cancel)
    pub cursor_shader_editor_original: String,

    // Agent state
    /// Available agent identities for the AI Inspector dropdown (identity, name)
    pub available_agent_ids: Vec<(String, String)>,

    // Shader management state
    /// List of available shader files in the shaders folder
    pub available_shaders: Vec<String>,
    /// List of available cubemap prefixes (e.g., "textures/cubemaps/env-outside")
    pub available_cubemaps: Vec<String>,
    /// Name for new shader (in create dialog)
    pub new_shader_name: String,
    /// Whether to show the create shader dialog
    pub show_create_shader_dialog: bool,
    /// Whether to show the delete confirmation dialog
    pub show_delete_shader_dialog: bool,

    // Shader editor search state
    /// Search query for shader editor
    pub shader_search_query: String,
    /// Byte positions of search matches
    pub shader_search_matches: Vec<usize>,
    /// Current match index (0-based)
    pub shader_search_current: usize,
    /// Whether search bar is visible
    pub shader_search_visible: bool,

    // Per-shader configuration state
    /// Cache for parsed shader metadata
    pub shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor shader metadata
    pub cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Whether the per-shader settings section is expanded
    pub shader_settings_expanded: bool,
    /// Whether the per-cursor-shader settings section is expanded
    pub cursor_shader_settings_expanded: bool,

    // Current window state (for "Use Current Size" button)
    /// Current terminal columns (actual rendered size, may differ from config)
    pub current_cols: usize,
    /// Current terminal rows (actual rendered size, may differ from config)
    pub current_rows: usize,

    // VSync mode support (for runtime validation)
    /// Supported vsync modes for the current display
    pub supported_vsync_modes: Vec<par_term_config::VsyncMode>,
    /// Warning message when an unsupported vsync mode is selected
    pub vsync_warning: Option<String>,

    // Keybinding recording state
    /// Index of the keybinding currently being recorded (None = not recording)
    pub keybinding_recording_index: Option<usize>,
    /// The recorded key combination string (displayed during recording)
    pub keybinding_recorded_combo: Option<String>,

    // Notification test state
    /// Flag to request sending a test notification
    pub test_notification_requested: bool,

    // New UI state for reorganized settings
    /// Currently selected settings tab (new sidebar navigation)
    pub selected_tab: SettingsTab,
    /// Set of collapsed section IDs (sections start open by default, collapsed when user collapses them)
    pub collapsed_sections: HashSet<String>,

    // Integrations tab action state
    /// Pending shell integration action (install/uninstall)
    pub shell_integration_action: Option<crate::integrations_tab::ShellIntegrationAction>,

    // Profiles tab inline management state
    /// Inline profile management UI (embedded in Profiles tab)
    pub profile_modal_ui: ProfileModalUI,
    /// Flag: profile save was requested from inline UI
    pub profile_save_requested: bool,
    /// Flag: open a profile was requested from inline UI
    pub profile_open_requested: Option<ProfileId>,
    // Shader install workflow state
    /// Whether a shader install/uninstall operation is running
    pub(crate) shader_installing: bool,
    /// Status message for shader installs
    pub(crate) shader_status: Option<String>,
    /// Error message for shader installs
    pub(crate) shader_error: Option<String>,
    /// Whether to show overwrite prompt for modified bundled shaders
    pub(crate) shader_overwrite_prompt_visible: bool,
    /// List of modified bundled shader files
    pub(crate) shader_conflicts: Vec<String>,
    /// Channel receiver for async shader installs
    shader_install_receiver: Option<std::sync::mpsc::Receiver<Result<ShaderInstallResult, String>>>,

    // Automation tab state
    /// Index of trigger currently being edited (None = not editing)
    pub editing_trigger_index: Option<usize>,
    /// Temporary trigger name for edit form
    pub temp_trigger_name: String,
    /// Temporary trigger regex pattern for edit form
    pub temp_trigger_pattern: String,
    /// Temporary trigger actions for edit form
    pub temp_trigger_actions: Vec<par_term_config::automation::TriggerActionConfig>,
    /// Temporary require_user_action flag for trigger edit form
    pub temp_trigger_require_user_action: bool,
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
    /// Pending coprocess start/stop actions: (config_index, start=true/stop=false)
    pub pending_coprocess_actions: Vec<(usize, bool)>,
    /// Running state of coprocesses (indexed by config position, updated by main window)
    pub coprocess_running: Vec<bool>,
    /// Last error messages per coprocess (indexed by config position, updated by main window)
    pub coprocess_errors: Vec<String>,
    /// Buffered stdout output per coprocess (indexed by config position, drained from core)
    pub coprocess_output: Vec<Vec<String>>,
    /// Which coprocess output viewers are expanded (indexed by config position)
    pub coprocess_output_expanded: Vec<bool>,
    // === Script management state ===
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
    /// Pending script start/stop actions: (config_index, start=true/stop=false)
    pub pending_script_actions: Vec<(usize, bool)>,
    /// Running state of scripts (indexed by config position, updated by main window)
    pub script_running: Vec<bool>,
    /// Last error messages per script (indexed by config position, updated by main window)
    pub script_errors: Vec<String>,
    /// Buffered output per script (indexed by config position, drained from script manager)
    pub script_output: Vec<Vec<String>>,
    /// Which script output viewers are expanded (indexed by config position)
    pub script_output_expanded: Vec<bool>,
    /// Panel state per script: (title, content) from SetPanel commands
    pub script_panels: Vec<Option<(String, String)>>,

    /// Flag to request opening the debug log file
    pub open_log_requested: bool,

    /// Flag to request identifying panes (flash indices on terminal window)
    pub identify_panes_requested: bool,

    // Self-update state
    /// User requested to install the available update
    pub update_install_requested: bool,
    /// User requested an immediate update check
    pub check_now_requested: bool,
    /// Status text for update UI display
    pub update_status: Option<String>,
    /// Result of self-update operation
    pub update_result: Option<Result<UpdateResult, String>>,
    /// Last update check result (synced from WindowManager)
    pub last_update_result: Option<UpdateCheckResult>,
    /// Whether an update install is in progress
    pub update_installing: bool,
    /// Channel receiver for async update installs
    update_install_receiver: Option<std::sync::mpsc::Receiver<Result<UpdateResult, String>>>,

    // Snippets tab state
    /// Index of snippet currently being edited (None = not editing)
    pub editing_snippet_index: Option<usize>,
    /// Temporary snippet ID for edit form
    pub temp_snippet_id: String,
    /// Temporary snippet title for edit form
    pub temp_snippet_title: String,
    /// Temporary snippet content for edit form
    pub temp_snippet_content: String,
    /// Temporary snippet keybinding for edit form
    pub temp_snippet_keybinding: String,
    /// Temporary snippet folder for edit form
    pub temp_snippet_folder: String,
    /// Temporary snippet description for edit form
    pub temp_snippet_description: String,
    /// Temporary snippet keybinding enabled for edit form
    pub temp_snippet_keybinding_enabled: bool,
    /// Temporary snippet auto_execute for edit form
    pub temp_snippet_auto_execute: bool,
    /// Temporary snippet custom variables for edit form (ordered pairs for stable UI)
    pub temp_snippet_variables: Vec<(String, String)>,
    /// Whether the add-new-snippet form is active
    pub adding_new_snippet: bool,
    /// Whether currently recording a keybinding for a snippet
    pub recording_snippet_keybinding: bool,
    /// Recorded keybinding combo for snippet (displayed during recording)
    pub snippet_recorded_combo: Option<String>,

    // Actions tab state
    /// Index of action currently being edited (None = not editing)
    pub editing_action_index: Option<usize>,
    /// Temporary action type for edit form (0=ShellCommand, 1=InsertText, 2=KeySequence)
    pub temp_action_type: usize,
    /// Temporary action ID for edit form
    pub temp_action_id: String,
    /// Temporary action title for edit form
    pub temp_action_title: String,
    /// Temporary action command (for ShellCommand type)
    pub temp_action_command: String,
    /// Temporary action args (for ShellCommand type)
    pub temp_action_args: String,
    /// Temporary action text (for InsertText type)
    pub temp_action_text: String,
    /// Temporary action keys (for KeySequence type)
    pub temp_action_keys: String,
    /// Temporary action keybinding for edit form
    pub temp_action_keybinding: String,
    /// Whether the add-new-action form is active
    pub adding_new_action: bool,
    /// Whether currently recording a keybinding for an action
    pub recording_action_keybinding: bool,
    /// Recorded keybinding combo for action (displayed during recording)
    pub action_recorded_combo: Option<String>,

    // Dynamic profile sources editing state
    /// Index of dynamic source currently being edited (None = not editing)
    pub dynamic_source_editing: Option<usize>,
    /// Temp copy of the source being edited
    pub dynamic_source_edit_buffer: Option<par_term_config::DynamicProfileSource>,
    /// Temp buffer for new header key being added
    pub dynamic_source_new_header_key: String,
    /// Temp buffer for new header value being added
    pub dynamic_source_new_header_value: String,

    // Import/export preferences state
    /// Temporary URL for import-from-URL feature
    pub temp_import_url: String,
    /// Status message for import/export operations
    pub import_export_status: Option<String>,
    /// Whether the import/export status is an error (true) or success (false)
    pub import_export_is_error: bool,

    // Reset to defaults dialog state
    /// Whether to show the reset to defaults confirmation dialog
    pub show_reset_defaults_dialog: bool,

    // Arrangements tab state
    /// Name for saving a new arrangement
    pub arrangement_save_name: String,
    /// Arrangement ID pending restore confirmation
    pub arrangement_confirm_restore: Option<ArrangementId>,
    /// Arrangement ID pending delete confirmation
    pub arrangement_confirm_delete: Option<ArrangementId>,
    /// Name pending overwrite confirmation (when saving with duplicate name)
    pub arrangement_confirm_overwrite: Option<String>,
    /// Arrangement ID being renamed
    pub arrangement_rename_id: Option<ArrangementId>,
    /// Text buffer for rename operation
    pub arrangement_rename_text: String,
    /// Pending arrangement actions to send to the main window
    pub pending_arrangement_actions: Vec<SettingsWindowAction>,
    /// Cached arrangement manager data (synced from WindowManager)
    pub arrangement_manager: ArrangementManager,

    // Callbacks for main-crate operations
    /// Application version string (set by main crate via env!("CARGO_PKG_VERSION"))
    pub app_version: &'static str,

    /// Detected installation type (set by main crate)
    pub installation_type: InstallationType,

    /// Callback: install shaders with manifest (set by main crate)
    pub shader_install_fn: Option<fn(bool) -> Result<ShaderInstallResult, String>>,

    /// Callback: detect modified bundled shaders
    pub shader_detect_modified_fn: Option<ShaderDetectModifiedFn>,

    /// Callback: uninstall shaders
    pub shader_uninstall_fn: Option<fn(bool) -> Result<ShaderUninstallResult, String>>,

    /// Callback: check if shader files exist
    pub shader_has_files_fn: Option<fn(&std::path::Path) -> bool>,

    /// Callback: count shader files
    pub shader_count_files_fn: Option<fn(&std::path::Path) -> usize>,

    // Test detection state (prettifier tab)
    /// Multiline sample text for testing detection
    pub test_detection_content: String,
    /// Optional preceding command for test detection
    pub test_detection_command: String,
    /// Flag set by UI button to request detection test
    pub test_detection_requested: bool,
    /// Result of last detection test: (format_id, confidence, matched_rules, threshold)
    pub test_detection_result: Option<(String, f32, Vec<String>, f32)>,

    /// Callback: check if shell integration is installed
    pub shell_integration_is_installed_fn: Option<fn() -> bool>,

    /// Callback: get detected shell type
    pub shell_integration_detected_shell_fn: Option<fn() -> par_term_config::ShellType>,

    /// Callback: install shell integration
    pub shell_integration_install_fn: Option<fn() -> Result<ShellIntegrationInstallResult, String>>,

    /// Callback: uninstall shell integration
    pub shell_integration_uninstall_fn:
        Option<fn() -> Result<ShellIntegrationUninstallResult, String>>,
}

mod display;
mod state;
