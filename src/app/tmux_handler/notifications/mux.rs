//! par-mux app wiring (T4.4/T4.5): the control-mode transport over a
//! par-mux daemon, reusing the tmux notification consumers unchanged.
//!
//! The daemon client yields the same core notifications a `tmux -CC`
//! gateway does, so everything downstream of the drain — `ParserBridge`,
//! the grouped `TmuxSync` dispatch, and every handler in this directory —
//! is the tmux path's code used as-is. What lives here is only what the
//! design doc says adapts: the transport itself, the session-started
//! wiring (client size through the daemon, not `set-option` plus gateway
//! PTY writes), reattach resync, and the detach UX — dropping the socket
//! leaves the daemon and its sessions running (the D5 promise, made
//! user-visible in the toasts and window title).
//!
//! Compiled only under the `mux` feature (declared empty while the core's
//! `mux` module is unpublished); `scripts/with-local-core.sh` extends the
//! feature for local runs.

use crate::app::tmux_handler::tmux_state::{TmuxState, TmuxTransport};
use crate::app::window_state::WindowState;
use crate::tmux::{ParserBridge, TmuxNotification, escape_keys_for_tmux};
use par_term_mux::{
    AgentEntry, AttachOutcome, MuxSessionClient, VersionCheck, check_daemon_version,
};
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::cell::{RefCell, RefMut};
use std::io;

/// The daemon transport: a par-mux client behind the [`TmuxTransport`]
/// seam. Interior mutability because the routing hooks that reach it
/// (`send_input_via_tmux`, `notify_tmux_of_resize`) hold `&WindowState`.
pub(crate) struct MuxTransport {
    client: RefCell<MuxSessionClient>,
}

impl MuxTransport {
    /// Connect to a daemon at `path`, spawning one when no live server
    /// owns it (losing the spawn race talks to the winner's daemon).
    /// Test entry: the app runtime reaches the daemon through
    /// [`Self::connect_or_spawn`] by session name; the wiring test binds
    /// an in-process server at an explicit path.
    #[cfg(test)]
    pub(crate) fn connect_or_spawn_at(path: &std::path::Path) -> io::Result<Self> {
        Ok(Self {
            client: RefCell::new(MuxSessionClient::connect_or_spawn_at(path)?),
        })
    }

    /// The wrapped client. Borrows are short-lived and single-threaded
    /// (the event loop owns `WindowState`); never held across another
    /// borrow.
    pub(crate) fn client(&self) -> RefMut<'_, MuxSessionClient> {
        self.client.borrow_mut()
    }
}

impl TmuxTransport for MuxTransport {
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
        self.client().drain_core_notifications()
    }

    fn send_command(&self, command: &str) -> io::Result<Vec<String>> {
        self.client().send(command)
    }
}

/// The grid size pushed to the daemon for a par-mux tab: the renderer's
/// grid minus the scrollbar strip (see `mux_scrollbar_reserved_cols`).
pub(crate) fn mux_client_grid(renderer: &par_term_render::renderer::Renderer) -> (u16, u16) {
    let (cols, rows) = renderer.grid_size();
    let reserved = crate::app::tmux_handler::mux_scrollbar_reserved_cols(
        renderer.scrollbar_width(),
        renderer.cell_width(),
    );
    (cols.saturating_sub(reserved).max(1) as u16, rows as u16)
}

/// Push the renderer's grid size so the daemon re-fits the window holding
/// `pane` — `refresh-client -t %N -C` broadcasts `%layout-change` with the
/// new geometry (core T4.C; the `-t` target is required by the server).
pub(crate) fn push_client_size(
    transport: &dyn TmuxTransport,
    pane: Option<TmuxPaneId>,
    cols: u16,
    rows: u16,
) {
    let Some(pane) = pane else {
        crate::debug_trace!(
            "MUX",
            "client size push skipped — no focused pane to target"
        );
        return;
    };
    if let Err(e) = transport.send_command(&format!("refresh-client -t %{pane} -C {cols}x{rows}")) {
        crate::debug_error!("MUX", "client size push failed: {e}");
    }
}

/// Route input bytes to the daemon as tmux key names — the same
/// `escape_keys_for_tmux` form the gateway sends, targeted at the focused
/// pane when known. Always consumes once a transport is attached: the
/// panes live in the daemon, so falling through to a PTY write would go
/// nowhere. An unknown target DROPS the input with a visible error — the
/// daemon rejects untargeted `send-keys` (`send-keys requires -t`), so
/// sending it would fail invisibly anyway.
pub(crate) fn route_input(
    transport: &dyn TmuxTransport,
    focused: Option<TmuxPaneId>,
    data: &[u8],
) -> bool {
    let Some(pane) = focused else {
        crate::debug_error!("MUX", "input dropped — no focused pane to target");
        return true;
    };
    let escaped = escape_keys_for_tmux(data);
    let command = format!("send-keys -t %{pane} {escaped}");
    if let Err(e) = transport.send_command(&command) {
        crate::debug_error!("MUX", "send-keys failed: {e}");
    }
    true
}

/// Route raw bytes as hex (`-H`) — the literal-bytes form used where
/// key-name translation would mangle the payload (e.g. Shift+Enter).
pub(crate) fn route_literal_bytes(
    transport: &dyn TmuxTransport,
    focused: Option<TmuxPaneId>,
    bytes: &[u8],
) -> bool {
    let Some(pane) = focused else {
        crate::debug_trace!("MUX", "literal send-keys with no focused pane — dropped");
        return true;
    };
    let hex = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if let Err(e) = transport.send_command(&format!("send-keys -t %{pane} -H {hex}")) {
        crate::debug_error!("MUX", "send-keys -H failed: {e}");
    }
    true
}

/// The attach sequence minus tab allocation, as a free function so the
/// wiring test can drive it against a live daemon without a
/// `WindowState`: query the daemon's build (the stale-daemon check —
/// first, because a daemon that predates the client is the cheapest
/// explanation for everything downstream misbehaving), create-or-attach,
/// report existing windows (the caller allocates tabs for them — a created
/// session gets its tab from the `%window-add` push), push the client size
/// (the first pane names the window the server resizes), collect each
/// pane's replayed screen for seeding (`refresh-client -t` replies carry
/// the screen; they are NOT `%output` pushes), and read the agent roster
/// for the initial fill (A2b task 1: `list-agents` on attach and
/// reattach — the single call site of the roster query in app code).
pub(crate) fn attach_sequence(
    transport: &MuxTransport,
    name: &str,
    size: Option<(u16, u16)>,
) -> io::Result<AttachSequence> {
    // Degrade to None rather than failing the attach: the version query is
    // diagnostic, and a daemon that cannot answer it is handled by the
    // mismatch check, not by refusing to attach.
    let daemon_version = transport.client().daemon_version().ok();
    let outcome = transport.client().create_or_attach(name)?;
    let mut existing_windows = Vec::new();
    if matches!(outcome, AttachOutcome::Attached(_)) {
        for window in transport.client().list_windows()? {
            existing_windows.push(window.id);
        }
    }
    // Bind the list before looping: a RefMut in the `for` expression would
    // live through the body, where the next command borrows again.
    let panes = transport.client().list_panes()?;
    if let (Some((cols, rows)), Some(first)) = (size, panes.first()) {
        transport.client().set_client_size(*first, cols, rows)?;
    }
    let mut screens = Vec::new();
    for pane in panes {
        let reply = transport.client().refresh_pane(pane)?;
        // The clear is part of the seed: the client pane may already hold
        // stale content (bytes that arrived between the daemon's snapshot
        // and delivery), and the replay must define the baseline rather
        // than paint over it.
        let mut bytes = b"\x1b[H\x1b[2J".to_vec();
        bytes.extend_from_slice(&reply.join("\n").into_bytes());
        screens.push((pane, bytes));
    }
    // The roster fill degrades to empty rather than failing the attach: a
    // query that cannot run leaves the panes absent from the roster (they
    // render as nothing), which is the honest reading — a failed attach
    // would drop the whole session.
    let agents = transport.client().list_agents().unwrap_or_else(|e| {
        crate::debug_error!("MUX", "list-agents roster fill failed: {e}");
        Vec::new()
    });
    Ok(AttachSequence {
        daemon_version,
        outcome,
        existing_windows,
        screens,
        agents,
    })
}

/// What [`attach_sequence`] learned — the tuple it returned, named: the
/// daemon's `version` reply (raw; `None` only when the query itself failed
/// at transport level), the attach outcome, windows needing tabs,
/// per-pane replayed screens, and the roster fill.
pub(crate) struct AttachSequence {
    pub(crate) daemon_version: Option<String>,
    pub(crate) outcome: AttachOutcome,
    pub(crate) existing_windows: Vec<TmuxWindowId>,
    pub(crate) screens: Vec<(TmuxPaneId, Vec<u8>)>,
    pub(crate) agents: Vec<AgentEntry>,
}

