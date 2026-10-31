//! par-mux app wiring (T4.4/T4.5): the control-mode transport over a
//! par-mux daemon, reusing the tmux notification consumers unchanged.
//!
//! The daemon client yields the same core notifications a `tmux -CC`
//! gateway does, so everything downstream of the drain — `ParserBridge`,
//! the grouped `TmuxSync` dispatch, and every handler in this directory —
//! is the tmux path's code used as-is. What lives here is only what the
//! design doc says adapts: the app wiring — the session-started wiring
//! (client size through the daemon, not `set-option` plus gateway PTY
//! writes), input routing, reattach resync, and the detach UX — dropping
//! the socket leaves the daemon and its sessions running (the D5
//! promise, made user-visible in the toasts and window title). The
//! transport itself lives in `mux_transport.rs`; the attach sequence it
//! drives lives in `mux_attach.rs`.
//!
//! Compiled only under the `mux` feature (default-on; forwards the core's
//! `mux` module, first shipped in core 0.50).

pub(crate) use super::mux_attach::{AttachSequence, attach_sequence, mux_attach_refusal};
pub(crate) use super::mux_panes::MuxLaunchOutcome;
pub(crate) use super::mux_reconnect::restore_windows;
pub(crate) use super::mux_transport::MuxTransport;
use crate::app::tmux_handler::tmux_state::{TmuxState, TmuxTransport};
use crate::app::window_state::WindowState;
use crate::tmux::send_keys_arguments;
use par_term_mux::{AttachOutcome, MuxSessionClient, VersionCheck, check_daemon_version};
use par_term_tmux::TmuxPaneId;
use std::io;

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

/// The per-cell pixel size pushed beside the grid (`refresh-client -p`):
/// the one renderer metric every daemon pane shares, so CSI 14t/16t answers
/// and image cell-span math match the client's font. Rounded to nearest —
/// the wire requires a positive integer, and sub-pixel cell metrics only
/// exist before DPI rounding.
pub(crate) fn mux_client_cell_px(renderer: &par_term_render::renderer::Renderer) -> (u16, u16) {
    let px = |v: f32| v.round().max(1.0) as u16;
    (px(renderer.cell_width()), px(renderer.cell_height()))
}

/// The theme fg/bg as `rrggbb` (no leading `#` — the control wire is
/// whitespace-split and a `#` would read as a comment in shells clients
/// paste from; core `parse_hex_color` enforces the shape).
pub(crate) fn mux_client_colors_hex(config: &crate::config::Config) -> (String, String) {
    let theme = config.load_theme();
    let hex = |c: crate::config::Color| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    (hex(theme.foreground), hex(theme.background))
}

/// Push the renderer's grid and cell size so the daemon re-fits the window
/// holding `pane` — `refresh-client -t %N -C -p` broadcasts
/// `%layout-change` with the new geometry (core T4.C; the `-t` target is
/// required by the server) and re-derives every pane's pixel metrics.
pub(crate) fn push_client_size(
    transport: &dyn TmuxTransport,
    pane: Option<TmuxPaneId>,
    cols: u16,
    rows: u16,
    cell_px: (u16, u16),
) {
    let Some(pane) = pane else {
        crate::debug_trace!(
            "MUX",
            "client size push skipped — no focused pane to target"
        );
        return;
    };
    if let Err(e) = transport.send_command_no_wait(&format!(
        "refresh-client -t %{pane} -C {cols}x{rows} -p {}x{}",
        cell_px.0, cell_px.1
    )) {
        crate::debug_error!("MUX", "client size push failed: {e}");
    }
}

/// Push the client's theme colors (`set-client-colors -f/-b rrggbb`) so
/// pane OSC 10/11 answers reflect the client's actual dark/light theme.
/// Fire-and-forget like the size push: a daemon predating the command
/// answers an error block nobody reads, which is the correct degradation.
pub(crate) fn push_client_colors(transport: &dyn TmuxTransport, fg: &str, bg: &str) {
    if let Err(e) = transport.send_command_no_wait(&format!("set-client-colors -f {fg} -b {bg}")) {
        crate::debug_error!("MUX", "client colors push failed: {e}");
    }
}

