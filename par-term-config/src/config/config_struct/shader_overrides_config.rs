//! Per-shader configuration overrides.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{CursorShaderConfig, ShaderConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-file overrides for background and cursor shader settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShaderOverridesConfig {
    /// Per-shader configuration overrides (key = shader filename)
    /// These override settings embedded in shader metadata and global defaults
    #[serde(default)]
    pub shader_configs: HashMap<String, ShaderConfig>,

    /// Per-cursor-shader configuration overrides (key = shader filename)
    #[serde(default)]
    pub cursor_shader_configs: HashMap<String, CursorShaderConfig>,
}
