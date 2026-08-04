//! Progress bar overlay settings (OSC 9;4 and OSC 934).
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{ProgressBarPosition, ProgressBarStyle};
use serde::{Deserialize, Serialize};

/// Progress bar overlay style, placement and state colours.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressBarConfig {
    /// Enable progress bar overlay
    /// When enabled, progress bars from OSC 9;4 and OSC 934 sequences are displayed
    #[serde(default = "crate::defaults::bool_true")]
    pub progress_bar_enabled: bool,

    /// Progress bar visual style
    /// - bar: Simple thin bar (default)
    /// - barwithtext: Bar with percentage text and labels
    #[serde(default)]
    pub progress_bar_style: ProgressBarStyle,

    /// Progress bar position
    /// - top: Display at the top of the terminal (default)
    /// - bottom: Display at the bottom of the terminal
    #[serde(default)]
    pub progress_bar_position: ProgressBarPosition,

    /// Progress bar height in pixels
    #[serde(default = "crate::defaults::progress_bar_height")]
    pub progress_bar_height: f32,

    /// Progress bar opacity (0.0-1.0)
    #[serde(default = "crate::defaults::progress_bar_opacity")]
    pub progress_bar_opacity: f32,

    /// Color for normal progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_normal_color")]
    pub progress_bar_normal_color: [u8; 3],

    /// Color for warning progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_warning_color")]
    pub progress_bar_warning_color: [u8; 3],

    /// Color for error progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_error_color")]
    pub progress_bar_error_color: [u8; 3],

    /// Color for indeterminate progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_indeterminate_color")]
    pub progress_bar_indeterminate_color: [u8; 3],
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            progress_bar_enabled: crate::defaults::bool_true(),
            progress_bar_style: ProgressBarStyle::default(),
            progress_bar_position: ProgressBarPosition::default(),
            progress_bar_height: crate::defaults::progress_bar_height(),
            progress_bar_opacity: crate::defaults::progress_bar_opacity(),
            progress_bar_normal_color: crate::defaults::progress_bar_normal_color(),
            progress_bar_warning_color: crate::defaults::progress_bar_warning_color(),
            progress_bar_error_color: crate::defaults::progress_bar_error_color(),
            progress_bar_indeterminate_color: crate::defaults::progress_bar_indeterminate_color(),
        }
    }
}
