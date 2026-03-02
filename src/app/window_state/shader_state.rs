//! Shader hot-reload and metadata-cache state for a window.
//!
//! Groups the fields that track the shader file-watcher, the two metadata
//! caches (background and cursor shader), and the last reload error so they
//! can be reasoned about independently from the rest of `WindowState`.

use crate::config::{CursorShaderMetadataCache, ShaderMetadataCache};
use crate::shader_watcher::ShaderWatcher;

/// Shader file-watcher, metadata caches, and reload-error state.
pub(crate) struct ShaderState {
    /// Shader file watcher for hot reload support
    pub(crate) shader_watcher: Option<ShaderWatcher>,
    /// Cache for parsed background-shader metadata (used for config resolution)
    pub(crate) shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor-shader metadata (used for config resolution)
    pub(crate) cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Last shader reload error message (for display in UI)
    pub(crate) shader_reload_error: Option<String>,
    /// Background shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) background_shader_reload_result: Option<Option<String>>,
    /// Cursor shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) cursor_shader_reload_result: Option<Option<String>>,
}

impl ShaderState {
    /// Create with pre-built metadata caches (requires the shaders directory path).
    pub(crate) fn new(shaders_dir: std::path::PathBuf) -> Self {
        Self {
            shader_watcher: None,
            shader_metadata_cache: ShaderMetadataCache::with_shaders_dir(shaders_dir.clone()),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(shaders_dir),
            shader_reload_error: None,
            background_shader_reload_result: None,
            cursor_shader_reload_result: None,
        }
    }
}
