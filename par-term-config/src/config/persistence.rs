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
    ///
    /// When no config file exists, a default one is written to disk and
    /// returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file resolves outside the config
    /// directory (a redirected symlink), if it cannot be read, if it is not
    /// valid YAML for the [`Config`] schema, or — on the first-run path — if
    /// writing the default config fails.
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

            // SEC-005: Emit a startup warning when allow_all_env_vars: true is detected.
            // This setting allows any environment variable (including secrets) to be
            // substituted into config values, which can expose sensitive data if a
            // shared or imported config file uses ${SECRET_VAR} references.
            if allow_all {
                eprintln!(
                    "[par-term SECURITY WARNING] Config option `allow_all_env_vars: true` is set.\n\
                     This allows ALL environment variables to be interpolated into config values,\n\
                     including sensitive variables such as API keys, tokens, and passwords.\n\
                     A shared or imported config with ${{SENSITIVE_VAR}} references could expose\n\
                     your secrets. Only use this setting in a non-shared, local-only config.\n\
                     Recommendation: use a CLAUDE.local.md-style local override, or remove\n\
                     `allow_all_env_vars: true` and add needed variables to the allowlist instead."
                );
            }

            let contents =
                super::env_vars::substitute_variables_with_allowlist(&contents, allow_all);
            let mut config: Config = serde_yaml_ng::from_str(&contents)?;

            // Migrate legacy values that may be stored in user configs.
            config.migrate_legacy_values();

            // Warn about triggers with prompt_before_run: false, since the
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
    ///
    /// SEC-008/SEC-021: written through [`crate::atomic_save`], which stages the
    /// write in a sibling `0600` temp file, **fsyncs it**, renames it over the
    /// real config and then fsyncs the directory. This used to be an inline
    /// temp-and-rename with no fsync, which still lost the file on a power cut:
    /// the rename can reach disk before the data it points at.
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created, if the
    /// config cannot be serialized to YAML, or if writing, syncing or renaming
    /// the temp file fails.
    pub fn save(&self) -> Result<()> {
        crate::atomic_save::save_yaml_atomic(&Self::config_path(), self)
    }

    /// Get the configuration file path.
    ///
    /// Always [`Config::config_dir`] plus `config.yaml`. These two used to resolve
    /// the directory independently, which meant any change to one silently
    /// desynchronised the other.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.yaml")
    }

    /// Get the configuration directory path.
    ///
    /// On Unix this honours `XDG_CONFIG_HOME`, falling back to `~/.config/par-term`.
    /// On Windows it is `%APPDATA%\par-term`; the XDG variables are a
    /// freedesktop convention and are deliberately not consulted there.
    ///
    /// Only `XDG_CONFIG_HOME` is read. `XDG_DATA_HOME`, `XDG_STATE_HOME`,
    /// `XDG_CACHE_HOME` and `XDG_RUNTIME_DIR` are **not** honoured — par-term keeps
    /// all of its state under the config directory. See ENH-008.
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir()
                .map(|dir| dir.join("par-term"))
                .unwrap_or_else(|| PathBuf::from("."))
        }
        #[cfg(not(target_os = "windows"))]
        {
            unix_config_dir(std::env::var_os("XDG_CONFIG_HOME"), dirs::home_dir())
        }
    }

    /// Get the shell integration directory (same as config dir)
    pub fn shell_integration_dir() -> PathBuf {
        Self::config_dir()
    }

    /// Resolve the session logs directory path, expanding a leading `~/`.
    ///
    /// QA-029: this is a pure path resolver and does **not** create the
    /// directory. It used to `create_dir_all` as a side effect, which meant the
    /// settings window created a directory just by rendering — including a
    /// directory per keystroke while the user typed a new path into the
    /// "Log directory" field, and at the process umask rather than the `0o700`
    /// that SEC-010 requires of a directory holding session logs.
    ///
    /// The two callers that actually write logs (`Tab::toggle_session_logging`
    /// and the auto-log path in `Tab::new`) already create the directory
    /// themselves and then chmod it to `0o700`, so nothing is lost by removing
    /// the side effect here.
    pub fn logs_dir(&self) -> PathBuf {
        match self.session_log.session_log_directory.strip_prefix("~/") {
            Some(rest) => match dirs::home_dir() {
                Some(home) => home.join(rest),
                None => PathBuf::from(&self.session_log.session_log_directory),
            },
            None => PathBuf::from(&self.session_log.session_log_directory),
        }
    }

    /// Resolve the tmux executable path at runtime.
    /// If the configured path is absolute and exists, use it.
    /// If it's "tmux" (the default), search PATH and common installation locations.
    /// This handles cases where PATH may be incomplete (e.g., app launched from Finder).
    pub fn resolve_tmux_path(&self) -> String {
        let configured = &self.tmux.tmux_path;

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
        if self.integrations.shell_integration_state != InstallPromptState::Ask {
            return false;
        }

        // Check if already prompted for this version
        if let Some(ref prompted) = self
            .integrations
            .integration_versions
            .shell_integration_prompted_version
            && prompted == current_version
        {
            return false;
        }

        // Check if installed and up to date
        if let Some(ref installed) = self
            .integrations
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
        if let Some(ref wd) = self.shell.working_directory {
            let expanded = Self::expand_home_dir(wd);
            if std::path::Path::new(&expanded).exists() {
                return Some(expanded);
            }
            log::warn!(
                "Configured working_directory '{}' does not exist, using default",
                wd
            );
        }

        match self.shell.startup_directory_mode {
            StartupDirectoryMode::Home => {
                // Return home directory
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
            StartupDirectoryMode::Previous => {
                // Return last working directory if it exists
                if let Some(ref last_dir) = self.shell.last_working_directory {
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
                if let Some(ref custom_dir) = self.shell.startup_directory {
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
    ///
    /// Updates `self.last_working_directory` and persists it to `state.yaml`,
    /// so the value survives across sessions.
    ///
    /// SEC-021: written through [`crate::atomic_save`] at mode `0o600`. The
    /// previous inline temp-and-rename had neither an fsync nor a mode, so the
    /// path the user was last working in was world-readable.
    ///
    /// # Errors
    ///
    /// Returns an error if the state directory cannot be created, if the state
    /// cannot be serialized to YAML, or if writing, syncing or renaming the
    /// temp file fails. The in-memory field is updated before any of these can
    /// fail.
    pub fn save_last_working_directory(&mut self, directory: &str) -> Result<()> {
        self.shell.last_working_directory = Some(directory.to_string());

        let state_path = Self::state_file_path();

        // Create a minimal state struct for persistence
        #[derive(Serialize)]
        struct SessionState {
            last_working_directory: Option<String>,
        }

        let state = SessionState {
            last_working_directory: Some(directory.to_string()),
        };

        crate::atomic_save::save_yaml_atomic(&state_path, &state)?;

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
                    self.shell.last_working_directory = Some(dir);
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
        self.image
            .pane_backgrounds
            .iter()
            .find(|pb| pb.index == index)
            .map(|pb| (pb.image.clone(), pb.mode, pb.opacity, pb.darken))
    }

    /// Migrate legacy config values that may be stored in older user config files.
    ///
    /// - `minimum_contrast == 1.0` was the old default; map it to 0.0 (disabled).
    /// - `minimum_contrast` is clamped to 0.99 max so 1.0 is never an active value.
    pub(crate) fn migrate_legacy_values(&mut self) {
        if (self.font_rendering.minimum_contrast - 1.0_f32).abs() < f32::EPSILON {
            log::info!("minimum_contrast was 1.0 (legacy default), resetting to 0.0 (disabled)");
            self.font_rendering.minimum_contrast = 0.0;
        } else {
            self.font_rendering.minimum_contrast = self.font_rendering.minimum_contrast.min(0.99);
        }
    }

    /// Collect and emit security warnings for any triggers configured with
    /// `prompt_before_run: false` that also contain dangerous actions
    /// (`RunCommand`, `SendText`, or `SplitPane`).
    ///
    /// Called during config load so that users are immediately informed when
    /// their configuration reduces the security posture. In addition to
    /// writing a prominent warning to stderr, the insecure trigger names are
    /// stored in [`Config::insecure_trigger_names`] so the UI layer can
    /// render a persistent visual warning banner.
    ///
    /// Triggers with `prompt_before_run: false` that do not also set
    /// `i_accept_the_risk: true` are recorded in
    /// [`Config::unaccepted_risk_trigger_names`] and will be blocked at
    /// execution time.
    pub(crate) fn warn_insecure_triggers(&mut self) {
        self.insecure_trigger_names.clear();
        self.unaccepted_risk_trigger_names.clear();
        for trigger in &self.automation.triggers {
            if !trigger.prompt_before_run && trigger.actions.iter().any(|a| a.is_dangerous()) {
                crate::automation::warn_prompt_before_run_false(
                    &trigger.name,
                    trigger.i_accept_the_risk,
                );
                self.insecure_trigger_names.push(trigger.name.clone());
                if !trigger.i_accept_the_risk {
                    self.unaccepted_risk_trigger_names
                        .push(trigger.name.clone());
                }
            }
        }
    }
}

/// Resolve the Unix configuration directory from the XDG variable and home dir.
///
/// Split out of [`Config::config_dir`] so it can be tested without touching the
/// process environment: `std::env::set_var` is not thread-safe — glibc's `setenv`
/// can reallocate and free `environ` under a concurrent `getenv` — so tests pass
/// the values in instead.
#[cfg(not(target_os = "windows"))]
fn unix_config_dir(xdg_config_home: Option<std::ffi::OsString>, home: Option<PathBuf>) -> PathBuf {
    // The XDG spec requires these variables to hold an absolute path and says an
    // implementation "should consider the path invalid and ignore it" otherwise,
    // which also covers the empty-string case.
    if let Some(xdg) = xdg_config_home {
        let path = PathBuf::from(xdg);
        if path.is_absolute() {
            return path.join("par-term");
        }
    }
    match home {
        Some(home) => home.join(".config").join("par-term"),
        None => PathBuf::from("."),
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod xdg_tests {
    use super::unix_config_dir;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn home() -> Option<PathBuf> {
        Some(PathBuf::from("/home/u"))
    }

    #[test]
    fn absolute_xdg_config_home_wins() {
        assert_eq!(
            unix_config_dir(Some(OsString::from("/dotfiles/cfg")), home()),
            PathBuf::from("/dotfiles/cfg/par-term")
        );
    }

    #[test]
    fn unset_falls_back_to_dot_config() {
        assert_eq!(
            unix_config_dir(None, home()),
            PathBuf::from("/home/u/.config/par-term")
        );
    }

    /// The spec requires an absolute path; relative and empty values are invalid
    /// and must not be honoured, or a stray `XDG_CONFIG_HOME=.` would scatter
    /// config into whatever directory par-term happened to start in.
    #[test]
    fn relative_or_empty_xdg_is_ignored() {
        for bad in ["", "relative/path", "."] {
            assert_eq!(
                unix_config_dir(Some(OsString::from(bad)), home()),
                PathBuf::from("/home/u/.config/par-term"),
                "expected {bad:?} to be rejected as non-absolute"
            );
        }
    }

    #[test]
    fn no_home_and_no_xdg_falls_back_to_cwd() {
        assert_eq!(unix_config_dir(None, None), PathBuf::from("."));
    }
}
