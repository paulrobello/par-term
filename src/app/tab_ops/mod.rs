//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs,
//! managing split panes, and handling profile operations.
//!
//! Sub-modules:
//! - `lifecycle`  — tab creation, closing, and navigation
//! - `tab_reopen` — session undo: reopen closed tab, tab-bar resize helper
//! - `tab_helpers` — tab duplication, query predicates, and close-confirmation gate
//! - `pane_ops` — split pane operations (split, navigate, resize, close panes)
//! - `profile_ops` — profile management (open, apply, auto-switch profiles)

mod lifecycle;
mod pane_ops;
pub(crate) mod pane_transfer;
mod profile_auto_switch;
mod profile_ops;
mod tab_helpers;
mod tab_reopen;

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
    /// Set when the entry records a par-mux window killed daemon-side by tab
    /// close (`kill-window`), not a local tab. It cannot be reopened; Cmd+Z
    /// consumes it with an explanatory toast rather than restoring an older,
    /// unrelated entry (UX.md M3).
    pub ended_mux_window: Option<crate::tmux::TmuxWindowId>,
    /// Set when the entry records a mux tab HIDDEN by its last-pane close
    /// (UX.md M4): the daemon window kept running and the tab stayed in the
    /// manager. Cmd+Z re-shows it; there is nothing to restore.
    pub hidden_mux_window: Option<HiddenMuxTab>,
}

/// The re-show target of a [`ClosedTabInfo::hidden_mux_window`] entry: the
/// still-live local tab and the daemon window it mirrors.
pub(crate) struct HiddenMuxTab {
    pub tab_id: crate::tab::TabId,
    pub window_id: crate::tmux::TmuxWindowId,
}