impl TmuxState {
    /// Runtime palette rows for the attached par-mux session — the explicit
    /// detach affordance, present only while a transport is installed so
    /// the palette never offers a dead action (the roster-picker pattern:
    /// runtime rows joined at open time, not dispatch-table built-ins).
    ///
    /// Lives on `TmuxState` (like the roster's `palette_rows`) rather than
    /// `WindowState` so the egui open path's closure captures only this
    /// field — a `&self` method call on the whole `WindowState` would
    /// capture `*self` and collide with the render closure's mutable use.
    pub(crate) fn mux_palette_rows(&self) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        if self.transport.is_none() {
            return Vec::new();
        }
        let label = match &self.tmux_session_name {
            Some(name) => format!("Detach par-mux Session '{name}' (keeps running)"),
            None => "Detach par-mux Session (keeps running)".to_string(),
        };
        vec![crate::command_palette::catalog::PaletteEntry {
            action_id: "mux-detach".to_string(),
            label,
            chord: None,
            priority: 0,
        }]
    }
}

/// An in-flight profile-open attach: a worker thread owns the daemon
/// connect/spawn (the core retries the daemon socket for up to 10s — far
/// too long to hold the event loop), reports the outcome over the channel,
/// and [`WindowState::poll_mux_attach`] consumes it on the main thread.
pub(crate) struct MuxAttachPending {
    name: String,
    rx: std::sync::mpsc::Receiver<io::Result<par_term_emu_core_rust::mux::MuxClient>>,
}

