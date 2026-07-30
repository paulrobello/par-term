//! Tab and tab bar behaviour settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{
    NewTabPosition, RemoteTabTitleFormat, TabBarMode, TabBarPosition, TabStyle, TabTitleMode,
};
use serde::{Deserialize, Serialize};

/// Tab style presets, tab bar placement and new-tab behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabConfig {
    /// Tab visual style preset (dark, light, compact, minimal, high_contrast, automatic)
    /// Applies cosmetic color/size/spacing presets to the tab bar
    #[serde(default)]
    pub tab_style: TabStyle,

    /// Tab style to use when system is in light mode (used when tab_style is Automatic)
    #[serde(default = "crate::defaults::light_tab_style")]
    pub light_tab_style: TabStyle,

    /// Tab style to use when system is in dark mode (used when tab_style is Automatic)
    #[serde(default = "crate::defaults::dark_tab_style")]
    pub dark_tab_style: TabStyle,

    /// Tab bar visibility mode (always, when_multiple, never)
    #[serde(default)]
    pub tab_bar_mode: TabBarMode,

    /// Controls how tab titles are automatically updated (auto or osc_only)
    #[serde(default)]
    pub tab_title_mode: TabTitleMode,

    /// Format for tab title when shell integration detects a remote host
    #[serde(default)]
    pub remote_tab_title_format: RemoteTabTitleFormat,

    /// When true, explicit OSC title sequences override remote_tab_title_format
    #[serde(default = "crate::defaults::bool_true")]
    pub remote_tab_title_osc_priority: bool,

    /// Tab bar height in pixels
    #[serde(default = "crate::defaults::tab_bar_height")]
    pub tab_bar_height: f32,

    /// Tab bar position (top, bottom, left)
    #[serde(default)]
    pub tab_bar_position: TabBarPosition,

    /// Tab bar width in pixels (used when tab_bar_position is Left)
    #[serde(default = "crate::defaults::tab_bar_width")]
    pub tab_bar_width: f32,

    /// Show close button on tabs
    #[serde(default = "crate::defaults::bool_true")]
    pub tab_show_close_button: bool,

    /// Show tab index numbers (for Cmd+1-9)
    #[serde(default = "crate::defaults::bool_false")]
    pub tab_show_index: bool,

    /// New tab inherits working directory from active tab
    #[serde(default = "crate::defaults::bool_true")]
    pub tab_inherit_cwd: bool,

    /// Maximum tabs per window (0 = unlimited)
    #[serde(default = "crate::defaults::zero")]
    pub max_tabs: usize,

    /// Show the profile drawer toggle button on the right edge of the terminal
    /// When disabled, the profile drawer can still be opened via keyboard shortcut
    #[serde(default = "crate::defaults::bool_false")]
    pub show_profile_drawer_button: bool,

    /// When true, the new-tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows the
    /// profile selection dropdown instead of immediately opening a default tab
    #[serde(default = "crate::defaults::bool_false")]
    pub new_tab_shortcut_shows_profiles: bool,

    /// Where to insert new tabs in the tab bar.
    /// `end` appends to the end (default); `after_active` inserts right of the active tab.
    #[serde(default)]
    pub new_tab_position: NewTabPosition,
}

impl Default for TabConfig {
    fn default() -> Self {
        Self {
            tab_style: TabStyle::default(),
            light_tab_style: crate::defaults::light_tab_style(),
            dark_tab_style: crate::defaults::dark_tab_style(),
            tab_bar_mode: TabBarMode::default(),
            tab_title_mode: TabTitleMode::default(),
            remote_tab_title_format: RemoteTabTitleFormat::default(),
            remote_tab_title_osc_priority: true,
            tab_bar_height: crate::defaults::tab_bar_height(),
            tab_bar_position: TabBarPosition::default(),
            tab_bar_width: crate::defaults::tab_bar_width(),
            tab_show_close_button: crate::defaults::bool_true(),
            tab_show_index: crate::defaults::bool_false(),
            tab_inherit_cwd: crate::defaults::bool_true(),
            max_tabs: crate::defaults::zero(),
            show_profile_drawer_button: crate::defaults::bool_false(),
            new_tab_shortcut_shows_profiles: crate::defaults::bool_false(),
            new_tab_position: NewTabPosition::default(),
        }
    }
}
