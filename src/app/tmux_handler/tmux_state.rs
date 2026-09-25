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
    /// The attached par-mux session's id (`$N`), the target of
    /// `set-environment`. `None` while no mux session is attached.
    pub(crate) mux_session_id: Option<u64>,
    /// Replayed screens awaiting their pane mapping (reattach seeding:
    /// `refresh-client -t` replies, applied once the layout consumers
    /// create the panes)
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) mux_screen_seeds: std::collections::HashMap<TmuxPaneId, Vec<u8>>,
    /// Daemon pane titles awaiting their pane mapping (reattach title
    /// restore + pushes that arrived before the layout consumer created
    /// the native pane), applied by the same end-of-poll sweep as the
    /// seeds. `(title, is_user)`: a user `-T` title re-marks the pane
    /// user-named; an OSC-sourced title is just the initial title.
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) mux_pane_titles: std::collections::HashMap<TmuxPaneId, (String, bool)>,
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
            mux_session_id: None,
            mux_screen_seeds: std::collections::HashMap::new(),
            mux_pane_titles: std::collections::HashMap::new(),
            #[cfg(feature = "mux")]
            agent_roster: super::notifications::agent_roster::AgentRoster::new(),
            #[cfg(feature = "mux")]
            mux_attach_pending: None,
            tmux_pane_to_native_pane: std::collections::HashMap::new(),
            native_pane_to_tmux_pane: std::collections::HashMap::new(),
        }
    }

    /// The attached session name for persistence, split by kind:
    /// `(tmux_session_name, mux_session_name)`. Only the par-mux attach
    /// installs a `transport`, so a set transport means the name belongs to
    /// the daemon — persisting it as a tmux name made the next launch
    /// restore it through the tmux gateway, spawning a real `tmux -CC`
    /// session of the same name instead of reattaching to the daemon.
    pub(crate) fn persisted_session_names(&self) -> (Option<String>, Option<String>) {
        match (&self.tmux_session_name, self.transport.is_some()) {
            (Some(name), true) => (None, Some(name.clone())),
            (name, false) => (name.clone(), None),
            (None, true) => (None, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without a mux transport the attached name is a tmux gateway session
    /// and persists as one — the real-tmux restore path is unchanged.
    #[test]
    fn a_gateway_session_name_persists_as_tmux() {
        let mut state = TmuxState::new(None);
        assert_eq!(state.persisted_session_names(), (None, None));
        state.tmux_session_name = Some("work".to_string());
        assert_eq!(
            state.persisted_session_names(),
            (Some("work".to_string()), None)
        );
    }
}
