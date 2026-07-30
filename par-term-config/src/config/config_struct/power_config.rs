//! Focus/blur power saving settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Shader pausing and reduced frame rates for unfocused windows and inactive tabs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    /// Pause shader animations when window loses focus
    /// This reduces GPU usage when the terminal is not actively being viewed
    #[serde(default = "crate::defaults::bool_true")]
    pub pause_shaders_on_blur: bool,

    /// Reduce refresh rate when window is not focused
    /// When true, uses unfocused_fps instead of max_fps when window is blurred
    #[serde(default = "crate::defaults::bool_true")]
    pub pause_refresh_on_blur: bool,

    /// Target FPS when window is not focused (only used if pause_refresh_on_blur is true)
    /// Lower values save more power but may delay terminal output visibility
    #[serde(default = "crate::defaults::unfocused_fps")]
    pub unfocused_fps: u32,

    /// Target FPS for inactive (non-visible) tabs
    /// Inactive tabs only need enough polling to detect activity, bells, and shell exit.
    /// Lower values significantly reduce CPU from mutex contention with many tabs open.
    /// Default: 2 (500ms interval)
    #[serde(default = "crate::defaults::inactive_tab_fps")]
    pub inactive_tab_fps: u32,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            pause_shaders_on_blur: crate::defaults::bool_true(),
            pause_refresh_on_blur: crate::defaults::bool_true(),
            unfocused_fps: crate::defaults::unfocused_fps(),
            inactive_tab_fps: crate::defaults::inactive_tab_fps(),
        }
    }
}
