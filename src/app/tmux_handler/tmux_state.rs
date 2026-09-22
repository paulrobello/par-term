//! tmux integration state for a window.
//!
//! Groups the fields that manage tmux control-mode connectivity: the session
//! handle, sync manager, pane-ID mappings, and prefix-key state machine.

use crate::pane::PaneId;
use crate::tab::TabId;
use crate::tmux::{PrefixKey, PrefixState, TmuxPaneId, TmuxSession, TmuxSync};

/// A control-mode transport: where notifications come from and where
/// commands go.
///
/// The gateway implementation is the `tmux -CC` process behind the gateway
/// tab's PTY (written directly by the gateway session); the par-mux
/// implementation (`mux` feature, `notifications::mux`) drives a par-mux
/// daemon client. Both speak the same core notification type, which is
/// what keeps the `notifications/` consumers transport-agnostic.
#[cfg_attr(not(feature = "mux"), allow(dead_code))]
pub(crate) trait TmuxTransport {
    /// Drain pending core notifications. The flag is set when the source
    /// died without a graceful `%exit` (daemon killed abruptly) — callers
    /// surface that as `SessionEnded`.
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    );
    /// Run one control-mode command, returning the reply block body.
    fn send_command(&self, command: &str) -> std::io::Result<Vec<String>>;
}

/// tmux integration state.
pub(crate) struct TmuxState {
    /// tmux control mode session (if connected)
    pub(crate) tmux_session: Option<TmuxSession>,
    /// Pluggable control-mode transport (a par-mux daemon client under the
    /// `mux` feature; the gateway session owns its own PTY path)
    pub(crate) transport: Option<Box<dyn TmuxTransport>>,
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
    /// Focused tmux pane for par-mux input routing (the gateway session
    /// tracks its own; the transport has nowhere else to keep it)
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) mux_focused_pane: Option<TmuxPaneId>,
    /// Replayed screens awaiting their pane mapping (reattach seeding:
    /// `refresh-client -t` replies, applied once the layout consumers
    /// create the panes)
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) mux_screen_seeds: std::collections::HashMap<TmuxPaneId, Vec<u8>>,
    /// Cached par-mux agent roster (A2b task 1): the single owner of
    /// agent state on the app side — filled by `list-agents` on
    /// attach/reattach, updated by `%agent-state-changed` pushes, read by
    /// every roster surface. Exists only under the `mux` feature because
    /// its types live in the feature-gated client crate.
    #[cfg(feature = "mux")]
    pub(crate) agent_roster: super::notifications::agent_roster::AgentRoster,
    /// In-flight profile-open attach: a worker thread runs the daemon
    /// connect/spawn (the core retries the socket for up to 10s — far too
    /// long to block the event loop), and `poll_mux_attach` completes the
    /// attach on the main thread each frame. Exists only under the `mux`
    /// feature like the transport types it carries.
    #[cfg(feature = "mux")]
    pub(crate) mux_attach_pending: Option<super::notifications::mux::MuxAttachPending>,
    /// Mapping from tmux pane IDs to native pane IDs for output routing
    pub(crate) tmux_pane_to_native_pane: std::collections::HashMap<TmuxPaneId, PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs for input routing
    pub(crate) native_pane_to_tmux_pane: std::collections::HashMap<PaneId, TmuxPaneId>,
}

impl TmuxState {
    pub(crate) fn new(tmux_prefix_key: Option<PrefixKey>) -> Self {
        Self {
            tmux_session: None,
            transport: None,
            tmux_sync: TmuxSync::new(),
            tmux_session_name: None,
            tmux_gateway_tab_id: None,
            tmux_prefix_key,
            tmux_prefix_state: PrefixState::new(),
            mux_focused_pane: None,
            mux_screen_seeds: std::collections::HashMap::new(),
            #[cfg(feature = "mux")]
            agent_roster: super::notifications::agent_roster::AgentRoster::new(),
            #[cfg(feature = "mux")]
            mux_attach_pending: None,
            tmux_pane_to_native_pane: std::collections::HashMap::new(),
            native_pane_to_tmux_pane: std::collections::HashMap::new(),
        }
    }
}
