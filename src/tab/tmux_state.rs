//! Tmux-related state for a terminal tab.
//!
//! Groups all fields related to tmux gateway mode and pane identity.

/// Tmux-related state for a terminal tab.
#[derive(Default)]
pub(crate) struct TabTmuxState {
    /// Whether this tab is in tmux gateway mode
    pub(crate) tmux_gateway_active: bool,
    /// The tmux pane ID this tab represents (when in gateway mode)
    pub(crate) tmux_pane_id: Option<crate::tmux::TmuxPaneId>,
    /// When true, a deferred call to `set_tmux_control_mode(false)` is pending.
    ///
    /// Set when `handle_tmux_session_ended` could not acquire the terminal lock via
    /// `try_lock()`. The notification poll loop retries on each subsequent frame until
    /// the lock is available, ensuring the terminal parser exits tmux control mode even
    /// if the lock was transiently held at cleanup time.
    pub(crate) pending_tmux_mode_disable: bool,
}
