//! Scrollbar appearance and behaviour settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Scrollbar placement, size, colours, command marks and auto-hide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbarConfig {
    /// Auto-hide scrollbar after inactivity (milliseconds, 0 = never hide)
    #[serde(default = "crate::defaults::scrollbar_autohide_delay")]
    pub scrollbar_autohide_delay: u64,

    /// Scrollbar position (left or right)
    #[serde(default = "crate::defaults::scrollbar_position")]
    pub scrollbar_position: String,

    /// Scrollbar width in pixels
    #[serde(default = "crate::defaults::scrollbar_width")]
    pub scrollbar_width: f32,

    /// Scrollbar thumb color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "crate::defaults::scrollbar_thumb_color")]
    pub scrollbar_thumb_color: [f32; 4],

    /// Scrollbar track color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "crate::defaults::scrollbar_track_color")]
    pub scrollbar_track_color: [f32; 4],

    /// Show command markers on the scrollbar (requires shell integration)
    #[serde(default = "crate::defaults::bool_true")]
    pub scrollbar_command_marks: bool,

    /// Show tooltips when hovering over scrollbar command markers
    #[serde(default = "crate::defaults::bool_false")]
    pub scrollbar_mark_tooltips: bool,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            scrollbar_autohide_delay: crate::defaults::scrollbar_autohide_delay(),
            scrollbar_position: crate::defaults::scrollbar_position(),
            scrollbar_width: crate::defaults::scrollbar_width(),
            scrollbar_thumb_color: crate::defaults::scrollbar_thumb_color(),
            scrollbar_track_color: crate::defaults::scrollbar_track_color(),
            scrollbar_command_marks: crate::defaults::bool_true(),
            scrollbar_mark_tooltips: crate::defaults::bool_false(),
        }
    }
}
