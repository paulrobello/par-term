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
//! Compiled only under the `mux` feature (default-on; forwards the core's
//! `mux` module, first shipped in core 0.50).

use crate::app::tmux_handler::tmux_state::{TmuxState, TmuxTransport};
use crate::app::window_state::WindowState;
use crate::tmux::escape_keys_for_tmux;
use par_term_mux::{
    AgentEntry, AttachOutcome, MuxSessionClient, VersionCheck, check_daemon_version,
};
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::io;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long an in-flight fire-and-forget send may stay unanswered before
/// the poll loop reports the daemon unresponsive. Must stay comfortably
/// under the toast deadline the hung-daemon card requires (1 s) and well
/// under the core client's 10 s reply timeout it exists to pre-empt.
const DAEMON_UNRESPONSIVE_AFTER: Duration = Duration::from_millis(800);

/// How long a reply-needing `send_command` waits for the send worker to
/// release the client before failing fast. A worker that holds the client
/// this long is itself stuck on a daemon reply — queueing behind it would
/// freeze the caller for the worker's remaining timeout.
const SEND_LOCK_WAIT: Duration = Duration::from_millis(250);

/// The fire-and-forget outbox depth. Bounded so a wedged daemon cannot
/// accrue an unbounded backlog (typing and paste chunks enqueue far faster
/// than a 10 s reply timeout drains); overflow drops commands and counts
/// them for the health event.
const OUTBOX_CAPACITY: usize = 512;

/// The daemon transport: a par-mux client behind the [`TmuxTransport`]
/// seam. Interior mutability because the routing hooks that reach it
/// (`send_input_via_tmux`, `notify_tmux_of_resize`) hold `&WindowState`.
///
/// Sends split by need: commands whose reply nobody reads (keystrokes,
/// size pushes, pastes) are queued to the send worker, which alone waits
/// on daemon replies — the event loop never does. Reply-needing commands
/// still run inline under a bounded lock, and the worker shares the
/// client behind the same mutex, so replies stay strictly ordered.
pub(crate) struct MuxTransport {
    client: Arc<Mutex<MuxSessionClient>>,
    outbox: SyncSender<String>,
    health: Arc<MuxHealth>,
}

impl MuxTransport {
    fn new(client: MuxSessionClient) -> Self {
        let client = Arc::new(Mutex::new(client));
        let health = Arc::new(MuxHealth::default());
        let (outbox, inbox) = sync_channel::<String>(OUTBOX_CAPACITY);
        spawn_send_worker(Arc::clone(&client), inbox, Arc::clone(&health));
        Self {
            client,
            outbox,
            health,
        }
    }

    /// Connect to a daemon at `path`, spawning one when no live server
    /// owns it (losing the spawn race talks to the winner's daemon).
    /// Test entry: the app runtime reaches the daemon through
    /// [`Self::connect_or_spawn`] by session name; the wiring test binds
    /// an in-process server at an explicit path.
    #[cfg(test)]
    pub(crate) fn connect_or_spawn_at(path: &std::path::Path) -> io::Result<Self> {
        Ok(Self::new(MuxSessionClient::connect_or_spawn_at(path)?))
    }

