//! Session logging settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::SessionLogFormat;
use serde::{Deserialize, Serialize};

/// Automatic session recording: format, destination and redaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLogConfig {
    /// Automatically record all terminal sessions
    /// When enabled, all terminal output is logged to files in the log directory
    #[serde(default = "crate::defaults::bool_false")]
    pub auto_log_sessions: bool,

    /// Log format for session recording
    /// - plain: Simple text output without escape sequences
    /// - html: Rendered output with colors preserved
    /// - asciicast: asciinema-compatible format for replay/sharing (default)
    #[serde(default)]
    pub session_log_format: SessionLogFormat,

    /// Directory where session logs are saved
    /// Default: ~/.local/share/par-term/logs/
    #[serde(default = "crate::defaults::session_log_directory")]
    pub session_log_directory: String,

    /// Automatically save session log when tab/window closes
    /// When true, ensures the session is fully written before the tab closes
    #[serde(default = "crate::defaults::bool_true")]
    pub archive_on_close: bool,

    /// Redact input during password prompts in session logs.
    /// When enabled, the session logger detects password prompts (sudo, ssh, etc.)
    /// by monitoring terminal output for common prompt patterns, and replaces
    /// any keyboard input recorded during those prompts with a redaction marker.
    /// This prevents passwords and other credentials from being written to disk.
    ///
    /// WARNING: Session logs may still contain sensitive data even with this
    /// enabled. This heuristic catches common password prompts but cannot
    /// guarantee detection of all sensitive input scenarios.
    #[serde(default = "crate::defaults::bool_true")]
    pub session_log_redact_passwords: bool,
}

impl Default for SessionLogConfig {
    fn default() -> Self {
        Self {
            auto_log_sessions: crate::defaults::bool_false(),
            session_log_format: SessionLogFormat::default(),
            session_log_directory: crate::defaults::session_log_directory(),
            archive_on_close: crate::defaults::bool_true(),
            session_log_redact_passwords: crate::defaults::bool_true(),
        }
    }
}
