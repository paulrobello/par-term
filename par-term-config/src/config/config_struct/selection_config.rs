//! Selection and clipboard settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::DroppedFileQuoteStyle;
use serde::{Deserialize, Serialize};

/// Copy-on-select, paste behaviour and dropped-file quoting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionConfig {
    /// Automatically copy selected text to clipboard
    #[serde(default = "crate::defaults::bool_true")]
    pub auto_copy_selection: bool,

    /// Include trailing newline when copying lines
    /// Note: Inverted logic from old strip_trailing_newline_on_copy
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "strip_trailing_newline_on_copy"
    )]
    pub copy_trailing_newline: bool,

    /// Paste on middle mouse button click
    #[serde(default = "crate::defaults::bool_true")]
    pub middle_click_paste: bool,

    /// Delay in milliseconds between pasted lines (0 = no delay)
    /// Useful for slow terminals or remote connections that can't handle rapid paste
    #[serde(default = "crate::defaults::paste_delay_ms")]
    pub paste_delay_ms: u64,

    /// When `true` (default), log a warning when clipboard paste content contains
    /// control characters that were stripped by the paste sanitizer.
    ///
    /// Control characters in paste content (ESC, C0, C1) can inject terminal escape
    /// sequences and execute commands when pasted into a shell. The sanitizer always
    /// strips them; this flag controls whether a warning is logged so users can
    /// investigate unexpected behaviour.
    ///
    /// Set to `false` to suppress warnings for pastes that regularly contain
    /// control characters (e.g., when pasting binary content intentionally).
    #[serde(default = "crate::defaults::bool_true")]
    pub warn_paste_control_chars: bool,

    /// Quote style for dropped file paths
    /// - single_quotes: Wrap in single quotes (safest for most shells)
    /// - double_quotes: Wrap in double quotes
    /// - backslash: Escape special characters with backslashes
    /// - none: Insert path as-is (not recommended)
    #[serde(default)]
    pub dropped_file_quote_style: DroppedFileQuoteStyle,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            auto_copy_selection: crate::defaults::bool_true(),
            copy_trailing_newline: crate::defaults::bool_false(),
            middle_click_paste: crate::defaults::bool_true(),
            paste_delay_ms: crate::defaults::paste_delay_ms(),
            warn_paste_control_chars: crate::defaults::bool_true(),
            dropped_file_quote_style: DroppedFileQuoteStyle::default(),
        }
    }
}
