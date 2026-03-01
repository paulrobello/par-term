//! Settings UI for par-term terminal emulator.
//!
//! This crate provides an egui-based settings interface for configuring
//! terminal options at runtime. It is designed to be decoupled from the
//! main terminal implementation through trait interfaces.

use par_term_config::{Config, Profile, ProfileId};

/// Callback type for detecting modified bundled shaders.
pub type ShaderDetectModifiedFn = fn() -> Result<Vec<String>, String>;

// Trait interfaces for decoupling from main crate
mod traits;
pub use traits::*;

// Window arrangements (pure data types)
pub mod arrangements;
pub use arrangements::{
    ArrangementId, ArrangementManager, MonitorInfo, TabSnapshot, WindowArrangement, WindowSnapshot,
};

// Shell detection utility
pub mod shell_detection;

// Profile management modal UI
pub mod profile_modal_ui;
pub use profile_modal_ui::{ProfileModalAction, ProfileModalUI};

// Nerd Font integration (font loading + icon presets)
pub mod nerd_font;

// Reorganized settings tabs
pub mod actions_tab;
pub mod advanced_tab;
pub mod ai_inspector_tab;
pub mod appearance_tab;
pub mod arrangements_tab;
pub mod automation_tab;
pub mod badge_tab;
pub mod effects_tab;
pub mod input_tab;
pub mod integrations_tab;
pub mod notifications_tab;
pub mod prettifier_tab;
pub mod profiles_tab;
pub mod progress_bar_tab;
pub mod quick_settings;
pub mod scripts_tab;
pub mod search_keywords;
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

// SettingsUI struct and impl
mod settings_ui;
pub use settings_ui::SettingsUI;

pub use sidebar::SettingsTab;

// Re-export types that settings consumers need
pub use par_term_config::{
    self as config, BackgroundImageMode as BgMode, CursorShaderMetadataCache as CursorShaderCache,
    ProfileManager, ProfileSource, ShaderMetadataCache as ShaderCache, Theme, VsyncMode,
};

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

/// Result of processing a settings window event.
///
/// This enum bridges the settings UI crate with the main application.
/// The main application processes these actions after events are handled
/// by the settings window.
#[derive(Debug, Clone)]
pub enum SettingsWindowAction {
    /// No action needed
    None,
    /// Close the settings window
    Close,
    /// Apply config changes to terminal windows (live update)
    ApplyConfig(Config),
    /// Save config to disk
    SaveConfig(Config),
    /// Apply background shader from editor
    ApplyShader(ShaderEditorResult),
    /// Apply cursor shader from editor
    ApplyCursorShader(CursorShaderEditorResult),
    /// Send a test notification to verify permissions
    TestNotification,
    /// Save profiles from inline editor to all windows
    SaveProfiles(Vec<Profile>),
    /// Open a profile in the focused terminal window
    OpenProfile(ProfileId),
    /// Start a coprocess by config index on the active tab
    StartCoprocess(usize),
    /// Stop a coprocess by config index on the active tab
    StopCoprocess(usize),
    /// Start a script by config index on the active tab
    StartScript(usize),
    /// Stop a script by config index on the active tab
    StopScript(usize),
    /// Open the debug log file in the system's default editor/viewer
    OpenLogFile,
    /// Save the current window layout as an arrangement
    SaveArrangement(String),
    /// Restore a saved window arrangement
    RestoreArrangement(ArrangementId),
    /// Delete a saved window arrangement
    DeleteArrangement(ArrangementId),
    /// Rename a saved window arrangement
    RenameArrangement(ArrangementId, String),
    /// Move a saved window arrangement one position up in the list
    MoveArrangementUp(ArrangementId),
    /// Move a saved window arrangement one position down in the list
    MoveArrangementDown(ArrangementId),
    /// User requested an immediate update check
    ForceUpdateCheck,
    /// User requested to install the available update
    InstallUpdate(String),
    /// Flash pane indices on the terminal window
    IdentifyPanes,
    /// Install shell integration for the detected shell
    InstallShellIntegration,
    /// Uninstall shell integration from all shells
    UninstallShellIntegration,
}

/// Lightweight information about a saved arrangement (used by trait interface).
#[derive(Debug, Clone)]
pub struct ArrangementInfo {
    /// Unique identifier
    pub id: ArrangementId,
    /// Display name
    pub name: String,
    /// Number of windows in the arrangement
    pub window_count: usize,
}

/// Result of a shader installation operation.
#[derive(Debug, Clone)]
pub struct ShaderInstallResult {
    /// Number of shaders installed
    pub installed: usize,
    /// Number of shaders skipped (unchanged)
    pub skipped: usize,
    /// Number of obsolete shaders removed
    pub removed: usize,
}

/// Result of a shader uninstallation operation.
#[derive(Debug, Clone)]
pub struct ShaderUninstallResult {
    /// Number of shaders removed
    pub removed: usize,
    /// Number of modified shaders kept
    pub kept: usize,
    /// Whether confirmation is needed for modified files
    pub needs_confirmation: bool,
}

/// Result of shell integration installation.
#[derive(Debug, Clone)]
pub struct ShellIntegrationInstallResult {
    /// Shell type that was configured
    pub shell: String,
    /// Path to the integration script
    pub script_path: String,
    /// RC file that was modified
    pub rc_file: String,
    /// Whether a shell restart is needed
    pub needs_restart: bool,
}

/// Result of shell integration uninstallation.
#[derive(Debug, Clone)]
pub struct ShellIntegrationUninstallResult {
    /// Whether rc files were cleaned
    pub cleaned: bool,
    /// Whether manual intervention is needed
    pub needs_manual: bool,
    /// Number of integration scripts removed
    pub scripts_removed: usize,
}

/// Result of a self-update operation.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    /// Old version before the update
    pub old_version: String,
    /// New version after the update
    pub new_version: String,
    /// Path where the binary was installed
    pub install_path: String,
    /// Whether a restart is needed
    pub needs_restart: bool,
}

/// Installation type detected for the running binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationType {
    /// Installed via Homebrew
    Homebrew,
    /// Installed via `cargo install`
    CargoInstall,
    /// Running from a macOS .app bundle
    MacOSBundle,
    /// Standalone binary
    StandaloneBinary,
}

/// Result of an update check.
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// No update available, up to date
    UpToDate,
    /// An update is available
    UpdateAvailable(UpdateCheckInfo),
    /// Update checking is disabled
    Disabled,
    /// Check was skipped (cooldown not elapsed)
    Skipped,
    /// Error occurred during check
    Error(String),
}

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateCheckInfo {
    /// Version string (e.g., "0.16.0")
    pub version: String,
    /// Release notes/changelog
    pub release_notes: Option<String>,
    /// URL to release page
    pub release_url: String,
    /// When the release was published
    pub published_at: Option<String>,
}

/// Format a timestamp string for display in the UI.
pub fn format_timestamp(timestamp: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => timestamp.to_string(),
    }
}

/// Get the path to the debug log file.
pub fn log_path() -> std::path::PathBuf {
    #[cfg(unix)]
    {
        std::path::PathBuf::from("/tmp/par_term_debug.log")
    }
    #[cfg(windows)]
    {
        std::env::temp_dir().join("par_term_debug.log")
    }
}

/// Create a configured HTTP agent for URL fetching.
pub fn http_agent() -> ureq::Agent {
    ureq::Agent::new_with_defaults()
}