/// Route input bytes to the daemon as tmux key names — the same
/// `send_keys_arguments` form the gateway sends, targeted at the focused
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
    let command = format!("send-keys -t %{pane} {}", send_keys_arguments(data));
    if let Err(e) = transport.send_command_no_wait(&command) {
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
    if let Err(e) = transport.send_command_no_wait(&format!("send-keys -t %{pane} -H {hex}")) {
        crate::debug_error!("MUX", "send-keys -H failed: {e}");
    }
    true
}

impl TmuxState {
    /// The roster surfaces' `visible` predicate, admitting both namespaces:
    /// a daemon pane renders while the app maps it (the module doc's rule),
    /// and a synthetic local pane renders while the detector still sees the
    /// pane live AND the daemon has not claimed it — the claim check makes
    /// the non-shadowing rule immediate rather than waiting out the next
    /// detection pass.
    pub(crate) fn roster_pane_visible(&self, pane: par_term_tmux::TmuxPaneId) -> bool {
        match super::agent_roster::AgentRoster::decode_local_pane(pane) {
            Some((tab_id, pane_id)) => {
                self.agent_roster.local_seen(pane)
                    && !self
                        .tmux_pane_owners
                        .values()
                        .any(|owner| *owner == (tab_id, pane_id))
            }
            None => self.tmux_pane_owners.contains_key(&pane),
        }
    }

    /// Runtime palette rows for the attached par-mux session, present only
    /// while a transport is installed so the palette never offers a dead
    /// action (the roster-picker pattern: runtime rows joined at open time,
    /// not dispatch-table built-ins).
    ///
    /// Lives on `TmuxState` (like the roster's `palette_rows`) rather than
    /// `WindowState` so the egui open path's closure captures only this
    /// field — a `&self` method call on the whole `WindowState` would
    /// capture `*self` and collide with the render closure's mutable use.
    pub(crate) fn mux_palette_rows(&self) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        if self.transport.is_none() {
            return Vec::new();
        }
        // Detach is the built-in `detach` action (UX.md A23), a catalog row
        // of its own; only the mux-specific restart stays a runtime row.
        vec![
            // Restart the focused daemon pane's process in place
            // (`respawn-pane`; `-k` when it is still running).
            crate::command_palette::catalog::PaletteEntry {
                action_id: "mux-restart-pane".to_string(),
                label: "Restart Pane Process (par-mux)".to_string(),
                chord: None,
                priority: 0,
            },
        ]
    }
}

/// An in-flight profile-open attach: a worker thread owns the daemon
/// connect/spawn (the core retries the daemon socket for up to 10s — far
/// too long to hold the event loop), reports the outcome over the channel,
/// and [`WindowState::poll_mux_attach`] consumes it on the main thread.
pub(crate) struct MuxAttachPending {
    pub(crate) name: String,
    pub(crate) rx: std::sync::mpsc::Receiver<io::Result<par_term_emu_core_rust::mux::MuxClient>>,
}

