//! Inline image and terminal background settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{BackgroundMode, ImageScalingMode};
use serde::{Deserialize, Serialize};

/// Inline image scaling (Sixel/iTerm2/Kitty) and the terminal background source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Scaling quality for inline images (nearest = sharp/pixel art, linear = smooth)
    #[serde(default)]
    pub image_scaling_mode: ImageScalingMode,

    /// Preserve aspect ratio when scaling inline images to fit terminal cells
    #[serde(default = "crate::defaults::bool_true")]
    pub image_preserve_aspect_ratio: bool,

    /// Background mode selection (default, color, or image)
    #[serde(default)]
    pub background_mode: BackgroundMode,

    /// Per-pane background image configurations
    #[serde(default)]
    pub pane_backgrounds: Vec<crate::config::PaneBackgroundConfig>,

    /// Custom solid background color [R, G, B] (0-255)
    /// Used when background_mode is "color"
    /// Transparency is controlled by window_opacity
    #[serde(default = "crate::defaults::background_color")]
    pub background_color: [u8; 3],
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            image_scaling_mode: ImageScalingMode::default(),
            image_preserve_aspect_ratio: crate::defaults::bool_true(),
            background_mode: BackgroundMode::default(),
            pane_backgrounds: Vec::new(),
            background_color: crate::defaults::background_color(),
        }
    }
}
