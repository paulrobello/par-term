//! Tab bar colour and per-tab appearance settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Tab bar palette, dimming, sizing and border appearance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabBarColorsConfig {
    /// Tab bar background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_bar_background")]
    pub tab_bar_background: [u8; 3],

    /// Active tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_background")]
    pub tab_active_background: [u8; 3],

    /// Inactive tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_inactive_background")]
    pub tab_inactive_background: [u8; 3],

    /// Hovered tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_hover_background")]
    pub tab_hover_background: [u8; 3],

    /// Active tab text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_text")]
    pub tab_active_text: [u8; 3],

    /// Inactive tab text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_inactive_text")]
    pub tab_inactive_text: [u8; 3],

    /// Active tab indicator/underline color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_indicator")]
    pub tab_active_indicator: [u8; 3],

    /// Activity indicator dot color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_activity_indicator")]
    pub tab_activity_indicator: [u8; 3],

    /// Bell indicator color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_bell_indicator")]
    pub tab_bell_indicator: [u8; 3],

    /// Close button color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_close_button")]
    pub tab_close_button: [u8; 3],

    /// Close button hover color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_close_button_hover")]
    pub tab_close_button_hover: [u8; 3],

    /// Enable visual dimming of inactive tabs
    /// When true, inactive tabs are rendered with reduced opacity
    #[serde(default = "crate::defaults::bool_true")]
    pub dim_inactive_tabs: bool,

    /// Opacity level for inactive tabs (0.0-1.0)
    /// Only used when dim_inactive_tabs is true
    /// Lower values make inactive tabs more transparent/dimmed
    #[serde(default = "crate::defaults::inactive_tab_opacity")]
    pub inactive_tab_opacity: f32,

    /// Minimum tab width in pixels before horizontal scrolling is enabled
    /// When tabs cannot fit at this width, scroll buttons appear
    #[serde(default = "crate::defaults::tab_min_width")]
    pub tab_min_width: f32,

    /// Stretch tabs to fill the available tab bar width evenly (iTerm2 style)
    /// When false, tabs keep their minimum width and excess space is left unused
    #[serde(default = "crate::defaults::tab_stretch_to_fill")]
    pub tab_stretch_to_fill: bool,

    /// Render tab titles as limited HTML (bold/italic/underline/color spans)
    /// When false, titles are rendered as plain text
    #[serde(default = "crate::defaults::tab_html_titles")]
    pub tab_html_titles: bool,

    /// Tab border color [R, G, B] (0-255)
    /// A thin border around each tab to help distinguish them
    #[serde(default = "crate::defaults::tab_border_color")]
    pub tab_border_color: [u8; 3],

    /// Tab border width in pixels (0 = no border)
    #[serde(default = "crate::defaults::tab_border_width")]
    pub tab_border_width: f32,

    /// Render inactive tabs as outline only (no fill)
    /// When true, inactive tabs show only a border stroke with no background fill.
    /// Hovered inactive tabs brighten the outline instead of filling.
    #[serde(default = "crate::defaults::bool_true")]
    pub tab_inactive_outline_only: bool,
}

impl Default for TabBarColorsConfig {
    fn default() -> Self {
        Self {
            tab_bar_background: crate::defaults::tab_bar_background(),
            tab_active_background: crate::defaults::tab_active_background(),
            tab_inactive_background: crate::defaults::tab_inactive_background(),
            tab_hover_background: crate::defaults::tab_hover_background(),
            tab_active_text: crate::defaults::tab_active_text(),
            tab_inactive_text: crate::defaults::tab_inactive_text(),
            tab_active_indicator: crate::defaults::tab_active_indicator(),
            tab_activity_indicator: crate::defaults::tab_activity_indicator(),
            tab_bell_indicator: crate::defaults::tab_bell_indicator(),
            tab_close_button: crate::defaults::tab_close_button(),
            tab_close_button_hover: crate::defaults::tab_close_button_hover(),
            dim_inactive_tabs: crate::defaults::bool_true(),
            inactive_tab_opacity: crate::defaults::inactive_tab_opacity(),
            tab_min_width: crate::defaults::tab_min_width(),
            tab_stretch_to_fill: crate::defaults::tab_stretch_to_fill(),
            tab_html_titles: crate::defaults::tab_html_titles(),
            tab_border_color: crate::defaults::tab_border_color(),
            tab_border_width: crate::defaults::tab_border_width(),
            tab_inactive_outline_only: crate::defaults::bool_true(),
        }
    }
}