impl WindowState {
    /// Begin the profile-open attach for `name` WITHOUT blocking the event
    /// loop: the daemon connect/spawn runs on a worker thread and
    /// [`Self::poll_mux_attach`] finishes the attach on the main thread. A
    /// second request while one is already in flight is ignored.
    pub(crate) fn begin_mux_session_attach(&mut self, name: &str) {
        if self.tmux_state.transport.is_some() || self.tmux_state.mux_attach_pending.is_some() {
            // One session per window: name the holder instead of returning
            // silently, so a profile open that did nothing is explained.
            let holding = self
                .tmux_state
                .mux_attach_pending
                .as_ref()
                .map(|pending| format!("still attaching to par-mux session '{}'", pending.name))
                .or_else(|| {
                    self.tmux_state
                        .tmux_session_name
                        .as_deref()
                        .map(|name| format!("already attached to par-mux session '{name}'"))
                })
                .unwrap_or_else(|| "already attached to a par-mux session".to_string());
            self.show_toast(format!(
                "par-mux: this window is {holding} — open the profile in a new window"
            ));
            return;
        }
        // The self-attach guard runs before the worker spawns so a refusal
        // leaves no pending state and no transport behind.
        if let Some(reason) =
            mux_attach_refusal(&par_term_emu_core_rust::mux::ipc::default_socket_path(name))
        {
            log::error!("par-mux attach to '{name}' refused: {reason}");
            self.record_mux_error(format!("par-mux: attach to '{name}' refused — {reason}"));
            self.mux_restore_placeholder_retire();
            return;
        }
        // The profile path follows par-term's one-daemon-per-session naming.
        self.tmux_state.mux_daemon = Some(name.to_string());
        self.tmux_state.mux_daemon_rx = None;
        self.tmux_state.mux_attach_socket =
            Some(par_term_emu_core_rust::mux::ipc::default_socket_path(name));
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
                self.tmux_state.mux_attach_socket = None;
                self.record_mux_error("par-mux: attach failed (worker thread unavailable)");
                self.mux_restore_placeholder_retire();
            }
        }
    }

    /// The restore placeholder's attach ended without a daemon window.
    /// The marked tab stops being a placeholder and becomes an ordinary
    /// tab, so no later window-add (e.g. a manual tmux control session in
    /// this window) may close it out from under the user.
    fn mux_restore_placeholder_retire(&mut self) {
        self.tmux_state.mux_restore_placeholder_tab = None;
    }

    /// Finish an in-flight profile-open attach on the main thread. The
    /// still-connecting case restores the pending state for later frames;
    /// failure toasts visibly — the old path only wrote a DEBUG_LEVEL-gated
    /// log line, so a failed attach looked like nothing happened.
    pub(crate) fn poll_mux_attach(&mut self) {
        let Some(pending) = self.tmux_state.mux_attach_pending.take() else {
            return;
        };
        let outcome = match pending.rx.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.tmux_state.mux_attach_pending = Some(pending);
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            Ok(result) => Some(result),
        };
        // The attempt settled: its by-name daemon report and its socket (if
        // any) are read now or dropped, never left for the next attach to
        // pick up.
        let reported_daemon = self
            .tmux_state
            .mux_daemon_rx
            .take()
            .and_then(|rx| rx.try_recv().ok());
        let socket = self.tmux_state.mux_attach_socket.take();
        match outcome {
            None => {
                log::error!("mux attach worker died without reporting a result");
                self.record_mux_error("par-mux: attach failed (worker died)");
                self.mux_restore_placeholder_retire();
                self.mux_default_tab_attach_tail(false);
            }
            Some(Err(e)) => {
                log::error!("par-mux attach to '{}' failed: {e}", pending.name);
                self.record_mux_error(format!(
                    "par-mux: attach to '{}' failed — {e}",
                    pending.name
                ));
                self.mux_restore_placeholder_retire();
                self.mux_default_tab_attach_tail(false);
            }
            Some(Ok(client)) => {
                let socket = match reported_daemon {
                    Some((daemon, reported_socket)) => {
                        self.tmux_state.mux_daemon = Some(daemon);
                        Some(reported_socket)
                    }
                    None => socket,
                };
                let transport =
                    MuxTransport::with_socket(MuxSessionClient::from_core(client), socket);
                match self.install_mux_transport(&pending.name, transport) {
                    Err(e) => {
                        log::error!("par-mux attach to '{}' failed: {e}", pending.name);
                        self.record_mux_error(format!(
                            "par-mux: attach to '{}' failed — {e}",
                            pending.name
                        ));
                        self.mux_restore_placeholder_retire();
                        self.mux_default_tab_attach_tail(false);
                    }
                    Ok(_) => self.mux_default_tab_attach_tail(true),
                }
            }
        }
    }

    /// The mux-by-default attach tail: the requested tab already exists as
    /// a local shell (new_tab_local ran synchronously), so both outcomes
    /// only retire the origin marker. Failure says so with `log::warn!` —
    /// the tab the user asked for never disappears — and disarms the
    /// pending first-tab migration with it. Success leaves
    /// `mux_default_placeholder` armed: the first daemon window-add
    /// replaces the untouched local tab in its slot, and on a created
    /// session that window arrives via %window-add after this tail.
    fn mux_default_tab_attach_tail(&mut self, attached: bool) {
        if self.tmux_state.mux_default_tab.take().is_some() {
            if attached {
                return;
            }
            // The local first tab stays what it already is — a plain local
            // shell (criterion: the fallback is today's behavior).
            self.tmux_state.mux_default_placeholder = None;
            log::warn!(
                "par-mux: default-session attach failed — the window stays unattached; \
                 its tabs are local shells"
            );
        }
    }

    /// The attach tail shared by the sync and worker-thread entry points:
    /// run the attach sequence against a connected transport, install it,
    /// and give the user the toast + window-title feedback.
    pub(super) fn install_mux_transport(
        &mut self,
        name: &str,
        transport: MuxTransport,
    ) -> io::Result<AttachOutcome> {
        // Run the attach sequence with the concrete local so
        // `handle_tmux_window_add` can borrow the window state freely;
        // boxing into tmux_state happens only once attached.
        let size = self.renderer.as_ref().map(mux_client_grid);
        let cell_px = self.renderer.as_ref().map(mux_client_cell_px);
        let colors = mux_client_colors_hex(&self.config.load());
        let env = crate::tab::build_shell_env(self.config.load().shell.shell_env.as_ref())
            .unwrap_or_default();
        let attach = attach_sequence(&transport, name, size, cell_px, &colors, &env).map(
            |AttachSequence {
                 daemon_version,
                 outcome,
                 existing_windows,
                 window_names,
                 screens,
                 graphics,
                 agents,
                 titles,
             }| {
                // The stale-daemon check: the daemon outlives clients, so
                // this attach may have landed on one built before the
                // client's core — every daemon-side fix would then read as
                // "didn't work". Surfaced, never silent; a daemon so old it
                // cannot answer `version` is itself the mismatch.
                let client_stamp = par_term_emu_core_rust::mux::build_stamp();
                match check_daemon_version(daemon_version.as_deref().unwrap_or(""), client_stamp) {
                    VersionCheck::Match | VersionCheck::Unknown => {
                        self.tmux_state.mux_health = crate::session_chip::MuxHealth::Connected;
                    }
                    VersionCheck::Mismatch { daemon, client } => {
                        log::warn!(
                            "par-mux daemon older than the client — daemon {daemon}, \
                             client {client}; daemon-side fixes are missing until the \
                             daemon is restarted (par-mux --restart)"
                        );
                        self.tmux_state.mux_health = crate::session_chip::MuxHealth::Stale;
                        self.record_mux_error(format!(
                            "par-mux: daemon older than this client ({daemon} vs {client}) \
                             — run `par-mux --restart` in a terminal to pick up fixes \
                             (your session is restored)"
                        ));
                    }
                }
                // File-transfer verbs arrived in daemon core 0.68: an
                // older (or unprovable) daemon degrades drops to the
                // paste-path fallback instead of a failed upload.
                self.tmux_state.mux_file_transfer_supported =
                    par_term_mux::daemon_supports_file_transfer(
                        daemon_version.as_deref().unwrap_or(""),
                    );
                for window_id in &existing_windows {
                    if self.tmux_state.tmux_sync.get_tab(*window_id).is_none() {
                        self.handle_tmux_window_add(*window_id);
                    }
                }
                // Swap the placeholder titles for the daemon's own window
                // names — same setter the %window-renamed push uses, so a
                // daemon rename later lands identically. This closure runs
                // BEFORE the transport is installed, so the placeholders
                // above still read "tmux @N" here; a window the daemon did
                // not name keeps its placeholder, and UX.md T8 bars "tmux"
                // on a mux path — rewrite those (T8/M11).
                for (window_id, name) in &window_names {
                    if let Some(tab_id) = self.tmux_state.tmux_sync.get_tab(*window_id)
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.set_title(name);
                    }
                }
                for window_id in existing_windows {
                    if !window_names.iter().any(|(id, _)| id == &window_id)
                        && let Some(tab_id) = self.tmux_state.tmux_sync.get_tab(window_id)
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.set_title(&format!("par-mux @{}", window_id));
                    }
                }
                // Screens land once the layout consumers create the panes
                // (checked each poll in check_mux_notifications).
                self.tmux_state.mux_screen_seeds = screens
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>();
                // Graphics placements ride the same replies; they land
                // beside the screens and deliver after each pane's screen.
                self.tmux_state.mux_graphics_osc = graphics
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>();
                // Daemon pane titles wait the same way — the reattach
                // restore of pane renaming.
                self.tmux_state.mux_pane_titles = titles.into_iter().collect();
                self.tmux_state.agent_roster.fill_from_list(agents);
                outcome
            },
        );
        match attach {
            Ok(outcome) => {
                // Event-ring replay (`events --since`, core 0.72.0): on a
                // re-attach to the daemon the watermark was baselined on,
                // everything the daemon broadcast since that attach replays
                // here and rides the drain like live pushes — a pane that
                // exited, an agent that changed state, a window that moved
                // while no client was listening all land without polling.
                // The watermark is daemon-scoped (sequence numbers are
                // daemon-lifetime): a different socket re-baselines, and a
                // first attach applies nothing (the daemon-wide ring may
                // hold other sessions' windows, and a `%window-add` for an
                // unattached session would adopt it as a tab). A daemon
                // predating the verb fails the query and degrades to the
                // registration replay of held state it already does.
                let socket = transport.socket().map(std::path::Path::to_path_buf);
                let same_daemon = self.tmux_state.mux_event_seq_socket.is_some()
                    && self.tmux_state.mux_event_seq_socket == socket;
                let watermark = if same_daemon {
                    self.tmux_state.mux_event_seq
                } else {
                    0
                };
                match transport.client().events_since(watermark) {
                    Ok(replay) => {
                        // The head re-baselines even across a daemon swap:
                        // `--since` past the head comes back empty and the
                        // next attach resumes from the new daemon's truth.
                        self.tmux_state.mux_event_seq = replay.head;
                        self.tmux_state.mux_event_seq_socket = socket;
                        if same_daemon {
                            let mut parser =
                                par_term_emu_core_rust::tmux_control::TmuxControlParser::new(true);
                            for event in replay.events {
                                let line = format!("{}\n", event.line);
                                self.tmux_state
                                    .mux_replay_pending
                                    .extend(parser.parse(line.as_bytes()));
                            }
                        }
                    }
                    Err(e) => crate::debug_error!("MUX", "event replay unavailable: {e}"),
                }
                // Auto-reconnect install tail — runs BEFORE the transport
                // moves into the boxed trait object, because the daemon
                // client is reachable only through the concrete
                // `MuxTransport`, and BEFORE the first drain, so the
                // restore's rename pushes queue after each window's
                // `%window-add` and title their tabs in order.
                let reconnect_windows = self.consume_mux_reconnect_windows(name, &outcome);
                if let Some(windows) = reconnect_windows {
                    let titled = restore_windows(&mut transport.client(), &windows);
                    // The tabs these name do not exist until the first
                    // drain maps the windows; the drain applies the titles
                    // as it adopts each one.
                    self.tmux_state.mux_reconnect_titles = titled.into_iter().collect();
                }
                self.tmux_state.transport = Some(Box::new(transport));
                self.tmux_state.mux_snapshot_supported = None;
                self.tmux_state.mux_focused_pane = None;
                self.tmux_state.tmux_session_name = Some(name.to_string());
                self.tmux_state.mux_session_id = Some(match &outcome {
                    AttachOutcome::Created(s) | AttachOutcome::Attached(s) => s.id,
                });
                self.tmux_state.tmux_sync.enable();
                let verb = match &outcome {
                    AttachOutcome::Created(_) => "created",
                    AttachOutcome::Attached(_) => "attached to",
                };
                self.show_toast(format!(
                    "par-mux: {verb} session '{name}' (sessions survive detach)"
                ));
                self.update_window_title_for_mux();
                self.apply_mux_session_profile(name);
                self.refresh_mux_directory();
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
    /// The `detach` action (UX.md A23; `mux-detach` is its alias) dispatches
    /// here, as does the palette's runtime row; closing the
    /// window remains the implicit detach path.
    pub(crate) fn detach_mux_session(&mut self) -> bool {
        if self.tmux_state.transport.take().is_none() {
            return false;
        }
        // Mux-only state the shared cleanup below does not know about.
        self.tmux_state.mux_snapshot_supported = None;
        self.tmux_state.mux_file_transfer_supported = false;
        self.tmux_state.mux_focused_pane = None;
        self.tmux_state.mux_replay_pending.clear();
        self.tmux_state.mux_screen_seeds.clear();
        self.tmux_state.mux_graphics_osc.clear();
        self.tmux_state.mux_pane_titles.clear();
        self.tmux_state.mux_reconnect_titles.clear();
        self.tmux_state.mux_exited_panes.clear();
        self.tmux_state.clear_mux_seed_holds();
        self.tmux_state.agent_roster.clear();
        self.handle_tmux_session_ended();
        // Overwrite the shared cleanup's "connection lost" toast and chip
        // error: the session did not end, it survives in the daemon.
        self.tmux_state.mux_last_error = None;
        self.show_toast("par-mux: detached (session keeps running in the daemon)");
        self.tmux_state.mux_daemon = None;
        self.refresh_mux_directory();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        true
    }

    /// Dispatch a choice made in the last-tab close dialog (UX.md M1).
    /// Both destructive arms are session-scoped, not tab-scoped: Detach
    /// drops the attach connection, End session kills every window and
    /// pane in the session on the daemon.
    pub(crate) fn handle_mux_last_tab_action(
        &mut self,
        action: crate::mux_last_tab_ui::MuxLastTabAction,
    ) {
        match action {
            crate::mux_last_tab_ui::MuxLastTabAction::Detach => {
                self.detach_mux_session();
            }
            crate::mux_last_tab_ui::MuxLastTabAction::EndSession => {
                self.end_attached_mux_session();
            }
            crate::mux_last_tab_ui::MuxLastTabAction::Cancel
            | crate::mux_last_tab_ui::MuxLastTabAction::None => {}
        }
    }

    /// "End session" from the last-tab close dialog (UX.md M1). One
    /// `kill-session` on the daemon empties the session — including
    /// windows this client never mapped (`max_tabs` overflow, UX.md
    /// M14), which the per-window kill fallback could not reach. No
    /// local teardown runs here: the `%window-close` /
    /// `%sessions-changed` broadcasts tear the view down through the
    /// same path as a session killed by another client.
    pub(crate) fn end_attached_mux_session(&mut self) {
        let Some(transport) = self.tmux_state.transport.as_ref() else {
            return; // the view already ended; there is nothing to kill
        };
        let Some(session_id) = self.tmux_state.mux_session_id else {
            return; // attach pairs the transport with the session id
        };
        match transport.send_command(&format!("kill-session -t ${session_id}")) {
            Ok(_) => {
                log::info!("MUX: end session — kill-session ${session_id} sent");
            }
            Err(e) => {
                log::error!("MUX: end session — kill-session ${session_id} failed: {e}");
                self.record_mux_error(format!("par-mux: kill-session failed — {e}"));
            }
        }
    }

    /// Push the current theme colors to the daemon — the theme-change hook
    /// (config propagation) and nothing else calls this; attach reports the
    /// colors inside [`attach_sequence`]. No-op without a transport.
    pub(crate) fn push_mux_client_colors(&mut self) {
        let Some(transport) = &self.tmux_state.transport else {
            return;
        };
        let (fg, bg) = mux_client_colors_hex(&self.config.load());
        push_client_colors(&**transport, &fg, &bg);
    }

    /// End the mux view because the attached session no longer exists on
    /// the daemon (`%sessions-changed` re-query proved it gone — killed by
    /// another client or `kill-session`, while the daemon itself lives).
    /// The teardown mirrors [`Self::detach_mux_session`]: drop the
    /// transport, clear mux-only state, run the shared session-ended
    /// cleanup. The toast names what actually happened, unlike the shared
    /// cleanup's generic "tmux: Session ended".
    pub(super) fn end_mux_view_for_gone_session(&mut self) {
        crate::debug_info!(
            "MUX",
            "session gone from the daemon — ending the view (daemon keeps running)"
        );
        self.tmux_state.transport.take();
        self.tmux_state.mux_focused_pane = None;
        self.tmux_state.mux_screen_seeds.clear();
        self.tmux_state.mux_graphics_osc.clear();
        self.tmux_state.mux_pane_titles.clear();
        self.tmux_state.mux_exited_panes.clear();
        self.tmux_state.clear_mux_seed_holds();
        self.tmux_state.agent_roster.clear();
        self.handle_tmux_session_ended();
        self.tmux_state.mux_daemon = None;
        // Not a lost connection: the session was ended (here or elsewhere).
        self.tmux_state.mux_last_error = None;
        self.refresh_mux_directory();
        self.show_toast("par-mux: session ended on the daemon");
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// The adapted session-started wiring: no gateway tab to retitle and
    /// no `set-option window-size` (the daemon's policy is
    /// latest-report-wins, core T4.C) — just the name, the title, and the
    /// client size push.
    pub(super) fn handle_mux_session_started(&mut self, session_name: &str) {
        crate::debug_info!("MUX", "Session started: {session_name}");
        self.tmux_state.tmux_session_name = Some(session_name.to_string());
        self.update_window_title_for_mux();
        self.tmux_state.tmux_sync.enable();
        if let Some(renderer) = &self.renderer
            && let Some(transport) = &self.tmux_state.transport
        {
            let (cols, rows) = mux_client_grid(renderer);
            let cell_px = mux_client_cell_px(renderer);
            push_client_size(
                &**transport,
                self.focused_mux_pane_from_native(),
                cols,
                rows,
                cell_px,
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
}

#[cfg(test)]
pub(crate) mod tests {

    use super::super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, poll_until};

    use super::*;

    use crate::tmux::{ParserBridge, TmuxSync};
    use par_term_emu_core_rust::mux::MuxServer;

    use par_term_tmux::SyncAction;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// [`attach_sequence`] with the metrics a headless test cannot know:
    /// no renderer exists to measure cells, no config to theme from. The
    /// color pair is a real shape (`rrggbb`), not zeros, so a daemon-side
    /// rejection would be visible in the attach assertions.
    pub(crate) fn attach_test(
        transport: &MuxTransport,
        name: &str,
        size: Option<(u16, u16)>,
        env: &std::collections::HashMap<String, String>,
    ) -> io::Result<AttachSequence> {
        attach_sequence(
            transport,
            name,
            size,
            None,
            &("c0c0c0".to_string(), "121212".to_string()),
            env,
        )
    }

    pub(crate) fn socket_path(tag: &str) -> PathBuf {
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
    pub(crate) fn spawn_daemon(path: &Path) {
        let server = MuxServer::bind(path).expect("daemon binds");
        std::thread::spawn(move || server.run());
    }

    pub(crate) fn connect(path: &Path) -> MuxTransport {
        let deadline = Instant::now() + DAEMON_DEADLINE;
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
    pub(crate) fn poll_actions(transport: &MuxTransport, sync: &mut TmuxSync) -> Vec<SyncAction> {
        let (core_notes, _) = transport.drain();
        let frontend = ParserBridge::convert_all(core_notes);
        sync.process_notifications(&frontend)
    }

    /// Map every daemon pane to a native pane id (`10_000 + tmux id`) —
    /// the app's reattach adoption step, mirrored so unmapped panes drop
    /// no output.
    pub(crate) fn adopt_panes(transport: &MuxTransport, sync: &mut TmuxSync) {
        for pane in transport.client().list_panes().expect("list-panes") {
            sync.map_pane(pane, 10_000 + pane);
        }
    }

    /// A pane terminal's screen text — the read side of output routing.
    pub(crate) fn pane_text(
        ws: &crate::app::window_state::WindowState,
        tab_id: crate::tab::TabId,
        pane: crate::pane::PaneId,
    ) -> String {
        ws.tab_manager
            .get_tab(tab_id)
            .and_then(|tab| tab.pane_manager())
            .and_then(|pm| pm.get_pane(pane))
            .and_then(|p| p.terminal.try_read().ok())
            .map(|term| term.export_text())
            .unwrap_or_default()
    }

    /// A transport whose sends always fail — the forced transport error
    /// M6 tests. Drain stays empty-and-connected so the swap alone
    /// triggers no teardown before the split call.
    pub(crate) struct FailingTransport;

    impl TmuxTransport for FailingTransport {
        fn drain(
            &self,
        ) -> (
            Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
            bool,
        ) {
            (Vec::new(), false)
        }

        fn send_command(&self, _command: &str) -> io::Result<Vec<String>> {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "forced for test",
            ))
        }

        fn send_command_no_wait(&self, _command: &str) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "forced for test",
            ))
        }
    }

    /// Wait until the daemon lists `panes` panes, then push a client size
    /// for each: background windows' layouts are not pushed on their own,
    /// and the `refresh-client -C` broadcast is what lets the layout
    /// consumer create every mirror. Waiting on the pane list (not a sleep)
    /// means no window the test just created can miss its pump.
    pub(crate) fn pump_daemon_layouts(transport: &MuxTransport, panes: usize) {
        let mut listed: Vec<u64> = Vec::new();
        assert!(
            poll_until(|| {
                // The probe rides out one slow round-trip (the core's
                // fixed 10 s reply budget) instead of panicking; the
                // loop's deadline is the teeth.
                if let Ok(lines) = transport.query_settled("list-panes") {
                    listed = lines
                        .iter()
                        .filter_map(|l| {
                            l.trim()
                                .strip_prefix('%')
                                .and_then(|s| s.parse::<u64>().ok())
                        })
                        .collect();
                }
                listed.len() >= panes
            }),
            "the daemon never listed {panes} panes: {listed:?}"
        );
        for pane in listed {
            transport
                .send_command_no_wait(&format!("refresh-client -t %{pane} -C 80x24"))
                .expect("refresh-client");
        }
    }

    /// Poll `capture-pane` on `%pane` until `needle` shows up (or the
    /// daemon deadline passes). Returns the last capture either way; every
    /// caller asserts on it.
    pub(crate) fn capture_until(
        transport: &dyn crate::app::tmux_handler::tmux_state::TmuxTransport,
        pane: u64,
        needle: &str,
    ) -> String {
        let mut view = String::new();
        poll_until(|| {
            // The probe rides out one slow round-trip (the core's fixed
            // 10 s reply budget) instead of panicking; the loop's deadline
            // is the teeth.
            if let Ok(lines) = transport.query_settled(&format!("capture-pane -t %{pane} -p")) {
                view = lines.join("|");
            }
            view.contains(needle)
        });
        view
    }

    /// Print `name`'s value in `%pane`, bracketed so the capture can be
    /// parsed back out. The markers are split in the typed command so the
    /// shell's echo of the command line never matches.
    pub(crate) fn pane_var(transport: &MuxTransport, pane: u64, name: &str) -> String {
        let tag = format!("V{pane}{name}");
        transport
            .send_settled(&format!(
                "send-keys -t %{pane} -l 'echo \"<{tag}\"\"=${name}>\"'"
            ))
            .expect("send-keys");
        transport
            .send_settled(&format!("send-keys -t %{pane} Enter"))
            .expect("enter");
        let view = capture_until(transport, pane, &format!("<{tag}="));
        let start = view
            .find(&format!("<{tag}="))
            .unwrap_or_else(|| panic!("{name} never printed in %{pane}: {view:?}"));
        let rest = &view[start + tag.len() + 2..];
        rest[..rest.find('>').expect("closing marker")].to_string()
    }

    /// A `WindowState` with no window, renderer, or tabs — the same seam
    /// `dispatch_tests::test_window_state` uses — so the manners test can
    /// hold the real `apply_agent_pushes` receiver without a live daemon.
    pub(crate) fn manners_state() -> crate::app::window_state::WindowState {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime)
    }

    /// A daemon that accepts but never answers — the SIGSTOP shape,
    /// in-process: the socket stays open (no `SessionEnded`) while every
    /// reply wait runs out its full timeout. Serves exactly one
    /// connection; the stream is read to completion so client writes
    /// never fill kernel buffers mid-test.
    pub(crate) fn spawn_silent_daemon(path: &Path) {
        use par_term_emu_core_rust::mux::{accept_connection, bind_local_listener};
        use std::io::Read;
        let listener = bind_local_listener(path).expect("silent daemon binds");
        std::thread::spawn(move || {
            let Ok((mut stream, _abort)) = accept_connection(&listener) else {
                return;
            };
            let mut sink = [0u8; 4096];
            loop {
                match stream.read(&mut sink) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });
    }

    pub(crate) fn pending_with_client(name: &str, path: &std::path::Path) -> MuxAttachPending {
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        MuxAttachPending {
            name: name.to_string(),
            rx,
        }
    }
}
