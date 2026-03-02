//! tmux integration state for a window.
//!
//! Groups the fields that manage tmux control-mode connectivity: the session
//! handle, sync manager, pane-ID mappings, and prefix-key state machine.

use crate::pane::PaneId;
use crate::tab::TabId;
use crate::tmux::{PrefixKey, PrefixState, TmuxPaneId, TmuxSession, TmuxSync};

/// tmux integration state.
pub(crate) struct TmuxState {
    /// tmux control mode session (if connected)
    pub(crate) tmux_session: Option<TmuxSession>,
    /// tmux state synchronization manager
    pub(crate) tmux_sync: TmuxSync,
    /// Current tmux session name (for window title display)
    pub(crate) tmux_session_name: Option<String>,
    /// Tab ID where the tmux gateway connection lives (where we write commands)
    pub(crate) tmux_gateway_tab_id: Option<TabId>,
    /// Parsed prefix key from config (cached for performance)
    pub(crate) tmux_prefix_key: Option<PrefixKey>,
    /// Prefix key state (whether we're waiting for command key)
    pub(crate) tmux_prefix_state: PrefixState,
    /// Mapping from tmux pane IDs to native pane IDs for output routing
    pub(crate) tmux_pane_to_native_pane: std::collections::HashMap<TmuxPaneId, PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs for input routing
    pub(crate) native_pane_to_tmux_pane: std::collections::HashMap<PaneId, TmuxPaneId>,
}

impl TmuxState {
    pub(crate) fn new(tmux_prefix_key: Option<PrefixKey>) -> Self {
        Self {
            tmux_session: None,
            tmux_sync: TmuxSync::new(),
            tmux_session_name: None,
            tmux_gateway_tab_id: None,
            tmux_prefix_key,
            tmux_prefix_state: PrefixState::new(),
            tmux_pane_to_native_pane: std::collections::HashMap::new(),
            native_pane_to_tmux_pane: std::collections::HashMap::new(),
        }
    }
}
