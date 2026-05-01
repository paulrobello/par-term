//! Cursor appearance and behavior settings for the terminal emulator.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.
//!
//! Covers cursor style, blink, color, visibility locks, guide lines, shadows,
//! boost/glow, and unfocused cursor behavior.

use crate::types::{CursorStyle, UnfocusedCursorStyle};
use serde::{Deserialize, Serialize};

/// Cursor appearance and behavior configuration.
///
/// Controls the visual style, blinking, color, visibility locks, guide line,
/// shadow, boost/glow, and unfocused cursor appearance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorConfig {
    // --- Blinking ---
    /// Enable cursor blinking
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_blink: bool,

    /// Cursor blink interval in milliseconds
    #[serde(default = "crate::defaults::cursor_blink_interval")]
    pub cursor_blink_interval: u64,

    // --- Style ---
    /// Cursor style (block, beam, underline)
    #[serde(default)]
    pub cursor_style: CursorStyle,

    // --- Color ---
    /// Cursor color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::cursor_color")]
    pub cursor_color: [u8; 3],

    /// Color of text under block cursor [R, G, B] (0-255)
    /// If not set (None), uses automatic contrast color
    /// Only affects block cursor style (beam and underline don't obscure text)
    #[serde(default)]
    pub cursor_text_color: Option<[u8; 3]>,

    // --- Visibility/Style Locks ---
    /// Lock cursor visibility - prevent applications from hiding the cursor
    /// When true, the cursor remains visible regardless of DECTCEM escape sequences
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_visibility: bool,

    /// Lock cursor style - prevent applications from changing the cursor style
    /// When true, the cursor style from config is always used, ignoring DECSCUSR escape sequences
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_style: bool,

    /// Lock cursor blink - prevent applications from enabling cursor blink
    /// When true and cursor_blink is false, applications cannot enable blinking cursor
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_blink: bool,

    // --- Guide Line ---
    /// Enable horizontal guide line at cursor row for better tracking in wide terminals
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_guide_enabled: bool,

    /// Cursor guide color [R, G, B, A] (0-255), subtle highlight spanning full terminal width
    #[serde(default = "crate::defaults::cursor_guide_color")]
    pub cursor_guide_color: [u8; 4],

    // --- Shadow ---
    /// Enable drop shadow behind cursor for better visibility against varying backgrounds
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_shadow_enabled: bool,

    /// Cursor shadow color [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::cursor_shadow_color")]
    pub cursor_shadow_color: [u8; 4],

    /// Cursor shadow offset in pixels [x, y]
    #[serde(default = "crate::defaults::cursor_shadow_offset")]
    pub cursor_shadow_offset: [f32; 2],

    /// Cursor shadow blur radius in pixels
    #[serde(default = "crate::defaults::cursor_shadow_blur")]
    pub cursor_shadow_blur: f32,

    // --- Boost / Glow ---
    /// Cursor boost (glow) intensity (0.0 = off, 1.0 = maximum boost)
    /// Adds a glow/highlight effect around the cursor for visibility
    #[serde(default = "crate::defaults::cursor_boost")]
    pub cursor_boost: f32,

    /// Cursor boost glow color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::cursor_boost_color")]
    pub cursor_boost_color: [u8; 3],

    // --- Unfocused ---
    /// Cursor appearance when window is unfocused
    /// - hollow: Show outline-only block cursor (default, standard terminal behavior)
    /// - same: Keep same cursor style as when focused
    /// - hidden: Hide cursor completely when unfocused
    #[serde(default)]
    pub unfocused_cursor_style: UnfocusedCursorStyle,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            cursor_blink: crate::defaults::bool_false(),
            cursor_blink_interval: crate::defaults::cursor_blink_interval(),
            cursor_style: CursorStyle::default(),
            cursor_color: crate::defaults::cursor_color(),
            cursor_text_color: None,
            lock_cursor_visibility: crate::defaults::bool_false(),
            lock_cursor_style: crate::defaults::bool_false(),
            lock_cursor_blink: crate::defaults::bool_false(),
            cursor_guide_enabled: crate::defaults::bool_false(),
            cursor_guide_color: crate::defaults::cursor_guide_color(),
            cursor_shadow_enabled: crate::defaults::bool_false(),
            cursor_shadow_color: crate::defaults::cursor_shadow_color(),
            cursor_shadow_offset: crate::defaults::cursor_shadow_offset(),
            cursor_shadow_blur: crate::defaults::cursor_shadow_blur(),
            cursor_boost: crate::defaults::cursor_boost(),
            cursor_boost_color: crate::defaults::cursor_boost_color(),
            unfocused_cursor_style: UnfocusedCursorStyle::default(),
        }
    }
}
