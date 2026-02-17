//! Trait definitions for settings UI dependencies.
//!
//! These traits define the interface between the settings UI crate
//! and the main terminal implementation. The main crate implements
//! these traits to provide concrete functionality.

use crate::{ArrangementInfo, UpdateCheckResult};
use par_term_config::{Profile, ProfileId};

/// Profile management operations.
///
/// Implemented by the main crate to provide profile management
/// functionality to the settings UI.
pub trait ProfileOps {
    /// Get all profiles in display order
    fn get_profiles(&self) -> Vec<Profile>;

    /// Save profiles to persistent storage
    fn save_profiles(&mut self, profiles: Vec<Profile>) -> anyhow::Result<()>;

    /// Get a profile by ID
    fn get_profile(&self, id: &ProfileId) -> Option<Profile>;

    /// Add or update a profile
    fn upsert_profile(&mut self, profile: Profile);

    /// Delete a profile by ID
    fn delete_profile(&mut self, id: &ProfileId) -> bool;

    /// Get available shell options
    fn get_available_shells(&self) -> Vec<String>;
}

/// Window arrangement operations.
///
/// Implemented by the main crate to provide window layout
/// save/restore functionality.
pub trait ArrangementOps {
    /// Get all saved arrangements
    fn get_arrangements(&self) -> Vec<ArrangementInfo>;

    /// Save current window layout as a new arrangement
    fn save_arrangement(&mut self, name: &str) -> anyhow::Result<uuid::Uuid>;

    /// Restore a saved arrangement
    fn restore_arrangement(&mut self, id: uuid::Uuid) -> anyhow::Result<()>;

    /// Delete a saved arrangement
    fn delete_arrangement(&mut self, id: uuid::Uuid) -> anyhow::Result<()>;

    /// Rename a saved arrangement
    fn rename_arrangement(&mut self, id: uuid::Uuid, new_name: &str) -> anyhow::Result<()>;

    /// Check if an arrangement with the given name exists
    fn arrangement_exists(&self, name: &str) -> bool;

    /// Find arrangement by name
    fn find_arrangement_by_name(&self, name: &str) -> Option<ArrangementInfo>;
}

/// Update checking and installation operations.
///
/// Implemented by the main crate to provide self-update
/// functionality.
pub trait UpdateOps {
    /// Check for updates (may be async)
    fn check_for_updates(&self) -> UpdateCheckResult;

    /// Get the last update check result
    fn last_check_result(&self) -> Option<UpdateCheckResult>;

    /// Install an available update
    fn install_update(&mut self, version: &str) -> anyhow::Result<()>;

    /// Get update installation status
    fn is_installing(&self) -> bool;

    /// Get installation progress message
    fn installation_status(&self) -> Option<String>;
}

/// Coprocess management operations.
///
/// Implemented by the main crate to provide coprocess
/// control functionality.
pub trait CoprocessOps {
    /// Get the number of configured coprocesses
    fn coprocess_count(&self) -> usize;

    /// Get the running status of a coprocess
    fn is_running(&self, index: usize) -> bool;

    /// Get the last error from a coprocess
    fn get_error(&self, index: usize) -> Option<String>;

    /// Get buffered output from a coprocess
    fn get_output(&self, index: usize) -> Vec<String>;

    /// Start a coprocess
    fn start(&mut self, index: usize) -> anyhow::Result<()>;

    /// Stop a coprocess
    fn stop(&mut self, index: usize) -> anyhow::Result<()>;

    /// Clear output buffer for a coprocess
    fn clear_output(&mut self, index: usize);
}

/// Script management operations.
///
/// Implemented by the main crate to provide script
/// control functionality.
pub trait ScriptOps {
    /// Get the number of configured scripts
    fn script_count(&self) -> usize;

    /// Get the running status of a script
    fn is_running(&self, index: usize) -> bool;

    /// Get the last error from a script
    fn get_error(&self, index: usize) -> Option<String>;

    /// Get buffered output from a script
    fn get_output(&self, index: usize) -> Vec<String>;

    /// Start a script
    fn start(&mut self, index: usize) -> anyhow::Result<()>;

    /// Stop a script
    fn stop(&mut self, index: usize) -> anyhow::Result<()>;

    /// Clear output buffer for a script
    fn clear_output(&mut self, index: usize);

    /// Get panel state for a script (title, content)
    fn get_panel(&self, index: usize) -> Option<(String, String)>;
}

/// Shader management operations.
///
/// Implemented by the main crate to provide shader
/// installation functionality.
pub trait ShaderOps {
    /// Install bundled shaders
    fn install_shaders(&mut self, force_overwrite: bool) -> anyhow::Result<InstallResult>;

    /// Get available shader files
    fn get_available_shaders(&self) -> Vec<String>;

    /// Get available cubemap prefixes
    fn get_available_cubemaps(&self) -> Vec<String>;

    /// Get shader directory path
    fn shaders_dir(&self) -> std::path::PathBuf;
}

/// Result of a shader installation operation.
#[derive(Debug, Clone)]
pub struct InstallResult {
    /// Number of shaders installed
    pub installed: usize,
    /// Number of shaders skipped (unchanged)
    pub skipped: usize,
    /// Number of obsolete shaders removed
    pub removed: usize,
}

/// Shell integration operations.
///
/// Implemented by the main crate to provide shell integration
/// installation functionality.
pub trait ShellIntegrationOps {
    /// Install shell integration for the given shell
    fn install_integration(&mut self, shell: par_term_config::ShellType) -> anyhow::Result<()>;

    /// Uninstall shell integration for the given shell
    fn uninstall_integration(&mut self, shell: par_term_config::ShellType) -> anyhow::Result<()>;

    /// Check if shell integration is installed
    fn is_integration_installed(&self, shell: par_term_config::ShellType) -> bool;

    /// Get the path to the shell integration script
    fn integration_script_path(&self, shell: par_term_config::ShellType) -> std::path::PathBuf;
}
