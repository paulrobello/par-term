//! Command separator line settings (requires shell integration).
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Horizontal separator lines drawn between shell commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSeparatorConfig {
    /// Show horizontal separator lines between commands (requires shell integration)
    #[serde(default = "crate::defaults::bool_false")]
    pub command_separator_enabled: bool,

    /// Thickness of command separator lines in pixels
    #[serde(default = "crate::defaults::command_separator_thickness")]
    pub command_separator_thickness: f32,

    /// Opacity of command separator lines (0.0-1.0)
    #[serde(default = "crate::defaults::command_separator_opacity")]
    pub command_separator_opacity: f32,

    /// Color separator lines by exit code (green=success, red=failure, gray=unknown)
    #[serde(default = "crate::defaults::bool_true")]
    pub command_separator_exit_color: bool,

    /// Custom color for separator lines when exit_color is disabled [R, G, B]
    #[serde(default = "crate::defaults::command_separator_color")]
    pub command_separator_color: [u8; 3],
}

impl Default for CommandSeparatorConfig {
    fn default() -> Self {
        Self {
            command_separator_enabled: crate::defaults::bool_false(),
            command_separator_thickness: crate::defaults::command_separator_thickness(),
            command_separator_opacity: crate::defaults::command_separator_opacity(),
            command_separator_exit_color: crate::defaults::bool_true(),
            command_separator_color: crate::defaults::command_separator_color(),
        }
    }
}
