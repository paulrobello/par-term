//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs,
//! managing split panes, and handling profile operations.
//!
//! Sub-modules:
//! - `lifecycle` — tab creation, closing, reopening, navigation, and duplication
//! - `pane_ops` — split pane operations (split, navigate, resize, close panes)
//! - `profile_ops` — profile management (open, apply, auto-switch profiles)

mod lifecycle;
mod pane_ops;
mod profile_ops;

/// Metadata captured when a tab is closed, used for session undo (reopen closed tab).
pub(crate) struct ClosedTabInfo {
    pub cwd: Option<String>,
    pub title: String,
    pub has_default_title: bool,
    pub index: usize,
    pub closed_at: std::time::Instant,
    pub pane_layout: Option<crate::session::SessionPaneNode>,
    pub custom_color: Option<[u8; 3]>,
    /// When `session_undo_preserve_shell` is enabled, the live Tab is kept here
    /// instead of being dropped. Dropping this ClosedTabInfo will drop the Tab,
    /// which kills the PTY.
    pub hidden_tab: Option<crate::tab::Tab>,
}
