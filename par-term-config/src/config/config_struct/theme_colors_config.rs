//! Colour theme selection settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Terminal colour theme, plus the light/dark pair used by auto dark mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColorsConfig {
    /// Color theme name to use for terminal colors
    #[serde(default = "crate::defaults::theme")]
    pub theme: String,

    /// Automatically switch theme based on system light/dark mode
    #[serde(default)]
    pub auto_dark_mode: bool,

    /// Theme to use when system is in light mode (used when auto_dark_mode is true)
    #[serde(default = "crate::defaults::light_theme")]
    pub light_theme: String,

    /// Theme to use when system is in dark mode (used when auto_dark_mode is true)
    #[serde(default = "crate::defaults::dark_theme")]
    pub dark_theme: String,
}

impl Default for ThemeColorsConfig {
    fn default() -> Self {
        Self {
            theme: crate::defaults::theme(),
            auto_dark_mode: false,
            light_theme: crate::defaults::light_theme(),
            dark_theme: crate::defaults::dark_theme(),
        }
    }
}
