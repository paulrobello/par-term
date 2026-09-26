//! The par-mux attach sequence: what a client does once connected —
//! version check, create-or-attach with the session environment, client
//! size/color pushes, screen replay seeding, roster fill, and pane-title
//! probing. Split from `mux.rs`, which owns the app wiring that calls
//! this from `install_mux_transport`.

use super::mux_transport::MuxTransport;
use crate::app::tmux_handler::tmux_state::TmuxTransport;
use par_term_mux::{AgentEntry, AttachOutcome};
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::io;

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
pub(super) fn stamp_pane_session_id(transport: &dyn TmuxTransport, session: Option<u64>) {
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
