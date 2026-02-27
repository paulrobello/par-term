//! Terminal behavior configuration types: cursor, input modes, session logging.

use serde::{Deserialize, Serialize};

// ============================================================================
// Cursor Types
// ============================================================================

/// Cursor style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    /// Block cursor (fills entire cell)
    #[default]
    Block,
    /// Beam cursor (vertical line at cell start)
    Beam,
    /// Underline cursor (horizontal line at cell bottom)
    Underline,
}

/// Unfocused cursor style - how the cursor appears when window loses focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UnfocusedCursorStyle {
    /// Show outline-only (hollow) block cursor when unfocused
    #[default]
    Hollow,
    /// Keep same cursor style when unfocused
    Same,
    /// Hide cursor completely when unfocused
    Hidden,
}

// ============================================================================
// Input / Modifier Remapping Types
// ============================================================================

/// Option/Alt key behavior mode
///
/// Controls what happens when Option (macOS) or Alt (Linux/Windows) key is pressed
/// with a character key. This is essential for emacs and vim users who rely on
/// Meta key combinations (M-x, M-f, M-b, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OptionKeyMode {
    /// Normal - sends special characters (default macOS behavior)
    /// Option+f → ƒ (special character)
    Normal,
    /// Meta - sets the high bit (8th bit) on the character
    /// Option+f → 0xE6 (f with high bit set)
    Meta,
    /// Esc - sends Escape prefix before the character (most compatible)
    /// Option+f → ESC f (escape then f)
    /// This is the most compatible mode for terminal applications like emacs and vim
    #[default]
    Esc,
}

/// Target modifier for remapping.
///
/// Allows remapping one modifier key to behave as another.
/// For example, remap Caps Lock to Ctrl, or swap Ctrl and Super.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModifierTarget {
    /// No remapping - use the key's normal function
    #[default]
    None,
    /// Remap to Control key
    Ctrl,
    /// Remap to Alt/Option key
    Alt,
    /// Remap to Shift key
    Shift,
    /// Remap to Super/Cmd/Windows key
    Super,
}

impl ModifierTarget {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ModifierTarget::None => "None (disabled)",
            ModifierTarget::Ctrl => "Ctrl",
            ModifierTarget::Alt => "Alt/Option",
            ModifierTarget::Shift => "Shift",
            ModifierTarget::Super => "Super/Cmd",
        }
    }

    /// All available targets for UI iteration
    pub fn all() -> &'static [ModifierTarget] {
        &[
            ModifierTarget::None,
            ModifierTarget::Ctrl,
            ModifierTarget::Alt,
            ModifierTarget::Shift,
            ModifierTarget::Super,
        ]
    }
}

/// Modifier remapping configuration.
///
/// Allows users to remap modifier keys to different functions.
/// This is useful for:
/// - Swapping Ctrl and Caps Lock
/// - Using Ctrl as Cmd on macOS
/// - Customizing modifier layout for ergonomic keyboards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModifierRemapping {
    /// What the left Ctrl key should act as
    #[serde(default)]
    pub left_ctrl: ModifierTarget,
    /// What the right Ctrl key should act as
    #[serde(default)]
    pub right_ctrl: ModifierTarget,
    /// What the left Alt key should act as
    #[serde(default)]
    pub left_alt: ModifierTarget,
    /// What the right Alt key should act as
    #[serde(default)]
    pub right_alt: ModifierTarget,
    /// What the left Super/Cmd key should act as
    #[serde(default)]
    pub left_super: ModifierTarget,
    /// What the right Super/Cmd key should act as
    #[serde(default)]
    pub right_super: ModifierTarget,
}

// ============================================================================
// Session Logging Types
// ============================================================================

/// Log format for session logging
///
/// Controls the format used when automatically logging terminal sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionLogFormat {
    /// Plain text - strips escape sequences, captures only printable output
    Plain,
    /// HTML - preserves colors and styling as HTML
    Html,
    /// Asciicast v2 - asciinema-compatible format for replay/sharing
    #[default]
    Asciicast,
}

impl SessionLogFormat {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            SessionLogFormat::Plain => "Plain Text",
            SessionLogFormat::Html => "HTML",
            SessionLogFormat::Asciicast => "Asciicast (asciinema)",
        }
    }

    /// All available formats for UI iteration
    pub fn all() -> &'static [SessionLogFormat] {
        &[
            SessionLogFormat::Plain,
            SessionLogFormat::Html,
            SessionLogFormat::Asciicast,
        ]
    }

    /// File extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            SessionLogFormat::Plain => "txt",
            SessionLogFormat::Html => "html",
            SessionLogFormat::Asciicast => "cast",
        }
    }
}

/// Log level for debug logging to file.
///
/// Controls the verbosity of log output written to the debug log file.
/// Environment variables `RUST_LOG` and `--log-level` CLI flag take precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// No logging (log file not created)
    #[default]
    Off,
    /// Errors only
    Error,
    /// Warnings and errors
    Warn,
    /// Informational messages
    Info,
    /// Debug messages
    Debug,
    /// Most verbose
    Trace,
}

impl LogLevel {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            LogLevel::Off => "Off",
            LogLevel::Error => "Error",
            LogLevel::Warn => "Warn",
            LogLevel::Info => "Info",
            LogLevel::Debug => "Debug",
            LogLevel::Trace => "Trace",
        }
    }

    /// All available levels for UI iteration
    pub fn all() -> &'static [LogLevel] {
        &[
            LogLevel::Off,
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ]
    }

    /// Convert to `log::LevelFilter`
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

/// Editor selection mode for semantic history
///
/// Controls how the editor is selected when opening files via semantic history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticHistoryEditorMode {
    /// Use custom editor command from `semantic_history_editor` setting
    Custom,
    /// Use $EDITOR or $VISUAL environment variable
    #[default]
    EnvironmentVariable,
    /// Use system default application for each file type
    SystemDefault,
}

impl SemanticHistoryEditorMode {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            SemanticHistoryEditorMode::Custom => "Custom Editor",
            SemanticHistoryEditorMode::EnvironmentVariable => "Environment Variable ($EDITOR)",
            SemanticHistoryEditorMode::SystemDefault => "System Default",
        }
    }

    /// All available modes for UI iteration
    pub fn all() -> &'static [SemanticHistoryEditorMode] {
        &[
            SemanticHistoryEditorMode::Custom,
            SemanticHistoryEditorMode::EnvironmentVariable,
            SemanticHistoryEditorMode::SystemDefault,
        ]
    }
}

/// Style for link highlight underlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LinkUnderlineStyle {
    /// Solid continuous underline
    Solid,
    /// Dotted/stipple underline (alternating pixels)
    #[default]
    Stipple,
}

impl LinkUnderlineStyle {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            LinkUnderlineStyle::Solid => "Solid",
            LinkUnderlineStyle::Stipple => "Stipple",
        }
    }

    /// All available styles for UI iteration
    pub fn all() -> &'static [LinkUnderlineStyle] {
        &[LinkUnderlineStyle::Solid, LinkUnderlineStyle::Stipple]
    }
}
