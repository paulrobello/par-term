//! Font, file drop, and download location configuration types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Font Types
// ============================================================================

/// Font mapping for a specific Unicode range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontRange {
    /// Start of Unicode range (inclusive), e.g., 0x4E00 for CJK
    pub start: u32,
    /// End of Unicode range (inclusive), e.g., 0x9FFF for CJK
    pub end: u32,
    /// Font family name to use for this range
    pub font_family: String,
}

/// Thin strokes / font smoothing mode
///
/// Controls font stroke weight adjustment for improved rendering,
/// particularly on high-DPI/Retina displays. Inspired by iTerm2's thin strokes feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinStrokesMode {
    /// Never apply thin strokes
    Never,
    /// Apply thin strokes only on Retina/HiDPI displays (default)
    #[default]
    RetinaOnly,
    /// Apply thin strokes only on dark backgrounds
    DarkBackgroundsOnly,
    /// Apply thin strokes only on Retina displays with dark backgrounds
    RetinaDarkBackgroundsOnly,
    /// Always apply thin strokes
    Always,
}

// ============================================================================
// File / Download Types
// ============================================================================

/// Default save location for downloaded files
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSaveLocation {
    /// Save to ~/Downloads (default)
    #[default]
    Downloads,
    /// Remember and re-use the last directory the user saved to
    LastUsed,
    /// Use the shell's current working directory
    Cwd,
    /// Use a custom directory path
    Custom(String),
}

impl DownloadSaveLocation {
    /// Get all non-Custom variants for settings UI dropdown
    pub fn variants() -> &'static [DownloadSaveLocation] {
        &[
            DownloadSaveLocation::Downloads,
            DownloadSaveLocation::LastUsed,
            DownloadSaveLocation::Cwd,
        ]
    }

    /// Display name for settings UI
    pub fn display_name(&self) -> &str {
        match self {
            DownloadSaveLocation::Downloads => "Downloads folder",
            DownloadSaveLocation::LastUsed => "Last used directory",
            DownloadSaveLocation::Cwd => "Current working directory",
            DownloadSaveLocation::Custom(_) => "Custom directory",
        }
    }
}

/// Quote style for dropped file paths
///
/// Controls how filenames containing special characters are quoted when dropped into the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DroppedFileQuoteStyle {
    /// Single quotes - safest for most shells (handles $, !, spaces, etc.)
    /// Example: '/path/to/file with spaces.txt'
    #[default]
    SingleQuotes,
    /// Double quotes - allows variable expansion
    /// Example: "/path/to/file with spaces.txt"
    DoubleQuotes,
    /// Backslash escaping - escape individual special characters
    /// Example: /path/to/file\ with\ spaces.txt
    Backslash,
    /// No quoting - insert path as-is (not recommended for paths with special chars)
    None,
}

impl DroppedFileQuoteStyle {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            DroppedFileQuoteStyle::SingleQuotes => "Single quotes ('...')",
            DroppedFileQuoteStyle::DoubleQuotes => "Double quotes (\"...\")",
            DroppedFileQuoteStyle::Backslash => "Backslash escaping (\\)",
            DroppedFileQuoteStyle::None => "None (raw path)",
        }
    }

    /// All available quote styles for UI iteration
    pub fn all() -> &'static [DroppedFileQuoteStyle] {
        &[
            DroppedFileQuoteStyle::SingleQuotes,
            DroppedFileQuoteStyle::DoubleQuotes,
            DroppedFileQuoteStyle::Backslash,
            DroppedFileQuoteStyle::None,
        ]
    }
}
