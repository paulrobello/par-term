//! Settings UI for the terminal emulator.
//!
//! This module provides an egui-based settings window for configuring
//! terminal options at runtime.

use crate::config::{BackgroundImageMode, Config, CursorShaderMetadataCache, ShaderMetadataCache};
use crate::profile::{Profile, ProfileId};
use crate::profile_modal_ui::ProfileModalUI;
use egui::{Color32, Context, Frame, Window, epaint::Shadow};
use rfd::FileDialog;
use std::collections::HashSet;

// Reorganized settings tabs (12 consolidated tabs)
pub mod actions_tab;
pub mod advanced_tab;
pub mod appearance_tab;
pub mod arrangements_tab;
pub mod automation_tab;
pub mod badge_tab;
pub mod effects_tab;
pub mod input_tab;
pub mod integrations_tab;
pub mod notifications_tab;
pub mod profiles_tab;
pub mod progress_bar_tab;
pub mod quick_settings;
pub(crate) mod scripts_tab;
pub mod section;
pub mod sidebar;
pub mod snippets_tab;
pub mod ssh_tab;
pub mod status_bar_tab;
pub mod terminal_tab;
pub mod window_tab;

// Background tab is still needed by effects_tab for delegation
pub mod background_tab;

// Shader editor components (used by background_tab)
mod cursor_shader_editor;
mod shader_dialogs;
mod shader_editor;
mod shader_utils;

pub use sidebar::SettingsTab;

/// Result of shader editor actions
#[derive(Debug, Clone)]
pub struct ShaderEditorResult {
    /// New shader source code to compile and apply
    pub source: String,
}

/// Result of cursor shader editor actions
#[derive(Debug, Clone)]
pub struct CursorShaderEditorResult {
    /// New cursor shader source code to compile and apply
    pub source: String,
}

/// Settings UI manager using egui
pub struct SettingsUI {
    /// Whether the settings window is currently visible
    pub visible: bool,

    /// Working copy of config being edited
    pub(crate) config: Config,

    /// Last opacity value that was forwarded for live updates
    pub(crate) last_live_opacity: f32,

    /// Whether config has unsaved changes
    pub(crate) has_changes: bool,

    /// Temp strings for optional fields (for UI editing)
    pub(crate) temp_font_bold: String,
    pub(crate) temp_font_italic: String,
    pub(crate) temp_font_bold_italic: String,
    pub(crate) temp_font_family: String,
    pub(crate) temp_font_size: f32,
    pub(crate) temp_line_spacing: f32,
    pub(crate) temp_char_spacing: f32,
    pub(crate) temp_enable_text_shaping: bool,
    pub(crate) temp_enable_ligatures: bool,
    pub(crate) temp_enable_kerning: bool,
    pub(crate) font_pending_changes: bool,
    pub(crate) temp_custom_shell: String,
    pub(crate) temp_shell_args: String,
    pub(crate) temp_working_directory: String,
    pub(crate) temp_startup_directory: String,
    pub(crate) temp_initial_text: String,
    pub(crate) temp_background_image: String,
    pub(crate) temp_custom_shader: String,
    pub(crate) temp_cursor_shader: String,

    /// Temporary strings for shader channel texture paths (iChannel0-3)
    pub(crate) temp_shader_channel0: String,
    pub(crate) temp_shader_channel1: String,
    pub(crate) temp_shader_channel2: String,
    pub(crate) temp_shader_channel3: String,

    /// Temporary string for cubemap path prefix (iCubemap)
    pub(crate) temp_cubemap_path: String,

    /// Temporary color for solid background color editing
    pub(crate) temp_background_color: [u8; 3],

    /// Temporary per-pane background image path for editing
    pub(crate) temp_pane_bg_path: String,
    /// Temporary per-pane background mode
    pub(crate) temp_pane_bg_mode: BackgroundImageMode,
    /// Temporary per-pane background opacity
    pub(crate) temp_pane_bg_opacity: f32,
    /// Index of the pane currently being configured (None = no pane selected)
    pub(crate) temp_pane_bg_index: Option<usize>,

    /// Search query used to filter settings sections
    pub(crate) search_query: String,

    // Background shader editor state
    /// Whether the shader editor window is visible
    pub(crate) shader_editor_visible: bool,
    /// The shader source code being edited
    pub(crate) shader_editor_source: String,
    /// Shader compilation error message (if any)
    pub(crate) shader_editor_error: Option<String>,
    /// Original source when editor was opened (for cancel)
    pub(crate) shader_editor_original: String,

    // Cursor shader editor state
    /// Whether the cursor shader editor window is visible
    pub(crate) cursor_shader_editor_visible: bool,
    /// The cursor shader source code being edited
    pub(crate) cursor_shader_editor_source: String,
    /// Cursor shader compilation error message (if any)
    pub(crate) cursor_shader_editor_error: Option<String>,
    /// Original cursor shader source when editor was opened (for cancel)
    pub(crate) cursor_shader_editor_original: String,

    // Shader management state
    /// List of available shader files in the shaders folder
    pub(crate) available_shaders: Vec<String>,
    /// List of available cubemap prefixes (e.g., "textures/cubemaps/env-outside")
    pub(crate) available_cubemaps: Vec<String>,
    /// Name for new shader (in create dialog)
    pub(crate) new_shader_name: String,
    /// Whether to show the create shader dialog
    pub(crate) show_create_shader_dialog: bool,
    /// Whether to show the delete confirmation dialog
    pub(crate) show_delete_shader_dialog: bool,

    // Shader editor search state
    /// Search query for shader editor
    pub(crate) shader_search_query: String,
    /// Byte positions of search matches
    pub(crate) shader_search_matches: Vec<usize>,
    /// Current match index (0-based)
    pub(crate) shader_search_current: usize,
    /// Whether search bar is visible
    pub(crate) shader_search_visible: bool,

