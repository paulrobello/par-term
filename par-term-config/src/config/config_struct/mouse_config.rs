//! Mouse behavior settings for the terminal emulator.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.
//!
//! Covers scroll speed, click timing thresholds, option-click cursor movement,
//! focus-follows-mouse, and horizontal scroll reporting.

use serde::{Deserialize, Serialize};

/// Mouse behavior configuration.
///
/// Controls scroll speed, click timing thresholds, option-click cursor
/// positioning, focus-follows-mouse, and horizontal scroll reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseConfig {
    /// Mouse wheel scroll speed multiplier
    #[serde(default = "crate::defaults::scroll_speed")]
    pub mouse_scroll_speed: f32,

    /// Double-click timing threshold in milliseconds
    #[serde(default = "crate::defaults::double_click_threshold")]
    pub mouse_double_click_threshold: u64,

    /// Triple-click timing threshold in milliseconds (typically same as double-click)
    #[serde(default = "crate::defaults::triple_click_threshold")]
    pub mouse_triple_click_threshold: u64,

    /// Option+Click (macOS) / Alt+Click (Linux/Windows) moves cursor to clicked position
    /// Sends cursor movement escape sequences to position text cursor at click location
    /// Useful for quick cursor positioning in shells and editors
    #[serde(default = "crate::defaults::bool_true")]
    pub option_click_moves_cursor: bool,

    /// Focus window automatically when mouse enters (without requiring a click)
    /// This is an accessibility feature that some users prefer
    #[serde(default = "crate::defaults::bool_false")]
    pub focus_follows_mouse: bool,

    /// Report horizontal scroll events to terminal applications when mouse reporting is enabled
    /// Horizontal scroll uses button codes 6 (left) and 7 (right) in the mouse protocol
    #[serde(default = "crate::defaults::bool_true")]
    pub report_horizontal_scroll: bool,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            mouse_scroll_speed: crate::defaults::scroll_speed(),
            mouse_double_click_threshold: crate::defaults::double_click_threshold(),
            mouse_triple_click_threshold: crate::defaults::triple_click_threshold(),
            option_click_moves_cursor: crate::defaults::bool_true(),
            focus_follows_mouse: crate::defaults::bool_false(),
            report_horizontal_scroll: crate::defaults::bool_true(),
        }
    }
}
