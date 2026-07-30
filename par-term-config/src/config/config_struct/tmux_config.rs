//! tmux control-mode integration settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// tmux control-mode integration: discovery, auto-attach and status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxConfig {
    /// Enable tmux control mode integration
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_enabled: bool,

    /// Path to tmux executable (default: "tmux" - uses PATH)
    #[serde(default = "crate::defaults::tmux_path")]
    pub tmux_path: String,

    /// Default session name when creating new tmux sessions
    #[serde(default = "crate::defaults::tmux_default_session")]
    pub tmux_default_session: Option<String>,

    /// Auto-attach to existing tmux session on startup
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_auto_attach: bool,

    /// Session name to auto-attach to (if tmux_auto_attach is true)
    #[serde(default = "crate::defaults::tmux_auto_attach_session")]
    pub tmux_auto_attach_session: Option<String>,

    /// Sync clipboard with tmux paste buffer
    /// When copying in par-term, also update tmux's paste buffer via set-buffer
    #[serde(default = "crate::defaults::bool_true")]
    pub tmux_clipboard_sync: bool,

    /// Hide the tmux control-mode gateway tab while tmux windows are active.
    /// When enabled, the tab running `tmux -CC` is hidden from the tab bar once
    /// the first tmux window tab appears. It is restored when the session ends.
    /// Default: false
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_hide_gateway_tab: bool,

    /// Profile to switch to when connected to tmux
    /// When profiles feature is implemented, this will automatically
    /// switch to the specified profile when entering tmux mode
    #[serde(default)]
    pub tmux_profile: Option<String>,

    /// Show tmux status bar in par-term UI
    /// When connected to tmux, display the status bar at the bottom of the terminal
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_show_status_bar: bool,

    /// Tmux status bar refresh interval in milliseconds
    /// How often to poll tmux for updated status bar content.
    /// Lower values mean more frequent updates but slightly more CPU usage.
    /// Default: 1000 (1 second)
    #[serde(default = "crate::defaults::tmux_status_bar_refresh_ms")]
    pub tmux_status_bar_refresh_ms: u64,

    /// Tmux prefix key for control mode
    /// In control mode, par-term intercepts this key combination and waits for a command key.
    /// Format: "C-b" (Ctrl+B, default), "C-Space" (Ctrl+Space), "C-a" (Ctrl+A), etc.
    /// The prefix + command key is translated to the appropriate tmux command.
    #[serde(default = "crate::defaults::tmux_prefix_key")]
    pub tmux_prefix_key: String,

    /// Use native tmux format strings for status bar content
    /// When true, queries tmux for the actual status-left and status-right values
    /// using `display-message -p '#{T:status-left}'` command.
    /// When false, uses par-term's configurable format strings below.
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_status_bar_use_native_format: bool,

    /// Tmux status bar left side format string.
    ///
    /// Supported variables:
    /// - `{session}` - Session name
    /// - `{windows}` - Window list with active marker (*)
    /// - `{pane}` - Focused pane ID
    /// - `{time:FORMAT}` - Current time with strftime format (e.g., `{time:%H:%M}`)
    /// - `{hostname}` - Machine hostname
    /// - `{user}` - Current username
    ///
    /// Default: `[{session}] {windows}`
    #[serde(default = "crate::defaults::tmux_status_bar_left")]
    pub tmux_status_bar_left: String,

    /// Tmux status bar right side format string.
    ///
    /// Same variables as `tmux_status_bar_left`.
    ///
    /// Default: `{pane} | {time:%H:%M}`
    #[serde(default = "crate::defaults::tmux_status_bar_right")]
    pub tmux_status_bar_right: String,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            tmux_enabled: crate::defaults::bool_false(),
            tmux_path: crate::defaults::tmux_path(),
            tmux_default_session: crate::defaults::tmux_default_session(),
            tmux_auto_attach: crate::defaults::bool_false(),
            tmux_auto_attach_session: crate::defaults::tmux_auto_attach_session(),
            tmux_clipboard_sync: crate::defaults::bool_true(),
            tmux_hide_gateway_tab: crate::defaults::bool_false(),
            tmux_profile: None,
            tmux_show_status_bar: crate::defaults::bool_false(),
            tmux_status_bar_refresh_ms: crate::defaults::tmux_status_bar_refresh_ms(),
            tmux_prefix_key: crate::defaults::tmux_prefix_key(),
            tmux_status_bar_use_native_format: crate::defaults::bool_false(),
            tmux_status_bar_left: crate::defaults::tmux_status_bar_left(),
            tmux_status_bar_right: crate::defaults::tmux_status_bar_right(),
        }
    }
}
