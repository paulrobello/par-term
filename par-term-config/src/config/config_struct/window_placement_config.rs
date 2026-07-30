//! Window placement and sizing constraints.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::WindowType;
use serde::{Deserialize, Serialize};

/// Window type, target monitor/Space, resize lock and window numbering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPlacementConfig {
    /// Window type (normal, fullscreen, or edge-anchored)
    /// - normal: Standard window (default)
    /// - fullscreen: Start in fullscreen mode
    /// - edge_top/edge_bottom/edge_left/edge_right: Edge-anchored dropdown-style window
    #[serde(default)]
    pub window_type: WindowType,

    /// Target monitor index for window placement (0 = primary)
    /// Use None to let the OS decide window placement
    #[serde(default)]
    pub target_monitor: Option<usize>,

    /// Target macOS Space (virtual desktop) for window placement (1-based ordinal)
    /// Use None to let the OS decide which Space to open on.
    /// Only effective on macOS; ignored on other platforms.
    #[serde(default)]
    pub target_space: Option<u32>,

    /// Lock window size to prevent resize
    /// When true, the window cannot be resized by the user
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_window_size: bool,

    /// Show window number in title bar
    /// Useful when multiple par-term windows are open
    #[serde(default = "crate::defaults::bool_false")]
    pub show_window_number: bool,
}

impl Default for WindowPlacementConfig {
    fn default() -> Self {
        Self {
            window_type: WindowType::default(),
            target_monitor: None,
            target_space: None,
            lock_window_size: crate::defaults::bool_false(),
            show_window_number: crate::defaults::bool_false(),
        }
    }
}
