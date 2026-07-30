//! Badge overlay settings (iTerm2-style session labels).
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Badge overlay text, colour, font and placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeConfig {
    /// Enable badge display
    #[serde(default = "crate::defaults::bool_false")]
    pub badge_enabled: bool,

    /// Badge text format with variable interpolation
    /// Supports \(session.username), \(session.hostname), \(session.path), etc.
    #[serde(default = "crate::defaults::badge_format")]
    pub badge_format: String,

    /// Badge text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::badge_color")]
    pub badge_color: [u8; 3],

    /// Badge opacity (0.0-1.0)
    #[serde(default = "crate::defaults::badge_color_alpha")]
    pub badge_color_alpha: f32,

    /// Badge font family (uses system font if not found)
    #[serde(default = "crate::defaults::badge_font")]
    pub badge_font: String,

    /// Use bold weight for badge font
    #[serde(default = "crate::defaults::bool_true")]
    pub badge_font_bold: bool,

    /// Top margin in pixels from terminal edge
    #[serde(default = "crate::defaults::badge_top_margin")]
    pub badge_top_margin: f32,

    /// Right margin in pixels from terminal edge
    #[serde(default = "crate::defaults::badge_right_margin")]
    pub badge_right_margin: f32,

    /// Maximum badge width as fraction of terminal width (0.0-1.0)
    #[serde(default = "crate::defaults::badge_max_width")]
    pub badge_max_width: f32,

    /// Maximum badge height as fraction of terminal height (0.0-1.0)
    #[serde(default = "crate::defaults::badge_max_height")]
    pub badge_max_height: f32,
}

impl Default for BadgeConfig {
    fn default() -> Self {
        Self {
            badge_enabled: crate::defaults::bool_false(),
            badge_format: crate::defaults::badge_format(),
            badge_color: crate::defaults::badge_color(),
            badge_color_alpha: crate::defaults::badge_color_alpha(),
            badge_font: crate::defaults::badge_font(),
            badge_font_bold: crate::defaults::bool_true(),
            badge_top_margin: crate::defaults::badge_top_margin(),
            badge_right_margin: crate::defaults::badge_right_margin(),
            badge_max_width: crate::defaults::badge_max_width(),
            badge_max_height: crate::defaults::badge_max_height(),
        }
    }
}
