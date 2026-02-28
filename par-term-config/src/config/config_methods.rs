//! Method implementations for the `Config` struct.
//!
//! Covers loading, saving, path resolution, theme/tab-style application,
//! keybinding management, shader helpers, and session-state persistence.

use super::config_struct::Config;
use crate::error::ConfigError;
use crate::themes::Theme;
use crate::types::{
    BackgroundImageMode, CursorShaderConfig, InstallPromptState, ShaderConfig, ShaderInstallPrompt,
    StartupDirectoryMode, TabStyle,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

impl Config {
    /// Validate that `path` (which must already exist on disk) resolves — via
    /// `canonicalize` — to a location inside `expected_base`.
    ///
    /// Uses `std::fs::canonicalize` so symlinks are fully resolved before the
    /// containment check.  Returns the canonical path on success, or a
    /// [`ConfigError::PathTraversal`] error if the resolved path escapes the
    /// expected directory.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::PathTraversal` when the canonical path does not
    /// start with the canonical `expected_base`.
    /// Returns `ConfigError::Io` if either path cannot be canonicalized.
    pub fn validate_config_path(path: &Path, expected_base: &Path) -> Result<PathBuf, ConfigError> {
        let canonical = fs::canonicalize(path).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("cannot canonicalize {}: {e}", path.display()),
            )
        })?;

        let canonical_base = fs::canonicalize(expected_base).unwrap_or_else(|_| {
            // If the base doesn't exist yet (first run), use the un-resolved path.
            expected_base.to_path_buf()
        });

        if !canonical.starts_with(&canonical_base) {
            return Err(ConfigError::PathTraversal(format!(
                "path '{}' resolves to '{}' which is outside the expected directory '{}'",
                path.display(),
                canonical.display(),
                canonical_base.display(),
            )));
        }

        Ok(canonical)
    }

    /// Lexically check that a **relative** shader name does not contain `..`
    /// components that would escape the shaders directory.
    ///
    /// This is a compile-time / pre-existence check used by [`Self::shader_path`]
    /// and [`Self::checked_shader_path`] before the file is opened.  Because the
    /// shader file might not exist yet (e.g., when the user is composing its name
    /// in the settings UI), we cannot call `canonicalize` here.
    ///
    /// Returns `Ok(())` when safe, `Err(ConfigError::PathTraversal)` when the
    /// name contains a parent-directory component.
    fn validate_shader_name(shader_name: &str) -> Result<(), ConfigError> {
        use std::path::Component;

        let path = Path::new(shader_name);

        // Reject any component that steps upward.
        for component in path.components() {
            if component == Component::ParentDir {
                return Err(ConfigError::PathTraversal(format!(
                    "shader name '{shader_name}' contains a parent-directory component ('..') \
                     which would escape the shaders directory",
                )));
            }
        }

        Ok(())
    }

    /// Apply tab style preset, overwriting the tab bar color/size fields.
    ///
    /// This is called when the user changes `tab_style` in settings.
    /// The `Dark` style corresponds to the existing defaults and does nothing.
    pub fn apply_tab_style(&mut self) {
        match self.tab_style {
            TabStyle::Dark => {
                // Default dark theme - restore original defaults
                self.tab_bar_background = crate::defaults::tab_bar_background();
                self.tab_active_background = crate::defaults::tab_active_background();
                self.tab_inactive_background = crate::defaults::tab_inactive_background();
                self.tab_hover_background = crate::defaults::tab_hover_background();
                self.tab_active_text = crate::defaults::tab_active_text();
                self.tab_inactive_text = crate::defaults::tab_inactive_text();
                self.tab_active_indicator = crate::defaults::tab_active_indicator();
                self.tab_border_color = crate::defaults::tab_border_color();
                self.tab_border_width = crate::defaults::tab_border_width();
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Light => {
                self.tab_bar_background = [235, 235, 235];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [225, 225, 225];
                self.tab_hover_background = [240, 240, 240];
                self.tab_active_text = [30, 30, 30];
                self.tab_inactive_text = [100, 100, 100];
                self.tab_active_indicator = [50, 120, 220];
                self.tab_border_color = [200, 200, 200];
                self.tab_border_width = 1.0;
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Compact => {
                // Smaller tabs, tighter spacing
                self.tab_bar_background = [35, 35, 35];
                self.tab_active_background = [55, 55, 55];
                self.tab_inactive_background = [35, 35, 35];
                self.tab_hover_background = [45, 45, 45];
                self.tab_active_text = [240, 240, 240];
                self.tab_inactive_text = [160, 160, 160];
                self.tab_active_indicator = [80, 140, 240];
                self.tab_border_color = [60, 60, 60];
                self.tab_border_width = 0.5;
                self.tab_bar_height = 22.0;
            }
            TabStyle::Minimal => {
                // Very clean, flat look with minimal decoration
                self.tab_bar_background = [30, 30, 30];
                self.tab_active_background = [30, 30, 30];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [40, 40, 40];
                self.tab_active_text = [255, 255, 255];
                self.tab_inactive_text = [120, 120, 120];
                self.tab_active_indicator = [100, 150, 255];
                self.tab_border_color = [30, 30, 30]; // No visible border
                self.tab_border_width = 0.0;
                self.tab_bar_height = 26.0;
            }
            TabStyle::HighContrast => {
                // Maximum contrast for accessibility
                self.tab_bar_background = [0, 0, 0];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [60, 60, 60];
                self.tab_active_text = [0, 0, 0];
                self.tab_inactive_text = [255, 255, 255];
                self.tab_active_indicator = [255, 255, 0];
                self.tab_border_color = [255, 255, 255];
                self.tab_border_width = 2.0;
                self.tab_bar_height = 30.0;
            }
            TabStyle::Automatic => {
                // No-op here: actual style is resolved and applied by apply_system_tab_style()
            }
        }
    }

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
            let mut config: Config = serde_yml::from_str(&contents)?;

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

    /// Merge default keybindings into the user's config.
    /// Only adds keybindings for actions that don't already exist in the user's config.
    /// This ensures new features with default keybindings are available to existing users.
    fn merge_default_keybindings(&mut self) {
        let default_keybindings = crate::defaults::keybindings();

        // Get the set of actions already configured by the user (owned strings to avoid borrow issues)
        let existing_actions: std::collections::HashSet<String> = self
            .keybindings
            .iter()
            .map(|kb| kb.action.clone())
            .collect();

        // Add any default keybindings whose actions are not already configured
        let mut added_count = 0;
        for default_kb in default_keybindings {
            if !existing_actions.contains(&default_kb.action) {
                log::info!(
                    "Adding new default keybinding: {} -> {}",
                    default_kb.key,
                    default_kb.action
                );
                self.keybindings.push(default_kb);
                added_count += 1;
            }
        }

        if added_count > 0 {
            log::info!(
                "Merged {} new default keybinding(s) into user config",
                added_count
            );
        }
    }

    /// Merge default status bar widgets into the user's config.
    /// Only adds widgets whose `WidgetId` doesn't already exist in the user's widget list.
    /// This ensures new built-in widgets are available to existing users.
    fn merge_default_widgets(&mut self) {
        let default_widgets = crate::status_bar::default_widgets();

        let existing_ids: std::collections::HashSet<crate::status_bar::WidgetId> = self
            .status_bar_widgets
            .iter()
            .map(|w| w.id.clone())
            .collect();

        let mut added_count = 0;
        for default_widget in default_widgets {
            if !existing_ids.contains(&default_widget.id) {
                log::info!(
                    "Adding new default status bar widget: {:?}",
                    default_widget.id
                );
                self.status_bar_widgets.push(default_widget);
                added_count += 1;
            }
        }

        if added_count > 0 {
            log::info!(
                "Merged {} new default status bar widget(s) into user config",
                added_count
            );
        }
    }

    /// Generate keybindings for snippets and actions that have keybindings configured.
    ///
    /// This method adds or updates keybindings for snippets and actions in the keybindings list,
    /// using the format "snippet:<id>" for snippets and "action:<id>" for actions.
    /// If a keybinding for a snippet/action already exists, it will be updated with the new key.
    pub fn generate_snippet_action_keybindings(&mut self) {
        use crate::config::KeyBinding;

        // Track actions we've seen to remove stale keybindings later
        let mut seen_actions = std::collections::HashSet::new();
        let mut added_count = 0;
        let mut updated_count = 0;

        // Generate keybindings for snippets
        for snippet in &self.snippets {
            if let Some(key) = &snippet.keybinding {
                let action = format!("snippet:{}", snippet.id);
                seen_actions.insert(action.clone());

                if !key.is_empty() && snippet.enabled && snippet.keybinding_enabled {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != *key {
                            log::info!(
                                "Updating keybinding for snippet '{}': {} -> {} (was: {})",
                                snippet.title,
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.clone();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for snippet '{}': {} -> {} (enabled={}, keybinding_enabled={})",
                            snippet.title,
                            key,
                            action,
                            snippet.enabled,
                            snippet.keybinding_enabled
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.clone(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for snippet '{}': {} (enabled={}, keybinding_enabled={})",
                        snippet.title,
                        key,
                        snippet.enabled,
                        snippet.keybinding_enabled
                    );
                }
            }
        }

        // Generate keybindings for actions
        for action_config in &self.actions {
            if let Some(key) = action_config.keybinding() {
                let action = format!("action:{}", action_config.id());
                seen_actions.insert(action.clone());

                if !key.is_empty() && action_config.keybinding_enabled() {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != key {
                            log::info!(
                                "Updating keybinding for action '{}': {} -> {} (was: {})",
                                action_config.title(),
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.to_string();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for action '{}': {} -> {} (keybinding_enabled={})",
                            action_config.title(),
                            key,
                            action,
                            action_config.keybinding_enabled()
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.to_string(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for action '{}': {} (keybinding_enabled={})",
                        action_config.title(),
                        key,
                        action_config.keybinding_enabled()
                    );
                }
            }
        }

        // Remove stale keybindings for snippets that no longer have keybindings or are disabled
        let original_len = self.keybindings.len();
        self.keybindings.retain(|kb| {
            // Keep if it's not a snippet/action keybinding
            if !kb.action.starts_with("snippet:") && !kb.action.starts_with("action:") {
                return true;
            }
            // Keep if we saw it during our scan
            seen_actions.contains(&kb.action)
        });
        let removed_count = original_len - self.keybindings.len();

        if added_count > 0 || updated_count > 0 || removed_count > 0 {
            log::info!(
                "Snippet/Action keybindings: {} added, {} updated, {} removed",
                added_count,
                updated_count,
                removed_count
            );
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yml::to_string(self)?;

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

    /// Get the shaders directory path (using XDG convention)
    pub fn shaders_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term").join("shaders")
            } else {
                PathBuf::from("shaders")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(".config").join("par-term").join("shaders")
            } else {
                PathBuf::from("shaders")
            }
        }
    }

    /// Get the full path to a shader file.
    ///
    /// If `shader_name` is an absolute path it is returned as-is (the user
    /// explicitly chose a location outside the shaders directory).
    ///
    /// For relative names the path is resolved under [`Self::shaders_dir`].
    /// Any relative name that contains `..` components is rejected: the
    /// function logs a warning and falls back to returning the shaders directory
    /// itself so that callers always receive a valid `PathBuf` without breaking
    /// existing call sites.  Use [`Self::checked_shader_path`] when you need a
    /// hard error instead of a fallback.
    pub fn shader_path(shader_name: &str) -> PathBuf {
        let path = PathBuf::from(shader_name);
        if path.is_absolute() {
            return path;
        }

        // Lexical traversal check for relative names.
        if let Err(e) = Self::validate_shader_name(shader_name) {
            log::warn!("{e} — falling back to shaders directory");
            return Self::shaders_dir();
        }

        Self::shaders_dir().join(shader_name)
    }

    /// Get the full path to a shader file, returning an error if the name
    /// would escape the shaders directory.
    ///
    /// This is a strict variant of [`Self::shader_path`] for callers that
    /// prefer a hard error over a silent fallback.
    pub fn checked_shader_path(shader_name: &str) -> Result<PathBuf, ConfigError> {
        let path = PathBuf::from(shader_name);
        if path.is_absolute() {
            return Ok(path);
        }

        Self::validate_shader_name(shader_name)?;
        Ok(Self::shaders_dir().join(shader_name))
    }

    /// Resolve a texture path, expanding ~ to home directory
    /// and resolving relative paths relative to the shaders directory.
    /// Returns the expanded path or the original if expansion fails
    pub fn resolve_texture_path(path: &str) -> PathBuf {
        if path.starts_with("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(&path[2..]);
        }
        let path_buf = PathBuf::from(path);
        if path_buf.is_absolute() {
            path_buf
        } else {
            Self::shaders_dir().join(path)
        }
    }

    /// Get the channel texture paths as an array of Options
    /// Returns [channel0, channel1, channel2, channel3] for iChannel0-3
    pub fn shader_channel_paths(&self) -> [Option<PathBuf>; 4] {
        [
            self.custom_shader_channel0
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel1
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel2
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel3
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
        ]
    }

    /// Get the cubemap path prefix (resolved)
    /// Returns None if not configured, otherwise the resolved path prefix
    pub fn shader_cubemap_path(&self) -> Option<PathBuf> {
        self.custom_shader_cubemap
            .as_ref()
            .map(|p| Self::resolve_texture_path(p))
    }

    /// Set the window title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Load theme configuration
    pub fn load_theme(&self) -> Theme {
        Theme::by_name(&self.theme).unwrap_or_default()
    }

    /// Apply system theme if auto_dark_mode is enabled.
    /// Returns true if the theme was changed.
    pub fn apply_system_theme(&mut self, is_dark: bool) -> bool {
        if !self.auto_dark_mode {
            return false;
        }
        let new_theme = if is_dark {
            &self.dark_theme
        } else {
            &self.light_theme
        };
        if self.theme != *new_theme {
            self.theme = new_theme.clone();
            true
        } else {
            false
        }
    }

    /// Apply tab style based on system theme when tab_style is Automatic.
    /// Returns true if the style was applied.
    pub fn apply_system_tab_style(&mut self, is_dark: bool) -> bool {
        if self.tab_style != TabStyle::Automatic {
            return false;
        }
        let target = if is_dark {
            self.dark_tab_style
        } else {
            self.light_tab_style
        };
        // Temporarily set to concrete style, apply colors, then restore Automatic
        self.tab_style = target;
        self.apply_tab_style();
        self.tab_style = TabStyle::Automatic;
        true
    }

    /// Get the user override config for a specific shader (if any)
    pub fn get_shader_override(&self, shader_name: &str) -> Option<&ShaderConfig> {
        self.shader_configs.get(shader_name)
    }

    /// Get the user override config for a specific cursor shader (if any)
    pub fn get_cursor_shader_override(&self, shader_name: &str) -> Option<&CursorShaderConfig> {
        self.cursor_shader_configs.get(shader_name)
    }

    /// Get or create a mutable reference to a shader's config override
    pub fn get_or_create_shader_override(&mut self, shader_name: &str) -> &mut ShaderConfig {
        self.shader_configs
            .entry(shader_name.to_string())
            .or_default()
    }

    /// Get or create a mutable reference to a cursor shader's config override
    pub fn get_or_create_cursor_shader_override(
        &mut self,
        shader_name: &str,
    ) -> &mut CursorShaderConfig {
        self.cursor_shader_configs
            .entry(shader_name.to_string())
            .or_default()
    }

    /// Remove a shader config override (revert to defaults)
    pub fn remove_shader_override(&mut self, shader_name: &str) {
        self.shader_configs.remove(shader_name);
    }

    /// Remove a cursor shader config override (revert to defaults)
    pub fn remove_cursor_shader_override(&mut self, shader_name: &str) {
        self.cursor_shader_configs.remove(shader_name);
    }

    /// Check if the shaders folder is missing or empty
    /// Returns true if user should be prompted to install shaders
    pub fn should_prompt_shader_install(&self) -> bool {
        // Only prompt if the preference is set to "ask"
        if self.shader_install_prompt != ShaderInstallPrompt::Ask {
            return false;
        }

        let shaders_dir = Self::shaders_dir();

        // Check if directory doesn't exist
        if !shaders_dir.exists() {
            return true;
        }

        // Check if directory is empty or has no .glsl files
        if let Ok(entries) = std::fs::read_dir(&shaders_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension()
                    && ext == "glsl"
                {
                    return false; // Found at least one shader
                }
            }
        }

        true // Directory exists but has no .glsl files
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

    /// Check if shaders should be prompted (version-aware logic)
    ///
    /// # Arguments
    /// * `current_version` - The application version (from root crate's `VERSION` constant)
    pub fn should_prompt_shader_install_versioned(&self, current_version: &str) -> bool {
        if self.shader_install_prompt != ShaderInstallPrompt::Ask {
            return false;
        }

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shaders_prompted_version
            && prompted == current_version
        {
            return false;
        }

        // Check if installed and up to date
        if let Some(ref installed) = self.integration_versions.shaders_installed_version
            && installed == current_version
        {
            return false;
        }

        // Also check if shaders folder exists and has files
        let shaders_dir = Self::shaders_dir();
        !shaders_dir.exists() || !Self::has_shader_files(&shaders_dir)
    }

    /// Check if a directory contains shader files (.glsl)
    fn has_shader_files(dir: &PathBuf) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension()
                    && ext == "glsl"
                {
                    return true;
                }
            }
        }
        false
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

        let yaml = serde_yml::to_string(&state)?;

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

    /// Load the last working directory from state file
    /// Emit security warnings for any triggers configured with
    /// `require_user_action: false` that also contain dangerous actions
    /// (`RunCommand` or `SendText`).
    ///
    /// Called during config load so that users are immediately informed when
    /// their configuration reduces the security posture. The warning is written
    /// to stderr via [`crate::automation::warn_require_user_action_false`].
    fn warn_insecure_triggers(&self) {
        for trigger in &self.triggers {
            if !trigger.require_user_action && trigger.actions.iter().any(|a| a.is_dangerous()) {
                crate::automation::warn_require_user_action_false(&trigger.name);
            }
        }
    }

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
                if let Ok(state) = serde_yml::from_str::<SessionState>(&contents)
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
}
