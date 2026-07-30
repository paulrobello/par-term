//! Split-pane layout and appearance settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{DividerStyle, PaneTitlePosition};
use serde::{Deserialize, Serialize};

/// Split-pane divider, padding, title-bar and focus-indicator settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    /// Width of dividers between panes in pixels (visual width)
    #[serde(default = "crate::defaults::pane_divider_width")]
    pub pane_divider_width: Option<f32>,

    /// Width of the drag hit area for resizing panes (should be >= divider width)
    /// A larger hit area makes it easier to grab the divider for resizing
    #[serde(default = "crate::defaults::pane_divider_hit_width")]
    pub pane_divider_hit_width: f32,

    /// Padding inside panes in pixels (space between content and border/divider)
    #[serde(default = "crate::defaults::pane_padding")]
    pub pane_padding: f32,

    /// Minimum pane size in cells (columns for horizontal splits, rows for vertical)
    /// Prevents panes from being resized too small to be useful
    #[serde(default = "crate::defaults::pane_min_size")]
    pub pane_min_size: usize,

    /// Pane background opacity (0.0 = fully transparent, 1.0 = fully opaque)
    /// Lower values allow background image/shader to show through pane backgrounds
    #[serde(default = "crate::defaults::pane_background_opacity")]
    pub pane_background_opacity: f32,

    /// Pane divider color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_divider_color")]
    pub pane_divider_color: [u8; 3],

    /// Pane divider hover color [R, G, B] (0-255) - shown when mouse hovers over divider
    #[serde(default = "crate::defaults::pane_divider_hover_color")]
    pub pane_divider_hover_color: [u8; 3],

    /// Enable visual dimming of inactive panes
    #[serde(default = "crate::defaults::bool_false")]
    pub dim_inactive_panes: bool,

    /// Opacity level for inactive panes (0.0-1.0)
    #[serde(default = "crate::defaults::inactive_pane_opacity")]
    pub inactive_pane_opacity: f32,

    /// Show title bar on each pane
    #[serde(default = "crate::defaults::bool_false")]
    pub show_pane_titles: bool,

    /// Height of pane title bars in pixels
    #[serde(default = "crate::defaults::pane_title_height")]
    pub pane_title_height: f32,

    /// Position of pane title bars (top or bottom)
    #[serde(default)]
    pub pane_title_position: PaneTitlePosition,

    /// Pane title text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_title_color")]
    pub pane_title_color: [u8; 3],

    /// Pane title background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_title_bg_color")]
    pub pane_title_bg_color: [u8; 3],

    /// Pane title font family (empty string = use terminal font)
    #[serde(default)]
    pub pane_title_font: String,

    /// Style of dividers between panes (solid, double, dashed, shadow)
    #[serde(default)]
    pub pane_divider_style: DividerStyle,

    /// Maximum panes per tab (0 = unlimited)
    #[serde(default = "crate::defaults::max_panes")]
    pub max_panes: usize,

    /// Show visual indicator (border) around focused pane
    #[serde(default = "crate::defaults::bool_true")]
    pub pane_focus_indicator: bool,

    /// Color of the focused pane indicator [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_focus_color")]
    pub pane_focus_color: [u8; 3],

    /// Width of the focused pane indicator border in pixels
    #[serde(default = "crate::defaults::pane_focus_width")]
    pub pane_focus_width: f32,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            pane_divider_width: crate::defaults::pane_divider_width(),
            pane_divider_hit_width: crate::defaults::pane_divider_hit_width(),
            pane_padding: crate::defaults::pane_padding(),
            pane_min_size: crate::defaults::pane_min_size(),
            pane_background_opacity: crate::defaults::pane_background_opacity(),
            pane_divider_color: crate::defaults::pane_divider_color(),
            pane_divider_hover_color: crate::defaults::pane_divider_hover_color(),
            dim_inactive_panes: crate::defaults::bool_false(),
            inactive_pane_opacity: crate::defaults::inactive_pane_opacity(),
            show_pane_titles: crate::defaults::bool_false(),
            pane_title_height: crate::defaults::pane_title_height(),
            pane_title_position: PaneTitlePosition::default(),
            pane_title_color: crate::defaults::pane_title_color(),
            pane_title_bg_color: crate::defaults::pane_title_bg_color(),
            pane_title_font: String::new(),
            pane_divider_style: DividerStyle::default(),
            max_panes: crate::defaults::max_panes(),
            pane_focus_indicator: crate::defaults::bool_true(),
            pane_focus_color: crate::defaults::pane_focus_color(),
            pane_focus_width: crate::defaults::pane_focus_width(),
        }
    }
}
