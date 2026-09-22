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

use crate::app::tmux_handler::tmux_state::TmuxTransport;
use crate::app::window_state::WindowState;
use crate::tmux::{ParserBridge, TmuxNotification, escape_keys_for_tmux};
use par_term_mux::{AgentEntry, AttachOutcome, MuxSessionClient};
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::cell::{RefCell, RefMut};
use std::io::{self, ErrorKind};

/// The daemon transport: a par-mux client behind the [`TmuxTransport`]
/// seam. Interior mutability because the routing hooks that reach it
/// (`send_input_via_tmux`, `notify_tmux_of_resize`) hold `&WindowState`.
pub(crate) struct MuxTransport {
    client: RefCell<MuxSessionClient>,
}

impl MuxTransport {
    /// Connect to the daemon serving `name`, spawning one when none is
    /// running — the transparency entry point, and the app's first real
    /// exercise of the daemon binary walk.
    pub(crate) fn connect_or_spawn(name: &str) -> io::Result<Self> {
        Ok(Self {
            client: RefCell::new(MuxSessionClient::connect_or_spawn(name)?),
        })
    }

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
/// nowhere.
pub(crate) fn route_input(
    transport: &dyn TmuxTransport,
    focused: Option<TmuxPaneId>,
    data: &[u8],
) -> bool {
    let escaped = escape_keys_for_tmux(data);
    let command = match focused {
        Some(pane) => format!("send-keys -t %{pane} {escaped}"),
        None => format!("send-keys {escaped}"),
    };
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
/// `WindowState`: create-or-attach, report existing windows (the caller
/// allocates tabs for them — a created session gets its tab from the
/// `%window-add` push), push the client size (the first pane names the
/// window the server resizes), collect each pane's replayed screen for
/// seeding (`refresh-client -t` replies carry the screen; they are NOT
/// `%output` pushes), and read the agent roster for the initial fill
/// (A2b task 1: `list-agents` on attach and reattach — the single call
/// site of the roster query in app code).
pub(crate) fn attach_sequence(
    transport: &MuxTransport,
    name: &str,
    size: Option<(u16, u16)>,
) -> io::Result<AttachSequence> {
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
        screens.push((pane, reply.join("\n").into_bytes()));
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
        outcome,
        existing_windows,
        screens,
        agents,
    })
}

/// What [`attach_sequence`] learned — the tuple it returned, named: the
/// attach outcome, windows needing tabs, per-pane replayed screens, and
/// the roster fill.
pub(crate) struct AttachSequence {
    pub(crate) outcome: AttachOutcome,
    pub(crate) existing_windows: Vec<TmuxWindowId>,
    pub(crate) screens: Vec<(TmuxPaneId, Vec<u8>)>,
    pub(crate) agents: Vec<AgentEntry>,
}

impl WindowState {
    /// Attach to (or create) the par-mux session `name` and install the
    /// transport. The `mux_session_name` profile path calls this at
    /// startup. The gateway machinery is untouched — no gateway tab, no
    /// `set-option`; the daemon owns the sessions.
    pub(crate) fn start_mux_session(&mut self, name: &str) -> io::Result<AttachOutcome> {
        if self.tmux_state.transport.is_some() {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "a par-mux transport is already attached",
            ));
        }
        let transport = MuxTransport::connect_or_spawn(name)?;
        // Run the attach sequence with the concrete local so
        // `handle_tmux_window_add` can borrow the window state freely;
        // boxing into tmux_state happens only once attached.
        let size = self.renderer.as_ref().map(|r| -> (u16, u16) {
            let (cols, rows) = r.grid_size();
            (cols as u16, rows as u16)
        });
        let attach = attach_sequence(&transport, name, size).map(
            |AttachSequence {
                 outcome,
                 existing_windows,
                 screens,
                 agents,
             }| {
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
            let (cols, rows) = renderer.grid_size();
            push_client_size(
                &**transport,
                self.tmux_state.mux_focused_pane,
                cols as u16,
                rows as u16,
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
                    // The daemon pushes focus changes; remember the pane for
                    // input routing before the shared focus handler runs.
                    self.tmux_state.mux_focused_pane = Some(pane_id);
                    self.handle_tmux_pane_focus_changed(pane_id);
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
        let ready: Vec<TmuxPaneId> = self
            .tmux_state
            .mux_screen_seeds
            .keys()
            .filter(|pane| self.tmux_state.tmux_sync.get_native_pane(**pane).is_some())
            .copied()
            .collect();
        let mut delivered = Vec::new();
        for pane in ready {
            let Some(native) = self.tmux_state.tmux_sync.get_native_pane(pane) else {
                continue;
            };
            // try_lock: intentional — same delivery discipline as the
            // PaneOutput consumer.
            for tab in self.tab_manager.tabs_mut() {
                if let Some(pane_manager) = tab.pane_manager_mut()
                    && let Some(pane_obj) = pane_manager.get_pane_mut(native)
                    && let Some(data) = self.tmux_state.mux_screen_seeds.get(&pane)
                    && let Ok(term) = pane_obj.terminal.try_read()
                {
                    term.process_data(data);
                    delivered.push(pane);
                    break;
                }
            }
        }
        for pane in delivered {
            self.tmux_state.mux_screen_seeds.remove(&pane);
        }
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
        // the per-pane screen replay — everything `start_mux_session` runs
        // short of tab allocation. The size differs from the daemon's
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
