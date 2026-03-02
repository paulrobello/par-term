//! Path validation and shader-path helpers for `Config`.
//!
//! Covers:
//! - Traversal-safe path validation (`validate_config_path`, `validate_shader_name`)
//! - Shader directory / file path resolution (`shaders_dir`, `shader_path`, etc.)
//! - Shader-channel and cubemap accessors
//! - Shader config overrides
//! - Shader install-prompt helpers

use super::config_struct::Config;
use crate::error::ConfigError;
use crate::types::{CursorShaderConfig, ShaderConfig, ShaderInstallPrompt};
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
            self.shader
                .custom_shader_channel0
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.shader
                .custom_shader_channel1
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.shader
                .custom_shader_channel2
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.shader
                .custom_shader_channel3
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
        ]
    }

    /// Get the cubemap path prefix (resolved)
    /// Returns None if not configured, otherwise the resolved path prefix
    pub fn shader_cubemap_path(&self) -> Option<PathBuf> {
        self.shader
            .custom_shader_cubemap
            .as_ref()
            .map(|p| Self::resolve_texture_path(p))
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
}
