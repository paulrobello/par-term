//! Terminal background image and transparency settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::BackgroundImageMode;
use serde::{Deserialize, Serialize};

/// Background image source and how window transparency applies to cells and text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundConfig {
    /// When true, only the default terminal background is transparent.
    /// Colored backgrounds (syntax highlighting, status bars, etc.) remain opaque.
    /// This keeps text readable while allowing window transparency.
    #[serde(default = "crate::defaults::bool_true")]
    pub transparency_affects_only_default_background: bool,

    /// When true, text is always rendered at full opacity regardless of window transparency.
    /// This ensures text remains crisp and readable even with transparent backgrounds.
    #[serde(default = "crate::defaults::bool_true")]
    pub keep_text_opaque: bool,

    /// Background image path (optional, supports ~ for home directory)
    #[serde(default)]
    pub background_image: Option<String>,

    /// Enable or disable background image rendering (even if a path is set)
    #[serde(default = "crate::defaults::bool_true")]
    pub background_image_enabled: bool,

    /// Background image display mode
    /// - fit: Scale to fit window while maintaining aspect ratio
    /// - fill: Scale to fill window while maintaining aspect ratio (may crop)
    /// - stretch: Stretch to fill window, ignoring aspect ratio (default)
    /// - tile: Repeat image in a tiled pattern
    /// - center: Center image at original size
    #[serde(default)]
    pub background_image_mode: BackgroundImageMode,

    /// Background image opacity (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "crate::defaults::background_image_opacity")]
    pub background_image_opacity: f32,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            transparency_affects_only_default_background: crate::defaults::bool_true(),
            keep_text_opaque: crate::defaults::bool_true(),
            background_image: None,
            background_image_enabled: crate::defaults::bool_true(),
            background_image_mode: BackgroundImageMode::default(),
            background_image_opacity: crate::defaults::background_image_opacity(),
        }
    }
}