    // Per-shader configuration state
    /// Cache for parsed shader metadata
    pub(crate) shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor shader metadata
    pub(crate) cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Whether the per-shader settings section is expanded
    pub(crate) shader_settings_expanded: bool,
    /// Whether the per-cursor-shader settings section is expanded
    pub(crate) cursor_shader_settings_expanded: bool,

    // Current window state (for "Use Current Size" button)
    /// Current terminal columns (actual rendered size, may differ from config)
    pub(crate) current_cols: usize,
    /// Current terminal rows (actual rendered size, may differ from config)
    pub(crate) current_rows: usize,

    // VSync mode support (for runtime validation)
    /// Supported vsync modes for the current display
    pub(crate) supported_vsync_modes: Vec<crate::config::VsyncMode>,
    /// Warning message when an unsupported vsync mode is selected
    pub(crate) vsync_warning: Option<String>,

    // Keybinding recording state
    /// Index of the keybinding currently being recorded (None = not recording)
    pub(crate) keybinding_recording_index: Option<usize>,
    /// The recorded key combination string (displayed during recording)
    pub(crate) keybinding_recorded_combo: Option<String>,

    // Notification test state
    /// Flag to request sending a test notification
    pub(crate) test_notification_requested: bool,

    // New UI state for reorganized settings
    /// Currently selected settings tab (new sidebar navigation)
    pub(crate) selected_tab: SettingsTab,
    /// Set of collapsed section IDs (sections start open by default, collapsed when user collapses them)
    pub(crate) collapsed_sections: HashSet<String>,

    // Integrations tab action state
    /// Pending shell integration action (install/uninstall)
    pub(crate) shell_integration_action: Option<integrations_tab::ShellIntegrationAction>,

    // Profiles tab inline management state
    /// Inline profile management UI (embedded in Profiles tab)
    pub(crate) profile_modal_ui: ProfileModalUI,
    /// Flag: profile save was requested from inline UI
    pub(crate) profile_save_requested: bool,
    /// Flag: open a profile was requested from inline UI
    pub(crate) profile_open_requested: Option<ProfileId>,
    // Shader install workflow state
    /// Whether a shader install/uninstall operation is running
    shader_installing: bool,
    /// Status message for shader installs
    shader_status: Option<String>,
    /// Error message for shader installs
    shader_error: Option<String>,
    /// Whether to show overwrite prompt for modified bundled shaders
    shader_overwrite_prompt_visible: bool,
    /// List of modified bundled shader files
    shader_conflicts: Vec<String>,
    /// Channel receiver for async shader installs
    shader_install_receiver:
        Option<std::sync::mpsc::Receiver<Result<crate::shader_installer::InstallResult, String>>>,

    // Automation tab state
    /// Index of trigger currently being edited (None = not editing)
    pub(crate) editing_trigger_index: Option<usize>,
    /// Temporary trigger name for edit form
    pub(crate) temp_trigger_name: String,
    /// Temporary trigger regex pattern for edit form
    pub(crate) temp_trigger_pattern: String,
    /// Temporary trigger actions for edit form
    pub(crate) temp_trigger_actions: Vec<crate::config::automation::TriggerActionConfig>,
    /// Whether the add-new-trigger form is active
    pub(crate) adding_new_trigger: bool,
    /// Regex validation error for trigger pattern
    pub(crate) trigger_pattern_error: Option<String>,
    /// Index of coprocess currently being edited (None = not editing)
    pub(crate) editing_coprocess_index: Option<usize>,
    /// Temporary coprocess name for edit form
    pub(crate) temp_coprocess_name: String,
    /// Temporary coprocess command for edit form
    pub(crate) temp_coprocess_command: String,
    /// Temporary coprocess args for edit form
    pub(crate) temp_coprocess_args: String,
    /// Temporary coprocess auto_start for edit form
    pub(crate) temp_coprocess_auto_start: bool,
    /// Temporary coprocess copy_terminal_output for edit form
    pub(crate) temp_coprocess_copy_output: bool,
    /// Temporary coprocess restart policy for edit form
    pub(crate) temp_coprocess_restart_policy: crate::config::automation::RestartPolicy,
    /// Temporary coprocess restart delay for edit form
    pub(crate) temp_coprocess_restart_delay_ms: u64,
    /// Whether the add-new-coprocess form is active
    pub(crate) adding_new_coprocess: bool,
    /// Flag to request trigger resync after save
    pub trigger_resync_requested: bool,
    /// Pending coprocess start/stop actions: (config_index, start=true/stop=false)
    pub(crate) pending_coprocess_actions: Vec<(usize, bool)>,
    /// Running state of coprocesses (indexed by config position, updated by main window)
    pub coprocess_running: Vec<bool>,
    /// Last error messages per coprocess (indexed by config position, updated by main window)
    pub coprocess_errors: Vec<String>,
    /// Buffered stdout output per coprocess (indexed by config position, drained from core)
    pub coprocess_output: Vec<Vec<String>>,
    /// Which coprocess output viewers are expanded (indexed by config position)
    pub(crate) coprocess_output_expanded: Vec<bool>,
    // === Script management state ===
    /// Index of script currently being edited (None = not editing)
    pub(crate) editing_script_index: Option<usize>,
    /// Temporary script name for edit form
    pub(crate) temp_script_name: String,
    /// Temporary script path for edit form
    pub(crate) temp_script_path: String,
    /// Temporary script args for edit form
    pub(crate) temp_script_args: String,
    /// Temporary script auto_start for edit form
    pub(crate) temp_script_auto_start: bool,
    /// Temporary script enabled for edit form
    pub(crate) temp_script_enabled: bool,
    /// Temporary script restart policy for edit form
    pub(crate) temp_script_restart_policy: crate::config::automation::RestartPolicy,
    /// Temporary script restart delay for edit form
    pub(crate) temp_script_restart_delay_ms: u64,
    /// Temporary script subscriptions for edit form (comma-separated)
    pub(crate) temp_script_subscriptions: String,
    /// Whether the add-new-script form is active
    pub(crate) adding_new_script: bool,
    /// Pending script start/stop actions: (config_index, start=true/stop=false)
    pub(crate) pending_script_actions: Vec<(usize, bool)>,
    /// Running state of scripts (indexed by config position, updated by main window)
    pub script_running: Vec<bool>,
    /// Last error messages per script (indexed by config position, updated by main window)
    pub script_errors: Vec<String>,
    /// Buffered output per script (indexed by config position, drained from script manager)
    pub script_output: Vec<Vec<String>>,
    /// Which script output viewers are expanded (indexed by config position)
    pub(crate) script_output_expanded: Vec<bool>,
    /// Panel state per script: (title, content) from SetPanel commands
    pub script_panels: Vec<Option<(String, String)>>,

