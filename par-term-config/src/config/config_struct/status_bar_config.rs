//! [`StatusBarConfig`]: Status bar settings.

use crate::types::StatusBarPosition;
use serde::{Deserialize, Serialize};

/// Configuration for the status bar displayed at the top or bottom of the terminal.
///
/// Extracted from the monolithic `Config` struct via `#[serde(flatten)]`.
/// All fields that were previously `status_bar_*` on `Config` are now
/// grouped here, keeping the YAML format fully backward-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusBarConfig {
    /// Enable the status bar
    #[serde(default = "default_status_bar_enabled")]
    pub status_bar_enabled: bool,

    /// Status bar position (top or bottom)
    #[serde(default)]
    pub status_bar_position: StatusBarPosition,

    /// Status bar height in pixels
    #[serde(default = "default_status_bar_height")]
    pub status_bar_height: f32,

    /// Status bar background color [R, G, B] (0-255)
    #[serde(default = "default_status_bar_bg_color")]
    pub status_bar_bg_color: [u8; 3],

    /// Status bar background alpha (0.0-1.0)
    #[serde(default = "default_status_bar_bg_alpha")]
    pub status_bar_bg_alpha: f32,

    /// Status bar foreground (text) color [R, G, B] (0-255)
    #[serde(default = "default_status_bar_fg_color")]
    pub status_bar_fg_color: [u8; 3],

    /// Status bar font family (empty string = use terminal font)
    #[serde(default)]
    pub status_bar_font: String,

    /// Status bar font size in points
    #[serde(default = "default_status_bar_font_size")]
    pub status_bar_font_size: f32,

    /// Separator string between widgets
    #[serde(default = "default_status_bar_separator")]
    pub status_bar_separator: String,

    /// Auto-hide the status bar when in fullscreen mode
    #[serde(default = "default_status_bar_auto_hide_fullscreen")]
    pub status_bar_auto_hide_fullscreen: bool,

    /// Auto-hide the status bar when mouse is inactive
    #[serde(default = "default_status_bar_auto_hide_mouse_inactive")]
    pub status_bar_auto_hide_mouse_inactive: bool,

    /// Timeout in seconds before hiding status bar after last mouse activity
    #[serde(default = "default_status_bar_mouse_inactive_timeout")]
    pub status_bar_mouse_inactive_timeout: f32,

    /// Polling interval in seconds for system monitor data (CPU, memory, network)
    #[serde(default = "default_status_bar_system_poll_interval")]
    pub status_bar_system_poll_interval: f32,

    /// Polling interval in seconds for git branch detection
    #[serde(default = "default_status_bar_git_poll_interval")]
    pub status_bar_git_poll_interval: f32,

    /// Time format string for the Clock widget (chrono strftime syntax)
    #[serde(default = "default_status_bar_time_format")]
    pub status_bar_time_format: String,

    /// Show ahead/behind and dirty indicators on the Git Branch widget
    #[serde(default = "default_status_bar_git_show_status")]
    pub status_bar_git_show_status: bool,

    /// Widget configuration list
    #[serde(default = "crate::status_bar::default_widgets")]
    pub status_bar_widgets: Vec<crate::status_bar::StatusBarWidgetConfig>,
}

// ── Default value functions ────────────────────────────────────────────────

fn default_status_bar_enabled() -> bool {
    false
}

fn default_status_bar_height() -> f32 {
    22.0
}

fn default_status_bar_bg_color() -> [u8; 3] {
    [30, 30, 30]
}

fn default_status_bar_bg_alpha() -> f32 {
    0.95
}

fn default_status_bar_fg_color() -> [u8; 3] {
    [200, 200, 200]
}

fn default_status_bar_font_size() -> f32 {
    12.0
}

fn default_status_bar_separator() -> String {
    " \u{2502} ".to_string() // " │ "
}

fn default_status_bar_auto_hide_fullscreen() -> bool {
    true
}

fn default_status_bar_auto_hide_mouse_inactive() -> bool {
    false
}

fn default_status_bar_mouse_inactive_timeout() -> f32 {
    3.0
}

fn default_status_bar_system_poll_interval() -> f32 {
    2.0
}

fn default_status_bar_git_poll_interval() -> f32 {
    5.0
}

fn default_status_bar_time_format() -> String {
    "%H:%M:%S".to_string()
}

fn default_status_bar_git_show_status() -> bool {
    true
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            status_bar_enabled: default_status_bar_enabled(),
            status_bar_position: StatusBarPosition::default(),
            status_bar_height: default_status_bar_height(),
            status_bar_bg_color: default_status_bar_bg_color(),
            status_bar_bg_alpha: default_status_bar_bg_alpha(),
            status_bar_fg_color: default_status_bar_fg_color(),
            status_bar_font: String::new(),
            status_bar_font_size: default_status_bar_font_size(),
            status_bar_separator: default_status_bar_separator(),
            status_bar_auto_hide_fullscreen: default_status_bar_auto_hide_fullscreen(),
            status_bar_auto_hide_mouse_inactive: default_status_bar_auto_hide_mouse_inactive(),
            status_bar_mouse_inactive_timeout: default_status_bar_mouse_inactive_timeout(),
            status_bar_system_poll_interval: default_status_bar_system_poll_interval(),
            status_bar_git_poll_interval: default_status_bar_git_poll_interval(),
            status_bar_time_format: default_status_bar_time_format(),
            status_bar_git_show_status: default_status_bar_git_show_status(),
            status_bar_widgets: crate::status_bar::default_widgets(),
        }
    }
}
