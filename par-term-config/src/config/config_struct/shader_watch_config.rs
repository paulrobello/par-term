//! Shader hot-reload settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Automatic reloading of background and cursor shader files on change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderWatchConfig {
    /// Enable automatic shader reloading when shader files are modified
    /// This watches custom_shader and cursor_shader files for changes
    #[serde(default = "crate::defaults::bool_false")]
    pub shader_hot_reload: bool,

    /// Debounce delay in milliseconds before reloading shader after file change
    /// Helps avoid multiple reloads during rapid saves from editors
    #[serde(default = "crate::defaults::shader_hot_reload_delay")]
    pub shader_hot_reload_delay: u64,
}

impl Default for ShaderWatchConfig {
    fn default() -> Self {
        Self {
            shader_hot_reload: crate::defaults::bool_false(),
            shader_hot_reload_delay: crate::defaults::shader_hot_reload_delay(),
        }
    }
}
