//! tmux integration state for a window.
//!
//! Groups the fields that manage tmux control-mode connectivity: the session
//! handle, sync manager, pane-ID mappings, and prefix-key state machine.

use crate::pane::PaneId;
use crate::tab::TabId;
use crate::tmux::{PrefixKey, TmuxPaneId, TmuxSession, TmuxSync};

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
    /// Run one read-only query, returning within `budget` (a `TimedOut`
    /// error past it) whatever command is already in flight. The default
    /// is [`TmuxTransport::send_command`]; the par-mux transport asks on a
    /// connection of its own, since replies on the attached one arrive in
    /// order behind any slow command already sent.
    fn query_bounded(
        &self,
        command: &str,
        _budget: std::time::Duration,
    ) -> std::io::Result<Vec<String>> {
        self.send_command(command)
    }
    /// Fire-and-forget form of [`TmuxTransport::send_command`] for
    /// commands whose reply the caller never reads (keystrokes, size
    /// pushes, pastes). The default runs the synchronous call; the par-mux
    /// transport queues on its send worker so a hung daemon cannot stall
    /// the event loop waiting for a reply.
    fn send_command_no_wait(&self, command: &str) -> std::io::Result<()> {
        self.send_command(command).map(|_| ())
    }
    /// A daemon-liveness transition to surface as a toast: `Some(message)`
    /// when the daemon just became unresponsive (or just recovered);
    /// `None` every other frame, and always for transports without a
    /// daemon behind them.
    fn daemon_health_event(&self) -> Option<String> {
        None
    }
    /// Queue `job` to run off the event loop; its result comes back through
    /// [`TmuxTransport::take_job_results`]. `false` when the transport has
    /// no worker to run it (test doubles, a worker already gone, a full
    /// queue) — the caller then skips the work.
    #[cfg(feature = "mux")]
    fn submit_job(&self, _job: MuxJob) -> bool {
        false
    }
    /// Results of jobs finished since the last call, in completion order.
    #[cfg(feature = "mux")]
    fn take_job_results(&self) -> Vec<MuxJobResult> {
        Vec::new()
    }
}

/// Daemon round-trips the par-mux send worker runs off the event loop —
/// the queries a window that appeared mid-session needs before its panes
/// can map and show their screens. A hung daemon holds each for the core
/// client's reply timeout, which must never land on the winit thread.
#[cfg(feature = "mux")]
pub(crate) enum MuxJob {
    /// `list-panes` + one `pane-info` per pane: which panes `window` holds.
    DiscoverWindow(crate::tmux::TmuxWindowId),
    /// One `refresh-client -t %N` per pane: each pane's current screen.
    SeedPanes(Vec<TmuxPaneId>),
}

/// What a [`MuxJob`] learned, applied on the event loop.
#[cfg(feature = "mux")]
pub(crate) enum MuxJobResult {
    /// The window's panes with their grid sizes, in daemon order.
    WindowPanes {
        window: crate::tmux::TmuxWindowId,
        panes: Vec<(TmuxPaneId, (u16, u16))>,
    },
    /// Each pane's `refresh-client` reply.
    PaneSeeds(Vec<(TmuxPaneId, std::io::Result<Vec<String>>)>),
}

