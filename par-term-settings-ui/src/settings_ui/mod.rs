//! SettingsUI struct and implementation.
//!
//! This module contains the main SettingsUI manager struct and all its
//! methods for displaying and managing the settings window.

use par_term_config::{
    Config, CursorShaderMetadataCache, ProfileId, ShaderControlParseResult, ShaderMetadataCache,
};
use std::collections::{HashMap, HashSet};

use crate::actions_tab::ActionsTabState;
use crate::advanced_tab::AdvancedTabState;
use crate::ai_inspector_tab::AiInspectorTabState;
use crate::arrangements_tab::ArrangementsTabState;
use crate::automation_tab::AutomationTabState;
use crate::background_tab::BackgroundTabState;
use crate::profile_modal_ui::ProfileModalUI;
use crate::profiles_tab::ProfilesTabState;
use crate::scripts_tab::ScriptsTabState;
use crate::sidebar::SettingsTab;
use crate::snippets_tab::SnippetsTabState;
use crate::{
    ArrangementManager, InstallationType, SettingsWindowAction, ShaderDetectModifiedFn,
    ShaderInstallResult, ShaderLintFn, ShaderUninstallResult, UpdateCheckResult, UpdateResult,
};

/// Settings UI manager using egui
///
/// # ARC-002 — decomposition in progress (2026-07-30)
///
/// This struct held 221 flat fields. Per-tab form state is being moved into
/// `<tab>/state.rs` structs; nine groups (111 fields) have moved so far. Count
/// what is left, and what has moved, rather than trusting a written-down number:
///
/// ```text
/// awk '/^pub struct SettingsUI/,/^}/' par-term-settings-ui/src/settings_ui/mod.rs \
///   | grep -cE '^    (pub(\(crate\))? )?[a-z][a-z0-9_]*:'
/// grep -chE '^    pub [a-z][a-z0-9_]*:' par-term-settings-ui/src/*_tab/state.rs \
///   | awk '{s+=$1} END {print s}'
/// ```
///
/// A group whose fields are not all their types' defaults writes `Default` out
/// by hand rather than deriving it; see `grouped_tab_state_keeps_its_non_default_seeds`.
///
/// Two rules constrain what may still move:
///
/// - `has_changes` and the per-frame change flag are cross-cutting and stay
///   here. A tab-state struct cannot reach them, and losing an assignment
///   compiles fine while silently dropping the edit.
/// - Fields the **root crate** reads (runtime coprocess/script status, the
///   pending-action queues, the installed callbacks, `config`, `visible`) stay
///   here too; moving one is a breaking change to a published crate.
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

    /// Background tab per-pane image editor state
    pub background_tab: BackgroundTabState,

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

    /// AI Inspector tab prompt-library state
    pub ai_inspector_tab: AiInspectorTabState,

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
    /// Cache for parsed shader controls and warnings
    pub shader_controls_cache: HashMap<String, ShaderControlParseResult>,
    /// Cache for parsed cursor shader metadata
    pub cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Latest shader lint/readability output for the selected background shader
    pub shader_lint_result: Option<String>,
    /// Latest shader lint/readability error for the selected background shader
    pub shader_lint_error: Option<String>,
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

    /// Automation tab form state
    pub automation_tab: AutomationTabState,
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
    /// Scripts tab form state
    pub scripts_tab: ScriptsTabState,
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

    /// Snippets tab form state
    pub snippets_tab: SnippetsTabState,

    /// Actions tab form state
    pub actions_tab: ActionsTabState,

    /// Profiles tab dynamic-source editor state
    pub profiles_tab: ProfilesTabState,

    /// Advanced tab import/export state
    pub advanced_tab: AdvancedTabState,

    // Reset to defaults dialog state
    /// Whether to show the reset to defaults confirmation dialog
    pub show_reset_defaults_dialog: bool,

    /// Arrangements tab form state
    pub arrangements_tab: ArrangementsTabState,
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

    /// Callback: run shader lint/readability analysis
    pub shader_lint_fn: Option<ShaderLintFn>,

    /// Callback: check if shader files exist
    pub shader_has_files_fn: Option<fn(&std::path::Path) -> bool>,

    /// Callback: count shader files
    pub shader_count_files_fn: Option<fn(&std::path::Path) -> usize>,

    /// Callback: check if shell integration is installed
    pub shell_integration_is_installed_fn: Option<fn() -> bool>,

    /// Callback: get detected shell type
    pub shell_integration_detected_shell_fn: Option<fn() -> par_term_config::ShellType>,
}

mod async_ops;
mod display;
mod sections;
mod state;