    /// Flag to request opening the debug log file
    pub(crate) open_log_requested: bool,

    /// Flag to request identifying panes (flash indices on terminal window)
    pub(crate) identify_panes_requested: bool,

    // Self-update state
    /// User requested to install the available update
    pub(crate) update_install_requested: bool,
    /// User requested an immediate update check
    pub(crate) check_now_requested: bool,
    /// Status text for update UI display
    pub(crate) update_status: Option<String>,
    /// Result of self-update operation
    pub(crate) update_result: Option<Result<crate::self_updater::UpdateResult, String>>,
    /// Last update check result (synced from WindowManager)
    pub(crate) last_update_result: Option<crate::update_checker::UpdateCheckResult>,
    /// Whether an update install is in progress
    pub(crate) update_installing: bool,
    /// Channel receiver for async update installs
    update_install_receiver:
        Option<std::sync::mpsc::Receiver<Result<crate::self_updater::UpdateResult, String>>>,

    // Snippets tab state
    /// Index of snippet currently being edited (None = not editing)
    pub(crate) editing_snippet_index: Option<usize>,
    /// Temporary snippet ID for edit form
    pub(crate) temp_snippet_id: String,
    /// Temporary snippet title for edit form
    pub(crate) temp_snippet_title: String,
    /// Temporary snippet content for edit form
    pub(crate) temp_snippet_content: String,
    /// Temporary snippet keybinding for edit form
    pub(crate) temp_snippet_keybinding: String,
    /// Temporary snippet folder for edit form
    pub(crate) temp_snippet_folder: String,
    /// Temporary snippet description for edit form
    pub(crate) temp_snippet_description: String,
    /// Temporary snippet keybinding enabled for edit form
    pub(crate) temp_snippet_keybinding_enabled: bool,
    /// Temporary snippet auto_execute for edit form
    pub(crate) temp_snippet_auto_execute: bool,
    /// Temporary snippet custom variables for edit form (ordered pairs for stable UI)
    pub(crate) temp_snippet_variables: Vec<(String, String)>,
    /// Whether the add-new-snippet form is active
    pub(crate) adding_new_snippet: bool,
    /// Whether currently recording a keybinding for a snippet
    pub(crate) recording_snippet_keybinding: bool,
    /// Recorded keybinding combo for snippet (displayed during recording)
    pub(crate) snippet_recorded_combo: Option<String>,

    // Actions tab state
    /// Index of action currently being edited (None = not editing)
    pub(crate) editing_action_index: Option<usize>,
    /// Temporary action type for edit form (0=ShellCommand, 1=InsertText, 2=KeySequence)
    pub(crate) temp_action_type: usize,
    /// Temporary action ID for edit form
    pub(crate) temp_action_id: String,
    /// Temporary action title for edit form
    pub(crate) temp_action_title: String,
    /// Temporary action command (for ShellCommand type)
    pub(crate) temp_action_command: String,
    /// Temporary action args (for ShellCommand type)
    pub(crate) temp_action_args: String,
    /// Temporary action text (for InsertText type)
    pub(crate) temp_action_text: String,
    /// Temporary action keys (for KeySequence type)
    pub(crate) temp_action_keys: String,
    /// Temporary action keybinding for edit form
    pub(crate) temp_action_keybinding: String,
    /// Whether the add-new-action form is active
    pub(crate) adding_new_action: bool,
    /// Whether currently recording a keybinding for an action
    pub(crate) recording_action_keybinding: bool,
    /// Recorded keybinding combo for action (displayed during recording)
    pub(crate) action_recorded_combo: Option<String>,

    // Dynamic profile sources editing state
    /// Index of dynamic source currently being edited (None = not editing)
    pub(crate) dynamic_source_editing: Option<usize>,
    /// Temp copy of the source being edited
    pub(crate) dynamic_source_edit_buffer: Option<crate::profile::DynamicProfileSource>,
    /// Temp buffer for new header key being added
    pub(crate) dynamic_source_new_header_key: String,
    /// Temp buffer for new header value being added
    pub(crate) dynamic_source_new_header_value: String,

    // Import/export preferences state
    /// Temporary URL for import-from-URL feature
    pub(crate) temp_import_url: String,
    /// Status message for import/export operations
    pub(crate) import_export_status: Option<String>,
    /// Whether the import/export status is an error (true) or success (false)
    pub(crate) import_export_is_error: bool,

    // Reset to defaults dialog state
    /// Whether to show the reset to defaults confirmation dialog
    pub(crate) show_reset_defaults_dialog: bool,

