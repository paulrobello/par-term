//! The `use_mux_by_default` attach step and the pending first-tab migration.
//!
//! Split from `lifecycle.rs` for the repo's 800-line production ceiling.

#[cfg(feature = "mux")]
use crate::app::tmux_handler::tmux_state::MuxDefaultTabOrigin;
use crate::app::window_state::WindowState;
#[cfg(feature = "mux")]
use par_term_config::TabId;

/// The `use_mux_by_default` attach-on-first-new-tab step: begin the
/// attach to the default session so the window joins (or creates) it
/// while the requested tab is created locally by `new_tab_local`.
/// The attach never blocks — a refused or failed attach is logged and
/// the local shell is simply the tab; a successful attach hands the
/// untouched tab to the window-add handler for replacement.
#[cfg(feature = "mux")]
pub(super) fn new_tab_via_mux(ws: &mut WindowState) {
    if cfg!(test) {
        // Unit tests must not start real daemons behind their
        // assertions: the attach worker would connect-or-spawn the
        // default session on any machine with par-mux installed. The
        // flip's decision rule is pinned by the pure-rule tests.
        return;
    }
    if !WindowState::new_tab_wants_mux(
        ws.config.load().tmux.use_mux_by_default,
        ws.tmux_state.transport.is_some(),
        ws.is_gateway_active(),
    ) || ws.tmux_state.mux_attach_pending.is_some()
    {
        return;
    }
    ws.tmux_state.mux_default_tab = Some(MuxDefaultTabOrigin::NewTab);
    ws.begin_mux_session_attach(crate::app::tmux_handler::tmux_state::DEFAULT_MUX_SESSION);
    if ws.tmux_state.mux_attach_pending.is_none() {
        // Refused before any worker started (the self-attach guard, a
        // dead socket dir, worker-spawn failure): the tab below is
        // simply a local shell; say so.
        ws.tmux_state.mux_default_tab = None;
        log::warn!("par-mux: default-session attach refused; the new tab stays a local shell");
    }
}
/// Arm the pending first-tab migration right after `new_tab_local` created
/// the synchronous local shell: only when an attach is in flight for this
/// request, the origin is a plain new-tab (not a profile or restore), the
/// active tab changed exactly by the new tab, and no earlier request still
/// holds the marker.
#[cfg(feature = "mux")]
pub(super) fn arm_default_placeholder(ws: &mut WindowState, previous: Option<TabId>) {
    let armed = ws.tmux_state.mux_attach_pending.is_some()
        && matches!(
            ws.tmux_state.mux_default_tab,
            Some(MuxDefaultTabOrigin::NewTab)
        )
        && ws.tab_manager.active_tab_id() != previous
        && ws.tmux_state.mux_default_placeholder.is_none();
    if armed {
        ws.tmux_state.mux_default_placeholder = ws.tab_manager.active_tab_id();
    }
}
