//! Tests for the `use_mux_by_default` new-tab routing rule and the
//! pending first-tab migration that follows a successful default attach.

use super::WindowState;
use crate::app::tmux_handler::notifications::mux::tests::manners_state;
use crate::app::tmux_handler::pane_write::tests::RecordingTransport;

/// After a successful default attach, the pending local first tab is
/// replaced by the first daemon window's tab in the SAME slot — the
/// placeholder is closed (not an undo entry) and the daemon tab takes
/// its position, even when `max_tabs` is already full (net count zero).
#[test]
fn default_placeholder_is_replaced_in_the_same_slot() {
    let mut ws = manners_state();
    ws.new_tab_local();
    let placeholder = ws.tab_manager.active_tab_id().expect("first tab");
    ws.new_tab_local();
    let sibling = ws.tab_manager.active_tab_id().expect("second tab");
    ws.tmux_state.mux_default_placeholder = Some(placeholder);
    let (transport, _) = RecordingTransport::new();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.config.rcu(|old| {
        let mut config = (**old).clone();
        config.tabs.max_tabs = 2;
        std::sync::Arc::new(config)
    });

    ws.handle_tmux_window_add(42);

    let daemon = ws
        .tmux_state
        .tmux_sync
        .get_tab(42)
        .expect("daemon window mapped");
    let order: Vec<_> = ws.tab_manager.tabs().iter().map(|t| t.id).collect();
    assert_eq!(
        order,
        vec![daemon, sibling],
        "the daemon tab must sit in the replaced tab's slot"
    );
    assert!(
        ws.tab_manager.get_tab(placeholder).is_none(),
        "the placeholder must be gone — no duplicate tab"
    );
    assert!(
        ws.overlay_state.closed_tabs.is_empty(),
        "the placeholder close must not become a session-undo entry"
    );
    assert!(ws.tmux_state.mux_default_placeholder.is_none());
}

/// Once input went through the pending tab's PTY, migration is cancelled:
/// the local shell the user already used stays, and the daemon window's
/// tab appends normally.
#[test]
fn typing_keeps_the_default_placeholder_local() {
    let mut ws = manners_state();
    ws.new_tab_local();
    let placeholder = ws.tab_manager.active_tab_id().expect("first tab");
    ws.tmux_state.mux_default_placeholder = Some(placeholder);
    ws.tab_manager
        .get_tab(placeholder)
        .expect("tab")
        .terminal
        .blocking_read()
        .write(b"x")
        .expect("pty write");
    let (transport, _) = RecordingTransport::new();
    ws.tmux_state.transport = Some(Box::new(transport));

    ws.handle_tmux_window_add(42);

    assert!(
        ws.tab_manager.get_tab(placeholder).is_some(),
        "a used first tab must stay local"
    );
    assert_eq!(ws.tab_manager.tabs()[0].id, placeholder, "order kept");
    assert_eq!(
        ws.tab_manager.tab_count(),
        2,
        "the daemon window still arrives as its own tab"
    );
    assert!(ws.tmux_state.mux_default_placeholder.is_none());
}

/// A failed (or refused, or timed-out) default attach clears the pending
/// marker and leaves the local first tab alive and writable — today's
/// fallback, per criterion 2.
#[test]
fn failed_default_attach_keeps_a_functional_local_tab() {
    use crate::app::tmux_handler::notifications::mux::MuxAttachPending;
    use crate::app::tmux_handler::tmux_state::MuxDefaultTabOrigin;

    for kind in [
        std::io::ErrorKind::ConnectionRefused,
        std::io::ErrorKind::TimedOut,
    ] {
        let mut ws = manners_state();
        ws.new_tab_local();
        let placeholder = ws.tab_manager.active_tab_id().expect("first tab");
        ws.tmux_state.mux_default_placeholder = Some(placeholder);
        ws.tmux_state.mux_default_tab = Some(MuxDefaultTabOrigin::NewTab);
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Err(std::io::Error::new(kind, "attach failed")))
            .unwrap();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "main".to_string(),
            rx,
        });

        ws.poll_mux_attach();

        assert!(ws.tmux_state.transport.is_none(), "the attach failed");
        assert!(ws.tmux_state.mux_default_placeholder.is_none());
        let tab = ws.tab_manager.get_tab(placeholder).expect("tab kept");
        assert!(
            tab.terminal.blocking_read().write(b"\x03").is_ok(),
            "the fallback local shell must stay functional"
        );
    }
}

/// The routing rule's existing pins (unchanged).
#[test]
fn the_flip_on_routes_an_unattached_new_tab_through_mux() {
    assert!(WindowState::new_tab_wants_mux(true, false, false));
}

/// The flip off keeps the classic local tab.
#[test]
fn the_flip_off_keeps_the_local_tab() {
    assert!(!WindowState::new_tab_wants_mux(false, false, false));
}

/// An attached window's tabs are already daemon windows; the rule must
/// not reroute them.
#[test]
fn an_attached_window_is_already_routed() {
    assert!(!WindowState::new_tab_wants_mux(true, true, false));
}

/// A gateway window owns a multiplexer connection of its own; the
/// default flip must not stack a par-mux attach on top.
#[test]
fn a_gateway_window_is_not_rerouted() {
    assert!(!WindowState::new_tab_wants_mux(true, false, true));
}