    // Arrangements tab state
    /// Name for saving a new arrangement
    pub(crate) arrangement_save_name: String,
    /// Arrangement ID pending restore confirmation
    pub(crate) arrangement_confirm_restore: Option<crate::arrangements::ArrangementId>,
    /// Arrangement ID pending delete confirmation
    pub(crate) arrangement_confirm_delete: Option<crate::arrangements::ArrangementId>,
    /// Name pending overwrite confirmation (when saving with duplicate name)
    pub(crate) arrangement_confirm_overwrite: Option<String>,
    /// Arrangement ID being renamed
    pub(crate) arrangement_rename_id: Option<crate::arrangements::ArrangementId>,
    /// Text buffer for rename operation
    pub(crate) arrangement_rename_text: String,
    /// Pending arrangement actions to send to the main window
    pub(crate) pending_arrangement_actions: Vec<crate::settings_window::SettingsWindowAction>,
    /// Cached arrangement manager data (synced from WindowManager)
    pub(crate) arrangement_manager: crate::arrangements::ArrangementManager,
}

impl SettingsUI {
    /// Create a new settings UI
    pub fn new(config: Config) -> Self {
        // Extract values before moving config
        let initial_cols = config.cols;
        let initial_rows = config.rows;
        let initial_collapsed: HashSet<String> =
            config.collapsed_settings_sections.iter().cloned().collect();

        Self {
            visible: false,
            temp_font_bold: config.font_family_bold.clone().unwrap_or_default(),
            temp_font_italic: config.font_family_italic.clone().unwrap_or_default(),
            temp_font_bold_italic: config.font_family_bold_italic.clone().unwrap_or_default(),
            temp_font_family: config.font_family.clone(),
            temp_font_size: config.font_size,
            temp_line_spacing: config.line_spacing,
            temp_char_spacing: config.char_spacing,
            temp_enable_text_shaping: config.enable_text_shaping,
            temp_enable_ligatures: config.enable_ligatures,
            temp_enable_kerning: config.enable_kerning,
            font_pending_changes: false,
            temp_custom_shell: config.custom_shell.clone().unwrap_or_default(),
            temp_shell_args: config
                .shell_args
                .as_ref()
                .map(|args| args.join(" "))
                .unwrap_or_default(),
            temp_working_directory: config.working_directory.clone().unwrap_or_default(),
            temp_startup_directory: config.startup_directory.clone().unwrap_or_default(),
            temp_initial_text: config.initial_text.clone(),
            temp_background_image: config.background_image.clone().unwrap_or_default(),
            temp_custom_shader: config.custom_shader.clone().unwrap_or_default(),
            temp_cursor_shader: config.cursor_shader.clone().unwrap_or_default(),
            temp_shader_channel0: config.custom_shader_channel0.clone().unwrap_or_default(),
            temp_shader_channel1: config.custom_shader_channel1.clone().unwrap_or_default(),
            temp_shader_channel2: config.custom_shader_channel2.clone().unwrap_or_default(),
            temp_shader_channel3: config.custom_shader_channel3.clone().unwrap_or_default(),
            temp_cubemap_path: config.custom_shader_cubemap.clone().unwrap_or_default(),
            temp_background_color: config.background_color,
            temp_pane_bg_path: String::new(),
            temp_pane_bg_mode: BackgroundImageMode::default(),
            temp_pane_bg_opacity: 1.0,
            temp_pane_bg_index: None,
            last_live_opacity: config.window_opacity,
            current_cols: initial_cols,
            current_rows: initial_rows,
            supported_vsync_modes: vec![
                crate::config::VsyncMode::Immediate,
                crate::config::VsyncMode::Mailbox,
                crate::config::VsyncMode::Fifo,
            ], // Will be updated when renderer is available
            vsync_warning: None,
            config,
            has_changes: false,
            search_query: String::new(),
            shader_editor_visible: false,
            shader_editor_source: String::new(),
            shader_editor_error: None,
            shader_editor_original: String::new(),
            cursor_shader_editor_visible: false,
            cursor_shader_editor_source: String::new(),
            cursor_shader_editor_error: None,
            cursor_shader_editor_original: String::new(),
            available_shaders: Self::scan_shaders_folder(),
            available_cubemaps: Self::scan_cubemaps_folder(),
            new_shader_name: String::new(),
            show_create_shader_dialog: false,
            show_delete_shader_dialog: false,
            shader_search_query: String::new(),
            shader_search_matches: Vec::new(),
            shader_search_current: 0,
            shader_search_visible: false,
            shader_metadata_cache: ShaderMetadataCache::with_shaders_dir(
                crate::config::Config::shaders_dir(),
            ),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(
                crate::config::Config::shaders_dir(),
            ),
            shader_settings_expanded: true,
            cursor_shader_settings_expanded: true,
            keybinding_recording_index: None,
            keybinding_recorded_combo: None,
            test_notification_requested: false,
            selected_tab: SettingsTab::default(),
            collapsed_sections: initial_collapsed,
            shell_integration_action: None,
            profile_modal_ui: ProfileModalUI::new(),
            profile_save_requested: false,
            profile_open_requested: None,
            shader_installing: false,
            shader_status: None,
            shader_error: None,
            shader_overwrite_prompt_visible: false,
            shader_conflicts: Vec::new(),
            shader_install_receiver: None,
            editing_trigger_index: None,
            temp_trigger_name: String::new(),
            temp_trigger_pattern: String::new(),
            temp_trigger_actions: Vec::new(),
            adding_new_trigger: false,
            trigger_pattern_error: None,
            editing_coprocess_index: None,
            temp_coprocess_name: String::new(),
            temp_coprocess_command: String::new(),
            temp_coprocess_args: String::new(),
            temp_coprocess_auto_start: false,
            temp_coprocess_copy_output: true,
            temp_coprocess_restart_policy: crate::config::automation::RestartPolicy::Never,
            temp_coprocess_restart_delay_ms: 0,
            adding_new_coprocess: false,
            trigger_resync_requested: false,
            pending_coprocess_actions: Vec::new(),
            coprocess_running: Vec::new(),
            coprocess_errors: Vec::new(),
            coprocess_output: Vec::new(),
            coprocess_output_expanded: Vec::new(),
            editing_script_index: None,
            temp_script_name: String::new(),
            temp_script_path: String::new(),
            temp_script_args: String::new(),
            temp_script_auto_start: false,
            temp_script_enabled: true,
            temp_script_restart_policy: crate::config::automation::RestartPolicy::Never,
            temp_script_restart_delay_ms: 0,
            temp_script_subscriptions: String::new(),
            adding_new_script: false,
            pending_script_actions: Vec::new(),
            script_running: Vec::new(),
            script_errors: Vec::new(),
            script_output: Vec::new(),
            script_output_expanded: Vec::new(),
            script_panels: Vec::new(),
            open_log_requested: false,
            identify_panes_requested: false,
            update_install_requested: false,
            check_now_requested: false,
            update_status: None,
            update_result: None,
            last_update_result: None,
            update_installing: false,
            update_install_receiver: None,
            editing_snippet_index: None,
            temp_snippet_id: String::new(),
            temp_snippet_title: String::new(),
            temp_snippet_content: String::new(),
            temp_snippet_keybinding: String::new(),
            temp_snippet_folder: String::new(),
            temp_snippet_description: String::new(),
            temp_snippet_keybinding_enabled: true,
            temp_snippet_auto_execute: false,
            temp_snippet_variables: Vec::new(),
            adding_new_snippet: false,
            editing_action_index: None,
            temp_action_type: 0,
            temp_action_id: String::new(),
            temp_action_title: String::new(),
            temp_action_command: String::new(),
            temp_action_args: String::new(),
            temp_action_text: String::new(),
            temp_action_keys: String::new(),
            temp_action_keybinding: String::new(),
            adding_new_action: false,
            recording_snippet_keybinding: false,
            snippet_recorded_combo: None,
            recording_action_keybinding: false,
            action_recorded_combo: None,
            dynamic_source_editing: None,
            dynamic_source_edit_buffer: None,
            dynamic_source_new_header_key: String::new(),
            dynamic_source_new_header_value: String::new(),
            temp_import_url: String::new(),
            import_export_status: None,
            import_export_is_error: false,
            show_reset_defaults_dialog: false,
            arrangement_save_name: String::new(),
            arrangement_confirm_restore: None,
            arrangement_confirm_delete: None,
            arrangement_confirm_overwrite: None,
            arrangement_rename_id: None,
            arrangement_rename_text: String::new(),
            pending_arrangement_actions: Vec::new(),
            arrangement_manager: crate::arrangements::ArrangementManager::new(),
        }
    }