/// A delayed mux paste in flight: the chunks still to send (each line
/// already ends in `\r`; the bracketed end sequence trails the last
/// chunk), the per-chunk delay, and when the next chunk is due. The
/// daemon-side analogue of `TerminalManager::paste_with_delay`.
#[cfg(feature = "mux")]
pub(crate) struct PendingMuxPaste {
    pub(crate) pane: TmuxPaneId,
    pub(crate) chunks: std::collections::VecDeque<Vec<u8>>,
    pub(crate) delay: std::time::Duration,
    pub(crate) next_due: std::time::Instant,
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
    /// Parsed prefix key from config (cached for performance). In a tmux
    /// gateway tab it arms the leader (UX.md K4).
    pub(crate) tmux_prefix_key: Option<PrefixKey>,
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
    /// Daemon panes whose process exited and are HELD daemon-side
    /// (`%pane-exited %N [code]`), keyed by the daemon pane id so the
    /// state follows the pane through break/join. The value is the exit
    /// code (`None` = signal death or unreadable). `%pane-respawned`
    /// removes the entry; the exited-pane overlay and the restart key
    /// read it. Lives for the session like `mux_pane_titles`.
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) mux_exited_panes: std::collections::HashMap<TmuxPaneId, Option<i32>>,
    /// Daemon panes with a screen seed requested off-loop and not yet
    /// landed, counted per outstanding request. While a pane is here its
    /// live output waits in `mux_held_output`: the seed is a clear plus a
    /// snapshot, so output applied before it lands would be erased.
    #[cfg(feature = "mux")]
    pub(crate) mux_seeds_in_flight: std::collections::HashMap<TmuxPaneId, u32>,
    /// Live output held behind an in-flight seed, appended to the seed
    /// when it lands so it replays after the snapshot, in arrival order.
    #[cfg(feature = "mux")]
    pub(crate) mux_held_output: std::collections::HashMap<TmuxPaneId, Vec<u8>>,
    /// Off-loop window discoveries (`MuxJob::DiscoverWindow`) not yet
    /// applied — keeps the event loop polling until they land.
    #[cfg(feature = "mux")]
    pub(crate) mux_discoveries_in_flight: u32,
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
    /// The session-restore placeholder tab of a mux-attached window.
    /// Restore spawns one local shell tab (the `vec![None]` shape) so a
    /// failed attach never leaves an empty window; the first daemon
    /// window tab created by `handle_tmux_window_add` closes it so the
    /// restored window holds only daemon tabs. `None` outside the
    /// restore→attach window — profile-open attaches own no placeholder.
    pub(crate) mux_restore_placeholder_tab: Option<TabId>,
    /// A mux paste being fed to the daemon line-by-line under the
    /// configured `paste_delay_ms` (see `paste_via_tmux`). Interior
    /// mutability because the paste entry takes `&WindowState` while the
    /// drain that empties the queue takes `&mut`.
    #[cfg(feature = "mux")]
    pub(crate) pending_mux_paste: std::cell::RefCell<Option<PendingMuxPaste>>,
    /// Per-tab pane mappings keyed by the daemon-unique tmux pane id.
    /// Native pane ids restart at 1 in every tab's PaneManager, so a flat
    /// window-wide map lets two tabs' pane 1 collide — input, output, and
    /// layout reconciliation then cross between daemon windows. The owning
    /// tab travels with the pane id.
    pub(crate) tmux_pane_owners: std::collections::HashMap<TmuxPaneId, (TabId, PaneId)>,
    /// The daemon (socket stem) serving the attached par-mux session.
    /// Restore reattaches by name through `par-mux-<name>.sock`, so a
    /// session whose daemon has another name is not persisted (it would
    /// come back as an empty namesake in a new daemon).
    pub(crate) mux_daemon: Option<String>,
    /// Daemon health for the session chip (UX.md V1), updated from each
    /// `daemon_health_event` transition and the attach's version check.
    pub(crate) mux_health: crate::session_chip::MuxHealth,
    /// The last par-mux error, shown on the session chip until the user
    /// dismisses it (UX.md M12). Session teardown deliberately leaves it:
    /// "connection lost" must still be readable after the view ends.
    pub(crate) mux_last_error: Option<String>,
    /// The one par-mux error toast on screen (`record_mux_error`): a new
    /// par-mux error replaces it, and the daemon's "responding again"
    /// retires it.
    pub(crate) mux_error_toast: Option<u64>,
    /// The by-name attach worker reports which daemon it chose here, with
    /// that daemon's socket (`attach_mux_session_by_name`);
    /// `poll_mux_attach` reads it.
    #[cfg(feature = "mux")]
    pub(crate) mux_daemon_rx: Option<std::sync::mpsc::Receiver<(String, std::path::PathBuf)>>,
    /// The socket the in-flight attach connects through. `poll_mux_attach`
    /// hands it to the transport, whose bounded queries (the close path's
    /// running-job check) open their own connection there.
    #[cfg(feature = "mux")]
    pub(crate) mux_attach_socket: Option<std::path::PathBuf>,
    /// The last finished par-mux session directory scan (UX.md A16/A22):
    /// what the session picker and the palette's attach rows list.
    #[cfg(feature = "mux")]
    pub(crate) mux_directory: Option<super::notifications::mux_directory::MuxDirectory>,
    /// A directory scan in flight.
    #[cfg(feature = "mux")]
    pub(crate) mux_directory_scan: Option<super::notifications::mux_directory::PendingScan>,
    /// Test seam: scan and attach sockets here instead of the per-user
    /// default directory, so tests never touch the user's live daemons.
    #[cfg(feature = "mux")]
    pub(crate) mux_socket_dir_override: Option<std::path::PathBuf>,
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
            mux_focused_pane: None,
            mux_session_id: None,
            mux_screen_seeds: std::collections::HashMap::new(),
            mux_pane_titles: std::collections::HashMap::new(),
            mux_exited_panes: std::collections::HashMap::new(),
            #[cfg(feature = "mux")]
            mux_seeds_in_flight: std::collections::HashMap::new(),
            #[cfg(feature = "mux")]
            mux_held_output: std::collections::HashMap::new(),
            #[cfg(feature = "mux")]
            mux_discoveries_in_flight: 0,
            #[cfg(feature = "mux")]
            agent_roster: super::notifications::agent_roster::AgentRoster::new(),
            #[cfg(feature = "mux")]
            mux_attach_pending: None,
            mux_restore_placeholder_tab: None,
            #[cfg(feature = "mux")]
            pending_mux_paste: std::cell::RefCell::new(None),
            tmux_pane_owners: std::collections::HashMap::new(),
            mux_daemon: None,
            mux_health: crate::session_chip::MuxHealth::Connected,
            mux_last_error: None,
            mux_error_toast: None,
            #[cfg(feature = "mux")]
            mux_daemon_rx: None,
            #[cfg(feature = "mux")]
            mux_attach_socket: None,
            #[cfg(feature = "mux")]
            mux_directory: None,
            #[cfg(feature = "mux")]
            mux_directory_scan: None,
            #[cfg(feature = "mux")]
            mux_socket_dir_override: None,
        }
    }

    /// Drop off-loop seed bookkeeping with the view it belonged to (detach,
    /// session end): a seed landing afterwards finds no count and is
    /// discarded, and held output never outlives its session.
    #[cfg(feature = "mux")]
    pub(crate) fn clear_mux_seed_holds(&mut self) {
        self.mux_seeds_in_flight.clear();
        self.mux_held_output.clear();
        self.mux_discoveries_in_flight = 0;
    }

    /// Whether off-loop daemon work is outstanding: the event loop must
    /// keep polling to apply its result, even while the daemon is quiet.
    #[cfg(feature = "mux")]
    pub(crate) fn mux_jobs_in_flight(&self) -> bool {
        self.transport.is_some()
            && (!self.mux_seeds_in_flight.is_empty() || self.mux_discoveries_in_flight > 0)
    }

    /// UX.md T8: whether the attached control session is par-mux. Only
    /// the par-mux attach installs a `transport` (the real-tmux gateway
    /// does not), so a set transport means user-facing strings must say
    /// "par-mux", never "tmux". During a teardown the transport may
    /// already be gone — pair with `mux_session_id` there (see
    /// `handle_tmux_session_ended`).
    pub(crate) fn is_mux_attached(&self) -> bool {
        self.transport.is_some()
    }

    /// The attached session name for persistence, split by kind:
    /// `(tmux_session_name, mux_session_name)`. Only the par-mux attach
    /// installs a `transport`, so a set transport means the name belongs to
    /// the daemon — persisting it as a tmux name made the next launch
    /// restore it through the tmux gateway, spawning a real `tmux -CC`
    /// session of the same name instead of reattaching to the daemon.
    pub(crate) fn persisted_session_names(&self) -> (Option<String>, Option<String>) {
        match (&self.tmux_session_name, self.transport.is_some()) {
            // A session served by a daemon named otherwise cannot be found
            // again by name: restoring it would spawn a new daemon holding
            // an empty session of the same name.
            (Some(name), true) if self.mux_daemon.as_ref().is_some_and(|d| d != name) => {
                (None, None)
            }
            (Some(name), true) => {
                // An emptied session must not come back on restore: an
                // empty sync table at save time means every window was
                // closed and unmapped. On a daemon that holds exited panes
                // (core 0.57+) exiting the shells no longer empties it —
                // held windows stay mapped and restore as fresh shells. Persisting the name would
                // make the next launch create-or-attach to the emptied
                // session and hand back a fresh window the user
                // deliberately closed (observed live 2026-09-26).
                if !self.tmux_sync.has_windows() {
                    return (None, None);
                }
                (None, Some(name.clone()))
            }
            (name, false) => (name.clone(), None),
            (None, true) => (None, None),
        }
    }

    /// Replace `tab_id`'s pane mappings, keeping every other tab's entries
    /// intact. Each arriving layout describes one daemon window; replacing
    /// the whole table (the flat-map behavior) unmapped every other
    /// window's panes.
    pub(crate) fn set_tab_pane_mappings(
        &mut self,
        tab_id: TabId,
        mappings: &std::collections::HashMap<TmuxPaneId, PaneId>,
    ) {
        self.tmux_pane_owners
            .retain(|_, (owner, _)| *owner != tab_id);
        self.tmux_pane_owners.extend(
            mappings
                .iter()
                .map(|(&tmux, &native)| (tmux, (tab_id, native))),
        );
    }

    /// The owning tab and native pane for a daemon pane.
    pub(crate) fn tmux_pane_owner(&self, tmux_id: TmuxPaneId) -> Option<(TabId, PaneId)> {
        self.tmux_pane_owners.get(&tmux_id).copied()
    }

    /// The daemon pane displayed by `native_id` inside `tab_id`. Input
    /// routing resolves pane ids hit-tested in the active tab, so the
    /// (tab, pane) pair — not the pane id alone — identifies the daemon
    /// pane.
    pub(crate) fn tmux_pane_in_tab(&self, tab_id: TabId, native_id: PaneId) -> Option<TmuxPaneId> {
        self.tmux_pane_owners
            .iter()
            .find(|(_, (owner, pane))| *owner == tab_id && *pane == native_id)
            .map(|(&tmux, _)| tmux)
    }

    /// Whether `tab_id` mirrors daemon panes and every one of them is held
    /// (its process exited). `false` for a tab with no mapped panes: an
    /// empty set proves nothing about the daemon window behind it.
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) fn all_tab_panes_held(&self, tab_id: TabId) -> bool {
        let panes = self.tab_tmux_pane_ids(tab_id);
        !panes.is_empty() && panes.iter().all(|p| self.mux_exited_panes.contains_key(p))
    }

    /// `tab_id`'s daemon panes — the existing set for layout delta
    /// reconciliation, which must not see other windows' panes.
    pub(crate) fn tab_tmux_pane_ids(&self, tab_id: TabId) -> std::collections::HashSet<TmuxPaneId> {
        self.tmux_pane_owners
            .iter()
            .filter(|(_, (owner, _))| *owner == tab_id)
            .map(|(&tmux, _)| tmux)
            .collect()
    }

    /// `tab_id`'s tmux→native mappings, materialized for the pane
    /// manager's layout calls.
    pub(crate) fn tab_mappings(
        &self,
        tab_id: TabId,
    ) -> std::collections::HashMap<TmuxPaneId, PaneId> {
        self.tmux_pane_owners
            .iter()
            .filter(|(_, (owner, _))| *owner == tab_id)
            .map(|(&tmux, &(_, native))| (tmux, native))
            .collect()
    }

    /// Drop a closed pane's mapping.
    pub(crate) fn remove_tmux_pane_mapping(&mut self, tmux_id: TmuxPaneId) {
        self.tmux_pane_owners.remove(&tmux_id);
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
