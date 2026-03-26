//! Window appearance and geometry configuration sub-struct extracted from `Config`.
//!
//! # Extraction Status (ARC-002)
//!
//! This is the first phase of window config extraction. The fields extracted here
//! (`window_opacity`, `window_always_on_top`, `window_decorations`, `blur_enabled`,
//! `blur_radius`, `window_padding`, `hide_window_padding_on_split`,
//! `snap_window_to_grid`) are those most cohesively tied to the window *appearance*
//! rather than its *behavior* or *layout*.
//!
//! Remaining window-related fields (`window_title`, `window_type`, `target_monitor`,
//! `target_space`, `lock_window_size`, `show_window_number`, `max_fps`, `vsync_mode`,
//! `power_preference`, `reduce_flicker`, etc.) remain on `Config` for now.
//! They require a dedicated migration due to the large number of call sites.
//!
//! Fields serialise at the top level via `#[serde(flatten)]`, so existing
//! `config.yaml` files require no changes.

use serde::{Deserialize, Serialize};

/// Window visual appearance settings extracted from the top-level `Config`.
///
/// See `Config::window` (flattened onto `Config`) for usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Window opacity/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "crate::defaults::window_opacity")]
    pub window_opacity: f32,

    /// Keep window always on top of other windows
    #[serde(default = "crate::defaults::bool_false")]
    pub window_always_on_top: bool,

    /// Show window decorations (title bar, borders)
    #[serde(default = "crate::defaults::bool_true")]
    pub window_decorations: bool,

    /// Enable window blur effect (macOS only).
    /// Blurs content behind the transparent window for better readability.
    #[serde(default = "crate::defaults::bool_false")]
    pub blur_enabled: bool,

    /// Blur radius in points (0–64, macOS only).
    /// Higher values = more blur. Default: 10.
    #[serde(default = "crate::defaults::blur_radius")]
    pub blur_radius: u32,

    /// Window padding in pixels.
    #[serde(default = "crate::defaults::window_padding")]
    pub window_padding: f32,

    /// Automatically hide window padding when panes are split.
    /// When true (default), window padding becomes 0 when the active tab has multiple panes.
    #[serde(default = "crate::defaults::bool_true")]
    pub hide_window_padding_on_split: bool,

    /// Snap window dimensions to exact terminal cell boundaries during resize,
    /// eliminating blank background gaps. Disabled automatically in split-pane mode.
    #[serde(default = "crate::defaults::snap_window_to_grid")]
    pub snap_window_to_grid: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_opacity: crate::defaults::window_opacity(),
            window_always_on_top: crate::defaults::bool_false(),
            window_decorations: crate::defaults::bool_true(),
            blur_enabled: crate::defaults::bool_false(),
            blur_radius: crate::defaults::blur_radius(),
            window_padding: crate::defaults::window_padding(),
            hide_window_padding_on_split: crate::defaults::bool_true(),
            snap_window_to_grid: crate::defaults::snap_window_to_grid(),
        }
    }
}
