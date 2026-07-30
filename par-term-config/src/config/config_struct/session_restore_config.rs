//! Session restore and closed-tab undo settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Startup arrangement restore, session restore and closed-tab undo retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRestoreConfig {
    /// Name of arrangement to auto-restore on startup (None = disabled)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_restore_arrangement: Option<String>,

    /// Whether to restore the previous session (tabs, panes, CWDs) on startup
    #[serde(default = "crate::defaults::bool_false")]
    pub restore_session: bool,

    /// Seconds to keep closed tab metadata for undo (0 = disabled)
    #[serde(default = "crate::defaults::session_undo_timeout_secs")]
    pub session_undo_timeout_secs: u32,

    /// Maximum number of closed tabs to remember for undo
    #[serde(default = "crate::defaults::session_undo_max_entries")]
    pub session_undo_max_entries: usize,

    /// When true, closing a tab hides the shell instead of killing it.
    /// Undo restores the full session with scrollback and running processes.
    #[serde(default = "crate::defaults::session_undo_preserve_shell")]
    pub session_undo_preserve_shell: bool,
}

impl Default for SessionRestoreConfig {
    fn default() -> Self {
        Self {
            auto_restore_arrangement: None,
            restore_session: crate::defaults::bool_false(),
            session_undo_timeout_secs: crate::defaults::session_undo_timeout_secs(),
            session_undo_max_entries: crate::defaults::session_undo_max_entries(),
            session_undo_preserve_shell: crate::defaults::session_undo_preserve_shell(),
        }
    }
}