impl WindowState {
    /// Split the focused par-mux pane daemon-side: targeted `split-window`
    /// via the transport (the command the tmux gateway writes to its PTY),
    /// then let the %layout-change consumer create the native pane and
    /// mapping — the same flow the gateway split path relies on. Returns
    /// false when no transport is attached or the command fails, so
    /// callers fall through rather than leaving a daemon/local mismatch.
    /// On success the reply's new pane id becomes the focused pane, so
    /// subsequent input lands in the freshly split pane.
    pub(crate) fn split_pane_via_mux(&mut self, vertical: bool) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        // The daemon REQUIRES -t on split-window (untargeted is a wire
        // error), and mux_focused_pane is unset after a fresh attach until
        // a click or focus push — fall back to the focused native pane,
        // exactly as input routing does. No mux pane focused (a local tab)
        // returns false so the caller proceeds with a local split.
        let Some(target) = self.focused_mux_pane_from_native() else {
            crate::debug_trace!("MUX", "split skipped — no mux pane focused (local tab?)");
            return false;
        };
        // tmux's -h is a side-by-side split (par-term "vertical"); -v stacks.
        let flag = if vertical { "-h" } else { "-v" };
        let cmd = format!("split-window {flag} -t %{target}");
        match transport.send_command(&cmd) {
            Ok(reply) => {
                if let Some(id) = reply
                    .iter()
                    .find_map(|line| line.trim().strip_prefix('%').and_then(|s| s.parse().ok()))
                {
                    self.tmux_state.mux_focused_pane = Some(id);
                    true
                } else {
                    // The daemon rejects bad splits with an %error block,
                    // which arrives as an Ok reply body — a missing pane id
                    // IS the failure signal. Consume (a daemon pane was the
                    // target; a local split would strand it) and surface it.
                    let body = reply.join("\n");
                    log::error!("par-mux split-window rejected: {body}");
                    self.show_toast(format!("par-mux: split failed — {body}"));
                    true
                }
            }
            Err(e) => {
                log::error!("par-mux split-window failed: {e}");
                self.show_toast(format!("par-mux: split failed — {e}"));
                false
            }
        }
    }

    /// Close the focused par-mux pane daemon-side — the mirror of
    /// [`Self::split_pane_via_mux`]: targeted `kill-pane` via the
    /// transport, then the daemon's `%layout-change` broadcast drives the
    /// layout consumer's removal path (`handle_pane_removal`) exactly as
    /// the gateway close flow does. Returns true when the close was
    /// consumed here (a daemon pane was the target — a native close would
    /// delete the local pane and dangle the tmux→native mapping while the
    /// daemon pane lives on); false when no transport is attached or no
    /// mux pane is focused, so the caller falls through to the local
    /// close.
    ///
    /// Unlike the split path, a TRANSPORT-level failure still consumes: a
    /// resolved daemon mapping means the pane's identity is owned
    /// daemon-side, so locally closing it on a dead connection races the
    /// SessionEnded teardown and can strand the mapping either way.
    pub(crate) fn close_pane_via_mux(&mut self) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        // Same resolution as input routing and the split path: mux focus
        // when set, else the focused native pane's mapping.
        let Some(target) = self.focused_mux_pane_from_native() else {
            crate::debug_trace!("MUX", "close skipped — no mux pane focused (local tab?)");
            return false;
        };
        let cmd = format!("kill-pane -t %{target}");
        let result = transport.send_command(&cmd);
        let ok = result.as_ref().map(|body| body.is_empty()).unwrap_or(false);
        match result {
            // kill-pane's success reply is an empty body; the daemon
            // rejects bad targets (a window's last pane) with an %error
            // block, which also arrives as an Ok body — non-empty IS the
            // failure signal, same shape as the split path.
            Ok(body) if !ok => {
                let text = body.join("\n");
                log::error!("par-mux kill-pane rejected: {text}");
                self.show_toast(format!("par-mux: close failed — {text}"));
                true
            }
            Ok(_) => true,
            Err(e) => {
                log::error!("par-mux kill-pane failed: {e}");
                self.show_toast(format!("par-mux: close failed — {e}"));
                true
            }
        }
    }

    /// Begin the profile-open attach for `name` WITHOUT blocking the event
    /// loop: the daemon connect/spawn runs on a worker thread and
    /// [`Self::poll_mux_attach`] finishes the attach on the main thread. A
    /// second request while one is already in flight is ignored.
    pub(crate) fn begin_mux_session_attach(&mut self, name: &str) {
        if self.tmux_state.transport.is_some() || self.tmux_state.mux_attach_pending.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_name = name.to_string();
        match std::thread::Builder::new()
            .name("mux-attach".into())
            .spawn(move || {
                let client = par_term_emu_core_rust::mux::MuxClient::connect_or_spawn(&worker_name);
                let _ = tx.send(client);
            }) {
            Ok(_handle) => {
                self.tmux_state.mux_attach_pending = Some(MuxAttachPending {
                    name: name.to_string(),
                    rx,
                });
            }
            Err(e) => {
                log::error!("mux attach worker could not start: {e}");
                self.show_toast("par-mux: attach failed (worker thread unavailable)");
            }
        }
    }

    /// Finish an in-flight profile-open attach on the main thread. The
    /// still-connecting case restores the pending state for later frames;
    /// failure toasts visibly — the old path only wrote a DEBUG_LEVEL-gated
    /// log line, so a failed attach looked like nothing happened.
    pub(crate) fn poll_mux_attach(&mut self) {
        let Some(pending) = self.tmux_state.mux_attach_pending.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.tmux_state.mux_attach_pending = Some(pending);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::error!("mux attach worker died without reporting a result");
                self.show_toast("par-mux: attach failed (worker died)");
            }
            Ok(Err(e)) => {
                log::error!("par-mux attach to '{}' failed: {e}", pending.name);
                self.show_toast(format!(
                    "par-mux: attach to '{}' failed — {e}",
                    pending.name
                ));
            }
            Ok(Ok(client)) => {
                let transport = MuxTransport {
                    client: RefCell::new(MuxSessionClient::from_core(client)),
                };
                if let Err(e) = self.install_mux_transport(&pending.name, transport) {
                    log::error!("par-mux attach to '{}' failed: {e}", pending.name);
                    self.show_toast(format!(
                        "par-mux: attach to '{}' failed — {e}",
                        pending.name
                    ));
                }
            }
        }
    }

    /// The attach tail shared by the sync and worker-thread entry points:
    /// run the attach sequence against a connected transport, install it,
    /// and give the user the toast + window-title feedback.
    fn install_mux_transport(
        &mut self,
        name: &str,
        transport: MuxTransport,
    ) -> io::Result<AttachOutcome> {
        // Run the attach sequence with the concrete local so
        // `handle_tmux_window_add` can borrow the window state freely;
        // boxing into tmux_state happens only once attached.
        let size = self.renderer.as_ref().map(mux_client_grid);
        let attach = attach_sequence(&transport, name, size).map(
            |AttachSequence {
                 daemon_version,
                 outcome,
                 existing_windows,
                 screens,
                 agents,
             }| {
                // The stale-daemon check: the daemon outlives clients, so
                // this attach may have landed on one built before the
                // client's core — every daemon-side fix would then read as
                // "didn't work". Surfaced, never silent; a daemon so old it
                // cannot answer `version` is itself the mismatch.
                let client_stamp = par_term_emu_core_rust::mux::build_stamp();
                match check_daemon_version(daemon_version.as_deref().unwrap_or(""), client_stamp) {
                    VersionCheck::Match | VersionCheck::Unknown => {}
                    VersionCheck::Mismatch { daemon, client } => {
                        log::warn!(
                            "par-mux daemon older than the client — daemon {daemon}, \
                             client {client}; daemon-side fixes are missing until the \
                             daemon is restarted (pkill -f par-mux)"
                        );
                        self.show_toast(format!(
                            "par-mux: daemon older than this client ({daemon} vs {client}) \
                             — restart it (pkill -f par-mux) to pick up daemon fixes"
                        ));
                    }
                }
                for window_id in existing_windows {
                    if self.tmux_state.tmux_sync.get_tab(window_id).is_none() {
                        self.handle_tmux_window_add(window_id);
                    }
                }
                // Screens land once the layout consumers create the panes
                // (checked each poll in check_mux_notifications).
                self.tmux_state.mux_screen_seeds = screens
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>();
                self.tmux_state.agent_roster.fill_from_list(agents);
                outcome
            },
        );
        match attach {
            Ok(outcome) => {
                self.tmux_state.transport = Some(Box::new(transport));
                self.tmux_state.mux_focused_pane = None;
                self.tmux_state.tmux_session_name = Some(name.to_string());
                self.tmux_state.tmux_sync.enable();
                let verb = match &outcome {
                    AttachOutcome::Created(_) => "created",
                    AttachOutcome::Attached(_) => "attached to",
                };
                self.show_toast(format!(
                    "par-mux: {verb} session '{name}' (sessions survive detach)"
                ));
                self.update_window_title_for_mux();
                Ok(outcome)
            }
            Err(e) => Err(e),
        }
    }

    /// Detach from the par-mux session explicitly: drop the transport (the
    /// socket drop IS the detach — the daemon and its sessions keep
    /// running, the D5 promise), then run the shared session-ended cleanup
    /// so the display tabs, pane mappings, sync state, and window title
    /// tear down exactly as they do when the daemon dies. Returns `false`
    /// when no transport is attached.
    ///
    /// The palette's runtime `mux-detach` row dispatches here; closing the
    /// window remains the implicit detach path.
    pub(crate) fn detach_mux_session(&mut self) -> bool {
        if self.tmux_state.transport.take().is_none() {
            return false;
        }
        // Mux-only state the shared cleanup below does not know about.
        self.tmux_state.mux_focused_pane = None;
        self.tmux_state.mux_screen_seeds.clear();
        self.tmux_state.agent_roster.clear();
        self.handle_tmux_session_ended();
        // Overwrite the shared cleanup's "tmux: Session ended" toast: the
        // session did not end, it survives in the daemon.
        self.show_toast("par-mux: detached (session keeps running in the daemon)");
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        true
    }

    /// The adapted session-started wiring: no gateway tab to retitle and
    /// no `set-option window-size` (the daemon's policy is
    /// latest-report-wins, core T4.C) — just the name, the title, and the
    /// client size push.
    fn handle_mux_session_started(&mut self, session_name: &str) {
        crate::debug_info!("MUX", "Session started: {session_name}");
        self.tmux_state.tmux_session_name = Some(session_name.to_string());
        self.update_window_title_for_mux();
        self.tmux_state.tmux_sync.enable();
        if let Some(renderer) = &self.renderer
            && let Some(transport) = &self.tmux_state.transport
        {
            let (cols, rows) = mux_client_grid(renderer);
            push_client_size(
                &**transport,
                self.focused_mux_pane_from_native(),
                cols,
                rows,
            );
        }
    }

    /// Update the window title with the par-mux session name
    /// (`window_title - [mux: name]`) — the mux analogue of
    /// `update_window_title_with_tmux`.
    fn update_window_title_for_mux(&self) {
        let title = match &self.tmux_state.tmux_session_name {
            Some(name) => format!("{} - [mux: {}]", self.config.load().window_title, name),
            None => self.config.load().window_title.clone(),
        };
        let formatted = self.format_title(&title);
        self.with_window(|w| w.set_title(&formatted));
    }

    /// Apply `%agent-state-changed` pushes to the roster cache. Returns
    /// whether any push landed (a roster surface may need to re-render).
    ///
    /// Extracted from [`Self::check_mux_notifications`] so the manners rule
    /// (REPORT.md E3: an agent needing attention joins a list and the
    /// indicator glows — it never steals focus) is testable without a live
    /// daemon. Everything this does beyond `AgentRoster::apply_push` is
    /// return the redraw flag; introducing a focus call, window raise,
    /// notification, or palette open here is the interruption E3 forbids,
    /// and the test below is the alarm.
    pub(super) fn apply_agent_pushes(
        &mut self,
        pushes: Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
    ) -> bool {
        let mut needs_redraw = false;
        for push in pushes {
            if let par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged {
                pane_id,
                agent,
                state,
                source,
            } = push
            {
                match AgentEntry::from_push(&pane_id, &agent, &state, &source) {
                    Some(entry) => {
                        // Explicit field reads (reason included) keep the
                        // entry honest in the log while the wire cannot
                        // carry a reason yet.
                        crate::debug_info!(
                            "MUX",
                            "agent roster push: %{} {} {} source={:?} reason={:?}",
                            entry.pane,
                            entry.agent,
                            entry.state,
                            entry.source,
                            entry.reason
                        );
                        self.tmux_state.agent_roster.apply_push(entry);
                        // Roster surfaces (A2b tasks 2/3) render from this
                        // cache, so a push is a potential visual change.
                        needs_redraw = true;
                    }
                    None => crate::debug_log!(
                        "MUX",
                        "dropped unattributed agent push: {pane_id} {agent} {state}"
                    ),
                }
            }
        }
        needs_redraw
    }

    /// Drain the par-mux transport and dispatch through the same grouped
    /// consumer path as `check_tmux_notifications`: session/window
    /// structure before layout before output. Called from the shared poll
    /// loop while a transport is installed.
    pub(super) fn check_mux_notifications(&mut self) -> bool {
        let (core_notifications, disconnected) = match &self.tmux_state.transport {
            Some(transport) => transport.drain(),
            None => return false,
        };
        if disconnected {
            // The daemon died without `%exit`. Drop the dead socket; the
            // synthesized `SessionEnded` below then runs the shared
            // end-of-session cleanup exactly once.
            let _ = self.tmux_state.transport.take();
            self.tmux_state.mux_focused_pane = None;
        }
        if core_notifications.is_empty() && !disconnected {
            self.apply_pending_mux_screen_seeds();
            return false;
        }

        // The roster push is a core variant the ParserBridge deliberately
        // drops (a named arm there cannot compile against the published
        // pin), so partition it out here — the roster cache is its
        // consumer. This destructure is the single push call site.
        let (agent_pushes, core_notifications): (Vec<_>, Vec<_>) =
            core_notifications.into_iter().partition(|n| {
                matches!(
                    n,
                    par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged { .. }
                )
            });

        let mut notifications = ParserBridge::convert_all(core_notifications);
        if disconnected
            && !notifications
                .iter()
                .any(|n| matches!(n, TmuxNotification::SessionEnded))
        {
            notifications.push(TmuxNotification::SessionEnded);
        }

        // The roster must not outlive its daemon: clear it on an abrupt
        // death (the flag) or a graceful end (the notification), before
        // any surface reads a ghost.
        if disconnected
            || notifications
                .iter()
                .any(|n| matches!(n, TmuxNotification::SessionEnded))
        {
            self.tmux_state.agent_roster.clear();
        }

        crate::debug_info!("MUX", "Processing {} notifications", notifications.len());

        let mut needs_redraw = self.apply_agent_pushes(agent_pushes);

        // Same bucket split as polling.rs — direct handlers TmuxSync cannot
        // translate, then the sync groups in dependency order.
        let mut direct_notifications = Vec::new();
        let mut session_sync = Vec::new();
        let mut layout_sync = Vec::new();
        let mut output_sync = Vec::new();
        let mut other_sync = Vec::new();

        for notification in notifications {
            match &notification {
                TmuxNotification::ControlModeStarted
                | TmuxNotification::SessionStarted(_)
                | TmuxNotification::SessionRenamed(_)
                | TmuxNotification::PaneFocusChanged { .. }
                | TmuxNotification::Error(_) => {
                    direct_notifications.push(notification);
                }
                TmuxNotification::WindowAdd(_)
                | TmuxNotification::WindowClose(_)
                | TmuxNotification::WindowRenamed { .. }
                | TmuxNotification::SessionEnded => {
                    session_sync.push(notification);
                }
                TmuxNotification::LayoutChange { .. } => {
                    layout_sync.push(notification);
                }
                TmuxNotification::Output { .. } => {
                    output_sync.push(notification);
                }
                TmuxNotification::Pause | TmuxNotification::Continue => {
                    other_sync.push(notification);
                }
            }
        }

        // --- Direct dispatch (notifications TmuxSync does not handle) ---
        // Focus pushes are deferred until after the layout groups: a split's
        // `%window-pane-changed` for the NEW pane arrives in the same batch
        // as the `%layout-change` that creates it, so applying it here (the
        // pane not yet mapped) lost it, and native focus — the only routing
        // source of truth — stayed on the old pane.
        let mut deferred_focus: Option<TmuxPaneId> = None;
        for notification in direct_notifications {
            match notification {
                TmuxNotification::SessionStarted(session_name) => {
                    self.handle_mux_session_started(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::SessionRenamed(session_name) => {
                    self.handle_tmux_session_renamed(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::PaneFocusChanged { pane_id } => {
                    deferred_focus = Some(pane_id);
                    needs_redraw = true;
                }
                TmuxNotification::Error(msg) => {
                    self.handle_tmux_error(&msg);
                }
                TmuxNotification::ControlModeStarted => {
                    crate::debug_info!("MUX", "Control mode started");
                }
                _ => {}
            }
        }

        // --- TmuxSync dispatch: group 1 — session/window structure ---
        let session_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&session_sync);
        needs_redraw |= self.process_sync_actions(session_actions);

        // --- TmuxSync dispatch: group 2 — layout changes ---
        let layout_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&layout_sync);
        needs_redraw |= self.process_sync_actions(layout_actions);

        // Fallback: layouts for windows not yet mapped (on-the-fly mapping).
        for notification in &layout_sync {
            if let TmuxNotification::LayoutChange { window_id, layout } = notification
                && self.tmux_state.tmux_sync.get_tab(*window_id).is_none()
            {
                self.handle_tmux_layout_change(*window_id, layout);
                needs_redraw = true;
            }
        }

        // Deferred focus push: every pane this batch created is mapped now.
        if let Some(pane_id) = deferred_focus {
            self.tmux_state.mux_focused_pane = Some(pane_id);
            self.handle_tmux_pane_focus_changed(pane_id);
        }

        // --- TmuxSync dispatch: group 3 — pane output ---
        let output_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&output_sync);
        needs_redraw |= self.process_sync_actions(output_actions);

        // Fallback: output for panes not yet mapped.
        for notification in output_sync {
            if let TmuxNotification::Output { pane_id, data } = notification
                && self.tmux_state.tmux_sync.get_native_pane(pane_id).is_none()
            {
                self.handle_tmux_output(pane_id, &data);
                needs_redraw = true;
            }
        }

        // --- TmuxSync dispatch: group 4 — flow control (pause/continue) ---
        let other_actions = self.tmux_state.tmux_sync.process_notifications(&other_sync);
        needs_redraw |= self.process_sync_actions(other_actions);

        self.apply_pending_mux_screen_seeds();

        needs_redraw
    }

    /// Feed replayed screens to panes once their mappings exist — panes are
    /// created by the layout consumers on a later poll, so seeds from
    /// `attach_sequence` wait here until `get_native_pane` resolves.
    /// Delivery is the same `process_data` call the PaneOutput consumer
    /// uses; a seed is consumed only once delivered (a locked terminal
    /// retries next frame).
    fn apply_pending_mux_screen_seeds(&mut self) {
        if self.tmux_state.mux_screen_seeds.is_empty() {
            return;
        }
        let ready: Vec<TmuxPaneId> = self.tmux_state.mux_screen_seeds.keys().copied().collect();
        for pane in ready {
            self.deliver_pending_mux_seed(pane);
        }
    }

    /// Feed one pane's pending reattach seed to its mapped native pane —
    /// the single delivery site, called from the end-of-poll sweep
    /// ([`Self::apply_pending_mux_screen_seeds`]) and, before newer live
    /// output, from `handle_tmux_output`. A seed is consumed only once
    /// delivered; a locked terminal retries on the next attempt (the same
    /// try_lock discipline as the PaneOutput consumer).
    ///
    /// The native pane resolves through the SAME lookup output routing
    /// uses — app map first, sync map as fallback. The layout consumers
    /// populate only `tmux_pane_to_native_pane`; resolving through
    /// `tmux_sync.get_native_pane` alone (an earlier form) always missed in
    /// production because nothing in the app populates the sync map, so
    /// every seed sat pending forever and panes reattached blank.
    pub(super) fn deliver_pending_mux_seed(&mut self, tmux_pane: TmuxPaneId) -> bool {
        let Some(data) = self.tmux_state.mux_screen_seeds.get(&tmux_pane).cloned() else {
            return false;
        };
        let Some(native) = self
            .tmux_state
            .tmux_pane_to_native_pane
            .get(&tmux_pane)
            .copied()
            .or_else(|| self.tmux_state.tmux_sync.get_native_pane(tmux_pane))
        else {
            return false;
        };
        for tab in self.tab_manager.tabs_mut() {
            if let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane_obj) = pane_manager.get_pane_mut(native)
                && let Ok(term) = pane_obj.terminal.try_read()
            {
                term.process_data(&data);
                self.tmux_state.mux_screen_seeds.remove(&tmux_pane);
                crate::debug_info!(
                    "MUX",
                    "delivered pending mux seed before live output for %{}",
                    tmux_pane
                );
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::TmuxSync;
    use par_term_emu_core_rust::mux::MuxServer;
    use par_term_tmux::SyncAction;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// Marker echoed inside a pane before detach; the reattach screen
    /// replay must carry it back as `PaneOutput`.
    const MARKER: &str = "par-term-mux-wiring-marker";

    fn socket_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "par-term-mux-wiring-{}-{tag}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// In-process daemon (the compat.rs pattern): bind and serve on a
    /// thread; `run()` serves until process end.
    fn spawn_daemon(path: &Path) {
        let server = MuxServer::bind(path).expect("daemon binds");
        std::thread::spawn(move || server.run());
    }

    fn connect(path: &Path) -> MuxTransport {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match MuxTransport::connect_or_spawn_at(path) {
                Ok(transport) => return transport,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => panic!("daemon never accepted a connection: {e}"),
            }
        }
    }

    /// The three calls `check_mux_notifications` makes per poll — drain,
    /// convert, sync — so the actions asserted here are the ones the app
    /// dispatch would feed to the `notifications/` consumers.
    fn poll_actions(transport: &MuxTransport, sync: &mut TmuxSync) -> Vec<SyncAction> {
        let (core_notes, _) = transport.drain();
        let frontend = ParserBridge::convert_all(core_notes);
        sync.process_notifications(&frontend)
    }

    fn wait_for(
        transport: &MuxTransport,
        sync: &mut TmuxSync,
        wanted: impl Fn(&SyncAction) -> bool,
    ) -> Vec<SyncAction> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            let actions = poll_actions(transport, sync);
            let hit = actions.iter().any(&wanted);
            seen.extend(actions);
            if hit {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("expected action never arrived within 10s; saw: {seen:?}");
    }

    /// Map every daemon pane to a native pane id (`10_000 + tmux id`) —
    /// the app's reattach adoption step, mirrored so unmapped panes drop
    /// no output.
    fn adopt_panes(transport: &MuxTransport, sync: &mut TmuxSync) {
        for pane in transport.client().list_panes().expect("list-panes") {
            sync.map_pane(pane, 10_000 + pane);
        }
    }

    #[test]
    fn reattach_drives_layout_and_screens_from_live_sync_actions() {
        let path = socket_path("reattach");
        spawn_daemon(&path);

        // First client: create the session, split a pane, and leave a
        // marker on screen, then drop the connection. The daemon and its
        // session must outlive it (D5).
        {
            let mut first = MuxSessionClient::connect(&path).expect("first client connects");
            let outcome = first.create_or_attach("wiring").expect("create");
            assert!(matches!(outcome, AttachOutcome::Created(_)));

            // Wait for the session's initial window before splitting.
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut created = false;
            while Instant::now() < deadline && !created {
                created = first
                    .poll_actions()
                    .iter()
                    .any(|a| matches!(a, SyncAction::CreateTab { .. }));
                if !created {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
            assert!(created, "first client never saw the session's window");

            first.send("split-window -t %0 -h").expect("split-window");
            first
                .send_keys(0, format!("echo {MARKER}\n").as_bytes())
                .expect("send-keys marker");
            // Give the shell a moment to run the echo before the drop.
            std::thread::sleep(Duration::from_millis(300));
        }

        // Reattach through the app's attach sequence: the transport, the
        // create-or-attach, the window list, the client size report, and
        // the per-pane screen replay — everything a profile-open attach
        // runs short of tab allocation. The size differs from the daemon's
        // 80x24 default so the `-C` refit genuinely broadcasts a layout.
        let transport = connect(&path);
        let attach =
            attach_sequence(&transport, "wiring", Some((120, 40))).expect("attach_sequence");
        assert!(
            matches!(attach.outcome, AttachOutcome::Attached(ref s) if s.name == "wiring"),
            "reattached to the persisted session: {:?}",
            attach.outcome
        );
        assert_eq!(
            attach.existing_windows.len(),
            1,
            "the surviving window is reported"
        );
        assert!(
            attach
                .screens
                .iter()
                .any(|(_, data)| String::from_utf8_lossy(data).contains(MARKER)),
            "the screen replay carries the pre-detach marker: {:?}",
            attach.screens
        );

        // The app-side contract: allocate a tab for the window and map it.
        let mut sync = TmuxSync::new();
        sync.enable();
        sync.map_window(attach.existing_windows[0], 100);
        adopt_panes(&transport, &mut sync);

        // The `-C` refit broadcast arrives as an UpdateLayout action — the
        // action the app dispatch hands to the unchanged layout consumer.
        wait_for(&transport, &mut sync, |a| {
            matches!(a, SyncAction::UpdateLayout { tab_id: 100, .. })
        });

        let _ = std::fs::remove_file(&path);
    }

    /// The stale-daemon check's live wiring: attach queries the daemon's
    /// `version`, and against an in-process daemon (the same crate build on
    /// both sides of the socket) the comparison must land on `Match` — the
    /// positive control for the toast path, which fires only on `Mismatch`.
    #[test]
    fn attach_reads_the_daemon_version_and_a_current_daemon_matches() {
        let path = socket_path("version");
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_sequence(&transport, "version-check", Some((80, 24))).expect("attach");
        let daemon_reply = attach
            .daemon_version
            .expect("the daemon answered `version` during the attach sequence");
        let client_stamp = par_term_emu_core_rust::mux::build_stamp();
        assert_eq!(
            check_daemon_version(&daemon_reply, client_stamp),
            VersionCheck::Match,
            "same-build daemon/client must compare clean: daemon replied \
             {daemon_reply:?}, client stamp {client_stamp:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dropping_the_transport_leaves_sessions_alive() {
        let path = socket_path("survive");
        spawn_daemon(&path);

        {
            let transport = connect(&path);
            let attach = attach_sequence(&transport, "keep", Some((80, 24))).expect("attach");
            assert!(matches!(attach.outcome, AttachOutcome::Created(_)));
            // Dropping the transport drops the socket — detach, D5.
        }

        let second = connect(&path);
        let attach = attach_sequence(&second, "keep", None).expect("reattach");
        assert!(
            matches!(attach.outcome, AttachOutcome::Attached(ref s) if s.name == "keep"),
            "the daemon and session survived the dropped socket: {:?}",
            attach.outcome
        );
        assert_eq!(
            second.client().list_panes().expect("list-panes").len(),
            1,
            "the surviving window still holds its pane"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The detach affordance's row gating: the palette row exists only while
    /// a transport is installed, and its label names the attached session.
    /// A failed worker attach must surface as a visible toast (the old
    /// path only wrote a DEBUG_LEVEL-gated log line) and clear the pending
    /// attach so a later profile open can try again.
    #[test]
    fn mux_attach_failure_toasts_and_clears_pending() {
        let mut ws = manners_state();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Err(io::Error::other("daemon unreachable")))
            .unwrap();
        drop(tx);

        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "test".to_string(),
            rx,
        });
        ws.poll_mux_attach();

        assert!(
            ws.tmux_state.mux_attach_pending.is_none(),
            "a reported failure must clear the pending attach"
        );
        assert!(ws.tmux_state.transport.is_none());
        assert!(
            ws.tmux_state.tmux_session_name.is_none(),
            "a failed attach must not claim a session name"
        );
        let toast = ws
            .overlay_state
            .toast_message
            .as_deref()
            .unwrap_or_default();
        assert!(
            toast.contains("attach to 'test' failed"),
            "the toast must name the failure, got: {toast}"
        );
    }

    /// While the worker is still connecting, the pending attach must stay
    /// queued (no toast, no state change) for later frames to re-poll.
    #[test]
    fn mux_attach_pending_waits_for_the_worker() {
        let mut ws = manners_state();
        let (_tx, rx) =
            std::sync::mpsc::channel::<io::Result<par_term_emu_core_rust::mux::MuxClient>>();

        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "slow".to_string(),
            rx,
        });
        ws.poll_mux_attach();

        assert!(
            ws.tmux_state.mux_attach_pending.is_some(),
            "no worker result yet — the pending attach must stay queued"
        );
        assert!(ws.overlay_state.toast_message.is_none());
    }

    /// The success arm: a connected core client delivered over the channel
    /// installs the transport, sets the session name, and toasts with the
    /// created/attached verb — everything the profile-open eye-check
    /// expects to see.
    #[test]
    fn mux_attach_success_installs_transport_and_toasts() {
        let path = socket_path("attach-poll");
        spawn_daemon(&path);

        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);

        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "pollme".to_string(),
            rx,
        });
        ws.poll_mux_attach();

        assert!(
            ws.tmux_state.transport.is_some(),
            "the transport must be installed on success"
        );
        assert_eq!(ws.tmux_state.tmux_session_name.as_deref(), Some("pollme"));
        let toast = ws
            .overlay_state
            .toast_message
            .as_deref()
            .unwrap_or_default();
        assert!(
            toast.contains("attached to session 'pollme'")
                || toast.contains("created session 'pollme'"),
            "the toast must announce the attach, got: {toast}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A session name the daemon's whitespace-split wire grammar cannot
    /// carry (spaces, quotes, backslash) must be rejected CLIENT-SIDE with
    /// a clear error and must NOT create a mangled session on the daemon —
    /// `Par Mux Test` used to arrive as `-s Par` and create `Par`.
    #[test]
    fn create_or_attach_rejects_names_the_wire_grammar_cannot_carry() {
        let path = socket_path("spaced-name");
        spawn_daemon(&path);

        let mut client = par_term_mux::MuxSessionClient::connect_or_spawn_at(&path)
            .expect("connect to test daemon");
        let err = client
            .create_or_attach("Par Mux Test")
            .expect_err("a spaced name must be rejected before the wire");
        assert!(
            err.to_string().contains("cannot contain spaces"),
            "the error must tell the user the naming rule, got: {err}"
        );

        // No garbage on the daemon: neither the full name nor its first
        // word may exist as a session.
        let sessions = client.list_sessions().expect("list sessions");
        assert!(
            sessions.iter().all(|s| s.name != "Par"),
            "the whitespace split must not have created a truncated session"
        );

        // A name the grammar CAN carry still round-trips create → attach.
        let created = client
            .create_or_attach("ParMux-Test")
            .expect("create with a wire-safe name");
        assert!(matches!(created, AttachOutcome::Created(_)));
        let mut second = par_term_mux::MuxSessionClient::connect(&path).expect("second client");
        let attached = second
            .create_or_attach("ParMux-Test")
            .expect("reattach with a wire-safe name");
        assert!(matches!(attached, AttachOutcome::Attached(_)));

        let _ = std::fs::remove_file(&path);
    }

    /// With a transport attached, the split keybinding must split
    /// DAEMON-side: the daemon grows to two panes, the reply's new pane id
    /// becomes the focused pane, and NO native pane is created (a native
    /// split here produces a local shell the mux input router then
    /// starves — measured live 2026-09-22: keystrokes executed in the
    /// daemon's %0 pane while the split-local pane froze at its seed).
    #[test]
    fn split_pane_via_mux_splits_daemon_side_and_moves_focus() {
        let path = socket_path("mux-split");
        spawn_daemon(&path);

        // Attach through the app's own install path (poll arm).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "splitme".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        // Focus comes from the focused NATIVE pane (the authoritative
        // source): create the window's tab and map the daemon pane as the
        // layout consumer does — a created session's %window-add fired
        // before the transport existed, so pumping would never map it.
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_to_native_pane.contains_key(&0) {
            if Instant::now() >= deadline {
                // Push a layout for window @0 so the consumer maps %0.
                break;
            }
            ws.tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("refresh-client -t %0 -C 80x24")
                .expect("size push broadcasts %layout-change");
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            ws.tmux_state.tmux_pane_to_native_pane.contains_key(&0),
            "the layout consumer never mapped %0"
        );

        assert!(
            ws.split_pane_via_mux(true),
            "daemon-side split must succeed"
        );

        // The daemon grew to two panes — the split went over the wire.
        let mut probe = par_term_mux::MuxSessionClient::connect(&path).expect("probe client");
        let panes = probe.list_panes().expect("list panes");
        assert_eq!(
            panes.len(),
            2,
            "the daemon must gain the split pane, got {panes:?}"
        );

        // Focus followed the reply's new pane id so input lands there.
        assert_eq!(
            ws.tmux_state.mux_focused_pane,
            Some(1),
            "focus must move to the reply's new pane id"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Closing a focused mux pane goes daemon-side (`kill-pane -t %N`): the
    /// daemon's pane count drops, the killed pane's tmux→native mapping is
    /// removed by the %layout-change reconciliation (no dangling mapping),
    /// and with no transport attached the close falls through untouched.
    #[test]
    fn close_pane_via_mux_kills_daemon_side_and_reconciles_the_mapping() {
        // No transport: the arm must decline so the native close path is
        // reached unchanged.
        let mut bare = manners_state();
        assert!(
            !bare.close_pane_via_mux(),
            "no transport attached — close_pane_via_mux must fall through"
        );

        let path = socket_path("mux-close");
        spawn_daemon(&path);

        // Attach through the app's install path and map %0 (the split
        // test's ladder: %window-add, then size pushes until mapped).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "closeme".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_to_native_pane.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "the layout consumer never mapped %0"
            );
            ws.tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("refresh-client -t %0 -C 80x24")
                .expect("size push broadcasts %layout-change");
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            ws.split_pane_via_mux(true),
            "split gives the window a second pane to close"
        );
        // The split pane's mapping arrives with the %layout-change push,
        // not the command reply — pump until the consumer has mapped %1.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_to_native_pane.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the layout consumer never mapped the split pane %1: {:?}",
                ws.tmux_state.tmux_pane_to_native_pane
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }

        // Close the focused pane (%1, the split's new pane) daemon-side.
        assert!(
            ws.close_pane_via_mux(),
            "a focused mux pane close must be consumed daemon-side"
        );

        // The daemon's pane count dropped — the kill went over the wire.
        let mut probe = par_term_mux::MuxSessionClient::connect(&path).expect("probe client");
        let panes = probe.list_panes().expect("list panes");
        assert_eq!(
            panes.len(),
            1,
            "the daemon must have dropped the killed pane, got {panes:?}"
        );

        // The %layout-change reconciliation removed the killed pane's
        // mapping — no dangling tmux→native entry for a dead daemon pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        while ws.tmux_state.tmux_pane_to_native_pane.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the killed pane's mapping was never reconciled away: {:?}",
                ws.tmux_state.tmux_pane_to_native_pane
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            ws.tmux_state.tmux_pane_to_native_pane.contains_key(&0),
            "the surviving pane's mapping must remain"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mux_palette_rows_track_the_attached_transport() {
        let mut ws = manners_state();
        assert!(
            ws.tmux_state.mux_palette_rows().is_empty(),
            "no transport installed — the palette must not offer a dead detach row"
        );

        let path = socket_path("palette-row");
        spawn_daemon(&path);
        let transport = connect(&path);
        let attach = attach_sequence(&transport, "rows", Some((80, 24))).expect("attach");
        assert!(matches!(attach.outcome, AttachOutcome::Created(_)));
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("rows".to_string());

        let rows = ws.tmux_state.mux_palette_rows();
        assert_eq!(rows.len(), 1, "exactly one detach row while attached");
        assert_eq!(rows[0].action_id, "mux-detach");
        assert!(
            rows[0].label.contains("rows"),
            "the label names the attached session: {}",
            rows[0].label
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The explicit detach: local mux state tears down through the shared
    /// session-ended cleanup, the toast says "detached" (not "Session
    /// ended"), and — the D5 promise, exercised through the app's own
    /// detach path this time — the daemon and session survive the dropped
    /// socket for a fresh client to reattach.
    #[test]
    fn detach_mux_session_tears_down_local_state_and_the_daemon_survives() {
        let path = socket_path("detach");
        spawn_daemon(&path);

        let mut ws = manners_state();
        // Detach with nothing attached is a no-op, not a crash.
        assert!(!ws.detach_mux_session());
        assert!(
            ws.overlay_state.toast_message.is_none(),
            "a no-op detach must not toast"
        );

        let transport = connect(&path);
        let attach = attach_sequence(&transport, "det", Some((80, 24))).expect("attach");
        assert!(matches!(attach.outcome, AttachOutcome::Created(_)));
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("det".to_string());
        ws.tmux_state.mux_focused_pane = Some(0);
        ws.tmux_state.mux_screen_seeds.insert(0, b"seed".to_vec());
        ws.tmux_state.agent_roster.fill_from_list(vec![AgentEntry {
            pane: 0,
            agent: "claude".to_string(),
            state: "idle".to_string(),
            source: par_term_mux::AgentSource::Hook,
            reason: None,
        }]);

        assert!(ws.detach_mux_session());
        assert!(
            ws.tmux_state.transport.is_none(),
            "the socket is dropped — that drop IS the detach"
        );
        assert_eq!(ws.tmux_state.mux_focused_pane, None);
        assert!(ws.tmux_state.mux_screen_seeds.is_empty());
        assert_eq!(ws.tmux_state.agent_roster.iter().count(), 0);
        assert_eq!(ws.tmux_state.tmux_session_name, None);
        assert!(ws.tmux_state.tmux_pane_to_native_pane.is_empty());
        assert_eq!(
            ws.overlay_state.toast_message.as_deref(),
            Some("par-mux: detached (session keeps running in the daemon)"),
            "the detach toast must replace the shared cleanup's 'Session ended'"
        );

        // D5 through the app's own detach: the daemon outlives the dropped
        // socket and a fresh client reattaches to the same session.
        let second = connect(&path);
        let reattach = attach_sequence(&second, "det", None).expect("reattach");
        assert!(
            matches!(reattach.outcome, AttachOutcome::Attached(ref s) if s.name == "det"),
            "the daemon survived the explicit detach: {:?}",
            reattach.outcome
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fresh_daemon_yields_an_empty_roster() {
        let path = socket_path("roster");
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_sequence(&transport, "roster", Some((80, 24))).expect("attach");
        // Absent means absent: a daemon whose panes have no agent reports
        // rostered nothing — the fill is empty, not idle-populated.
        assert!(
            attach.agents.is_empty(),
            "no hook has reported, so no pane is rostered: {:?}",
            attach.agents
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A full-screen TUI's reattach seed and live redraws must both render
    /// the real screen in a client emulator fed exactly as the app feeds
    /// its pane terminals: the seed through `process_data` at the pane's
    /// size, live `%output` payloads through `process` on the same
    /// terminal.
    ///
    /// Root-cause probe for the blank/partial TUI render card: the emitter
    /// alt-screens, paints a bordered grid, then pushes cursor-addressed
    /// incremental updates. If this fails, the transport or the seed is
    /// dropping real screen content, not the app renderer.
    #[test]
    fn tui_replay_and_live_output_render_in_a_client_emulator() {
        let path = socket_path("tui-live");
        spawn_daemon(&path);

        // Client A creates the session and starts a TUI emitter in its pane.
        {
            let mut first = MuxSessionClient::connect(&path).expect("first client");
            first.create_or_attach("tui").expect("create");
            std::thread::sleep(Duration::from_millis(300));

            // Alt-screen, bordered grid, then cursor-addressed single-cell
            // updates. Plain ANSI, no terminfo dependency.
            let script_path =
                std::env::temp_dir().join(format!("par-term-tui-probe-{}.py", std::process::id()));
            std::fs::write(
                &script_path,
                r##"import sys, time
out = sys.stdout
out.write("\x1b[?1049h\x1b[2J\x1b[H")
out.flush()
out.write("\x1b[41m")
out.flush()
for r in range(24):
    line = "".join("#" if r in (0, 23) or c in (0, 79) else "o" for c in range(80))
    out.write("\x1b[%d;1H%s" % (r + 1, line))
out.write("\x1b[0m")
out.flush()
for i in range(40):
    time.sleep(0.15)
    out.write("\x1b[12;40H%d" % (i % 10))
    out.flush()
out.write("\x1b[?1049l")
out.flush()
"##,
            )
            .expect("write emitter script");
            first
                .send_keys_literal(0, &format!("python3 {}", script_path.display()))
                .expect("send command");
            first.send_keys(0, b"\r").expect("send enter");

            // Wait for the TUI to actually paint (a fixed sleep raced shell
            // startup under load: the seed caught only the typed command).
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let screen = first
                    .send("capture-pane -t %0 -p")
                    .expect("capture-pane")
                    .join("\n");
                if screen.contains("####") {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the TUI never painted: {screen:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        } // client A drops — detach; the TUI keeps running daemon-side

        // The app's exact attach: create-or-attach, size push, per-pane replay.
        let transport = connect(&path);
        let attach = attach_sequence(&transport, "tui", Some((80, 24))).expect("attach_sequence");

        let seed = attach
            .screens
            .iter()
            .find(|(pane, _)| *pane == 0)
            .map(|(_, data)| data.clone())
            .expect("seed for %0");

        // The replay must carry the TUI's alt-screen grid — frame and fill.
        let seed_text = String::from_utf8_lossy(&seed);
        assert!(
            seed_text.contains("####"),
            "the seed carries the TUI frame: {seed_text:?}"
        );
        assert!(
            seed_text.contains("ooo"),
            "the seed carries the TUI fill: {seed_text:?}"
        );

        // The client emulator: fresh core Terminal at the pane's size, fed
        // the seed exactly as `apply_pending_mux_screen_seeds` feeds it.
        let mut term = par_term_emu_core_rust::terminal::Terminal::with_scrollback(80, 24, 1000);
        term.process(&seed);

        let after_seed = term.content();
        assert!(
            after_seed.contains("####") && after_seed.contains("ooo"),
            "the seed renders the TUI screen after emulator processing: {after_seed:?}"
        );

        // Attribute fidelity: the fill was painted on a red background;
        // the styled replay must restore it (the old plain-text seed
        // dropped every color — the "partial restore" symptom).
        let fill_bg = term.active_grid().get(40, 5).map(|c| c.bg());
        assert!(
            fill_bg
                == Some(par_term_emu_core_rust::color::Color::Named(
                    par_term_emu_core_rust::color::NamedColor::Red,
                )),
            "the replayed seed must restore the fill's background color: {fill_bg:?}"
        );

        // Live output drains into the same terminal — cursor-addressed
        // incremental updates must land on the seeded screen without a
        // full redraw.
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_update = false;
        while Instant::now() < deadline && !saw_update {
            let (notes, _) = transport.drain();
            for note in notes {
                if let par_term_emu_core_rust::tmux_control::TmuxNotification::Output {
                    pane_id,
                    data,
                } = note
                    && pane_id == "%0"
                {
                    term.process(&data);
                }
            }
            saw_update = term
                .content()
                .lines()
                .any(|line| line.as_bytes().get(39).is_some_and(|b| b.is_ascii_digit()));
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            saw_update,
            "an incremental TUI update must land on the seeded baseline: {:?}",
            term.content()
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The app-layer reattach repro: a running TUI, a detach, then a
    /// reattach driven through the REAL `WindowState` path — tab creation
    /// (`handle_tmux_window_add`), the `%layout-change` consumer creating
    /// native panes, `check_mux_notifications` routing, seed delivery, and
    /// live `%output` landing on the seeded pane. The transport-level
    /// wiring tests bypass all of that; this is the path the GUI runs, and
    /// the one that was green twice at transport level while panes still
    /// rendered blank live.
    ///
    /// The TUI gates its second frame on `read` (the core seed-state
    /// pattern): frame 1 must arrive via the SEED, frame 2 via LIVE
    /// `%output` after the reattached side releases it — so the test
    /// cannot pass on either path alone.
    #[test]
    fn reattach_renders_and_updates_a_tui_through_the_window_state_path() {
        let path = socket_path("ws-tui");
        spawn_daemon(&path);

        let tui = "printf \"\\033[?1049h\\033[2;3H\\033[1;44m\"; echo SEED | tr A-Z a-z; \
                   printf \"\\033[0m\\033[?25l\"; read -r x; \
                   printf \"\\033[4;5H\\033[1;42m\"; echo LATE | tr A-Z a-z; \
                   printf \"\\033[0m\"; sleep 60";

        // First client: create the session, start the TUI, wait for its
        // first frame, then drop the connection (detach, D5).
        {
            let mut first = MuxSessionClient::connect(&path).expect("first client");
            first.create_or_attach("wstui").expect("create");
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut created = false;
            while Instant::now() < deadline && !created {
                created = first
                    .poll_actions()
                    .iter()
                    .any(|a| matches!(a, SyncAction::CreateTab { .. }));
                if !created {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
            assert!(created, "first client never saw the session's window");
            // PaneOutput actions only flow for MAPPED panes (sync.rs), so
            // adopt the daemon's panes before waiting on frame 1 — the
            // adopt_panes pattern from the reattach wiring test.
            for pane in first.list_panes().expect("list-panes") {
                first.sync().map_pane(pane, 10_000 + pane);
            }
            first
                .send(&format!("send-keys -t %0 '{tui}' Enter"))
                .expect("start the TUI");
            let mut framed = false;
            while Instant::now() < deadline && !framed {
                framed = first.poll_actions().iter().any(|a| {
                    matches!(a, SyncAction::PaneOutput { data, .. }
                        if String::from_utf8_lossy(data).contains("seed"))
                });
                if !framed {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
            assert!(framed, "TUI frame 1 never reached the first client");
        }

        // The app attach: the real sequence, then manual installation into
        // a real (renderer-less) WindowState — the same steps
        // `install_mux_transport` performs minus the renderer-derived size.
        let transport = connect(&path);
        let attach = attach_sequence(&transport, "wstui", Some((80, 24))).expect("attach_sequence");
        assert!(
            attach
                .screens
                .iter()
                .any(|(_, data)| String::from_utf8_lossy(data).contains("seed")),
            "the reattach seed carries the TUI's first frame: {:?}",
            attach.screens
        );

        let mut ws = manners_state();
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wstui".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump the app's own notification loop — the layout consumer must
        // create the native pane, the seed must render in it, and both must
        // happen through the production dispatch path.
        let mut existing_windows = attach.existing_windows.clone();
        if existing_windows.is_empty() {
            // A created session reports its window via %window-add instead.
            existing_windows = vec![0];
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let native_of = |ws: &crate::app::window_state::WindowState| {
            ws.tmux_state
                .tmux_pane_to_native_pane
                .get(&0)
                .copied()
                .or_else(|| ws.tmux_state.tmux_sync.get_native_pane(0))
        };
        let native = loop {
            ws.check_mux_notifications();
            if let Some(native) = native_of(&ws) {
                break native;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the native pane\n\
                 window→tab: {:?}\n\
                 pane map: {:?}\n\
                 tab count: {}",
                existing_windows
                    .iter()
                    .map(|w| (*w, ws.tmux_state.tmux_sync.get_tab(*w)))
                    .collect::<Vec<_>>(),
                ws.tmux_state.tmux_pane_to_native_pane,
                ws.tab_manager.tab_count()
            );
            std::thread::sleep(Duration::from_millis(25));
        };

        let pane_state = |ws: &WindowState| -> Option<(bool, String)> {
            for tab in ws.tab_manager.tabs() {
                let Some(pm) = tab.pane_manager() else {
                    continue;
                };
                let Some(pane) = pm.get_pane(native) else {
                    continue;
                };
                let Ok(term) = pane.terminal.try_read() else {
                    continue;
                };
                return Some((
                    term.is_alt_screen_active(),
                    term.content().unwrap_or_default(),
                ));
            }
            None
        };

        let mut seeded = false;
        while Instant::now() < deadline && !seeded {
            ws.check_mux_notifications();
            seeded = matches!(pane_state(&ws), Some((alt, text)) if alt && text.contains("seed"));
            if !seeded {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            seeded,
            "the seed must render the TUI frame on the native pane's ALT screen: \
             {:?}",
            pane_state(&ws)
        );

        // Release the TUI's second frame through the REAL input path with
        // mux_focused_pane unset — the fresh-attach frozen-panes scenario:
        // keystrokes must resolve their target from the focused native pane
        // (the daemon rejects untargeted send-keys, and nothing sets
        // mux_focused_pane until a click or a daemon focus push).
        assert!(
            ws.tmux_state.mux_focused_pane.is_none(),
            "the scenario needs no tracked focus — fresh attach state"
        );
        assert!(
            ws.send_input_via_tmux(b"g\r"),
            "the mux transport must consume input"
        );
        let mut updated = false;
        while Instant::now() < deadline && !updated {
            ws.check_mux_notifications();
            updated = matches!(pane_state(&ws), Some((_, text)) if text.contains("late"));
            if !updated {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            updated,
            "input through the real path must reach the daemon and its output \
             land on the seeded native pane: {:?}",
            pane_state(&ws)
        );

        // Split through the real action with mux_focused_pane STILL unset —
        // the fresh-attach silent-no-split scenario: the daemon requires -t
        // on split-window, and the untargeted form used to be sent and its
        // %error reply silently counted as success.
        ws.split_pane_vertical();
        let mut split_landed = false;
        while Instant::now() < deadline && !split_landed {
            ws.check_mux_notifications();
            split_landed = ws.tmux_state.tmux_pane_to_native_pane.contains_key(&1);
            if !split_landed {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            split_landed,
            "the daemon-side split must create the second native pane via \
             %layout-change: map = {:?}",
            ws.tmux_state.tmux_pane_to_native_pane
        );

        // The daemon's focus push for the new pane must move NATIVE focus
        // (the only source of truth for routing). It arrives in the same
        // batch as the layout that creates the pane, so it must be applied
        // after the layout consumer, or the new pane is not yet mapped and
        // the push is lost (reported live: split did not focus the new pane).
        let new_native = ws.tmux_state.tmux_pane_to_native_pane[&1];
        assert_eq!(
            ws.tmux_state.native_pane_to_tmux_pane.get(&new_native),
            Some(&1),
            "the reverse map routes the new pane's input"
        );
        let focused_native = || {
            ws.tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager())
                .and_then(|pm| pm.focused_pane())
                .map(|p| p.id)
        };
        assert_eq!(
            focused_native(),
            Some(new_native),
            "native focus must move to the split's new pane"
        );

        // Input now targets the new pane and its output lands there.
        assert!(ws.send_input_via_tmux(b"echo SPLIT-OK | tr A-Z a-z\r"));
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut new_pane_text = String::new();
        while Instant::now() < deadline && !new_pane_text.contains("split-ok") {
            ws.check_mux_notifications();
            for tab in ws.tab_manager.tabs() {
                if let Some(pane) = tab.pane_manager().and_then(|pm| pm.get_pane(new_native))
                    && let Ok(term) = pane.terminal.try_read()
                {
                    new_pane_text = term.content().unwrap_or_default();
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let daemon_view = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("capture-pane -t %1 -p")
            .expect("capture")
            .join("|");
        assert!(
            new_pane_text.contains("split-ok"),
            "the new pane must render its own output: app={new_pane_text:?} daemon={daemon_view:?} \
             map={:?}",
            ws.tmux_state.tmux_pane_to_native_pane
        );

        // Mouse reports for a focused mux pane go to the DAEMON pane — the
        // local mirror has no PTY, so a local write dropped every click and
        // wheel event (mouse-aware TUIs ignored the mouse entirely). The
        // bytes land in the pane's input, so a shell echoes a raw marker.
        assert!(
            ws.route_mouse_report_to_mux(b"MOUSE-ROUTE-OK"),
            "a focused mux pane must take the mouse report"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut daemon_view = String::new();
        while Instant::now() < deadline && !daemon_view.contains("MOUSE-ROUTE-OK") {
            daemon_view = ws
                .tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("capture-pane -t %1 -p")
                .expect("capture")
                .join("|");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            daemon_view.contains("MOUSE-ROUTE-OK"),
            "the mouse report must reach the daemon pane: {daemon_view:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A window attached to a par-mux daemon must persist its session name
    /// as a MUX name, never a tmux one: the next launch restored a
    /// tmux-tagged name through the tmux gateway, which spawned a real
    /// `tmux -CC new-session -A` of the same name (measured live
    /// 2026-09-23: single pane, then a blank window, on successive
    /// reopens).
    #[test]
    fn an_attached_mux_session_persists_as_mux_not_tmux() {
        let path = socket_path("persist-kind");
        spawn_daemon(&path);
        let transport = connect(&path);
        attach_sequence(&transport, "kind", None).expect("attach");

        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("kind".to_string());
        assert_eq!(
            ws.tmux_state.persisted_session_names(),
            (None, Some("kind".to_string())),
            "a transport-attached name is a mux name"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A stale tracked focus must not capture a local tab's input. Before
    /// the fix, `mux_focused_pane` won over the focused native pane and
    /// outlived its tab: after the mux tab closed, every keystroke in the
    /// remaining local tab was sent to a daemon pane that no longer existed
    /// (reported live: "closed the par-mux panes, but the original native
    /// tab is unresponsive").
    #[test]
    fn a_stale_mux_focus_does_not_capture_local_tab_input() {
        let path = socket_path("stale-focus");
        spawn_daemon(&path);
        let transport = connect(&path);
        attach_sequence(&transport, "stale", None).expect("attach");

        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));
        // The mux tab is gone; only the tracked id survives.
        ws.tmux_state.mux_focused_pane = Some(0);
        assert!(
            ws.tmux_state.native_pane_to_tmux_pane.is_empty(),
            "no mux pane is mapped — the user is not on a mux pane"
        );

        assert!(
            !ws.send_input_via_tmux(b"x"),
            "input must fall through to the local PTY, not the daemon"
        );
        assert!(
            !ws.send_literal_bytes_via_tmux(b"\n"),
            "the literal-bytes path must fall through too"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A `WindowState` with no window, renderer, or tabs — the same seam
    /// `dispatch_tests::test_window_state` uses — so the manners test can
    /// hold the real `apply_agent_pushes` receiver without a live daemon.
    fn manners_state() -> crate::app::window_state::WindowState {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime)
    }

    fn push(
        pane: &str,
        agent: &str,
        state: &str,
        source: &str,
    ) -> par_term_emu_core_rust::tmux_control::TmuxNotification {
        par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged {
            pane_id: pane.to_string(),
            agent: agent.to_string(),
            state: state.to_string(),
            source: source.to_string(),
        }
    }

    /// REPORT.md E3, pinned (A2b task 4): an agent needing attention joins
    /// a list and the indicator glows — it never steals focus. The roster
    /// update path may set the redraw flag (that is the glow) and may
    /// change what the status widget and a user-opened palette RENDER; it
    /// must not itself focus a pane, raise the window, notify, or open or
    /// pre-select the palette. Assertions are on the absence of those
    /// calls' effects, not on rendered output — a snapshot would pass
    /// while a focus call fired beside it.
    ///
    /// This test failing is the intended alarm, not an obstacle to route
    /// around: a future change that genuinely needs to interrupt is an
    /// owner decision and a change to E3.
    #[test]
    fn roster_updates_never_steal_focus_raise_notify_or_open_the_palette() {
        let mut ws = manners_state();
        // Baseline: palette closed, nothing queued, no tab focused.
        assert!(!ws.overlay_ui.command_palette.visible);
        assert!(ws.overlay_state.toast_message.is_none());
        assert!(ws.tab_manager.active_tab_id().is_none());
        assert!(ws.focus_state.pending_focus_tab_switch.is_none());

        // Both arrivals the rule names at once: a NEW agent entering the
        // roster (a spawn is not an interruption) and a state transition
        // to blocked on an existing pane (waiting is not an interruption
        // either).
        let redraw = ws.apply_agent_pushes(vec![
            push("%0", "kimi", "idle", "hook"),
            push("%2", "claude", "blocked", "hook"),
        ]);

        // The glow half of E3 is intact: the roster took both arrivals
        // and the redraw flag is set.
        assert!(redraw, "a roster change is a potential visual change");
        let rosterd: Vec<(u64, &str)> = ws
            .tmux_state
            .agent_roster
            .iter()
            .map(|e| (e.pane, e.state.as_str()))
            .collect();
        assert_eq!(
            rosterd,
            vec![(0, "idle"), (2, "blocked")],
            "both arrivals must land in the cache the surfaces read"
        );

        // The manners half: none of the interruption channels moved.
        assert!(
            !ws.overlay_ui.command_palette.visible,
            "a roster update must not open the palette"
        );
        assert!(
            ws.overlay_state.toast_message.is_none(),
            "a roster update must not notify"
        );
        assert!(
            ws.tab_manager.active_tab_id().is_none(),
            "a roster update must not focus a pane or switch tabs"
        );
        assert!(
            ws.focus_state.pending_focus_tab_switch.is_none(),
            "a roster update must not queue a tab switch"
        );
    }
}
