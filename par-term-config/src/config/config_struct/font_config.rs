//! Font rendering configuration sub-struct extracted from `Config`.
//!
//! # Extraction Status (ARC-002)
//!
//! This is the first phase of a multi-phase Config extraction. Only the
//! font rendering *quality* settings are moved here in this phase:
//! `font_antialias`, `font_hinting`, `font_thin_strokes`, and `minimum_contrast`.
//!
//! The font *selection* fields (`font_family`, `font_size`, `line_spacing`, etc.)
//! remain inline on `Config` for now — they have 100+ call sites across the workspace
//! and require a dedicated migration effort.  Track under ARC-002.
//!
//! Fields serialise at the top level via `#[serde(flatten)]`, so existing
//! `config.yaml` files require no changes.

use crate::types::ThinStrokesMode;
use serde::{Deserialize, Serialize};

/// Font rendering quality settings extracted from the top-level `Config`.
///
/// Controls anti-aliasing, hinting, stroke weight, and minimum contrast.
/// These four settings always travel together through the codebase (they are
/// applied together in `config_propagation.rs` and `renderer_init.rs`), so
/// grouping them improves cohesion even as a first extraction step.
///
/// See `Config::font_rendering` (flattened onto `Config`) for usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontRenderingConfig {
    /// Enable anti-aliasing for font rendering.
    /// When false, text is rendered without smoothing (aliased/pixelated).
    #[serde(default = "crate::defaults::bool_true")]
    pub font_antialias: bool,

    /// Enable hinting for font rendering.
    /// Hinting improves text clarity at small sizes by aligning glyphs to pixel boundaries.
    /// Disable for a softer, more "true to design" appearance.
    #[serde(default = "crate::defaults::bool_true")]
    pub font_hinting: bool,

    /// Thin strokes / font smoothing mode.
    ///
    /// Controls stroke weight adjustment for improved rendering on different displays:
    /// - `never`: Standard stroke weight everywhere
    /// - `retina_only`: Lighter strokes on HiDPI displays (default)
    /// - `dark_backgrounds_only`: Lighter strokes on dark backgrounds
    /// - `retina_dark_backgrounds_only`: Lighter strokes only on HiDPI + dark backgrounds
    /// - `always`: Always use lighter strokes
    #[serde(default)]
    pub font_thin_strokes: ThinStrokesMode,

    /// Minimum contrast between text and background (iTerm2-compatible).
    ///
    /// When set, adjusts foreground colors to ensure a minimum perceived brightness
    /// difference against the background.
    /// - `0.0`: No adjustment (disabled)
    /// - Values near `1.0`: Maximum contrast (nearly black & white)
    ///
    /// Range: 0.0 to 1.0
    #[serde(default = "crate::defaults::minimum_contrast")]
    pub minimum_contrast: f32,
}

impl Default for FontRenderingConfig {
    fn default() -> Self {
        Self {
            font_antialias: crate::defaults::bool_true(),
            font_hinting: crate::defaults::bool_true(),
            font_thin_strokes: ThinStrokesMode::default(),
            minimum_contrast: crate::defaults::minimum_contrast(),
        }
    }
}
