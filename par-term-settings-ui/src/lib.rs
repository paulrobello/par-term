//! Settings UI for par-term terminal emulator.
//!
//! This crate provides an egui-based settings interface for configuring
//! terminal options. It is designed to be decoupled from the main terminal
//! implementation through trait interfaces.
//!
//! # Architecture
//!
//! The settings UI is separated from the main terminal through the following traits:
//!
//! - [`ProfileOperations`]: Profile management operations
//! - [`ArrangementOperations`]: Window arrangement save/restore
//! - [`UpdateOperations`]: Self-update checking and installation
//! - [`CoprocessOperations`]: Coprocess start/stop control
//! - [`ScriptOperations`]: Script start/stop control
//!
//! # Example
//!
//! ```ignore
//! use par_term_settings_ui::{SettingsUI, SettingsContext};
//!
//! // Create settings context with implementations
//! let context = SettingsContext {
//!     profiles: MyProfileOps,
//!     arrangements: MyArrangementOps,
//!     // ... other implementations
//! };
//!
//! // Create settings UI
//! let mut settings = SettingsUI::new(config, context);
//!
//! // Show in egui
//! let result = settings.show(ctx);
//! ```

// Public types and traits
mod traits;
pub use traits::*;

// Re-export types that settings consumers need
pub use par_term_config::{
    Config, Profile, ProfileId, ProfileManager, ProfileSource,
    BackgroundImageMode, CursorShaderMetadataCache, ShaderMetadataCache,
    Theme, VsyncMode,
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

/// Actions that can be triggered from the settings UI.
///
/// The main application handles these actions after the settings UI
/// produces them. This allows the settings UI to remain decoupled
/// from terminal-specific implementations.
#[derive(Debug, Clone)]
pub enum SettingsAction {
    /// No action needed
    None,
    /// Close the settings window
    Close,
    /// Apply config changes to terminal windows (live update)
    ApplyConfig(par_term_config::Config),
    /// Save config to disk
    SaveConfig(par_term_config::Config),
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
    SaveArrangement {
        /// Name for the arrangement
        name: String,
    },
    /// Restore a saved window arrangement
    RestoreArrangement {
        /// ID of the arrangement to restore
        id: uuid::Uuid,
    },
    /// Delete a saved window arrangement
    DeleteArrangement {
        /// ID of the arrangement to delete
        id: uuid::Uuid,
    },
    /// Rename a saved window arrangement
    RenameArrangement {
        /// ID of the arrangement to rename
        id: uuid::Uuid,
        /// New name for the arrangement
        new_name: String,
    },
    /// User requested an immediate update check
    ForceUpdateCheck,
    /// User requested to install the available update
    InstallUpdate {
        /// Version to install
        version: String,
    },
    /// Flash pane indices on the terminal window
    IdentifyPanes,
}

/// Information about an available update
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Version string (e.g., "v1.2.3")
    pub version: String,
    /// Release notes or changelog summary
    pub release_notes: Option<String>,
}

/// Result of an update check
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// No update available
    UpToDate,
    /// Update available
    UpdateAvailable(UpdateInfo),
    /// Error checking for updates
    Error(String),
}

/// Status of a coprocess or script
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Process is not running
    Stopped,
    /// Process is running
    Running,
    /// Process failed
    Failed,
}

/// Information about a saved window arrangement
#[derive(Debug, Clone)]
pub struct ArrangementInfo {
    /// Unique identifier
    pub id: uuid::Uuid,
    /// Display name
    pub name: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Context containing all trait implementations needed by the settings UI.
///
/// This struct is passed to the settings UI to provide access to
/// terminal-specific operations without creating tight coupling.
pub struct SettingsContext<
    P: ProfileOps,
    A: ArrangementOps,
    U: UpdateOps,
    C: CoprocessOps,
    S: ScriptOps,
> {
    /// Profile management operations
    pub profiles: P,
    /// Window arrangement operations
    pub arrangements: A,
    /// Update checking/installation operations
    pub updates: U,
    /// Coprocess management operations
    pub coprocesses: C,
    /// Script management operations
    pub scripts: S,
}

// Note: The full SettingsUI implementation would be added here.
// For now, this provides the trait interfaces that the main crate
// must implement to use this settings UI crate.