    /// The wrapped client, locked. Callers run short command sequences
    /// (attach, resync, probes); the send worker interleaves between
    /// calls, never inside one, because each call releases its guard
    /// before the next borrows.
    pub(crate) fn client(&self) -> MutexGuard<'_, MuxSessionClient> {
        self.client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Lock the client for an inline reply-needing send, giving up (and
    /// failing fast) once the send worker has held it past
    /// [`SEND_LOCK_WAIT`] — the queue-behind-a-hung-worker freeze guard.
    fn lock_for_send(&self) -> io::Result<MutexGuard<'_, MuxSessionClient>> {
        let deadline = Instant::now() + SEND_LOCK_WAIT;
        loop {
            match self.client.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    return Ok(poisoned.into_inner());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::other(
                            "mux send worker still holds the client — daemon unresponsive?",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

/// The send worker: the only thread that waits on a daemon reply for
/// fire-and-forget commands. Each queued command still runs the full
/// `send` (write + reply wait), so reply blocks stay consumed and paired
/// in order — the queue changes WHERE the 10 s wait happens, never the
/// protocol. Exits when the transport drops and closes the outbox.
fn spawn_send_worker(
    client: Arc<Mutex<MuxSessionClient>>,
    inbox: Receiver<String>,
    health: Arc<MuxHealth>,
) {
    std::thread::spawn(move || {
        while let Ok(command) = inbox.recv() {
            let mut client = client.lock().unwrap_or_else(|p| p.into_inner());
            health.send_started();
            let result = client.send(&command);
            health.send_finished(&result);
        }
    });
}

/// Daemon liveness as the poll loop needs it: whether the send currently
/// in flight has gone unanswered past the threshold (or the worker saw a
/// reply timeout), with per-episode toast dedup so the signal fires once
/// per hang and once per recovery — never per frame.
#[derive(Default)]
pub(crate) struct MuxHealth {
    state: Mutex<MuxHealthState>,
}

#[derive(Default)]
struct MuxHealthState {
    /// When the worker started the send still in flight, if one is.
    started: Option<Instant>,
    /// The last finished send timed out (any success clears it).
    timed_out: bool,
    /// A toast is already on screen for this episode.
    toast_shown: bool,
    /// Commands dropped because the outbox was full.
    dropped: u64,
}

/// The transition [`MuxHealth::poll_event`] reports.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DaemonHealthSignal {
    Unresponsive,
    Recovered,
}

impl MuxHealth {
    /// Worker hook: a send just left the event loop toward the daemon.
    pub(crate) fn send_started(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.started = Some(Instant::now());
    }

    /// Worker hook: the send completed. A timeout marks the daemon
    /// unresponsive until some later send succeeds; anything else (reply
    /// or non-timeout error) proves liveness.
    pub(crate) fn send_finished(&self, result: &io::Result<Vec<String>>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.started = None;
        state.timed_out = matches!(
            result,
            Err(e) if e.kind() == io::ErrorKind::TimedOut
        );
    }

    /// Outbox overflow bookkeeping for the unresponsive toast.
    pub(crate) fn count_dropped(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.dropped += 1;
    }

    /// The poll-loop view: `Some(Unresponsive)` on the first frame a send
    /// has been unanswered past `threshold` (or a timeout was observed),
    /// `Some(Recovered)` on the first frame a flagged daemon answers
    /// again, `None` otherwise.
    pub(crate) fn poll_event(&self, threshold: Duration) -> Option<DaemonHealthSignal> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let hung = state.timed_out
            || state
                .started
                .is_some_and(|started| started.elapsed() >= threshold);
        match (hung, state.toast_shown) {
            (true, false) => {
                state.toast_shown = true;
                Some(DaemonHealthSignal::Unresponsive)
            }
            (false, true) => {
                state.toast_shown = false;
                Some(DaemonHealthSignal::Recovered)
            }
            _ => None,
        }
    }
}

impl TmuxTransport for MuxTransport {
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
        // A worker send mid-reply-wait holds the lock; that is exactly the
        // hung case, where no notifications exist to drain anyway. Skip
        // the poll rather than queue behind it on the event loop.
        let Ok(mut client) = self.client.try_lock() else {
            return (Vec::new(), false);
        };
        client.drain_core_notifications()
    }

    fn send_command(&self, command: &str) -> io::Result<Vec<String>> {
        let mut client = self.lock_for_send()?;
        client.send(command)
    }

    fn send_command_no_wait(&self, command: &str) -> io::Result<()> {
        match self.outbox.try_send(command.to_string()) {
            Ok(()) => Ok(()),
            // A full outbox means the daemon wedged while input kept
            // arriving: drop the command (the unresponsive toast says why)
            // rather than block the event loop or grow without bound.
            Err(TrySendError::Full(_)) => {
                self.health.count_dropped();
                Ok(())
            }
            // Worker gone (transport being dropped): fall back inline.
            Err(TrySendError::Disconnected(_)) => self.send_command(command).map(|_| ()),
        }
    }

    fn daemon_health_event(&self) -> Option<String> {
        let dropped = self
            .health
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .dropped;
        self.health
            .poll_event(DAEMON_UNRESPONSIVE_AFTER)
            .map(|signal| match signal {
                DaemonHealthSignal::Unresponsive => {
                    if dropped > 0 {
                        format!(
                            "par-mux daemon not responding — input dropped ({dropped} commands)"
                        )
                    } else {
                        "par-mux daemon not responding — input queued until it recovers".into()
                    }
                }
                DaemonHealthSignal::Recovered => "par-mux daemon responding again".into(),
            })
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

/// The attach sequence minus tab allocation, as a free function so the
/// wiring test can drive it against a live daemon without a
/// `WindowState`: query the daemon's build (the stale-daemon check —
/// first, because a daemon that predates the client is the cheapest
/// explanation for everything downstream misbehaving), create-or-attach,
/// report existing windows (the caller allocates tabs for them — a created
/// session gets its tab from the `%window-add` push), push the client size
/// and cell pixels (the first pane names the window the server resizes)
/// plus the client theme colors, collect each pane's replayed screen for
/// seeding (`refresh-client -t` replies carry the screen; they are NOT
/// `%output` pushes), and read the agent roster for the initial fill (A2b
/// task 1: `list-agents` on attach and reattach — the single call site of
/// the roster query in app code).
///
/// `env` is par-term's shell environment (`build_shell_env`), handed to
/// the daemon as the session environment so mux panes match local tabs:
/// `new-session -e` on create, `set-environment` on reattach (which also
/// refreshes it after a par-term update or `shell_env` change).
pub(crate) fn attach_sequence(
    transport: &MuxTransport,
    name: &str,
    size: Option<(u16, u16)>,
    cell_px: Option<(u16, u16)>,
    colors: &(String, String),
    env: &std::collections::HashMap<String, String>,
) -> io::Result<AttachSequence> {
    // Degrade to None rather than failing the attach: the version query is
    // diagnostic, and a daemon that cannot answer it is handled by the
    // mismatch check, not by refusing to attach.
    let daemon_version = transport.client().daemon_version().ok();
    let outcome = transport.client().create_or_attach_with_env(name, env)?;
    let mut existing_windows = Vec::new();
    let mut window_names = Vec::new();
    if matches!(outcome, AttachOutcome::Attached(_)) {
        for window in transport.client().list_windows()? {
            existing_windows.push(window.id);
            // Keep the daemon's own names so the display tabs carry them
            // instead of the placeholder "tmux @N" (handle_tmux_window_add
            // cannot know them — the %window-add notification carries only
            // the id).
            if !window.name.is_empty() {
                window_names.push((window.id, window.name));
            }
        }
    }
    // Bind the list before looping: a RefMut in the `for` expression would
    // live through the body, where the next command borrows again.
    let panes = transport.client().list_panes()?;
    if let (Some((cols, rows)), Some(first)) = (size, panes.first()) {
        // Cell pixels default to a sane floor when the renderer is not up
        // yet (attach before first frame): the daemon re-derives from the
        // next resize push, which always carries the real metrics.
        let cell_px = cell_px.unwrap_or((10, 20));
        transport
            .client()
            .set_client_size(*first, cols, rows, cell_px)?;
    }
    // Theme colors: re-reported on every attach (the daemon does not
    // persist them), so pane OSC 10/11 answers match the client theme from
    // the first frame. A stale daemon answers an error block, which `send`
    // returns as a body — the attach proceeds.
    if let Err(e) = transport.client().set_client_colors(&colors.0, &colors.1) {
        crate::debug_error!("MUX", "set-client-colors on attach failed: {e}");
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
    // Read each pane's daemon title back — the reattach half of pane
    // renaming. `pane-title` answers with the EFFECTIVE title (user `-T`
    // when set, else the pane's OSC title), which does not say which kind
    // it is; the clear-and-requery probe decides: clearing the user title
    // changes the answer only when one was set. The probe restore re-sets
    // a user title it found (its broadcast re-affirms the same value).
    let mut titles = Vec::new();
    // `panes` was consumed by the screens loop; its members survive there.
    for pane in screens.iter().map(|(pane, _)| *pane) {
        let effective = transport.client().pane_title(pane).unwrap_or_else(|e| {
            crate::debug_error!("MUX", "pane-title query failed for %{pane}: {e}");
            String::new()
        });
        if effective.is_empty() {
            continue;
        }
        let is_user = probe_pane_title_is_user(transport, pane, &effective);
        titles.push((pane, (effective, is_user)));
    }
    Ok(AttachSequence {
        daemon_version,
        outcome,
        existing_windows,
        window_names,
        screens,
        agents,
        titles,
    })
}

/// Decide whether `effective` (a pane's queried title) is a user `-T`
/// title or the pane program's OSC title: clear the user title, re-query,
/// and compare — the answer moves only when a user title was set. A user
/// title is restored before returning, so the daemon keeps owning it.
fn probe_pane_title_is_user(transport: &MuxTransport, pane: TmuxPaneId, effective: &str) -> bool {
    let clear = format!("select-pane -t %{pane} -T ''");
    if let Err(e) = transport.client().send(&clear) {
        crate::debug_error!("MUX", "pane-title probe clear failed for %{pane}: {e}");
        return false;
    }
    let after_clear = transport.client().pane_title(pane).unwrap_or_default();
    if after_clear == effective {
        // No user title was set — the clear changed nothing.
        return false;
    }
    // A user title was set (and the clear just removed it): restore it.
    let restore = format!(
        "select-pane -t %{pane} -T {}",
        par_term_mux::quote_env_value(effective)
    );
    if let Err(e) = transport.client().send(&restore) {
        crate::debug_error!("MUX", "pane-title probe restore failed for %{pane}: {e}");
    }
    true
}

/// Why attaching to `target_socket` would render this window inside
/// itself: par-term launched from a par-mux pane inherits the pane's
/// identity (`PAR_MUX_ENV=1` + `PAR_MUX_SOCKET`, the pane env contract in
/// core `mux::pane`), so attaching to the daemon that owns that pane
/// mirrors the owning session into the pane that owns it — a display
/// feedback loop. `None` when the attach is fine: not inside a pane, or
/// targeting a different daemon.
///
/// Attaching to a DIFFERENT daemon from inside a pane stays the core
/// nesting rule's case: connecting to a live server is allowed, and
/// auto-spawning one is refused by core `nested_daemon_refusal` (override
/// `PAR_MUX_ALLOW_NESTED=1`) inside `MuxClient::connect_or_spawn_at` —
/// the worker in [`WindowState::begin_mux_session_attach`] surfaces that
/// refusal through its error toast unchanged.
pub(crate) fn mux_attach_refusal(target_socket: &std::path::Path) -> Option<&'static str> {
    std::env::var_os("PAR_MUX_ENV")?;
    let outer = std::env::var_os("PAR_MUX_SOCKET")?;
    if std::path::Path::new(&outer) == target_socket {
        return Some(
            "this par-term already runs inside that par-mux session — attaching \
             would render the session inside itself",
        );
    }
    None
}

/// Give the next pane spawned in session `$session` its own
/// `ITERM_SESSION_ID`. The variable lives in the shared session
/// environment, so par-term restamps it before every pane it asks the
/// daemon for; panes spawned by anyone else (an agent calling
/// `PAR_MUX_BIN`, the daemon's restore after a restart) reuse the last
/// stamp. Failure is logged by name only and never blocks the split.
fn stamp_pane_session_id(transport: &dyn TmuxTransport, session: Option<u64>) {
    let Some(session) = session else {
        return;
    };
    let value = par_term_mux::quote_env_value(&format!("w0t0p0:{}", uuid::Uuid::new_v4()));
    // The reply body is dropped unread: an %error from a daemon that
    // predates set-environment is harmless here (the pane still spawns).
    if let Err(e) = transport.send_command(&format!(
        "set-environment -t ${session} ITERM_SESSION_ID {value}"
    )) {
        crate::debug_error!("MUX", "set-environment ITERM_SESSION_ID failed: {e}");
    }
}

/// What [`attach_sequence`] learned — the tuple it returned, named: the
/// daemon's `version` reply (raw; `None` only when the query itself failed
/// at transport level), the attach outcome, windows needing tabs, those
/// windows' daemon names, per-pane replayed screens, the roster fill, and
/// per-pane daemon titles (`(title, is_user)` — the reattach restore of
/// pane renaming).
pub(crate) struct AttachSequence {
    pub(crate) daemon_version: Option<String>,
    pub(crate) outcome: AttachOutcome,
    pub(crate) existing_windows: Vec<TmuxWindowId>,
    pub(crate) window_names: Vec<(TmuxWindowId, String)>,
    pub(crate) screens: Vec<(TmuxPaneId, Vec<u8>)>,
    pub(crate) agents: Vec<AgentEntry>,
    pub(crate) titles: Vec<(TmuxPaneId, (String, bool))>,
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
    pub(crate) name: String,
    pub(crate) rx: std::sync::mpsc::Receiver<io::Result<par_term_emu_core_rust::mux::MuxClient>>,
}

/// What [`WindowState::launch_agent_via_mux`] did with a launch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MuxLaunchOutcome {
    /// No transport attached / no daemon pane resolvable — the caller
    /// falls through to the local launch path.
    NotMux,
    /// The agent command was typed into a fresh daemon pane.
    Launched,
    /// A daemon target resolved but a wire step failed (toast shown) —
    /// consumed, never fall through to a local tab beside the daemon.
    Failed,
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
        stamp_pane_session_id(transport.as_ref(), self.tmux_state.mux_session_id);
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
            // rejects a bad TARGET with an %error block, which also
            // arrives as an Ok body — non-empty IS the failure signal,
            // same shape as the split path. A valid target is always
            // killed, including a window's last pane (which closes the
            // window — the last-pane guard in `close_focused_pane` keeps
            // par-term from sending that).
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

    /// Launch a configured agent into a new daemon-side pane (the
    /// agent-launcher palette's mux arm). Splits the resolved mux pane,
    /// then types the command line into the new pane's shell: the daemon
    /// spawns shells on `split-window` (no command argument exists on the
    /// wire), so typing is the launch mechanism, and the pane lands in the
    /// current daemon window where the agent roster's hooks report it.
    ///
    /// The split's reply must carry the new pane id before any keys are
    /// sent — `mux_focused_pane` may hold a stale id from an earlier
    /// interaction, so it is never trusted as the send target here.
    pub(crate) fn launch_agent_via_mux(&mut self, command_line: &str) -> MuxLaunchOutcome {
        let Some(transport) = &self.tmux_state.transport else {
            return MuxLaunchOutcome::NotMux;
        };
        let Some(target) = self.any_mux_pane() else {
            crate::debug_trace!("MUX", "agent launch skipped — no daemon pane resolvable");
            return MuxLaunchOutcome::NotMux;
        };
        stamp_pane_session_id(transport.as_ref(), self.tmux_state.mux_session_id);
        let reply = match transport.send_command(&format!("split-window -h -t %{target}")) {
            Ok(reply) => reply,
            Err(e) => {
                log::error!("par-mux agent-launch split failed: {e}");
                self.show_toast(format!("par-mux: launch failed — {e}"));
                return MuxLaunchOutcome::Failed;
            }
        };
        let Some(pane) = reply.iter().find_map(|line| {
            line.trim()
                .strip_prefix('%')
                .and_then(|s| s.parse::<u64>().ok())
        }) else {
            // An %error block arrives as an Ok reply body with no pane id —
            // the same failure signal as the split path. The daemon target
            // resolved, so this is consumed-failed, never fall-through.
            let body = reply.join("\n");
            log::error!("par-mux agent-launch split rejected: {body}");
            self.show_toast(format!("par-mux: launch failed — {body}"));
            return MuxLaunchOutcome::Failed;
        };
        let keys = [
            format!(
                "send-keys -t %{pane} -l {}",
                par_term_mux::quote_env_value(command_line)
            ),
            format!("send-keys -t %{pane} Enter"),
        ];
        for cmd in keys {
            if let Err(e) = transport.send_command(&cmd) {
                log::error!("par-mux agent-launch {cmd:?} failed: {e}");
                self.show_toast(format!("par-mux: launch failed — {e}"));
                return MuxLaunchOutcome::Failed;
            }
        }
        log::info!("MUX: agent launched in %{pane}");
        MuxLaunchOutcome::Launched
    }

    /// Resolve any daemon pane to act as a launch anchor: the explicit mux
    /// focus, then the focused native pane's mapping (input routing's
    /// resolution), then the first mux tab's focused pane in tab order —
    /// so launching from a local tab while attached still lands daemon-side
    /// where the roster sees it, instead of a local tab whose command write
    /// would miss the daemon pane.
    fn any_mux_pane(&self) -> Option<u64> {
        if let Some(pane) = self.tmux_state.mux_focused_pane {
            return Some(pane);
        }
        if let Some(pane) = self.focused_mux_pane_from_native() {
            return Some(pane);
        }
        self.tab_manager.tabs().iter().find_map(|tab| {
            let pane_id = tab.pane_manager()?.focused_pane()?.id;
            self.tmux_state.tmux_pane_in_tab(tab.id, pane_id)
        })
    }

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
            self.show_toast(format!("par-mux: attach to '{name}' refused — {reason}"));
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
                let transport = MuxTransport::new(MuxSessionClient::from_core(client));
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
                // Swap the placeholder "tmux @N" titles for the daemon's own
                // window names — same setter the %window-renamed push uses,
                // so a daemon rename later lands identically.
                for (window_id, name) in window_names {
                    if let Some(tab_id) = self.tmux_state.tmux_sync.get_tab(window_id)
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.set_title(&name);
                    }
                }
                // Screens land once the layout consumers create the panes
                // (checked each poll in check_mux_notifications).
                self.tmux_state.mux_screen_seeds = screens
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
                self.tmux_state.transport = Some(Box::new(transport));
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
        self.tmux_state.mux_pane_titles.clear();
        self.tmux_state.agent_roster.clear();
        self.handle_tmux_session_ended();
        // Overwrite the shared cleanup's "tmux: Session ended" toast: the
        // session did not end, it survives in the daemon.
        self.show_toast("par-mux: detached (session keeps running in the daemon)");
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        true
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
        self.tmux_state.mux_pane_titles.clear();
        self.tmux_state.agent_roster.clear();
        self.handle_tmux_session_ended();
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
    use super::*;
    use crate::tmux::{ParserBridge, TmuxSync};
    use par_term_emu_core_rust::mux::MuxServer;
    use par_term_tmux::SyncAction;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// Marker echoed inside a pane before detach; the reattach screen
    /// replay must carry it back as `PaneOutput`.
    const MARKER: &str = "par-term-mux-wiring-marker";

    /// [`attach_sequence`] with the metrics a headless test cannot know:
    /// no renderer exists to measure cells, no config to theme from. The
    /// color pair is a real shape (`rrggbb`), not zeros, so a daemon-side
    /// rejection would be visible in the attach assertions.
    fn attach_test(
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
                .send_keys(0, format!("echo {MARKER}\r").as_bytes())
                .expect("send-keys marker");
            // Wait for the echo to land before the drop: a fixed sleep raced
            // shell startup (cmd.exe's banner on Windows outlasted 300 ms).
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let screen = first
                    .send("capture-pane -t %0 -p")
                    .expect("capture-pane")
                    .join("\n");
                if screen.lines().any(|line| line.trim() == MARKER) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "the marker never printed: {screen:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        // Reattach through the app's attach sequence: the transport, the
        // create-or-attach, the window list, the client size report, and
        // the per-pane screen replay — everything a profile-open attach
        // runs short of tab allocation. The size differs from the daemon's
        // 80x24 default so the `-C` refit genuinely broadcasts a layout.
        let transport = connect(&path);
        let attach = attach_test(&transport, "wiring", Some((120, 40)), &Default::default())
            .expect("attach_sequence");
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
    /// `version`, and against an in-process daemon at the client's own
    /// version the comparison must never claim `Mismatch` — the positive
    /// control for the toast path, which fires only on `Mismatch`. A core
    /// built from crates.io stamps `+unknown`, so the honest classification
    /// is `Unknown` there; `Match` itself is covered by client.rs's unit
    /// tests with known shas.
    #[test]
    fn attach_reads_the_daemon_version_and_a_current_daemon_matches() {
        let path = socket_path("version");
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_test(
            &transport,
            "version-check",
            Some((80, 24)),
            &Default::default(),
        )
        .expect("attach");
        let daemon_reply = attach
            .daemon_version
            .expect("the daemon answered `version` during the attach sequence");
        let client_stamp = par_term_emu_core_rust::mux::build_stamp();
        let verdict = check_daemon_version(&daemon_reply, client_stamp);
        assert!(
            !matches!(verdict, VersionCheck::Mismatch { .. }),
            "same-version daemon/client must not read stale: daemon replied \
             {daemon_reply:?}, client stamp {client_stamp:?}, verdict {verdict:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dropping_the_transport_leaves_sessions_alive() {
        let path = socket_path("survive");
        spawn_daemon(&path);

        {
            let transport = connect(&path);
            let attach = attach_test(&transport, "keep", Some((80, 24)), &Default::default())
                .expect("attach");
            assert!(matches!(attach.outcome, AttachOutcome::Created(_)));
            // Dropping the transport drops the socket — detach, D5.
        }

        let second = connect(&path);
        let attach = attach_test(&second, "keep", None, &Default::default()).expect("reattach");
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
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
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
            ws.tmux_state.tmux_pane_owners.contains_key(&0),
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

        // The install path recorded the session id, so the split stamped
        // its own ITERM_SESSION_ID (card 01a0d93b1c03 criterion 2).
        assert!(ws.tmux_state.mux_session_id.is_some());
        let transport = MuxTransport::new(probe);
        #[cfg(unix)]
        assert_ne!(
            pane_var(&transport, 0, "ITERM_SESSION_ID"),
            pane_var(&transport, 1, "ITERM_SESSION_ID"),
            "a split pane must not share the first pane's ITERM_SESSION_ID"
        );
        drop(transport);

        let _ = std::fs::remove_file(&path);
    }

    /// Two daemon windows become two tabs whose PaneManagers each number
    /// their panes from 1 again, so the pane mappings must be keyed per
    /// tab. The window-wide maps were REPLACED by each arriving layout:
    /// window @0's mappings vanished when @1's layout landed, and native
    /// pane 1 in both tabs collided, letting input and output cross
    /// between windows. Verified failing against the flat maps before the
    /// fix; asserts mappings, output, input, and daemon focus pushes all
    /// stay confined to each window's own tab.
    #[test]
    fn two_windows_keep_both_windows_panes_mapped() {
        let path = socket_path("mux-two-windows");
        spawn_daemon(&path);

        // Attach through the app's own install path (poll arm).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "twowin".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");

        // Window @0 -> tab A with pane %0 mapped. A created session's
        // %window-add fired before the transport existed, so the tab is
        // driven manually (the split test's pump).
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "window @0 never got a mapped pane"
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

        // Window @1 -> tab B. A fresh session numbers its panes %0, %1 in
        // creation order, so the second window owns %1.
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("new-window")
            .expect("new-window");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "window @1 never got a mapped pane"
            );
            ws.tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("refresh-client -t %1 -C 80x24")
                .expect("size push broadcasts %layout-change for @1");
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }

        // The corruption under window-wide maps: @1's layout replaced the
        // mappings, so @0's pane no longer resolved.
        assert!(
            ws.tmux_state.tmux_pane_owners.contains_key(&0),
            "window @0's pane stayed mapped after window @1 arrived"
        );
        assert!(
            ws.tmux_state.tmux_pane_owners.contains_key(&1),
            "window @1's pane is mapped"
        );
        let (tab_a, pane_a) = ws.tmux_state.tmux_pane_owners[&0];
        let (tab_b, pane_b) = ws.tmux_state.tmux_pane_owners[&1];
        assert_ne!(tab_a, tab_b, "each daemon window owns its own tab");

        // Output confinement: %1's bytes land in tab B's pane only. Under
        // the flat maps the all-tabs scan found tab A's colliding pane id
        // first and wrote there.
        ws.handle_tmux_output(1, b"two-windows-b\r");
        assert!(
            pane_text(&ws, tab_b, pane_b).contains("two-windows-b"),
            "window @1's output reaches its own pane"
        );
        assert!(
            !pane_text(&ws, tab_a, pane_a).contains("two-windows-b"),
            "window @1's output must not leak into window @0's pane"
        );

        // Daemon focus confinement: a %pane-focus-changed push for %1
        // focuses the pane inside tab B, not the (active) tab A's pane.
        ws.handle_tmux_pane_focus_changed(1);
        let focused_in = |tab_id: crate::tab::TabId| {
            ws.tab_manager
                .get_tab(tab_id)
                .and_then(|tab| tab.pane_manager())
                .and_then(|pm| pm.focused_pane())
                .map(|pane| pane.id)
        };
        assert_eq!(
            focused_in(tab_b),
            Some(pane_b),
            "the daemon's focus push moves focus inside the owning tab"
        );

        // Input confinement: the focused tab's own pane picks the daemon
        // target, whichever window is on screen.
        ws.tab_manager.switch_to(tab_b);
        assert_eq!(
            ws.focused_mux_pane_from_native(),
            Some(1),
            "keys typed in window @1's tab target window @1's pane"
        );
        ws.tab_manager.switch_to(tab_a);
        assert_eq!(
            ws.focused_mux_pane_from_native(),
            Some(0),
            "keys typed in window @0's tab still target window @0's pane"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A pane terminal's screen text — the read side of output routing.
    fn pane_text(
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

    /// Search and copy mode must read the focused pane's terminal — the
    /// daemon mirror — in a mux tab. `tab.terminal` there is a hidden
    /// local login shell, so Cmd+F, copy-mode entry/search, and line
    /// motions looked at a screen the user cannot see. Proven red against
    /// the tab-terminal reads before the fix.
    #[test]
    fn search_and_copy_mode_read_the_mux_mirror_not_the_hidden_shell() {
        let path = socket_path("mux-search-mirror");
        spawn_daemon(&path);

        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "findme".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");

        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "window @0 never got a mapped pane"
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
        let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
        ws.tab_manager.switch_to(tab_id);

        // Seed ONLY the mirror: daemon output routes to the mapped mirror,
        // never to the tab's hidden shell.
        ws.handle_tmux_output(0, b"mux-mirror-needle 4999\r\n");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !pane_text(&ws, tab_id, pane).contains("mux-mirror-needle") {
            assert!(
                Instant::now() < deadline,
                "the mirror never showed the marker"
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(25));
        }

        // The mirror's own cursor state, for the entry assertions below.
        let (mirror_col, mirror_row, mirror_sb) = ws
            .tab_manager
            .get_tab(tab_id)
            .and_then(|tab| tab.pane_manager())
            .and_then(|pm| pm.get_pane(pane))
            .and_then(|p| p.terminal.try_read().ok())
            .map(|term| {
                let (col, row) = term.cursor_position();
                (col, row, term.scrollback_len())
            })
            .expect("mirror terminal");

        // Cmd+F's searchable lines come from the focused pane's terminal.
        let searchable = ws
            .tab_manager
            .get_tab(tab_id)
            .and_then(|tab| {
                tab.try_with_read_terminal(|term| {
                    crate::app::window_state::search_highlight::get_all_searchable_lines(
                        term,
                        mirror_row + 1,
                    )
                    .map(|(_, line)| line)
                    .collect::<Vec<_>>()
                })
            })
            .expect("a readable terminal");
        assert!(
            searchable.iter().any(|l| l.contains("mux-mirror-needle")),
            "Cmd+F must search the mirror's screen, got {searchable:?}"
        );

        // Copy-mode entry anchors on the mirror's cursor, not the hidden
        // shell's.
        ws.enter_copy_mode();
        assert!(
            ws.copy_mode.active,
            "copy mode must enter (copy_mode_enabled defaults on)"
        );
        assert_eq!(
            (ws.copy_mode.cursor_col, ws.copy_mode.cursor_absolute_line),
            (mirror_col, mirror_sb + mirror_row),
            "copy-mode entry anchors on the mirror's cursor"
        );

        // Copy-mode search finds the marker through the mirror.
        ws.copy_mode.search_query = "mux-mirror-needle".to_string();
        ws.copy_mode.search_direction = crate::copy_mode::SearchDirection::Forward;
        ws.execute_copy_mode_search(false);
        let line = ws.get_copy_mode_line_text().expect("line under the cursor");
        assert!(
            line.contains("mux-mirror-needle"),
            "copy-mode search must land on the mirror's marker line, got {line:?}"
        );

        // And the yank of that line reads the same mirror, so every read
        // path agrees on one terminal. The daemon pane's shell prompt shares
        // the marker's row (the raw write landed at the pane cursor), so the
        // selection spans the marker's columns, not the whole row.
        let (needle_line, needle_col) =
            (ws.copy_mode.cursor_absolute_line, ws.copy_mode.cursor_col);
        ws.copy_mode.visual_mode = crate::copy_mode::VisualMode::Char;
        ws.copy_mode.selection_anchor = Some((needle_line, needle_col));
        ws.copy_mode.cursor_col = needle_col + "mux-mirror-needle".len();
        ws.sync_copy_mode_selection();
        let yanked = ws.get_selected_text_for_copy().expect("selection text");
        assert!(
            yanked.contains("mux-mirror-needle"),
            "yank must read the mirror, got {yanked:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The inspector's snapshot and command history must read the focused
    /// pane's terminal — the daemon mirror — in a mux tab. `tab.terminal`
    /// there is a hidden local login shell, so the agent reasoned about a
    /// screen and history it cannot see. Seeds a full OSC 133 command
    /// lifecycle into the mirror the way daemon `%output` delivers it, then
    /// asserts the snapshot carries that command.
    #[test]
    fn inspector_snapshot_reads_the_mux_mirror_not_the_hidden_shell() {
        let path = socket_path("mux-inspector-read");
        spawn_daemon(&path);

        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "inspect".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");

        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "window @0 never got a mapped pane"
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
        let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
        ws.tab_manager.switch_to(tab_id);

        // Seed ONLY the mirror with a complete shell-integration command
        // lifecycle, exactly as the daemon pane's shell would emit it.
        ws.handle_tmux_output(
            0,
            b"\x1b]133;C;echo mirror-cmd\x07mirror-output\r\n\x1b]133;D;7\x07",
        );
        // Bridge the queued OSC events into command history — the same
        // update the render pipeline's pane gather performs.
        {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            let p = tab
                .pane_manager()
                .and_then(|pm| pm.get_pane(pane))
                .expect("mirror pane");
            let mut term = p.terminal.try_write().expect("mirror write lock");
            let sb_len = term.scrollback_len();
            let (_, cursor_row) = term.cursor_position();
            term.update_scrollback_metadata(sb_len, cursor_row);
        }
        let mirror_history = {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            tab.try_with_read_terminal(|term| term.core_command_history())
        }
        .expect("readable mirror terminal");
        assert!(
            mirror_history
                .iter()
                .any(|(cmd, ..)| cmd == "echo mirror-cmd"),
            "seed must land in the mirror's command history, got {mirror_history:?}"
        );

        // The inspector snapshot must carry the mirror's command, not the
        // hidden shell's (empty) history.
        ws.overlay_ui.ai_inspector.open = true;
        ws.overlay_ui.ai_inspector.needs_refresh = true;
        ws.refresh_inspector_snapshot();
        let snapshot = ws
            .overlay_ui
            .ai_inspector
            .snapshot
            .as_ref()
            .expect("snapshot must gather");
        assert!(
            !ws.overlay_ui.ai_inspector.needs_refresh,
            "a successful gather clears the refresh flag"
        );
        let commands: Vec<_> = snapshot
            .commands
            .iter()
            .map(|c| c.command.as_str())
            .collect();
        assert!(
            commands.contains(&"echo mirror-cmd"),
            "the snapshot must read the mirror's command history, got {commands:?}"
        );
        let exit = snapshot
            .commands
            .iter()
            .find(|c| c.command == "echo mirror-cmd")
            .and_then(|c| c.exit_code);
        assert_eq!(
            exit,
            Some(7),
            "the mirror's exit code must survive the gather"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Run-and-notify's exit-status poll must watch the focused mirror in
    /// a mux tab: the command runs in the daemon pane, so only the
    /// mirror's shell-integration history ever records its exit code. The
    /// differential half pins the defect — polling the hidden local shell
    /// (`tab.terminal`, the pre-fix wiring) never sees the command and the
    /// agent is told "unknown exit code".
    #[test]
    fn run_and_notify_exit_poll_watches_the_mux_mirror() {
        let path = socket_path("mux-inspector-poll");
        spawn_daemon(&path);

        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "polltest".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");

        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "window @0 never got a mapped pane"
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
        let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
        ws.tab_manager.switch_to(tab_id);

        // Baseline recorded before the command lands, as run-and-notify
        // does, plus the two handles the poll could be wired to.
        let (baseline, mirror_handle, hidden_shell) = {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            (
                tab.try_with_read_terminal(|term| term.core_command_history().len())
                    .expect("readable mirror terminal"),
                tab.read_terminal_handle(),
                std::sync::Arc::clone(&tab.terminal),
            )
        };

        // The command completes in the daemon pane: its OSC 133 lifecycle
        // reaches the mirror through %output, events bridged as the pane
        // gather does.
        ws.handle_tmux_output(0, b"\x1b]133;C;pwd\x07/home/user\r\n\x1b]133;D;0\x07");
        {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            let p = tab
                .pane_manager()
                .and_then(|pm| pm.get_pane(pane))
                .expect("mirror pane");
            let mut term = p.terminal.try_write().expect("mirror write lock");
            let sb_len = term.scrollback_len();
            let (_, cursor_row) = term.cursor_position();
            term.update_scrollback_metadata(sb_len, cursor_row);
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("poll runtime");
        let exit = runtime.block_on(
            crate::app::window_state::action_handlers::inspector::poll_command_exit(
                Some(mirror_handle),
                baseline,
                300,
            ),
        );
        assert_eq!(exit, Some(0), "the poll must read the mirror's exit code");

        // Differential: the hidden local shell never saw the command — the
        // pre-fix wiring times out with no exit code.
        let none = runtime.block_on(
            crate::app::window_state::action_handlers::inspector::poll_command_exit(
                Some(hidden_shell),
                baseline,
                2,
            ),
        );
        assert_eq!(none, None, "polling the hidden shell must find nothing");

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
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
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
        while !ws.tmux_state.tmux_pane_owners.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the layout consumer never mapped the split pane %1: {:?}",
                ws.tmux_state.tmux_pane_owners
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
        while ws.tmux_state.tmux_pane_owners.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the killed pane's mapping was never reconciled away: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            ws.tmux_state.tmux_pane_owners.contains_key(&0),
            "the surviving pane's mapping must remain"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d95dd318 criterion 3, read-back half: a daemon
    /// `%pane-title-changed` push (set from a shell via `par-mux -c`, by
    /// another client, or by par-term's own rename) applies to the mapped
    /// native pane with user semantics — the name lands, the pane is
    /// marked user-named, and the daemon's clear reverts it to automatic
    /// titles.
    #[test]
    fn daemon_pane_title_pushes_apply_and_clear_reverts() {
        let path = socket_path("title-push");
        spawn_daemon(&path);

        // Attach + map %0 (the roster test's ladder).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "titlepush".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
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
        let native = ws.tmux_state.tmux_pane_owners[&0].1;
        let pane_title = |ws: &WindowState| -> Option<String> {
            for tab in ws.tab_manager.tabs() {
                if let Some(pane) = tab.pane_manager().and_then(|pm| pm.get_pane(native)) {
                    return Some(pane.title.clone());
                }
            }
            None
        };
        let pane_is_user_named = |ws: &WindowState| -> bool {
            ws.tab_manager
                .tabs()
                .iter()
                .find_map(|tab| {
                    tab.pane_manager()
                        .and_then(|pm| pm.get_pane(native))
                        .map(|p| p.user_named)
                })
                .unwrap_or(false)
        };

        // Another client renames the pane daemon-side: the broadcast must
        // land through the app's own drain → pane-title applier.
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("select-pane -t %0 -T 'build box'")
            .expect("rename daemon-side");
        let mut titled = false;
        while Instant::now() < deadline && !titled {
            ws.check_mux_notifications();
            titled = pane_title(&ws).as_deref() == Some("build box") && pane_is_user_named(&ws);
            if !titled {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            titled,
            "the daemon title push must land as a user-named title: {:?} user={}",
            pane_title(&ws),
            pane_is_user_named(&ws)
        );

        // The daemon's clear reverts the pane to automatic titles.
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("select-pane -t %0 -T ''")
            .expect("clear daemon-side");
        let mut reverted = false;
        while Instant::now() < deadline && !reverted {
            ws.check_mux_notifications();
            reverted = !pane_is_user_named(&ws) && pane_title(&ws).as_deref() != Some("build box");
            if !reverted {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            reverted,
            "the clear push must revert user-naming: {:?} user={}",
            pane_title(&ws),
            pane_is_user_named(&ws)
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d95dd318 criterion 3, write half: renaming through
    /// par-term's own write path (`WindowState::rename_pane`, what the
    /// title-bar popup and the `rename_pane` action drive) pushes
    /// `select-pane -T` daemon-side — the daemon owns the name — and a
    /// blank rename clears it there.
    #[cfg(unix)]
    #[test]
    fn rename_pane_pushes_select_pane_t_to_the_daemon() {
        let path = socket_path("rename-push");
        spawn_daemon(&path);

        // Attach + map %0 (the title-push test's ladder).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "renamepush".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
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
        let native = ws.tmux_state.tmux_pane_owners[&0].1;
        let local_title = |ws: &WindowState| -> Option<String> {
            ws.tab_manager.tabs().iter().find_map(|tab| {
                tab.pane_manager()
                    .and_then(|pm| pm.get_pane(native))
                    .map(|p| p.title.clone())
            })
        };

        // Rename through the app's write path: the local pane is named
        // immediately AND the daemon holds the title (queried back).
        ws.rename_pane(native, "build box");
        assert_eq!(
            local_title(&ws).as_deref(),
            Some("build box"),
            "the local pane is named in the same call"
        );
        let reply = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("pane-title -t %0")
            .expect("pane-title query");
        assert!(
            reply.iter().any(|l| l.trim() == "build box"),
            "the daemon owns the name: {reply:?}"
        );

        // Blank rename clears the daemon-side title too.
        ws.rename_pane(native, "");
        let reply = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("pane-title -t %0")
            .expect("pane-title query");
        assert!(
            !reply.iter().any(|l| l.trim() == "build box"),
            "a blank rename clears the daemon title: {reply:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d95dd318 criterion 4, mux half: a pane's daemon title
    /// survives detach/reattach, and the attach probe tells a user `-T`
    /// title (re-marked user-named) from the pane program's OSC title
    /// (restored as a plain, still-live title).
    #[cfg(unix)]
    #[test]
    fn reattach_probe_distinguishes_user_titles_from_osc_titles() {
        let path = socket_path("title-probe");
        spawn_daemon(&path);

        let transport = connect(&path);
        attach_test(&transport, "probe", None, &Default::default()).expect("attach");
        // User-rename %0; give %1 (split) a program OSC title.
        transport
            .send_command("split-window -h -t %0")
            .expect("split");
        transport
            .send_command("select-pane -t %0 -T 'renamed pane'")
            .expect("user rename");
        transport
            .send_command("send-keys -t %1 -l 'printf \"\\033]0;osc pane\\007\"'")
            .expect("send printf");
        transport
            .send_command("send-keys -t %1 Enter")
            .expect("enter");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let title = transport.client().pane_title(1).expect("pane-title query");
            if title == "osc pane" {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the OSC title never reached the daemon: {title:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(transport);

        // Reattach through the app's sequence: the probe must report the
        // user title as user (and restore it daemon-side) and the OSC
        // title as plain.
        let second = connect(&path);
        let attach = attach_test(&second, "probe", None, &Default::default()).expect("reattach");
        let find = |pane: u64| {
            attach
                .titles
                .iter()
                .find(|(p, _)| *p == pane)
                .map(|(_, (title, is_user))| (title.clone(), *is_user))
        };
        assert_eq!(
            find(0).as_ref().map(|(t, u)| (t.as_str(), *u)),
            Some(("renamed pane", true)),
            "the user title survives and is marked user: {:?}",
            attach.titles
        );
        assert_eq!(
            find(1).as_ref().map(|(t, u)| (t.as_str(), *u)),
            Some(("osc pane", false)),
            "the OSC title survives and is marked plain: {:?}",
            attach.titles
        );
        // The probe restored the user title it cleared to decide.
        assert_eq!(
            second.client().pane_title(0).expect("restored"),
            "renamed pane"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d9b3903a: closing a pane removes its roster entry. The
    /// daemon sends no removal push, so the %layout-change reconciliation
    /// (`handle_pane_removal`) is the close signal for the cache — before
    /// it, the status widget kept counting a closed pane's agent as
    /// blocked and the picker offered a row focus could not land on.
    #[test]
    fn closing_a_pane_removes_its_roster_entry() {
        let path = socket_path("roster-close");
        spawn_daemon(&path);

        // Attach + map %0 (the close test's ladder).
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "rosterclose".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
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
        assert!(ws.split_pane_via_mux(true), "split gives %1 to close");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the layout consumer never mapped the split pane %1"
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }

        // Both panes report: the widget counts both, scoped to mapped panes
        // exactly as the egui refresh site scopes them.
        ws.apply_agent_pushes(vec![
            push("%0", "kimi", "working", "hook"),
            push("%1", "claude", "blocked", "hook"),
        ]);
        let scoped_summary = |ws: &WindowState| {
            let map = &ws.tmux_state.tmux_pane_owners;
            ws.tmux_state
                .agent_roster
                .summary_line(&|pane| map.contains_key(&pane))
        };
        assert_eq!(
            scoped_summary(&ws).as_deref(),
            Some("\u{1f465} 1 blocked, 1 working")
        );

        // Close %1 (the split's new pane holds focus) and let the layout
        // reconciliation deliver the close signal.
        assert!(
            ws.close_pane_via_mux(),
            "close must be consumed daemon-side"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while ws.tmux_state.tmux_pane_owners.contains_key(&1) {
            assert!(
                Instant::now() < deadline,
                "the killed pane's mapping was never reconciled away"
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }

        // The cache dropped the closed pane's entry (not just its surface
        // row), and the survivor still counts.
        let rosterd: Vec<u64> = ws.tmux_state.agent_roster.iter().map(|e| e.pane).collect();
        assert_eq!(rosterd, vec![0], "the closed pane's entry must be gone");
        assert_eq!(
            scoped_summary(&ws).as_deref(),
            Some("\u{1f465} 1 working"),
            "the widget must stop counting the closed pane's agent"
        );
        let map = &ws.tmux_state.tmux_pane_owners;
        let rows = ws
            .tmux_state
            .agent_roster
            .palette_rows(&|pane| map.contains_key(&pane));
        let ids: Vec<&str> = rows.iter().map(|r| r.action_id.as_str()).collect();
        assert_eq!(
            ids,
            ["agent-roster-focus:0"],
            "the picker must not offer the closed pane"
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
        let attach =
            attach_test(&transport, "rows", Some((80, 24)), &Default::default()).expect("attach");
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
        let attach =
            attach_test(&transport, "det", Some((80, 24)), &Default::default()).expect("attach");
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
        assert!(ws.tmux_state.tmux_pane_owners.is_empty());
        assert_eq!(
            ws.overlay_state.toast_message.as_deref(),
            Some("par-mux: detached (session keeps running in the daemon)"),
            "the detach toast must replace the shared cleanup's 'Session ended'"
        );

        // D5 through the app's own detach: the daemon outlives the dropped
        // socket and a fresh client reattaches to the same session.
        let second = connect(&path);
        let reattach = attach_test(&second, "det", None, &Default::default()).expect("reattach");
        assert!(
            matches!(reattach.outcome, AttachOutcome::Attached(ref s) if s.name == "det"),
            "the daemon survived the explicit detach: {:?}",
            reattach.outcome
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Detach and daemon death tear down every mux tab at once.
    /// `close_tab` drops each tab inline: with the last TerminalManager
    /// Arc gone, `PtySession::drop` polls the reader thread up to 2 s per
    /// tab, all on the UI thread (card 01a0d9e6e8b17db09952d8c90884910c)
    /// — with four mux tabs that is a multi-second freeze.
    #[test]
    fn detach_with_four_mux_tabs_completes_under_100ms() {
        let path = socket_path("detach-fast");
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_test(&transport, "detfast", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        // Four windows = four mux tabs (a CREATED session reports its
        // first window via the daemon's own %window-add push).
        for _ in 0..3 {
            transport.send_command("new-window").expect("new-window");
            std::thread::sleep(Duration::from_millis(50));
        }
        // Background windows' layouts are not pushed on their own — the
        // refresh-client -C pump forces a %layout-change broadcast for
        // every pane so the layout consumer creates each mirror.
        for pane in transport.client().list_panes().expect("list panes") {
            transport
                .send_command_no_wait(&format!("refresh-client -t %{pane} -C 80x24"))
                .expect("refresh-client");
        }

        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws =
            crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("detfast".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until all four daemon windows became tabs with mirror
        // panes — each carrying a live hidden-shell PTY behind
        // `tab.terminal`, exactly what stalls an inline teardown.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if ws.tab_manager.tab_count() >= 4 && ws.tmux_state.tmux_pane_owners.len() >= 4 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "never created 4 tabs: {} tabs, panes {:?}",
                ws.tab_manager.tab_count(),
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        // Detach must tear all four tabs down without dropping a live
        // TerminalManager inline — PtySession::drop's reader-thread wait
        // belongs off the UI thread.
        let t0 = Instant::now();
        assert!(ws.detach_mux_session(), "detach runs");
        let elapsed = t0.elapsed();
        assert_eq!(ws.tab_manager.tab_count(), 0, "every mux tab is torn down");
        assert!(
            elapsed < Duration::from_millis(100),
            "detach of 4 mux tabs must stay under 100 ms: took {elapsed:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fresh_daemon_yields_an_empty_roster() {
        let path = socket_path("roster");
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach =
            attach_test(&transport, "roster", Some((80, 24)), &Default::default()).expect("attach");
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
    // The pane runs a POSIX shell TUI (printf/read/python3); Windows panes run PowerShell.
    #[cfg(unix)]
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
        let attach = attach_test(&transport, "tui", Some((80, 24)), &Default::default())
            .expect("attach_sequence");

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
    // The pane runs a POSIX shell TUI (printf/read/python3); Windows panes run PowerShell.
    #[cfg(unix)]
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
        let attach = attach_test(&transport, "wstui", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
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
                .tmux_pane_owners
                .get(&0)
                .map(|&(_, native)| native)
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
                ws.tmux_state.tmux_pane_owners,
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
            split_landed = ws.tmux_state.tmux_pane_owners.contains_key(&1);
            if !split_landed {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            split_landed,
            "the daemon-side split must create the second native pane via \
             %layout-change: map = {:?}",
            ws.tmux_state.tmux_pane_owners
        );

        // The daemon's focus push for the new pane must move NATIVE focus
        // (the only source of truth for routing). It arrives in the same
        // batch as the layout that creates the pane, so it must be applied
        // after the layout consumer, or the new pane is not yet mapped and
        // the push is lost (reported live: split did not focus the new pane).
        let (owner_tab, new_native) = ws.tmux_state.tmux_pane_owners[&1];
        assert_eq!(
            ws.tmux_state.tmux_pane_in_tab(owner_tab, new_native),
            Some(1),
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
            ws.tmux_state.tmux_pane_owners
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

    /// Triggers are scanned by the PTY reader thread — which a mux mirror
    /// pane does not have. Output routed into a mirror via
    /// `handle_tmux_output` used to reach the grid but never the trigger
    /// registry, and `check_trigger_actions` polled only `tab.terminal`
    /// (the hidden shell), so a pattern printed in a mux pane fired
    /// nothing (card 01a0d9b559487e42a651a18e93dfce93, criterion 1).
    ///
    /// Proves the whole chain through the real routing and dispatch paths:
    /// attach installs mirror panes carrying the config's trigger registry,
    /// a `%output` chunk seeded through `handle_tmux_output` is scanned on
    /// the mirror terminal, and `check_trigger_actions` polls every pane of
    /// the active tab — the RunCommand lands in the confirmation queue
    /// (prompt_before_run keeps it side-effect-free for the test).
    #[test]
    fn triggers_fire_on_mux_pane_output() {
        let path = socket_path("ws-triggers");
        spawn_daemon(&path);

        // A config carrying one trigger whose pattern only this test
        // prints. RunCommand + prompt_before_run queues a dialog action
        // instead of spawning a process.
        let mut config = crate::config::Config::default();
        config.automation.triggers = vec![par_term_config::TriggerConfig {
            name: "mux-wiring-trigger".to_string(),
            pattern: "TRIGGER-PATTERN-FIRED".to_string(),
            enabled: true,
            actions: vec![par_term_config::TriggerActionConfig::RunCommand {
                command: "echo".to_string(),
                args: vec!["mux-trigger-ran".to_string()],
            }],
            prompt_before_run: true,
            i_accept_the_risk: false,
            allowed_commands: Vec::new(),
        }];

        // Created-session attach (the paste-test pattern): the daemon
        // pushes the window on the first pump, the layout consumer
        // creates the native mirror pane carrying the trigger registry.
        let transport = connect(&path);
        let attach = attach_test(&transport, "wstrig", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws = crate::app::window_state::WindowState::new(config, runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wstrig".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer created the native mirror pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        let native = loop {
            ws.check_mux_notifications();
            if let Some(&(tab_id, native)) = ws.tmux_state.tmux_pane_owners.get(&0) {
                ws.tab_manager.switch_to(tab_id);
                break native;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the native pane: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        };

        // The mirror terminal must carry the trigger registry — mirrors are
        // created after Tab::new_internal's one-shot sync, so creation and
        // config propagation both have to install triggers into panes.
        {
            let tab = ws.tab_manager.active_tab().expect("mux tab active");
            let pane = tab
                .pane_manager()
                .and_then(|pm| pm.get_pane(native))
                .expect("mirror pane exists");
            let term = pane.terminal.try_read().expect("pane terminal free");
            assert!(
                term.trigger_names()
                    .values()
                    .any(|n| n == "mux-wiring-trigger"),
                "mirror pane must have the trigger installed: {:?}",
                term.trigger_names()
            );
        }

        // Seed pattern text through the production routing path — the
        // daemon pane's %output — and run the per-frame trigger dispatch.
        ws.handle_tmux_output(0, b"TRIGGER-PATTERN-FIRED\r\n");
        ws.check_trigger_actions();

        let queued: Vec<&par_term_emu_core_rust::terminal::ActionResult> = ws
            .trigger_state
            .pending_trigger_actions
            .iter()
            .map(|p| &p.action)
            .collect();
        assert!(
            queued
                .iter()
                .any(|a| matches!(a, par_term_emu_core_rust::terminal::ActionResult::RunCommand { command, args, .. }
                    if command == "echo" && args.first().is_some_and(|a| a == "mux-trigger-ran"))),
            "the mux pane's trigger must fire through poll+dispatch: {queued:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Session logging hooks the PTY output callback on `tab.terminal` —
    /// in a mux tab that terminal is the hidden shell, and the mirror
    /// panes that render daemon output have no PTY reader thread to fire
    /// the callback. Output seeded through `handle_tmux_output` must
    /// reach the tab's session log, including for a pane created by a
    /// later layout change while logging is active (card
    /// 01a0d9b559487e42a651a18e93dfce93, criterion 2).
    #[test]
    fn session_logging_captures_mux_pane_output() {
        let path = socket_path("ws-sessionlog");
        spawn_daemon(&path);

        // Plain-format session log in an isolated directory.
        let logs = tempfile::tempdir().expect("tempdir");
        let mut config = crate::config::Config::default();
        config.session_log.session_log_directory = logs.path().display().to_string();
        config.session_log.session_log_format = crate::config::SessionLogFormat::Plain;

        // Created-session attach (the triggers-test pattern): the daemon
        // pushes the window on the first pump, the layout consumer
        // creates the native mirror pane.
        let transport = connect(&path);
        let attach = attach_test(&transport, "wssesslog", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws = crate::app::window_state::WindowState::new(config, runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wssesslog".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer created the native mirror pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        let tab_id = loop {
            ws.check_mux_notifications();
            if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
                break tab_id;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        };
        ws.tab_manager.switch_to(tab_id);

        // Toggle logging on through the production entry (the hotkey path).
        {
            let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
            let started = tab.toggle_session_logging(&ws.config.load());
            assert!(
                matches!(started, Ok(true)),
                "session logging must start: {started:?}"
            );
        }

        // Seed output through the production routing path — the daemon
        // pane's %output.
        ws.handle_tmux_output(0, b"SESSION-LOG-MUX-MARKER-1\r\n");

        // Split while logging is active — a pane born from a later layout
        // change must log too (the re-attachment path).
        ws.split_pane_vertical();
        let mut split_landed = false;
        while Instant::now() < deadline && !split_landed {
            ws.check_mux_notifications();
            split_landed = ws.tmux_state.tmux_pane_owners.contains_key(&1);
            if !split_landed {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(
            split_landed,
            "the daemon-side split must create the second pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        ws.handle_tmux_output(1, b"SESSION-LOG-MUX-MARKER-2\r\n");

        // Stop (which flushes) and read the log back.
        {
            let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
            let stopped = tab.toggle_session_logging(&ws.config.load());
            assert!(
                matches!(stopped, Ok(false)),
                "session logging must stop: {stopped:?}"
            );
        }
        let mut logged = String::new();
        for entry in std::fs::read_dir(logs.path()).expect("log dir readable") {
            let entry = entry.expect("dir entry");
            if entry.path().is_file() {
                logged.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
            }
        }
        assert!(
            logged.contains("SESSION-LOG-MUX-MARKER-1"),
            "mux pane output must reach the session log: {logged:?}"
        );
        assert!(
            logged.contains("SESSION-LOG-MUX-MARKER-2"),
            "output of a pane created while logging is active must reach the log: {logged:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Plugin event subscriptions are wired by `pump_plugin_events`,
    /// which registered each subscribed plugin's forwarder on
    /// `tab.terminal` only — in a mux tab that is the hidden shell, so a
    /// bell rung in a mux pane (daemon `%output` through
    /// `handle_tmux_output`) never reached the plugin (card
    /// 01a0d9b559487e42a651a18e93dfce93, criterion 3).
    #[test]
    fn plugin_receives_bell_from_mux_pane() {
        if par_term_scripting::manager::python_interpreter().is_none() {
            eprintln!("skipping: no Python interpreter on PATH");
            return;
        }
        let path = socket_path("ws-pluginbell");
        spawn_daemon(&path);

        // A subscribed plugin fixture: status-bar-widget kind subscribed
        // to bell_rang, whose entry stays alive without speaking the
        // protocol (the forwarder exists from spawn; the process never
        // needs to answer).
        let plugins = tempfile::tempdir().expect("tempdir");
        let dir = plugins.path().join("com.example.belltest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            concat!(
                r#"{"schemaVersion":1,"id":"com.example.belltest","name":"Bell test","version":"0.1.0","#,
                r#""kinds":["status-bar-widget"],"activation":"manual","#,
                r#""entryPoints":{"statusBarWidget":{"command":"widget.py","args":[]}},"#,
                r#""statusBarWidget":{"displayName":"Bell test","section":"right","defaults":{},"#,
                r#""schema":[{"key":"on","type":"boolean","label":"On","defaultValue":true}]},"#,
                r#""subscriptions":["bell_rang"]}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("widget.py"), "import time\ntime.sleep(60)\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.join("widget.py"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }

        // Created-session attach (the triggers-test pattern): the daemon
        // pushes the window on the first pump, the layout consumer
        // creates the native mirror pane.
        let transport = connect(&path);
        let attach = attach_test(
            &transport,
            "wsplugbell",
            Some((80, 24)),
            &Default::default(),
        )
        .expect("attach_sequence");
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws =
            crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wsplugbell".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer created the native mirror pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
                ws.tab_manager.switch_to(tab_id);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        // Enable the plugin through the host API (the Settings toggle's
        // path) and run the per-frame registration sweep — the event
        // loop's entry for plugin event wiring.
        {
            let host = ws.status_bar_ui.plugin_host_mut();
            let (found, warnings) = par_term_scripting::manifest::discover_plugins(plugins.path());
            assert!(
                found.len() == 1,
                "fixture plugin must pass discovery: {warnings:?}"
            );
            host.refresh_discovery(plugins.path());
            host.apply_enabled(&[par_term_scripting::plugin_manager::EnabledPlugin {
                id: "com.example.belltest".to_string(),
                settings_json: "{}".to_string(),
            }]);
            host.poll();
        }
        let forwarder = ws
            .status_bar_ui
            .plugin_host()
            .subscription_forwarders()
            .get("com.example.belltest")
            .expect("subscribed plugin has a forwarder")
            .clone();
        ws.status_bar_ui.pump_plugin_events(&ws.tab_manager);

        // Ring the bell in the daemon pane — the production routing path.
        ws.handle_tmux_output(0, b"\x07");

        let events = forwarder.drain_events();
        assert!(
            events.iter().any(|e| e.kind == "bell_rang"),
            "the mux pane's bell must reach the subscribed plugin: {events:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Tab-script event subscriptions attach their forwarder on
    /// `tab.terminal` only (`start_script_at`) — in a mux tab that is the
    /// hidden shell, so a bell rung in a mux pane never reached a
    /// subscribed script (card 01a0d9b559487e42a651a18e93dfce93,
    /// criterion 3).
    #[test]
    fn tab_script_receives_bell_from_mux_pane() {
        if par_term_scripting::manager::python_interpreter().is_none() {
            eprintln!("skipping: no Python interpreter on PATH");
            return;
        }
        let path = socket_path("ws-scriptbell");
        spawn_daemon(&path);

        // A stay-alive script entry; the forwarder is created at spawn and
        // the process itself never needs to speak the protocol.
        let scripts = tempfile::tempdir().expect("tempdir");
        let entry = scripts.path().join("bell_script.py");
        std::fs::write(&entry, "import time\ntime.sleep(60)\n").unwrap();

        // Created-session attach (the triggers-test pattern).
        let transport = connect(&path);
        let attach = attach_test(
            &transport,
            "wsscriptbell",
            Some((80, 24)),
            &Default::default(),
        )
        .expect("attach_sequence");
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws =
            crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wsscriptbell".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer created the native mirror pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        let tab_id = loop {
            ws.check_mux_notifications();
            if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
                ws.tab_manager.switch_to(tab_id);
                break tab_id;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        };

        // Start a subscribed tab script on the mux tab (the Settings
        // start button shares `start_script_at`).
        let script_config = par_term_config::ScriptConfig {
            name: "mux-bell-script".to_string(),
            enabled: true,
            script_path: entry.display().to_string(),
            args: Vec::new(),
            auto_start: false,
            restart_policy: par_term_config::automation::RestartPolicy::Never,
            restart_delay_ms: 0,
            subscriptions: vec!["bell_rang".to_string()],
            env_vars: std::collections::HashMap::new(),
            allow_write_text: false,
            prompt_before_write_text: true,
            allow_run_command: false,
            allow_change_config: false,
            write_text_rate_limit: 0,
            run_command_rate_limit: 0,
        };
        {
            let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
            let terminal = tab.terminal.clone();
            let term = terminal.try_read().expect("tab terminal free");
            tab.scripting
                .start_script_at(&term, 0, &script_config)
                .expect("script starts");
        }

        // The per-frame pane-attachment reconcile — the same call the
        // event loop's script sweep runs for every tab with scripts.
        let pane_terminals: Vec<(
            crate::pane::PaneId,
            std::sync::Arc<tokio::sync::RwLock<par_term_terminal::TerminalManager>>,
        )> = {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            tab.pane_manager()
                .map(|pm| {
                    pm.all_panes()
                        .into_iter()
                        .map(|p| (p.id, std::sync::Arc::clone(&p.terminal)))
                        .collect()
                })
                .unwrap_or_default()
        };
        {
            let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
            tab.scripting.reconcile_pane_observers(&pane_terminals);
        }

        // Ring the bell in the daemon pane — the production routing path.
        ws.handle_tmux_output(0, b"\x07");

        let forwarder = {
            let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
            tab.scripting.script_forwarders[0]
                .clone()
                .expect("forwarder stored at start")
        };
        let events = forwarder.drain_events();
        assert!(
            events.iter().any(|e| e.kind == "bell_rang"),
            "the mux pane's bell must reach the subscribed tab script: {events:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Focus reporting (DECSET 1004) follows the same rule as every other
    /// mux input: the report bytes belong in the DAEMON pane. The focus
    /// handler wrote ESC[I / ESC[O into the mirror terminal, which has no
    /// PTY — an app in a mux pane that enabled focus tracking never heard
    /// the window focus change (card 01a0d9b55c2273a2be9d87a0718690c3).
    #[test]
    fn focus_reports_reach_the_daemon_pane() {
        let path = socket_path("ws-focus");
        spawn_daemon(&path);

        // Created-session attach (the triggers-test pattern).
        let transport = connect(&path);
        let attach = attach_test(&transport, "wsfocus", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws =
            crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wsfocus".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer created the native mirror pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        let tab_id = loop {
            ws.check_mux_notifications();
            if let Some(&(tab_id, native)) = ws.tmux_state.tmux_pane_owners.get(&0) {
                ws.tab_manager.switch_to(tab_id);
                break (tab_id, native);
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane: {:?}",
                ws.tmux_state.tmux_pane_owners
            );
            std::thread::sleep(Duration::from_millis(25));
        };

        // The app in the pane enables focus tracking (DECSET 1004) —
        // routed through the daemon's %output, so the MIRROR terminal
        // carries the mode.
        ws.handle_tmux_output(0, b"\x1b[?1004h");
        {
            let tab = ws.tab_manager.get_tab(tab_id.0).expect("mux tab");
            let pane = tab
                .pane_manager()
                .and_then(|pm| pm.get_pane(tab_id.1))
                .expect("mirror pane");
            let term = pane.terminal.try_read().expect("pane terminal free");
            assert!(
                term.focus_tracking_enabled(),
                "the mirror must carry the app's focus-tracking mode"
            );
        }

        // Make the daemon pane echo its input: `cat -v` renders ESC[I as
        // `^[[I`, so capture-pane is the oracle for delivery. Typed as
        // hex bytes — bare words are key names on the daemon's send-keys.
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("send-keys -t %0 -H 63 61 74 20 2d 76 0d")
            .expect("start cat -v");
        std::thread::sleep(Duration::from_millis(300));

        // Window blur then focus through the production entry. The first
        // call flips state (a fresh window reports focused), so both
        // ESC[O and ESC[I should be delivered.
        ws.handle_focus_change(false);
        ws.handle_focus_change(true);

        // The report bytes must have reached the daemon pane.
        let mut daemon_view = String::new();
        let mut saw_focus_out = false;
        let mut saw_focus_in = false;
        while Instant::now() < deadline && !(saw_focus_out && saw_focus_in) {
            daemon_view = ws
                .tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("capture-pane -t %0 -p")
                .expect("capture")
                .join("|");
            saw_focus_out = daemon_view.contains("^[[O");
            saw_focus_in = daemon_view.contains("^[[I");
            if !(saw_focus_out && saw_focus_in) {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        assert!(
            saw_focus_in && saw_focus_out,
            "the daemon pane must see ESC[O and ESC[I from the focus \
             changes: {daemon_view:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Paste is the one input stream that never got a mux branch: keyboard
    /// input goes through `send_input_via_tmux`'s transport check, mouse
    /// reports through `route_mouse_report_to_mux`, but the shared paste
    /// entry (`paste_via_tmux`, used by Cmd+V, option-click paths, and
    /// middle-click) only knew the real-tmux gateway — for a mux pane it
    /// returned false and the local fallback wrote into a PTY-less mirror,
    /// dropping the paste (card 01a0d099b83e7dc0a5725390a8c5e20c).
    #[test]
    fn paste_via_tmux_routes_to_the_daemon_pane() {
        let path = socket_path("ws-paste");
        spawn_daemon(&path);

        // Attach + install into a renderer-less WindowState — the same
        // steps `install_mux_transport` performs (the reattach pattern).
        let transport = connect(&path);
        let attach = attach_test(&transport, "wspaste", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let mut ws = manners_state();
        // Reported (reattached) windows get their tabs directly; a CREATED
        // session's tab arrives as the daemon's own %window-add push on the
        // first pump — synthesizing one here as well would double-create it
        // and leave an unmapped tab behind after any close.
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wspaste".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer creates the native pane — the
        // paste router resolves its target from the focused native pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if ws.tmux_state.tmux_pane_owners.contains_key(&0)
                || ws.tmux_state.tmux_sync.get_native_pane(0).is_some()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane"
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        // The shared paste entry must CONSUME for a focused mux pane (the
        // shell echoes the bytes, so capture-pane sees the marker).
        assert!(
            ws.paste_via_tmux("PASTE-ROUTE-OK"),
            "a focused mux pane must take the paste through the shared entry"
        );
        let mut daemon_view = String::new();
        while Instant::now() < deadline && !daemon_view.contains("PASTE-ROUTE-OK") {
            daemon_view = ws
                .tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("capture-pane -t %0 -p")
                .expect("capture")
                .join("|");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            daemon_view.contains("PASTE-ROUTE-OK"),
            "the paste must reach the daemon pane: {daemon_view:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The IME commit path must route through the daemon transport — the
    /// `send_input_via_tmux` seam — when a mux pane is focused, not into the
    /// PTY-less local mirror. The marker carries a multi-byte é so the test
    /// also proves UTF-8 committed text survives the wire (card
    /// 01a0d9e6e6167353b58822718f83b15d, criterion 1's mux clause).
    #[test]
    fn ime_commit_reaches_the_mux_pane_via_the_transport() {
        let path = socket_path("ws-ime");
        spawn_daemon(&path);

        // Attach + install into a renderer-less WindowState — the same
        // steps `install_mux_transport` performs (the reattach pattern).
        let transport = connect(&path);
        let attach = attach_test(&transport, "wsime", Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let mut ws = manners_state();
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("wsime".to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer creates the native pane — the
        // input router resolves its target from the focused native pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if ws.tmux_state.tmux_pane_owners.contains_key(&0)
                || ws.tmux_state.tmux_sync.get_native_pane(0).is_some()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane"
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        // The committed text must land in the daemon pane (the shell echoes
        // the bytes, so capture-pane sees the marker).
        ws.handle_ime_event(winit::event::Ime::Commit("é-IME-ROUTE-OK".into()));
        let mut daemon_view = String::new();
        while Instant::now() < deadline && !daemon_view.contains("é-IME-ROUTE-OK") {
            daemon_view = ws
                .tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("capture-pane -t %0 -p")
                .expect("capture")
                .join("|");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            daemon_view.contains("é-IME-ROUTE-OK"),
            "the IME commit must reach the daemon pane: {daemon_view:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Attach a WindowState to a fresh daemon, pump until pane %0 has its
    /// native mirror, then arm bracketed paste and run `cat -v` in the
    /// daemon pane: `cat -v` renders the pane's raw input bytes in caret
    /// notation (ESC → `^[`, CR → `^M`), so the exact wire form of a paste
    /// is assertable from `capture-pane`.
    fn paste_byte_mirror_state(
        tag: &str,
    ) -> (crate::app::window_state::WindowState, std::path::PathBuf) {
        let path = socket_path(tag);
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_test(&transport, tag, Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let mut ws = manners_state();
        // Reported (reattached) windows get their tabs directly; a CREATED
        // session's tab arrives as the daemon's own %window-add push on the
        // first pump — synthesizing one here as well would double-create it
        // and leave an unmapped tab behind after any close.
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some(tag.to_string());
        ws.tmux_state.tmux_sync.enable();

        // Pump until the layout consumer creates the native pane — the
        // paste router resolves its target from the focused native pane.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if ws.tmux_state.tmux_pane_owners.contains_key(&0)
                || ws.tmux_state.tmux_sync.get_native_pane(0).is_some()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane"
            );
            std::thread::sleep(Duration::from_millis(25));
        }

        // Run the byte mirror in the daemon pane: the printf emits
        // DECSET 2004 so the client mirror arms bracketed paste
        // deterministically (no reliance on the shell's own init); `stty
        // raw -echo` takes the pane's tty out of canonical mode so input
        // is neither echoed nor CR-mangled (ICRNL) before `cat -v` renders
        // it — the screen then shows exactly the bytes the paste sent,
        // in caret notation (ESC → `^[`, CR → `^M`).
        let line = b"printf '\\033[?2004h'; stty raw -echo; cat -v";
        let hex = line
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_command(&format!("send-keys -t %0 -H {hex}"))
            .expect("type byte-mirror command");
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_command("send-keys -t %0 Enter")
            .expect("run byte-mirror command");

        // Wait for the mirror to see the mode: its bracketed sequences
        // turn non-empty only after the daemon pane's %output carries the
        // escape sequence back.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if focused_mirror_bracketed(&ws).is_some_and(|(start, _)| !start.is_empty()) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the mirror never armed bracketed paste"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        // The mode escape only proves printf ran; give the shell's `cat -v`
        // a beat to become the foreground reader before any paste lands
        // (a paste that races the exec lands on the shell's prompt
        // instead, and the assertions read noise).
        std::thread::sleep(Duration::from_millis(150));
        (ws, path)
    }

    /// Pane %0's mirror-terminal bracketed-paste sequences, if the mirror
    /// is resolvable and unlocked.
    fn focused_mirror_bracketed(
        ws: &crate::app::window_state::WindowState,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let native = ws.tmux_state.tmux_pane_owners.get(&0)?.1;
        for tab in ws.tab_manager.tabs() {
            if let Some(pane) = tab.pane_manager().and_then(|pm| pm.get_pane(native))
                && let Ok(term) = pane.terminal.try_read()
            {
                return Some(term.bracketed_paste_sequences());
            }
        }
        None
    }

    /// Card 01a0d9b551c57243b9888e1ac3be6b55, criterion 1: a paste into a
    /// mux pane whose mirror has bracketed paste enabled must reach the
    /// daemon pane wrapped in the bracketed-paste sequences with `\n`
    /// converted to `\r` — asserted on the raw bytes the daemon pane
    /// receives (`cat -v` caret notation), not the rendered screen.
    #[test]
    fn mux_paste_wraps_bracketed_paste_and_converts_newlines() {
        let (ws, path) = paste_byte_mirror_state("ws-paste-bp");

        assert!(
            ws.paste_via_tmux("L1\nL2"),
            "a focused mux pane must take the paste"
        );
        let transport = ws.tmux_state.transport.as_ref().unwrap();
        let view = capture_until(&**transport, 0, "^[[201~");
        assert!(
            view.contains("^[[200~L1^ML2^[[201~"),
            "the daemon pane must receive the bracketed, CR-converted paste: {view:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d9b551c57243b9888e1ac3be6b55, criterion 2: a multi-line
    /// mux paste under a configured `paste_delay_ms` is paced — the second
    /// line is NOT in the daemon pane when the first arrives, and lands
    /// once the delay elapses (the test drives the same poll tick the app
    /// loop would).
    #[test]
    fn mux_paste_honors_paste_delay_ms() {
        let (mut ws, path) = paste_byte_mirror_state("ws-paste-delay");

        let mut cfg = crate::config::Config::default();
        cfg.selection.paste_delay_ms = 400;
        ws.config.store(std::sync::Arc::new(cfg));

        assert!(
            ws.paste_via_tmux("D1\nD2"),
            "a focused mux pane must take the paste"
        );

        // First line goes out immediately; the second must wait out the
        // 400ms delay.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut early = String::new();
        while Instant::now() < deadline {
            ws.check_mux_notifications();
            early = ws
                .tmux_state
                .transport
                .as_ref()
                .unwrap()
                .send_command("capture-pane -t %0 -p")
                .expect("capture")
                .join("|");
            if early.contains("D1^M") {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            early.contains("D1^M"),
            "the first line must arrive: {early:?}"
        );
        assert!(
            !early.contains("D2"),
            "the second line must wait out paste_delay_ms: {early:?}"
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut late = early.clone();
        while Instant::now() < deadline && !late.contains("D2") {
            ws.check_mux_notifications();
            late = ws
                .tmux_state
                .transport
                .as_ref()
                .unwrap()
                .send_command("capture-pane -t %0 -p")
                .expect("capture")
                .join("|");
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            late.contains("D2"),
            "the second line must land after the delay: {late:?}"
        );
        assert!(
            late.contains("^[[201~"),
            "the bracketed end must follow the last line: {late:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Attach a WindowState to a fresh single-window daemon session and
    /// pump until the layout consumer has created the tab and pane %0 —
    /// the shared prefix of the close-last-pane tests.
    fn attached_single_pane_state(
        tag: &str,
    ) -> (crate::app::window_state::WindowState, std::path::PathBuf) {
        let path = socket_path(tag);
        spawn_daemon(&path);

        let transport = connect(&path);
        let attach = attach_test(&transport, tag, Some((80, 24)), &Default::default())
            .expect("attach_sequence");
        let mut ws = manners_state();
        // Reported (reattached) windows get their tabs directly; a CREATED
        // session's tab arrives as the daemon's own %window-add push on the
        // first pump — synthesizing one here as well would double-create it
        // and leave an unmapped tab behind after any close.
        for window_id in &attach.existing_windows {
            ws.handle_tmux_window_add(*window_id);
        }
        ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some(tag.to_string());
        ws.tmux_state.tmux_sync.enable();

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            if ws.tmux_state.tmux_pane_owners.contains_key(&0)
                || ws.tmux_state.tmux_sync.get_native_pane(0).is_some()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the layout consumer never created the pane"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        (ws, path)
    }

    /// Card 01a0d9b5568175c084eb7f9d70c0ec4f, criterion 1 (recorded
    /// decision: the detach-like option): closing a mux tab's last pane
    /// closes the TAB — the Cmd+W shape — instead of kill-pane, so the
    /// daemon window and session survive.
    #[test]
    fn closing_a_mux_tabs_last_pane_closes_the_tab_and_keeps_the_window() {
        let (mut ws, path) = attached_single_pane_state("ws-lastpane");

        ws.close_focused_pane();

        // The tab closed locally, synchronously.
        assert!(
            ws.tab_manager.tabs().is_empty(),
            "closing the last pane must close the tab"
        );
        // The daemon window SURVIVED — no kill-pane went out.
        let windows = ws
            .tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_command("list-windows")
            .expect("list-windows")
            .join("|");
        assert!(
            windows.contains("@0"),
            "the daemon window must survive a last-pane close: {windows:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Card 01a0d9b5568175c084eb7f9d70c0ec4f, criterion 2: when the
    /// daemon DOES kill the window (here kill-pane from a second client,
    /// as the CLI would send), par-term receives %window-close and closes
    /// the tab instead of leaving it dead.
    #[test]
    fn a_daemon_side_kill_of_the_last_pane_closes_the_tab() {
        let (mut ws, path) = attached_single_pane_state("ws-winclose");

        // Bypass par-term's guard: kill the pane straight from the
        // transport, the way another client would.
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_command("kill-pane -t %0")
            .expect("kill-pane");

        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tab_manager.tabs().is_empty() && Instant::now() < deadline {
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            ws.tab_manager.tabs().is_empty(),
            "%window-close must close the dead window's tab"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Poll `capture-pane` on `%pane` until `needle` shows up (or 10s).
    fn capture_until(
        transport: &dyn crate::app::tmux_handler::tmux_state::TmuxTransport,
        pane: u64,
        needle: &str,
    ) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let view = transport
                .send_command(&format!("capture-pane -t %{pane} -p"))
                .expect("capture")
                .join("|");
            if view.contains(needle) || Instant::now() >= deadline {
                return view;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Print `name`'s value in `%pane`, bracketed so the capture can be
    /// parsed back out. The markers are split in the typed command so the
    /// shell's echo of the command line never matches.
    fn pane_var(transport: &MuxTransport, pane: u64, name: &str) -> String {
        let tag = format!("V{pane}{name}");
        transport
            .send_command(&format!(
                "send-keys -t %{pane} -l 'echo \"<{tag}\"\"=${name}>\"'"
            ))
            .expect("send-keys");
        transport
            .send_command(&format!("send-keys -t %{pane} Enter"))
            .expect("enter");
        let view = capture_until(transport, pane, &format!("<{tag}="));
        let start = view
            .find(&format!("<{tag}="))
            .unwrap_or_else(|| panic!("{name} never printed in %{pane}: {view:?}"));
        let rest = &view[start + tag.len() + 2..];
        rest[..rest.find('>').expect("closing marker")].to_string()
    }

    /// `len:first8:last8` of a value — the form [`pane_var_summary`] prints.
    fn summarize(value: &str) -> String {
        let head: String = value.chars().take(8).collect();
        let tail: String = value[value.len().saturating_sub(8)..].to_string();
        format!("{}:{head}:{tail}", value.len())
    }

    /// [`pane_var`] for values too long for one screen row: the shell
    /// prints [`summarize`]'s form instead of the value.
    fn pane_var_summary(transport: &MuxTransport, pane: u64, name: &str) -> String {
        let tag = format!("S{pane}{name}");
        let script = format!(
            "v=\"${name}\"; printf '<{tag}''=%s:%s:%s>\\n' \"${{#v}}\" \"${{v:0:8}}\" \"$(printf %s \"$v\" | tail -c 8)\""
        );
        transport
            .send_command(&format!(
                "send-keys -t %{pane} -l {}",
                par_term_mux::quote_env_value(&script)
            ))
            .expect("send-keys");
        transport
            .send_command(&format!("send-keys -t %{pane} Enter"))
            .expect("enter");
        let view = capture_until(transport, pane, &format!("<{tag}="));
        let start = view
            .find(&format!("<{tag}="))
            .unwrap_or_else(|| panic!("{name} never printed in %{pane}: {view:?}"));
        let rest = &view[start + tag.len() + 2..];
        rest[..rest.find('>').expect("closing marker")].to_string()
    }

    /// Card 01a0d93b1c03: mux panes get par-term's shell environment. A
    /// created session's first pane sees the attach env (`new-session -e`);
    /// a split gets its own ITERM_SESSION_ID; a reattach with a changed
    /// value refreshes it for panes created afterwards.
    #[cfg(unix)]
    #[test]
    fn mux_panes_get_the_attach_env_and_a_unique_iterm_session_id() {
        let path = socket_path("session-env");
        spawn_daemon(&path);
        let env: std::collections::HashMap<String, String> = [
            ("TERM_PROGRAM", "iTerm.app"),
            ("__PAR_TERM", "1"),
            ("PAR_TERM_ENV_PROBE", "first value"),
            ("ITERM_SESSION_ID", "w0t0p0:attach"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        let transport = connect(&path);
        let attach = attach_test(&transport, "envs", None, &env).expect("attach");
        let AttachOutcome::Created(session) = attach.outcome else {
            panic!("expected a created session");
        };
        // The first pane spawned inside new-session, so -e reached it —
        // including over the core's own TERM_PROGRAM=kitty default.
        assert_eq!(pane_var(&transport, 0, "TERM_PROGRAM"), "iTerm.app");
        assert_eq!(pane_var(&transport, 0, "__PAR_TERM"), "1");
        assert_eq!(pane_var(&transport, 0, "PAR_TERM_ENV_PROBE"), "first value");

        stamp_pane_session_id(&transport, Some(session.id));
        let reply = transport
            .send_command("split-window -h -t %0")
            .expect("split");
        assert!(
            reply.iter().any(|l| l.trim() == "%1"),
            "split reply: {reply:?}"
        );
        let first_id = pane_var(&transport, 0, "ITERM_SESSION_ID");
        let second_id = pane_var(&transport, 1, "ITERM_SESSION_ID");
        assert_eq!(first_id, "w0t0p0:attach");
        assert!(second_id.starts_with("w0t0p0:"), "{second_id:?}");
        assert_ne!(
            first_id, second_id,
            "each pane needs its own ITERM_SESSION_ID"
        );
        drop(transport);

        // Reattach with a changed value: panes created afterwards see it,
        // the pane already running keeps what it spawned with.
        let mut changed = env.clone();
        changed.insert("PAR_TERM_ENV_PROBE".to_string(), "it's changed".to_string());
        let second = connect(&path);
        let reattach = attach_test(&second, "envs", None, &changed).expect("reattach");
        assert!(matches!(reattach.outcome, AttachOutcome::Attached(_)));
        second
            .send_command("split-window -v -t %1")
            .expect("split after reattach");
        assert_eq!(pane_var(&second, 2, "PAR_TERM_ENV_PROBE"), "it's changed");
        assert_eq!(pane_var(&second, 0, "PAR_TERM_ENV_PROBE"), "first value");

        let _ = std::fs::remove_file(&path);
    }

    /// Criterion 1 of card 01a0d93b1c03: a mux pane shows the same values
    /// a local tab gets from `build_shell_env` for the parity keys, with a
    /// `shell_env` entry and a UTF-8 locale in the mix. PATH is compared
    /// whole: the daemon has the test process's PATH, and the session env
    /// must replace it with par-term's augmented one.
    #[cfg(unix)]
    #[test]
    fn mux_pane_env_matches_a_local_tab_for_the_parity_keys() {
        let path = socket_path("env-parity");
        spawn_daemon(&path);
        let shell_env: std::collections::HashMap<String, String> = [(
            "PAR_TERM_USER_VAR".to_string(),
            "from shell_env".to_string(),
        )]
        .into();
        let local = crate::tab::build_shell_env(Some(&shell_env)).expect("env");
        let mut sent = local.clone();
        sent.insert("PAR_TERM_PATH_PROBE".to_string(), local["PATH"].clone());
        let transport = connect(&path);
        attach_test(&transport, "parity", None, &sent).expect("attach");
        for key in [
            "TERM_PROGRAM",
            "TERM_PROGRAM_VERSION",
            "LC_TERMINAL",
            "LC_TERMINAL_VERSION",
            "__PAR_TERM",
            "LANG",
            "PAR_TERM_USER_VAR",
            "ITERM_SESSION_ID",
        ] {
            // Long values wrap across screen rows; compare the byte length
            // plus the ends, which is what the capture can carry intact.
            let expected = local.get(key).cloned().unwrap_or_default();
            let got = pane_var_summary(&transport, 0, key);
            assert_eq!(got, summarize(&expected), "{key}");
        }
        // PATH: the pane's shell rc may add entries of its own (a local
        // tab runs the same rc), so parity means every entry of par-term's
        // augmented PATH is present. The expected value rides in as a probe
        // var and the shell counts the entries missing from $PATH.
        let script = r#"n=0; printf '%s\n' "$PAR_TERM_PATH_PROBE" | tr : '\n' > /tmp/.ptprobe.$$; while read -r d; do [[ ":$PATH:" == *":$d:"* ]] || n=$((n+1)); done < /tmp/.ptprobe.$$; rm -f /tmp/.ptprobe.$$; echo "<MISS""=$n>""#;
        transport
            .send_command(&format!(
                "send-keys -t %0 -l {}",
                par_term_mux::quote_env_value(script)
            ))
            .expect("send-keys");
        transport
            .send_command("send-keys -t %0 Enter")
            .expect("enter");
        let view = capture_until(&transport, 0, "<MISS=");
        assert!(view.contains("<MISS=0>"), "PATH entries missing: {view:?}");
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
        attach_test(&transport, "kind", None, &Default::default()).expect("attach");

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
        attach_test(&transport, "stale", None, &Default::default()).expect("attach");

        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));
        // The mux tab is gone; only the tracked id survives.
        ws.tmux_state.mux_focused_pane = Some(0);
        assert!(
            ws.tmux_state.tmux_pane_owners.is_empty(),
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

    /// The converse of the stale-focus test: ON a mapped mux pane the
    /// literal-bytes path must route (Shift+Enter's raw LF reaches the
    /// daemon as send-keys -H). Before the fix, the tmux-connected gate
    /// refused first — a mux window has no tmux_session — so the branch
    /// below it was dead code and the LF fell onto the hidden local shell.
    #[test]
    fn literal_bytes_route_to_the_daemon_on_a_mapped_mux_pane() {
        let path = socket_path("literal-route");
        spawn_daemon(&path);
        let transport = connect(&path);
        attach_test(&transport, "litroute", None, &Default::default()).expect("attach");

        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));
        // A local tab whose focused pane mirrors daemon pane %0 — the
        // minimum mapping focused_mux_pane_from_native needs.
        let grid = None;
        let tab_id = ws
            .tab_manager
            .new_tab(
                &ws.config.load(),
                std::sync::Arc::clone(&ws.runtime),
                false,
                grid,
            )
            .expect("local tab");
        ws.tab_manager.switch_to(tab_id);
        let pane_id = ws
            .tab_manager
            .active_tab()
            .and_then(|tab| tab.focused_pane_id())
            .expect("focused pane");
        ws.tmux_state
            .set_tab_pane_mappings(tab_id, &[(0, pane_id)].into_iter().collect());

        assert!(
            ws.send_literal_bytes_via_tmux(b"\n"),
            "Shift+Enter's literal LF must route through the daemon"
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

    /// The done-unseen drain wiring (card 01a0d9b392c0): a `working` →
    /// `idle` push in an unfocused pane marks done-unseen, and the same
    /// transition in the pane the user is watching never marks — the drain
    /// clears it at the focused pane because no focus event follows to
    /// clear it later. Mark-then-see clears through [`AgentRoster::mark_seen`]
    /// (roster-level tests) and the two focus seams call the same method.
    #[test]
    fn agent_finish_marks_done_unseen_unless_the_pane_is_watched() {
        // Read the mark the way a user would — through the tooltip — since
        // the cache keeps no queryable flag.
        fn marked(ws: &crate::app::window_state::WindowState, pane: u64) -> bool {
            ws.tmux_state
                .agent_roster
                .tooltip_text(&|_: u64| true)
                .is_some_and(|text| {
                    text.lines().any(|line| {
                        line.contains(&format!("(pane {pane})")) && line.contains("done (unseen)")
                    })
                })
        }
        let mut ws = manners_state();
        // Unwatched: no focused pane at all.
        assert!(ws.apply_agent_pushes(vec![
            push("%0", "kimi", "working", "hook"),
            push("%0", "kimi", "idle", "hook"),
        ]));
        assert!(marked(&ws, 0), "an unfocused finisher carries the mark");

        // Watched: the finishing pane IS the focused pane.
        ws.tmux_state.mux_focused_pane = Some(1);
        assert!(ws.apply_agent_pushes(vec![
            push("%1", "omp", "working", "hook"),
            push("%1", "omp", "idle", "hook"),
        ]));
        assert!(
            !marked(&ws, 1),
            "a watched finish is seen at the transition — no ghost mark"
        );
    }

    /// The release push (pane.release_agent, core aed41c2) is the roster's
    /// one removal signal: an agent that quit leaves the cache at once
    /// instead of showing working until the pane dies. Bound by the same
    /// E3 manners rule as the state push — a disappearing row may redraw
    /// and may do nothing else.
    #[test]
    fn agent_release_pushes_drop_the_roster_row_without_interrupting() {
        let mut ws = manners_state();
        assert!(ws.apply_agent_pushes(vec![push("%0", "kimi", "working", "hook")]));
        assert_eq!(ws.tmux_state.agent_roster.iter().count(), 1);

        let redraw = ws.apply_agent_releases(vec![
            par_term_emu_core_rust::tmux_control::TmuxNotification::AgentReleased {
                pane_id: "%0".to_string(),
                agent: "kimi".to_string(),
            },
        ]);

        assert!(redraw, "a row disappearing is a potential visual change");
        assert!(
            ws.tmux_state.agent_roster.iter().count() == 0,
            "the released pane leaves the roster cache"
        );
        // E3: a release is not an interruption either.
        assert!(!ws.overlay_ui.command_palette.visible);
        assert!(ws.overlay_state.toast_message.is_none());
        assert!(ws.tab_manager.active_tab_id().is_none());
        assert!(ws.focus_state.pending_focus_tab_switch.is_none());
    }

    /// Run `f` with the pane-identity env a par-mux pane exports (core
    /// `mux::pane`'s contract), restoring whatever the test process had
    /// before — the agent_usage env-test containment: sole writer of these
    /// keys, restored before returning.
    fn with_pane_env<T>(socket: &Path, f: impl FnOnce() -> T) -> T {
        let saved_env = std::env::var_os("PAR_MUX_ENV");
        let saved_socket = std::env::var_os("PAR_MUX_SOCKET");
        // SAFETY: sole writer of both keys for the duration of `f`.
        unsafe {
            std::env::set_var("PAR_MUX_ENV", "1");
            std::env::set_var("PAR_MUX_SOCKET", socket);
        }
        let out = f();
        // SAFETY: restoring the sole-writer keys.
        unsafe {
            match saved_env {
                Some(v) => std::env::set_var("PAR_MUX_ENV", v),
                None => std::env::remove_var("PAR_MUX_ENV"),
            }
            match saved_socket {
                Some(v) => std::env::set_var("PAR_MUX_SOCKET", v),
                None => std::env::remove_var("PAR_MUX_SOCKET"),
            }
        }
        out
    }

    /// Run `f` with the pane-identity env removed — the launched-outside-
    /// any-pane case. The test process may itself run inside a mux pane,
    /// so absence cannot be assumed, only imposed.
    fn without_pane_env<T>(f: impl FnOnce() -> T) -> T {
        let saved_env = std::env::var_os("PAR_MUX_ENV");
        let saved_socket = std::env::var_os("PAR_MUX_SOCKET");
        // SAFETY: sole writer of both keys for the duration of `f`.
        unsafe {
            std::env::remove_var("PAR_MUX_ENV");
            std::env::remove_var("PAR_MUX_SOCKET");
        }
        let out = f();
        // SAFETY: restoring the sole-writer keys.
        unsafe {
            match saved_env {
                Some(v) => std::env::set_var("PAR_MUX_ENV", v),
                None => std::env::remove_var("PAR_MUX_ENV"),
            }
            match saved_socket {
                Some(v) => std::env::set_var("PAR_MUX_SOCKET", v),
                None => std::env::remove_var("PAR_MUX_SOCKET"),
            }
        }
        out
    }

    #[test]
    fn self_attach_guard_refuses_only_the_owning_socket() {
        let same = socket_path("guard");
        let other = socket_path("guard-other");

        let (refused, other_allowed) = with_pane_env(&same, || {
            (mux_attach_refusal(&same), mux_attach_refusal(&other))
        });
        let reason = refused.expect("attaching to the owning socket must be refused");
        assert!(
            reason.contains("inside"),
            "the refusal must name why: {reason}"
        );
        assert_eq!(
            other_allowed, None,
            "a different daemon is the core nesting case, not a self-attach"
        );

        let outer_allowed = without_pane_env(|| mux_attach_refusal(&same));
        assert_eq!(
            outer_allowed, None,
            "outside any pane the attach must proceed exactly as before"
        );
    }

    #[test]
    fn begin_attach_to_the_owning_session_creates_no_transport() {
        let name = "self-attach-guard";
        let target = par_term_emu_core_rust::mux::ipc::default_socket_path(name);
        let mut ws = manners_state();
        with_pane_env(&target, || {
            ws.begin_mux_session_attach(name);
        });
        assert!(
            ws.tmux_state.transport.is_none() && ws.tmux_state.mux_attach_pending.is_none(),
            "a refused attach must leave no transport and no pending worker behind"
        );
        assert!(
            ws.overlay_state
                .toast_message
                .as_deref()
                .is_some_and(|t| t.contains("refused")),
            "the refusal must toast, got {:?}",
            ws.overlay_state.toast_message
        );
    }

    /// A pending state with no live worker — the channel's sender is
    /// dropped, so nothing ever connects or spawns a daemon. Driving
    /// `begin_mux_session_attach` against it must stay on the early
    /// return: a real worker here would auto-spawn a daemon on the
    /// default socket (the leak the card-1 negative control taught).
    fn pending_without_worker(name: &str) -> MuxAttachPending {
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        MuxAttachPending {
            name: name.to_string(),
            rx,
        }
    }

    #[test]
    fn second_attach_while_attaching_explains_itself() {
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(pending_without_worker("first"));
        ws.begin_mux_session_attach("second");
        assert!(
            ws.tmux_state
                .mux_attach_pending
                .as_ref()
                .is_some_and(|p| p.name == "first"),
            "the in-flight attach must keep its place"
        );
        let toast = ws.overlay_state.toast_message.as_deref();
        assert!(
            toast.is_some_and(|t| t.contains("still attaching") && t.contains("first")),
            "the second open must explain instead of doing nothing, got {toast:?}"
        );
    }

    #[test]
    fn second_attach_while_attached_explains_itself() {
        let path = socket_path("second-attach");
        spawn_daemon(&path);
        let transport = connect(&path);
        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));
        ws.tmux_state.tmux_session_name = Some("live".to_string());
        ws.begin_mux_session_attach("other");
        let toast = ws.overlay_state.toast_message.as_deref();
        assert!(
            toast.is_some_and(|t| t.contains("already attached") && t.contains("live")),
            "the second open must name the attached session, got {toast:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A daemon that accepts but never answers — the SIGSTOP shape,
    /// in-process: the socket stays open (no `SessionEnded`) while every
    /// reply wait runs out its full timeout. Serves exactly one
    /// connection; the stream is read to completion so client writes
    /// never fill kernel buffers mid-test.
    fn spawn_silent_daemon(path: &Path) {
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

    /// The hung-daemon card's freeze repro, inverted: with the daemon
    /// silent, fire-and-forget sends (the per-keystroke seam) must return
    /// immediately instead of each burning the core client's 10 s reply
    /// timeout on the event loop.
    #[test]
    fn fire_and_forget_sends_never_block_on_a_silent_daemon() {
        let path = socket_path("silent-no-block");
        spawn_silent_daemon(&path);
        let transport = connect(&path);

        let started = Instant::now();
        for _ in 0..20 {
            transport
                .send_command_no_wait("send-keys -t %0 x")
                .expect("queueing must not block or fail");
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "20 queued sends took {:?} — the event loop would freeze",
            started.elapsed()
        );

        let _ = std::fs::remove_file(&path);
    }

    /// The hung-daemon card's toast criterion: a keystroke queued to a
    /// silent daemon must surface the unresponsive toast through the real
    /// per-frame drain (`check_mux_notifications`) within the 1 s
    /// deadline — health check to `show_toast`, not a unit shortcut.
    #[test]
    fn silent_daemon_surfaces_unresponsive_toast_within_1s() {
        let path = socket_path("silent-toast");
        spawn_silent_daemon(&path);
        let transport = connect(&path);

        let mut ws = manners_state();
        ws.tmux_state.transport = Some(Box::new(transport));

        // The user's first keystroke into the mux pane.
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command_no_wait("send-keys -t %0 x")
            .expect("queueing must not block");

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut toast_seen = None;
        while Instant::now() < deadline {
            ws.check_mux_notifications();
            if let Some(message) = ws.overlay_state.toast_message.clone() {
                toast_seen = Some(message);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let message = toast_seen.expect("unresponsive toast within 1 s");
        assert!(
            message.contains("not responding"),
            "toast must say the daemon is not responding, got: {message}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A healthy daemon never trips the health toast: the in-flight
    /// window closes in milliseconds and `poll_event` stays `None`.
    #[test]
    fn healthy_daemon_reports_no_health_events() {
        let path = socket_path("healthy-quiet");
        spawn_daemon(&path);
        let transport = connect(&path);

        transport
            .send_command_no_wait("send-keys -t %0 x")
            .expect("queue");
        // Long enough that a lingering in-flight send would have tripped
        // the 800 ms threshold if the reply never came.
        std::thread::sleep(Duration::from_millis(1200));
        for _ in 0..3 {
            assert_eq!(
                transport.daemon_health_event(),
                None,
                "a daemon that answers must not be flagged unresponsive"
            );
        }

        let _ = std::fs::remove_file(&path);
    }

    /// The health state machine's transitions, driven directly: threshold
    /// crossing fires once (not per poll), success clears the episode,
    /// and an observed reply timeout flags without needing a second send.
    #[test]
    fn mux_health_fires_once_per_episode_and_recovers() {
        let health = MuxHealth::default();
        let threshold = Duration::from_millis(50);

        health.send_started();
        assert_eq!(health.poll_event(threshold), None, "under threshold");
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(
            health.poll_event(threshold),
            Some(DaemonHealthSignal::Unresponsive)
        );
        assert_eq!(
            health.poll_event(threshold),
            None,
            "the toast must not re-fire every poll"
        );

        health.send_finished(&Ok(Vec::new()));
        assert_eq!(
            health.poll_event(threshold),
            Some(DaemonHealthSignal::Recovered)
        );
        assert_eq!(health.poll_event(threshold), None);

        let timeout = io::Error::new(io::ErrorKind::TimedOut, "no reply block within 10s");
        health.send_finished(&Err(timeout));
        assert_eq!(
            health.poll_event(threshold),
            Some(DaemonHealthSignal::Unresponsive),
            "an observed reply timeout flags without a new send"
        );
    }

    // =========================================================================
    // Tab operations as daemon window operations (card 01a0d9b55e53)
    // =========================================================================

    /// The shared prefix of the tab-ops tests: a daemon whose session
    /// exists, and a renderer-less `WindowState` attached through the REAL
    /// `install_mux_transport`. The probe client answers daemon-side
    /// queries without touching the app's transport.
    fn attached_state(
        tag: &str,
        session: &str,
    ) -> (crate::app::window_state::WindowState, MuxSessionClient) {
        let path = socket_path(tag);
        spawn_daemon(&path);
        // Prime the session with a first client so the attach takes the
        // Attached (reattach) path and list-windows reports its windows.
        {
            let mut primer = MuxSessionClient::connect(&path).expect("primer connects");
            primer.create_or_attach(session).expect("primer creates");
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if primer
                    .poll_actions()
                    .iter()
                    .any(|a| matches!(a, SyncAction::CreateTab { .. }))
                {
                    break;
                }
                assert!(Instant::now() < deadline, "primer never saw the window");
                std::thread::sleep(Duration::from_millis(25));
            }
        }

        let transport = connect(&path);
        let probe = MuxSessionClient::connect(&path).expect("probe connects");
        let mut ws = manners_state();
        ws.install_mux_transport(session, transport)
            .expect("install_mux_transport");
        (ws, probe)
    }

    /// Pump the app's notification loop until `cond` holds, panicking with
    /// the waited-for thing otherwise.
    fn pump_until(
        ws: &mut crate::app::window_state::WindowState,
        mut cond: impl FnMut(&crate::app::window_state::WindowState) -> bool,
        what: &str,
    ) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            ws.check_mux_notifications();
            if cond(ws) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("timed out waiting for {what}");
    }

    /// Cmd+T while attached asks the daemon for a new window; the tab
    /// arrives via %window-add (the shell-exit path in reverse) and is a
    /// mirror tab, not a local shell tab.
    #[test]
    fn new_tab_creates_a_daemon_window() {
        let (mut ws, mut probe) = attached_state("tabops-new", "tabops-new");
        let before: Vec<_> = probe
            .list_windows()
            .expect("list-windows")
            .into_iter()
            .map(|w| w.id)
            .collect();
        let tabs_before = ws.tab_manager.tab_count();

        ws.new_tab();

        pump_until(
            &mut ws,
            |ws| ws.tab_manager.tab_count() == tabs_before + 1,
            "the %window-add tab",
        );
        let after = probe.list_windows().expect("list-windows");
        assert_eq!(after.len(), before.len() + 1, "daemon gained a window");
        let new_window = after
            .iter()
            .map(|w| w.id)
            .find(|id| !before.contains(id))
            .expect("a new daemon window id");
        assert!(
            ws.tmux_state.tmux_sync.get_tab(new_window).is_some(),
            "the new tab maps to the new daemon window"
        );
    }

    // =========================================================================
    // Agent launcher mux arm (card 01a0d8da6ec47b)
    // =========================================================================

    /// The daemon pane ids `%N` from a `list-panes` reply.
    fn listed_panes(probe: &mut MuxSessionClient) -> Vec<u64> {
        probe
            .send("list-panes")
            .expect("list-panes")
            .iter()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix('%')
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .collect()
    }

    /// A launch splits the resolved daemon pane and TYPES the command line
    /// into the new pane's shell (the daemon spawns shells — typing IS the
    /// launch), and the new pane is a daemon pane mapped client-side, the
    /// eligibility the roster's pickers and hooks need.
    #[test]
    fn launch_agent_via_mux_types_into_a_fresh_daemon_pane() {
        let (mut ws, mut probe) = attached_state("agent-launch", "agent-launch");
        // A background window's layout is not pushed on its own — the
        // refresh-client -C pump forces the %layout-change that maps the
        // panes (the detach-fast pattern).
        let deadline = Instant::now() + Duration::from_secs(10);
        while ws.tmux_state.tmux_pane_owners.is_empty() {
            assert!(Instant::now() < deadline, "no daemon pane ever mapped");
            for pane in listed_panes(&mut probe) {
                let _ = probe.send(&format!("refresh-client -t %{pane} -C 80x24"));
            }
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(25));
        }
        let before = listed_panes(&mut probe);

        let outcome = ws.launch_agent_via_mux("echo PAR_TERM_AGENT_LAUNCHED");
        assert_eq!(outcome, MuxLaunchOutcome::Launched);

        // A new daemon pane exists; its shell ran the typed command.
        let deadline = Instant::now() + Duration::from_secs(10);
        let new_pane = loop {
            let fresh = listed_panes(&mut probe);
            if let Some(id) = fresh.iter().find(|id| !before.contains(id)) {
                break *id;
            }
            assert!(
                Instant::now() < deadline,
                "launch created no daemon pane; before={before:?} now={fresh:?}"
            );
            std::thread::sleep(Duration::from_millis(25));
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let view = loop {
            let captured = probe
                .send(&format!("capture-pane -t %{new_pane} -p"))
                .expect("capture-pane")
                .join("|");
            if captured.contains("PAR_TERM_AGENT_LAUNCHED") || Instant::now() >= deadline {
                break captured;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            view.contains("PAR_TERM_AGENT_LAUNCHED"),
            "typed command never ran in %{new_pane}: {view:?}"
        );
        // Client-side mapping (roster rows scope to mapped panes).
        pump_until(
            &mut ws,
            |ws| ws.tmux_state.tmux_pane_owners.contains_key(&new_pane),
            "the launched pane's client mapping",
        );
    }

    /// Dispatch guards: an unknown agent id and a missing default no-op
    /// (toast + false) instead of launching anything.
    #[test]
    fn launch_dispatch_rejects_unknown_agent_and_missing_default() {
        let mut ws = manners_state();
        assert!(!ws.launch_agent_by_id("bogus", false));
        assert!(!ws.launch_default_agent());
        // And the action-name spellings reach them through the dispatcher.
        assert!(!ws.execute_keybinding_action("launch-agent:bogus"));
        assert!(!ws.execute_keybinding_action("launch-default-agent"));
    }

    /// The local arm (no transport): dispatching a configured agent opens a
    /// new tab and the typed command runs in its shell — the snippet NewTab
    /// path carrying the launcher's command line.
    #[test]
    fn launch_agent_local_arm_opens_a_tab_and_runs_the_command() {
        // A multi-thread runtime: unlike the mirror-terminal tests, this
        // one pumps a REAL local shell, and the core's PTY reader needs
        // background task progress a dormant current-thread runtime
        // never makes.
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let mut ws =
            crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
        ws.config.store(std::sync::Arc::new(crate::config::Config {
            agents: vec![par_term_config::agent_launcher::AgentLaunchConfig {
                id: "probe".to_string(),
                name: "Probe".to_string(),
                command: "echo PAR_TERM_LOCAL_LAUNCH".to_string(),
                autonomy_args: String::new(),
                default: false,
            }],
            ..Default::default()
        }));
        assert!(ws.execute_keybinding_action("launch-agent:probe"));
        assert_eq!(ws.tab_manager.tab_count(), 1, "the launch opened a tab");

        // The delayed write lands after the shell initializes; poll the
        // tab's screen for the marker (Cmd+F's searchable-lines seam).
        let tab_id = ws.tab_manager.active_tab().unwrap().id;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let seen = ws.tab_manager.get_tab(tab_id).and_then(|tab| {
                tab.try_with_read_terminal(|term| {
                    crate::app::window_state::search_highlight::get_all_searchable_lines(
                        term,
                        term.dimensions().1,
                    )
                    .any(|(_, line)| line.contains("PAR_TERM_LOCAL_LAUNCH"))
                })
            });
            if seen == Some(true) {
                return;
            }
            if Instant::now() >= deadline {
                let diag = ws
                    .tab_manager
                    .get_tab(tab_id)
                    .and_then(|tab| {
                        tab.try_with_read_terminal(|term| {
                            (
                                term.dimensions(),
                                term.is_running(),
                                term.content().map(|c| c.chars().count()).unwrap_or(0),
                            )
                        })
                    })
                    .unwrap_or(((0, 0), false, 0));
                let lines = ws
                    .tab_manager
                    .get_tab(tab_id)
                    .and_then(|tab| {
                        tab.try_with_read_terminal(|term| {
                            crate::app::window_state::search_highlight::get_all_searchable_lines(
                                term,
                                term.dimensions().1,
                            )
                            .map(|(_, line)| line)
                            .collect::<Vec<_>>()
                        })
                    })
                    .unwrap_or_default();
                panic!(
                    "typed command never ran in the local launch tab (dims={}x{} running={} content_chars={}): {lines:?}",
                    diag.0.0, diag.0.1, diag.1, diag.2
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Closing a mux tab kills the daemon window; the tab tears down via
    /// %window-close and leaves no stale pane mappings behind.
    #[test]
    fn closing_a_tab_kills_the_daemon_window() {
        let (mut ws, mut probe) = attached_state("tabops-close", "tabops-close");
        let before = probe.list_windows().expect("list-windows");
        assert_eq!(before.len(), 1, "one daemon window before the close");
        let window_id = before[0].id;
        let tab_id = ws
            .tmux_state
            .tmux_sync
            .get_tab(window_id)
            .expect("tab mapped to the window");
        // Background windows' layouts are not pushed on their own — the
        // refresh-client -C pump forces a %layout-change broadcast so the
        // layout consumer maps the panes (the detach-fast pattern).
        for pane in probe.list_panes().expect("list panes") {
            probe
                .send(&format!("refresh-client -t %{pane} -C 80x24"))
                .expect("refresh-client");
        }
        // The layout consumer maps the window's panes only once the
        // %layout-change has been pumped — wait for it so mapping
        // cleanliness after the close is observable.
        pump_until(
            &mut ws,
            |ws| !ws.tmux_state.tab_tmux_pane_ids(tab_id).is_empty(),
            "the window's pane mappings",
        );
        let panes: Vec<_> = ws
            .tmux_state
            .tab_tmux_pane_ids(tab_id)
            .into_iter()
            .collect();
        ws.tab_manager.switch_to(tab_id);

        let was_last = ws.close_current_tab_immediately();

        assert!(!was_last, "the close returns before the notification lands");
        pump_until(
            &mut ws,
            |ws| ws.tmux_state.tmux_sync.get_tab(window_id).is_none(),
            "the %window-close teardown",
        );
        let after = probe.list_windows().expect("list-windows");
        assert!(after.is_empty(), "daemon window died with the tab");
        assert!(
            panes
                .iter()
                .all(|p| !ws.tmux_state.tmux_pane_owners.contains_key(p)),
            "no stale pane mapping for the killed window"
        );
    }

    /// Move-tab is blocked for mux tabs at every entry point: the request
    /// never queues and the user sees why.
    #[test]
    fn moving_a_mux_tab_to_another_window_is_blocked() {
        let (mut ws, mut probe) = attached_state("tabops-move", "tabops-move");
        // A second daemon window so has_multiple_tabs would allow a move.
        probe.send("new-window").expect("new-window");
        pump_until(
            &mut ws,
            |ws| ws.tab_manager.tab_count() == 2,
            "the second daemon window's tab",
        );
        let tab_id = ws
            .tmux_state
            .tmux_sync
            .get_tab(0)
            .expect("tab for window @0");
        ws.tab_manager.switch_to(tab_id);

        use crate::tab_bar_ui::TabBarAction;
        ws.handle_tab_bar_action_after_render(TabBarAction::MoveTabToNewWindow(tab_id));
        assert!(
            ws.overlay_ui.pending_move_tab_request.is_none(),
            "no move request queued for a mux tab"
        );
        assert!(
            ws.overlay_state.toast_message.is_some(),
            "the block is explained to the user"
        );
    }

    /// Attach carries the daemon's window names onto the display tabs, and
    /// renaming a mux tab forwards rename-window to the daemon.
    #[test]
    fn attach_carries_window_names_and_rename_reaches_the_daemon() {
        let path = socket_path("tabops-names");
        spawn_daemon(&path);
        {
            let mut primer = MuxSessionClient::connect(&path).expect("primer connects");
            primer.create_or_attach("tabops-names").expect("primer");
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if primer
                    .poll_actions()
                    .iter()
                    .any(|a| matches!(a, SyncAction::CreateTab { .. }))
                {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
            primer
                .send("rename-window -t @0 build")
                .expect("name the window before attach");
        }

        let transport = connect(&path);
        let mut probe = MuxSessionClient::connect(&path).expect("probe connects");
        let mut ws = manners_state();
        ws.install_mux_transport("tabops-names", transport)
            .expect("install_mux_transport");

        let tab_id = ws.tmux_state.tmux_sync.get_tab(0).expect("tab for @0");
        let title = ws
            .tab_manager
            .get_tab(tab_id)
            .expect("tab exists")
            .title
            .clone();
        assert_eq!(title, "build", "attach carries the daemon window name");

        use crate::tab_bar_ui::TabBarAction;
        ws.handle_tab_bar_action_after_render(TabBarAction::RenameTab(
            tab_id,
            "it's docs".to_string(),
        ));
        let named = probe.list_windows().expect("list-windows");
        assert_eq!(
            named[0].name, "it's docs",
            "rename-window reached the daemon, quoting intact"
        );
        let title = ws
            .tab_manager
            .get_tab(tab_id)
            .expect("tab exists")
            .title
            .clone();
        assert_eq!(title, "it's docs", "local title follows the rename");
    }

    // =========================================================================
    // Client metric reports: -p cell pixels, set-client-colors (card
    // 01a0db77d7a2)
    // =========================================================================

    /// A transport that records every command and answers nothing — the
    /// wire-shape oracle for the fire-and-forget pushes.
    #[derive(Default)]
    struct RecordingTransport {
        commands: std::cell::RefCell<Vec<String>>,
    }

    impl TmuxTransport for RecordingTransport {
        fn drain(
            &self,
        ) -> (
            Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
            bool,
        ) {
            (Vec::new(), false)
        }

        fn send_command(&self, command: &str) -> io::Result<Vec<String>> {
            self.commands.borrow_mut().push(command.to_string());
            Ok(Vec::new())
        }

        fn send_command_no_wait(&self, command: &str) -> io::Result<()> {
            self.commands.borrow_mut().push(command.to_string());
            Ok(())
        }
    }

    /// The resize/font pushes carry the cell pixels beside the grid
    /// (`-C WxH -p WxH` in one command), the theme push carries both
    /// halves of the color pair, and the theme hex the app derives is the
    /// configured theme's fg/bg as lowercase `rrggbb`.
    #[test]
    fn client_metric_pushes_carry_cells_and_colors() {
        let transport = RecordingTransport::default();
        push_client_size(&transport, Some(7), 80, 24, (12, 24));
        // No target pane: the push is dropped, not sent untargeted.
        push_client_size(&transport, None, 80, 24, (12, 24));
        push_client_colors(&transport, "c0c0c0", "1e1e2e");
        let commands = transport.commands.borrow();
        assert_eq!(
            &*commands,
            &[
                "refresh-client -t %7 -C 80x24 -p 12x24".to_string(),
                "set-client-colors -f c0c0c0 -b 1e1e2e".to_string(),
            ],
            "one -C/-p command per size push, one set-client-colors per \
             theme push, and nothing for a targetless push"
        );

        let config = crate::config::Config::default();
        let (fg, bg) = mux_client_colors_hex(&config);
        let theme = config.load_theme();
        let hex = |c: crate::config::Color| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b);
        assert_eq!(fg, hex(theme.foreground), "fg is the theme foreground");
        assert_eq!(bg, hex(theme.background), "bg is the theme background");
    }

    /// The attach reports reach the daemon and land in the pane terminal:
    /// after an attach carrying `-p 12x24` and bg `121212`, a CSI 16t
    /// query in the pane answers `6;12;24t` (not the 10x20 construction
    /// default) and an OSC 11 query answers the client bg. The pane's own
    /// program proves the reports live daemon-side, not just on the wire.
    #[test]
    fn attach_reports_cells_and_theme_to_the_daemon() {
        let path = socket_path("attach-metrics");
        spawn_daemon(&path);

        let transport = connect(&path);
        attach_sequence(
            &transport,
            "metrics",
            Some((90, 30)),
            Some((12, 24)),
            &("c0c0c0".to_string(), "121212".to_string()),
            &Default::default(),
        )
        .expect("attach with metrics");
        transport
            .client()
            // The marker is quote-split so the ECHOED command text never
            // matches it — only the executed echo's output does.
            .send_keys_literal(
                0,
                "printf \"\\033[16t\\033]11;?\\007\"; echo MET\"RIC\"DONE",
            )
            .expect("send metric query");
        transport.client().send_keys(0, b"\r").expect("enter");

        let deadline = Instant::now() + Duration::from_secs(15);
        let screen = loop {
            let screen = transport.client().refresh_pane(0).expect("replay");
            let screen = screen.join("\n");
            if screen.contains("METRICDONE") {
                break screen;
            }
            assert!(
                Instant::now() < deadline,
                "the metric program never printed: {screen:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            // xterm's XTWINOPS replies are height-first: 6;height;width.
            screen.contains("6;24;12t"),
            "CSI 16t answers with the reported 12x24 cells, not the default: {screen:?}"
        );
        assert!(
            screen.contains("rgb:1212/1212/1212"),
            "OSC 11 answers with the client bg, not the core theme: {screen:?}"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// `%sessions-changed` ends the view when the attached session is what
    /// died: killing the session's last window from a second client makes
    /// the next drain re-query `list-sessions`, find the session gone, and
    /// run the full end-of-view teardown (transport dropped, identity
    /// cleared, toast naming the daemon-side end) — while the daemon
    /// itself keeps running.
    #[test]
    fn sessions_changed_ends_views_whose_session_is_gone() {
        // Capture the path once: `socket_path` UNLINKS on every call (it
        // is a spawn helper, not a getter), so re-calling it after
        // `attached_state` bound the daemon would delete the live socket.
        let path = socket_path("gone-session");
        let (mut ws, mut probe) = attached_state("gone-session", "doomed");
        assert!(
            ws.tmux_state.transport.is_some() && ws.tmux_state.mux_session_id.is_some(),
            "precondition: the view is attached"
        );
        // The doomed session's only window, captured before the keeper
        // exists — list-windows is global, so this ordering is what names
        // it unambiguously.
        let window = probe
            .list_windows()
            .expect("probe lists windows")
            .first()
            .expect("the doomed session holds a window")
            .id;
        // A second session keeps the daemon non-empty: an emptied daemon
        // exits after its grace (EXIT_EMPTY_GRACE), and that exit is the
        // DISCONNECT path's scenario, not this one — here the daemon must
        // outlive the killed session.
        probe
            .create_or_attach("keeper")
            .expect("keeper session exists");
        probe
            .send(&format!("kill-window -t @{window}"))
            .expect("kill-window removes the session's last window");

        let redrawn = ws.check_mux_notifications();
        assert!(redrawn, "the ended view must request a redraw");
        assert!(
            ws.tmux_state.transport.is_none(),
            "the view ended — the transport is dropped"
        );
        assert!(ws.tmux_state.mux_session_id.is_none());
        assert!(ws.tmux_state.tmux_session_name.is_none());
        let toast = ws
            .overlay_state
            .toast_message
            .as_deref()
            .unwrap_or_default();
        assert!(
            toast.contains("session ended on the daemon"),
            "the toast names the daemon-side end, got: {toast}"
        );
        // The daemon survives the view: a fresh client still connects, the
        // keeper session remains, the doomed one is gone.
        let mut late = MuxSessionClient::connect(&path).expect("daemon lives");
        let names: Vec<String> = late
            .list_sessions()
            .expect("list after teardown")
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(
            names.contains(&"keeper".to_string()) && !names.contains(&"doomed".to_string()),
            "the daemon kept running with exactly the doomed session removed: {names:?}"
        );
    }
}
