//! Config persistence, path resolution, and session-state methods for `Config`.
//!
//! Covers:
//! - `load` / `save` (YAML file I/O with atomic write)
//! - XDG-compliant path helpers (`config_path`, `config_dir`, `state_file_path`, etc.)
//! - Session-state persistence (`save_last_working_directory`, `load_last_working_directory`)
//! - Startup-directory resolution (`get_effective_startup_directory`)
//! - Miscellaneous runtime helpers (`resolve_tmux_path`, `logs_dir`, `with_title`,
//!   `get_pane_background`, `should_prompt_shell_integration`, `should_prompt_integrations`)

use super::config_struct::Config;
use crate::types::{BackgroundImageMode, InstallPromptState, StartupDirectoryMode};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

impl Config {
    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        log::info!("Config path: {:?}", config_path);

        if config_path.exists() {
            // Validate that the config file has not been redirected (e.g. via a
            // symlink) to a location outside the expected config directory.
            let config_dir = Self::config_dir();
            if let Err(e) = Self::validate_config_path(&config_path, &config_dir) {
                log::error!("Config path validation failed: {e}");
                return Err(e.into());
            }

            log::info!("Loading existing config from {:?}", config_path);

            // Security: warn if the config file is readable by group or others.
            // The config file may contain sensitive values (API keys, SSH paths,
            // trigger commands) that should not be exposed to other users on a
            // shared system.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&config_path) {
                    let mode = metadata.permissions().mode();
                    // Check group-readable (0o040) or world-readable (0o004) bits.
                    if mode & 0o044 != 0 {
                        log::warn!(
                            "Config file {:?} has insecure permissions (mode {:04o}). \
                             It is readable by group or others, which may expose sensitive \
                             configuration values. Run: chmod 600 {:?}",
                            config_path,
                            mode & 0o777,
                            config_path,
                        );
                    }
                }
            }

            let contents = fs::read_to_string(&config_path)?;

            // Pre-scan the raw YAML for `allow_all_env_vars: true` before
            // variable substitution, since the config isn't parsed yet.
            let allow_all = super::env_vars::pre_scan_allow_all_env_vars(&contents);
            let contents =
                super::env_vars::substitute_variables_with_allowlist(&contents, allow_all);
            let mut config: Config = serde_yaml_ng::from_str(&contents)?;

            // Warn about triggers with require_user_action: false, since the
            // denylist is the only protection in that mode and it is bypassable.
            config.warn_insecure_triggers();

            // Merge in any new default keybindings that don't exist in user's config
            config.merge_default_keybindings();

            // Merge in any new default status bar widgets that don't exist in user's config
            config.merge_default_widgets();

            // Generate keybindings for snippets and actions
            config.generate_snippet_action_keybindings();

            // Load last working directory from state file (for "previous session" mode)
            config.load_last_working_directory();

            Ok(config)
        } else {
            log::info!(
                "Config file not found, creating default at {:?}",
                config_path
            );
            // Create default config and save it
            let mut config = Self::default();
            // Generate keybindings for snippets and actions
            config.generate_snippet_action_keybindings();
            if let Err(e) = config.save() {
                log::error!("Failed to save default config: {}", e);
                return Err(e);
            }

            // Load last working directory from state file (for "previous session" mode)
            config.load_last_working_directory();

            log::info!("Default config created successfully");
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml_ng::to_string(self)?;

        // Atomic save: write to temp file then rename to prevent corruption on crash
        let temp_path = config_path.with_extension("yaml.tmp");
        fs::write(&temp_path, &yaml)?;
        fs::rename(&temp_path, &config_path)?;

        Ok(())
    }

    /// Get the configuration file path (using XDG convention)
    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term").join("config.yaml")
            } else {
                PathBuf::from("config.yaml")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Use XDG convention on all platforms: ~/.config/par-term/config.yaml
            if let Some(home_dir) = dirs::home_dir() {
                home_dir
                    .join(".config")
                    .join("par-term")
                    .join("config.yaml")
            } else {
                // Fallback if home directory cannot be determined
                PathBuf::from("config.yaml")
            }
        }
    }

    /// Get the configuration directory path (using XDG convention)
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term")
            } else {
                PathBuf::from(".")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(".config").join("par-term")
            } else {
                PathBuf::from(".")
            }
        }
    }

    /// Get the shell integration directory (same as config dir)
    pub fn shell_integration_dir() -> PathBuf {
        Self::config_dir()
    }

    /// Get the session logs directory path, resolving ~ if present
    /// Creates the directory if it doesn't exist
    pub fn logs_dir(&self) -> PathBuf {
        let path = if self.session_log_directory.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&self.session_log_directory[2..])
            } else {
                PathBuf::from(&self.session_log_directory)
            }
        } else {
            PathBuf::from(&self.session_log_directory)
        };

        // Create directory if it doesn't exist
        if !path.exists()
            && let Err(e) = std::fs::create_dir_all(&path)
        {
            log::warn!("Failed to create logs directory {:?}: {}", path, e);
        }

        path
    }

    /// Resolve the tmux executable path at runtime.
    /// If the configured path is absolute and exists, use it.
    /// If it's "tmux" (the default), search PATH and common installation locations.
    /// This handles cases where PATH may be incomplete (e.g., app launched from Finder).
    pub fn resolve_tmux_path(&self) -> String {
        let configured = &self.tmux_path;

        // If it's an absolute path and exists, use it directly
        if configured.starts_with('/') && std::path::Path::new(configured).exists() {
            return configured.clone();
        }

        // If it's not just "tmux", return it and let the OS try
        if configured != "tmux" {
            return configured.clone();
        }

        // Search for tmux in PATH
        if let Ok(path_env) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            let executable = if cfg!(windows) { "tmux.exe" } else { "tmux" };

            for dir in path_env.split(separator) {
                let candidate = std::path::Path::new(dir).join(executable);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
            }
        }

        // Fall back to common paths for environments where PATH might be incomplete
        #[cfg(target_os = "macos")]
        {
            let macos_paths = [
                "/opt/homebrew/bin/tmux", // Homebrew on Apple Silicon
                "/usr/local/bin/tmux",    // Homebrew on Intel / MacPorts
            ];
            for path in macos_paths {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let linux_paths = [
                "/usr/bin/tmux",       // Most distros
                "/usr/local/bin/tmux", // Manual install
                "/snap/bin/tmux",      // Snap package
            ];
            for path in linux_paths {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }
        }

        // Final fallback - return configured value
        configured.clone()
    }

    /// Set the window title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Check if shell integration should be prompted
    ///
    /// # Arguments
    /// * `current_version` - The application version (from root crate's `VERSION` constant)
    pub fn should_prompt_shell_integration(&self, current_version: &str) -> bool {
        if self.shell_integration_state != InstallPromptState::Ask {
            return false;
        }

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shell_integration_prompted_version
            && prompted == current_version
        {
            return false;
        }

        // Check if installed and up to date
        if let Some(ref installed) = self
            .integration_versions
            .shell_integration_installed_version
            && installed == current_version
        {
            return false;
        }

        true
    }

    /// Check if either integration should be prompted
    ///
    /// # Arguments
    /// * `current_version` - The application version (from root crate's `VERSION` constant)
    pub fn should_prompt_integrations(&self, current_version: &str) -> bool {
        self.should_prompt_shader_install_versioned(current_version)
            || self.should_prompt_shell_integration(current_version)
    }

    /// Get the effective startup directory based on configuration mode.
    ///
    /// Priority:
    /// 1. Legacy `working_directory` if set (backward compatibility)
    /// 2. Based on `startup_directory_mode`:
    ///    - Home: Returns user's home directory
    ///    - Previous: Returns `last_working_directory` if valid, else home
    ///    - Custom: Returns `startup_directory` if set and valid, else home
    ///
    /// Returns None if the effective directory doesn't exist (caller should fall back to default).
    pub fn get_effective_startup_directory(&self) -> Option<String> {
        // Legacy working_directory takes precedence for backward compatibility
        if let Some(ref wd) = self.working_directory {
            let expanded = Self::expand_home_dir(wd);
            if std::path::Path::new(&expanded).exists() {
                return Some(expanded);
            }
            log::warn!(
                "Configured working_directory '{}' does not exist, using default",
                wd
            );
        }

        match self.startup_directory_mode {
            StartupDirectoryMode::Home => {
                // Return home directory
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
            StartupDirectoryMode::Previous => {
                // Return last working directory if it exists
                if let Some(ref last_dir) = self.last_working_directory {
                    let expanded = Self::expand_home_dir(last_dir);
                    if std::path::Path::new(&expanded).exists() {
                        return Some(expanded);
                    }
                    log::warn!(
                        "Previous session directory '{}' no longer exists, using home",
                        last_dir
                    );
                }
                // Fall back to home
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
            StartupDirectoryMode::Custom => {
                // Return custom directory if set and exists
                if let Some(ref custom_dir) = self.startup_directory {
                    let expanded = Self::expand_home_dir(custom_dir);
                    if std::path::Path::new(&expanded).exists() {
                        return Some(expanded);
                    }
                    log::warn!(
                        "Custom startup directory '{}' does not exist, using home",
                        custom_dir
                    );
                }
                // Fall back to home
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
        }
    }

    /// Expand ~ to home directory in a path string
    fn expand_home_dir(path: &str) -> String {
        if let Some(suffix) = path.strip_prefix("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(suffix).to_string_lossy().to_string();
        }
        path.to_string()
    }

    /// Get the state file path for storing session state (like last working directory)
    pub fn state_file_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("par-term").join("state.yaml")
            } else {
                PathBuf::from("state.yaml")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir
                    .join(".local")
                    .join("share")
                    .join("par-term")
                    .join("state.yaml")
            } else {
                PathBuf::from("state.yaml")
            }
        }
    }

    /// Save the last working directory to state file
    pub fn save_last_working_directory(&mut self, directory: &str) -> Result<()> {
        self.last_working_directory = Some(directory.to_string());

        // Save to state file for persistence across sessions
        let state_path = Self::state_file_path();
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create a minimal state struct for persistence
        #[derive(Serialize)]
        struct SessionState {
            last_working_directory: Option<String>,
        }

        let state = SessionState {
            last_working_directory: Some(directory.to_string()),
        };

        let yaml = serde_yaml_ng::to_string(&state)?;

        // Atomic save: write to temp file then rename to prevent corruption on crash
        let temp_path = state_path.with_extension("yaml.tmp");
        fs::write(&temp_path, &yaml)?;
        fs::rename(&temp_path, &state_path)?;

        log::debug!(
            "Saved last working directory to {:?}: {}",
            state_path,
            directory
        );
        Ok(())
    }

    /// Load the last working directory from state file
    pub fn load_last_working_directory(&mut self) {
        let state_path = Self::state_file_path();
        if !state_path.exists() {
            return;
        }

        #[derive(Deserialize)]
        struct SessionState {
            last_working_directory: Option<String>,
        }

        match fs::read_to_string(&state_path) {
            Ok(contents) => {
                if let Ok(state) = serde_yaml_ng::from_str::<SessionState>(&contents)
                    && let Some(dir) = state.last_working_directory
                {
                    log::debug!("Loaded last working directory from state file: {}", dir);
                    self.last_working_directory = Some(dir);
                }
            }
            Err(e) => {
                log::warn!("Failed to read state file {:?}: {}", state_path, e);
            }
        }
    }

    /// Get per-pane background config for a given pane index, if configured
    /// Returns (image_path, mode, opacity, darken) tuple for easy conversion to runtime type
    pub fn get_pane_background(
        &self,
        index: usize,
    ) -> Option<(String, BackgroundImageMode, f32, f32)> {
        self.pane_backgrounds
            .iter()
            .find(|pb| pb.index == index)
            .map(|pb| (pb.image.clone(), pb.mode, pb.opacity, pb.darken))
    }

    /// Emit security warnings for any triggers configured with
    /// `require_user_action: false` that also contain dangerous actions
    /// (`RunCommand` or `SendText`).
    ///
    /// Called during config load so that users are immediately informed when
    /// their configuration reduces the security posture. The warning is written
    /// to stderr via [`crate::automation::warn_require_user_action_false`].
    pub(crate) fn warn_insecure_triggers(&self) {
        for trigger in &self.triggers {
            if !trigger.require_user_action && trigger.actions.iter().any(|a| a.is_dangerous()) {
                crate::automation::warn_require_user_action_false(&trigger.name);
            }
        }
    }
}