    /// Update the current terminal dimensions (called when window resizes)
    pub fn update_current_size(&mut self, cols: usize, rows: usize) {
        self.current_cols = cols;
        self.current_rows = rows;
    }

    /// Update the list of supported vsync modes (called when renderer is initialized)
    pub fn update_supported_vsync_modes(&mut self, modes: Vec<crate::config::VsyncMode>) {
        self.supported_vsync_modes = modes;
        // Clear any previous warning
        self.vsync_warning = None;
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: crate::config::VsyncMode) -> bool {
        self.supported_vsync_modes.contains(&mode)
    }

    /// Set vsync warning message (called when an unsupported mode is detected)
    #[allow(dead_code)]
    pub fn set_vsync_warning(&mut self, warning: Option<String>) {
        self.vsync_warning = warning;
    }

    pub(crate) fn pick_file_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_file()
            .map(|p| p.display().to_string())
    }

    pub(crate) fn pick_folder_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_folder()
            .map(|p| p.display().to_string())
    }

    /// Update the config copy (e.g., when config is reloaded)
    pub fn update_config(&mut self, config: Config) {
        if !self.has_changes {
            self.config = config;
            self.last_live_opacity = self.config.window_opacity;

            // Refresh staged font values only if there aren't unsaved font edits
            if !self.font_pending_changes {
                self.sync_font_temps_from_config();
            }
        }
    }

    fn sync_font_temps_from_config(&mut self) {
        self.temp_font_family = self.config.font_family.clone();
        self.temp_font_size = self.config.font_size;
        self.temp_line_spacing = self.config.line_spacing;
        self.temp_char_spacing = self.config.char_spacing;
        self.temp_enable_text_shaping = self.config.enable_text_shaping;
        self.temp_enable_ligatures = self.config.enable_ligatures;
        self.temp_enable_kerning = self.config.enable_kerning;
        self.temp_font_bold = self.config.font_family_bold.clone().unwrap_or_default();
        self.temp_font_italic = self.config.font_family_italic.clone().unwrap_or_default();
        self.temp_font_bold_italic = self
            .config
            .font_family_bold_italic
            .clone()
            .unwrap_or_default();
        self.font_pending_changes = false;
    }

    /// Sync ALL temp fields from config (used when resetting to defaults or importing)
    pub(crate) fn sync_all_temps_from_config(&mut self) {
        // Font temps
        self.sync_font_temps_from_config();

        // Shell/terminal temps
        self.temp_custom_shell = self.config.custom_shell.clone().unwrap_or_default();
        self.temp_shell_args = self
            .config
            .shell_args
            .as_ref()
            .map(|args| args.join(" "))
            .unwrap_or_default();
        self.temp_working_directory = self.config.working_directory.clone().unwrap_or_default();
        self.temp_startup_directory = self.config.startup_directory.clone().unwrap_or_default();
        self.temp_initial_text = self.config.initial_text.clone();

        // Background temps
        self.temp_background_image = self.config.background_image.clone().unwrap_or_default();
        self.temp_background_color = self.config.background_color;

        // Shader temps
        self.temp_custom_shader = self.config.custom_shader.clone().unwrap_or_default();
        self.temp_cursor_shader = self.config.cursor_shader.clone().unwrap_or_default();
        self.temp_shader_channel0 = self
            .config
            .custom_shader_channel0
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel1 = self
            .config
            .custom_shader_channel1
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel2 = self
            .config
            .custom_shader_channel2
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel3 = self
            .config
            .custom_shader_channel3
            .clone()
            .unwrap_or_default();
        self.temp_cubemap_path = self
            .config
            .custom_shader_cubemap
            .clone()
            .unwrap_or_default();

        // Update live opacity tracking
        self.last_live_opacity = self.config.window_opacity;
    }

    /// Reset all settings to their default values
    fn reset_all_to_defaults(&mut self) {
        self.config = Config::default();
        self.sync_all_temps_from_config();
        self.has_changes = true;
        self.search_query.clear();
    }

    /// Show the reset to defaults confirmation dialog
    fn show_reset_defaults_dialog_window(&mut self, ctx: &Context) {
        if !self.show_reset_defaults_dialog {
            return;
        }

        let mut close_dialog = false;
        let mut do_reset = false;

        egui::Window::new("Reset to Defaults")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("⚠ Warning")
                            .color(egui::Color32::YELLOW)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.label("This will reset ALL settings to their default values.");
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Unsaved changes will be lost. This cannot be undone.")
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(15.0);

                    ui.horizontal(|ui| {
                        // Reset button with danger styling
                        let reset_button = egui::Button::new(
                            egui::RichText::new("Reset").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(180, 50, 50));

                        if ui.add(reset_button).clicked() {
                            do_reset = true;
                            close_dialog = true;
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        if do_reset {
            self.reset_all_to_defaults();
        }

        if close_dialog {
            self.show_reset_defaults_dialog = false;
        }
    }

    /// Begin shader install asynchronously with optional force overwrite.
    fn start_shader_install(&mut self, force_overwrite: bool) {
        use std::sync::mpsc;

        if self.shader_installing {
            return;
        }

        self.shader_error = None;
        self.shader_status = Some(if force_overwrite {
            "Reinstalling shaders (overwriting modified files)...".to_string()
        } else {
            "Reinstalling shaders...".to_string()
        });
        self.shader_installing = true;

        let (tx, rx) = mpsc::channel();
        self.shader_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = crate::shader_installer::install_shaders_with_manifest(force_overwrite);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async shader install.
    pub(crate) fn poll_shader_install_status(&mut self) {
        if let Some(receiver) = &self.shader_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.shader_installing = false;
            self.shader_install_receiver = None;
            match result {
                Ok(res) => {
                    let detail = if res.skipped > 0 {
                        format!(
                            "Installed {} shaders ({} skipped, {} removed)",
                            res.installed, res.skipped, res.removed
                        )
                    } else {
                        format!(
                            "Installed {} shaders ({} removed)",
                            res.installed, res.removed
                        )
                    };
                    self.shader_status = Some(detail);
                    self.shader_error = None;
                    self.config.integration_versions.shaders_installed_version =
                        Some(env!("CARGO_PKG_VERSION").to_string());
                }
                Err(e) => {
                    self.shader_error = Some(e);
                    self.shader_status = None;
                }
            }
        }
    }

    /// Begin self-update asynchronously.
    pub(crate) fn start_self_update(&mut self, version: String) {
        use std::sync::mpsc;

        if self.update_installing {
            return;
        }

        self.update_status = Some("Downloading and installing update...".to_string());
        self.update_result = None;
        self.update_installing = true;

        let (tx, rx) = mpsc::channel();
        self.update_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = crate::self_updater::perform_update(&version);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async self-update.
    pub(crate) fn poll_update_install_status(&mut self) {
        if let Some(receiver) = &self.update_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.update_installing = false;
            self.update_install_receiver = None;
            match &result {
                Ok(res) => {
                    self.update_status = Some(format!(
                        "Update installed! Restart par-term to use v{}",
                        res.new_version
                    ));
                }
                Err(e) => {
                    self.update_status = Some(format!("Update failed: {}", e));
                }
            }
            self.update_result = Some(result);
        }
    }

    /// Apply font changes from temp variables to config
    pub(crate) fn apply_font_changes(&mut self) {
        self.config.font_family = self.temp_font_family.clone();
        self.config.font_size = self.temp_font_size;
        self.config.line_spacing = self.temp_line_spacing;
        self.config.char_spacing = self.temp_char_spacing;
        self.config.enable_text_shaping = self.temp_enable_text_shaping;
        self.config.enable_ligatures = self.temp_enable_ligatures;
        self.config.enable_kerning = self.temp_enable_kerning;
        self.config.font_family_bold = if self.temp_font_bold.is_empty() {
            None
        } else {
            Some(self.temp_font_bold.clone())
        };
        self.config.font_family_italic = if self.temp_font_italic.is_empty() {
            None
        } else {
            Some(self.temp_font_italic.clone())
        };
        self.config.font_family_bold_italic = if self.temp_font_bold_italic.is_empty() {
            None
        } else {
            Some(self.temp_font_bold_italic.clone())
        };
        self.font_pending_changes = false;
    }

    /// Toggle settings window visibility
    /// Note: This is kept for the overlay mode in WindowState, but the primary
    /// settings interface is now a separate window managed by WindowManager.
    #[allow(dead_code)]
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Get a reference to the working config (for live sync)
    #[allow(dead_code)]
    pub fn current_config(&self) -> &Config {
        &self.config
    }

    /// Sync the current collapsed sections state into the config's persisted field.
    fn sync_collapsed_sections_to_config(&mut self) {
        self.config.collapsed_settings_sections = self.collapsed_sections.iter().cloned().collect();
    }

    /// Get a snapshot of the current collapsed section IDs for persistence on close.
    pub fn collapsed_sections_snapshot(&self) -> Vec<String> {
        self.collapsed_sections.iter().cloned().collect()
    }

    /// Check if a test notification was requested and clear the flag
    pub fn take_test_notification_request(&mut self) -> bool {
        let requested = self.test_notification_requested;
        self.test_notification_requested = false;
        requested
    }

    /// Sync profiles from the main window's profile manager into the inline editor.
    pub fn sync_profiles(&mut self, profiles: Vec<Profile>) {
        self.profile_modal_ui.load_profiles(profiles);
    }

    /// Take profile save request: returns working profiles if save was requested.
    pub fn take_profile_save_request(&mut self) -> Option<Vec<Profile>> {
        if self.profile_save_requested {
            self.profile_save_requested = false;
            Some(self.profile_modal_ui.get_working_profiles().to_vec())
        } else {
            None
        }
    }

    /// Take profile open request: returns and clears the profile ID to open.
    pub fn take_profile_open_request(&mut self) -> Option<ProfileId> {
        self.profile_open_requested.take()
    }

    /// Show the settings window and return results
    /// - First Option: Some(config) if save was clicked (persist to disk)
    /// - Second Option: Some(config) if any changes were made (apply immediately)
    /// - Third Option: Some(ShaderEditorResult) if background shader Apply was clicked
    /// - Fourth Option: Some(CursorShaderEditorResult) if cursor shader Apply was clicked
    #[allow(dead_code)]
    pub fn show(
        &mut self,
        ctx: &Context,
    ) -> (
        Option<Config>,
        Option<Config>,
        Option<ShaderEditorResult>,
        Option<CursorShaderEditorResult>,
    ) {
        if !self.visible && !self.shader_editor_visible && !self.cursor_shader_editor_visible {
            return (None, None, None, None);
        }

        log::info!("SettingsUI.show() called - visible: true");

        // Handle Escape key to close settings window
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                // Close cursor shader editor first if open
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                // Close background shader editor first if open
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            } else if self.visible {
                // Close settings window
                self.visible = false;
                return (None, None, None, None);
            }
        }

        // Ensure settings panel is fully opaque regardless of terminal opacity
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut save_requested = false;
        let mut discard_requested = false;
        let mut close_requested = false;
        let mut open = true;
        let mut changes_this_frame = false;

        // Only show the main settings window if visible
        if self.visible {
            let settings_viewport = ctx.input(|i| i.viewport_rect());
            Window::new("Settings")
                .resizable(true)
                .default_width(650.0)
                .default_height(700.0)
                .default_pos(settings_viewport.center())
                .pivot(egui::Align2::CENTER_CENTER)
                .open(&mut open)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(solid_bg)
                        .stroke(egui::Stroke::NONE)
                        .shadow(Shadow {
                            offset: [0, 0],
                            blur: 0,
                            spread: 0,
                            color: Color32::TRANSPARENT,
                        }),
                )
                .show(ctx, |ui| {
                    // Reserve space for fixed footer buttons
                    let available_height = ui.available_height();
                    let footer_height = 45.0;

                    // Scrollable content area (takes remaining space above footer)
                    egui::ScrollArea::vertical()
                        .max_height(available_height - footer_height)
                        .show(ui, |ui| {
                            ui.heading("Terminal Settings");
                            ui.horizontal(|ui| {
                                ui.label("Quick search:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.search_query)
                                        .hint_text("Type to filter settings"),
                                );
                            });
                            ui.separator();

                            self.show_settings_sections(ui, &mut changes_this_frame);
                        });

                    // Fixed footer with action buttons (outside ScrollArea)
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save_requested = true;
                        }

                        if ui.button("Discard").clicked() {
                            discard_requested = true;
                        }

                        if ui.button("Close").clicked() {
                            close_requested = true;
                        }

                        ui.separator();

                        if ui
                            .button("Edit Config File")
                            .on_hover_text("Open config.yaml in your default editor")
                            .clicked()
                        {
                            let config_path = Config::config_path();
                            if let Err(e) = open::that(&config_path) {
                                log::error!("Failed to open config file: {}", e);
                            }
                        }

                        if ui
                            .button("Reset to Defaults")
                            .on_hover_text("Reset all settings to their default values")
                            .clicked()
                        {
                            self.show_reset_defaults_dialog = true;
                        }

                        if self.has_changes {
                            ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                        }
                    });
                });
        }

        // Show shader editor windows (delegated to separate modules)
        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        // Show shader dialogs
        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);

        // Show reset to defaults confirmation dialog
        self.show_reset_defaults_dialog_window(ctx);

        // Update visibility based on window state (only if settings window is being shown)
        if self.visible && (!open || close_requested) {
            self.visible = false;
        }

        // Handle save request
        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            // Sync collapsed sections to config before saving
            self.sync_collapsed_sections_to_config();
            // Generate keybindings for snippets and actions before saving
            let mut config = self.config.clone();
            config.generate_snippet_action_keybindings();
            Some(config)
        } else {
            None
        };

        // Handle discard request
        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

        // Push live config while the settings window is open to guarantee real-time updates.
        let config_for_live_update = if self.visible {
            // Only log when the value actually changes to avoid spam
            if (self.config.window_opacity - self.last_live_opacity).abs() > f32::EPSILON {
                log::info!(
                    "SettingsUI: live opacity {:.3} (last {:.3})",
                    self.config.window_opacity,
                    self.last_live_opacity
                );
                self.last_live_opacity = self.config.window_opacity;
            }
            Some(self.config.clone())
        } else {
            None
        };

        (
            config_to_save,
            config_for_live_update,
            shader_apply_result,
            cursor_shader_apply_result,
        )
    }

    /// Show the settings UI as a full-window panel (for standalone settings window)
    /// This renders directly to a CentralPanel instead of creating an egui::Window
    pub fn show_as_panel(
        &mut self,
        ctx: &Context,
    ) -> (
        Option<Config>,
        Option<Config>,
        Option<ShaderEditorResult>,
        Option<CursorShaderEditorResult>,
    ) {
        // Handle Escape key to close shader editors (not the window itself - that's handled by winit)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            }
        }

        // Ensure settings panel is fully opaque
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut save_requested = false;
        let mut discard_requested = false;
        let mut changes_this_frame = false;

        // Render directly to CentralPanel (no egui::Window wrapper)
        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).fill(solid_bg))
            .show(ctx, |ui| {
                // Reserve space for fixed footer buttons
                let available_height = ui.available_height();
                let footer_height = 45.0;

                // Scrollable content area (takes remaining space above footer)
                egui::ScrollArea::vertical()
                    .max_height(available_height - footer_height)
                    .show(ui, |ui| {
                        ui.heading("Terminal Settings");
                        ui.horizontal(|ui| {
                            ui.label("Quick search:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Type to filter settings"),
                            );
                        });
                        ui.separator();

                        self.show_settings_sections(ui, &mut changes_this_frame);
                    });

                // Fixed footer with action buttons (outside ScrollArea)
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save_requested = true;
                    }

                    if ui.button("Discard").clicked() {
                        discard_requested = true;
                    }

                    ui.separator();

                    if ui
                        .button("Edit Config File")
                        .on_hover_text("Open config.yaml in your default editor")
                        .clicked()
                    {
                        let config_path = Config::config_path();
                        if let Err(e) = open::that(&config_path) {
                            log::error!("Failed to open config file: {}", e);
                        }
                    }

                    if ui
                        .button("Reset to Defaults")
                        .on_hover_text("Reset all settings to their default values")
                        .clicked()
                    {
                        self.show_reset_defaults_dialog = true;
                    }

                    if self.has_changes {
                        ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                    }
                });
            });

        // Show shader editor windows (delegated to separate modules)
        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        // Show shader dialogs
        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);

        // Show reset to defaults confirmation dialog
        self.show_reset_defaults_dialog_window(ctx);

        // Handle save request
        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            // Sync collapsed sections to config before saving
            self.sync_collapsed_sections_to_config();
            // Generate keybindings for snippets and actions before saving
            let mut config = self.config.clone();
            config.generate_snippet_action_keybindings();
            Some(config)
        } else {
            None
        };

        // Handle discard request
        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

        // Push live config for real-time updates
        let config_for_live_update = {
            if (self.config.window_opacity - self.last_live_opacity).abs() > f32::EPSILON {
                log::info!(
                    "SettingsUI: live opacity {:.3} (last {:.3})",
                    self.config.window_opacity,
                    self.last_live_opacity
                );
                self.last_live_opacity = self.config.window_opacity;
            }
            Some(self.config.clone())
        };

        (
            config_to_save,
            config_for_live_update,
            shader_apply_result,
            cursor_shader_apply_result,
        )
    }

    /// Show all settings sections using the new sidebar + tab layout.
    fn show_settings_sections(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        // Quick settings strip at the top
        quick_settings::show(ui, self, changes_this_frame);
        ui.separator();

        // Get available dimensions for the main content area
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let sidebar_width = 150.0;
        let content_width = (available_width - sidebar_width - 15.0).max(300.0);

        // Main content area with sidebar and tab content
        // Use allocate_ui_with_layout to ensure the horizontal layout fills available height
        let layout = egui::Layout::left_to_right(egui::Align::Min);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, available_height),
            layout,
            |ui| {
                // Left sidebar for tab navigation (fixed width)
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        sidebar::show(ui, &mut self.selected_tab, &self.search_query);
                    },
                );

                ui.separator();

                // Right content area for selected tab (fills remaining space)
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_tab_content")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                ui.set_min_width(content_width - 20.0);
                                self.show_tab_content(ui, changes_this_frame);
                            });
                    },
                );
            },
        );
    }

    /// Show the content for the currently selected tab.
    fn show_tab_content(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        // Take collapsed_sections out temporarily to avoid borrow conflicts.
        // Tab functions need &mut self for config changes AND &mut collapsed for tracking,
        // which would conflict if collapsed_sections were still inside self.
        let mut collapsed = std::mem::take(&mut self.collapsed_sections);

        match self.selected_tab {
            SettingsTab::Appearance => {
                appearance_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Window => {
                window_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Input => {
                input_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Terminal => {
                terminal_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Effects => {
                effects_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Badge => {
                badge_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::ProgressBar => {
                progress_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::StatusBar => {
                status_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Profiles => {
                profiles_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Ssh => {
                self.show_ssh_tab(ui, changes_this_frame);
            }
            SettingsTab::Notifications => {
                notifications_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Integrations => {
                self.show_integrations_tab(ui, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Automation => {
                automation_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Scripts => {
                scripts_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Snippets => {
                snippets_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Actions => {
                actions_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Arrangements => {
                arrangements_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Advanced => {
                advanced_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
        }

        // Restore collapsed_sections
        self.collapsed_sections = collapsed;
    }

    /// Check if a keybinding conflicts with existing keybindings.
    ///
    /// Returns Some(conflict_description) if there's a conflict, None otherwise.
    pub(crate) fn check_keybinding_conflict(
        &self,
        key: &str,
        exclude_id: Option<&str>,
    ) -> Option<String> {
        // Check against existing keybindings in config
        for binding in &self.config.keybindings {
            if binding.key == key {
                return Some(format!("Already bound to: {}", binding.action));
            }
        }

        // Check against snippets with keybindings (exclude the current snippet being edited)
        for snippet in &self.config.snippets {
            if let Some(snippet_key) = &snippet.keybinding
                && snippet_key == key
            {
                // Skip if this is the snippet being edited
                if exclude_id == Some(&snippet.id) {
                    continue;
                }
                return Some(format!("Already bound to snippet: {}", snippet.title));
            }
        }

        None
    }
}
