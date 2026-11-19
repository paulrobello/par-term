//! par-mux ENH-045 lockstep tests: a mirror terminal adopts the daemon's
//! xterm-resize-reflow advertisement (the pane-info `reflow=` token) the
//! layout adoption probes for, and keeps legacy reflow when the daemon
//! advertises nothing — the daemon-off default the card pins.

use super::mux::tests::{manners_state, pane_text, socket_path, spawn_daemon, spawn_reflow_daemon};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend};
use crate::app::window_state::WindowState;
use std::time::{Duration, Instant};

/// The mapped mirror's reflow mode — read through the pane's terminal
/// the same way `pane_text` reads it, so the assertion checks the state
/// the reflow path (`set_xterm_resize`) actually drives.
fn mirror_reflow_mode(
    ws: &WindowState,
    tab_id: crate::tab::TabId,
    pane: crate::pane::PaneId,
) -> bool {
    ws.tab_manager
        .get_tab(tab_id)
        .and_then(|tab| tab.pane_manager())
        .and_then(|pm| pm.get_pane(pane))
        .and_then(|p| p.terminal.try_read().ok())
        .map(|term| term.xterm_resize())
        .expect("mirror terminal")
}

/// A mirror must adopt the daemon's xterm-resize-reflow advertisement:
/// the layout adoption fires the ENH-045 probe (`pane-info`), the
/// daemon's factory-set `reflow=1` presence token rides the
/// `PaneModes` job result through the drain, and `set_xterm_resize`
/// lands on the mirror terminal — which keeps mirroring daemon output
/// after the flip.
#[test]
fn mirrors_adopt_the_daemons_reflow_advertisement() {
    let path = socket_path("mux-reflow-on");
    spawn_reflow_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "reflow".to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");

    ws.handle_tmux_window_add(0);
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
        assert!(
            Instant::now() < deadline,
            "window @0 never got a mapped pane"
        );
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %0 -C 80x24")
            .expect("size push broadcasts %layout-change");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
    let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
    ws.tab_manager.switch_to(tab_id);

    // The probe submitted at adoption flows through the drain: the
    // PaneModes result seeds `mux_reflow_modes`, the end-of-poll sweep
    // applies it to the mapped mirror.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !mirror_reflow_mode(&ws, tab_id, pane) {
        assert!(
            Instant::now() < deadline,
            "the reflow advertisement never reached the mirror"
        );
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }

    // A marker injected while the post-attach pump seed is still in
    // flight folds into the seed payload and repaints with the
    // snapshot's cursor position — the marker must wait for the seed to
    // settle (the mux seed-ordering causality rule).
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.mux_seeds_in_flight.is_empty() {
        assert!(
            Instant::now() < deadline,
            "the post-attach pump seed never settled: {:?}",
            ws.tmux_state.mux_seeds_in_flight
        );
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }

    // The mirror still mirrors after the mode flip: daemon %output
    // routes to the mapped mirror terminal.
    ws.handle_tmux_output(0, b"reflow-lockstep-4999\r\n");
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !pane_text(&ws, tab_id, pane).contains("reflow-lockstep-4999") {
        assert!(
            Instant::now() < deadline,
            "the mirror never showed the marker after the mode flip\n\
                 pane_text: {:?}\n\
                 mode: {}\n\
                 seeds in flight: {:?}\n\
                 held output: {:?}\n\
                 backlog: {:?}",
            pane_text(&ws, tab_id, pane),
            mirror_reflow_mode(&ws, tab_id, pane),
            ws.tmux_state.mux_seeds_in_flight,
            ws.tmux_state.mux_held_output,
            ws.tmux_state.mux_output_backlog,
        );
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }

    let _ = std::fs::remove_file(&path);
}

/// Without the advertisement (the default daemon, no `reflow=` token)
/// the mirror keeps legacy reflow: the probe either reports false or
/// the mirror is simply never touched — either way `xterm_resize()`
/// stays false, the daemon-off default the card pins.
#[test]
fn mirrors_keep_legacy_reflow_without_the_advertisement() {
    let path = socket_path("mux-reflow-off");
    spawn_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "legacy".to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");

    ws.handle_tmux_window_add(0);
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
        assert!(
            Instant::now() < deadline,
            "window @0 never got a mapped pane"
        );
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %0 -C 80x24")
            .expect("size push broadcasts %layout-change");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
    let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
    ws.tab_manager.switch_to(tab_id);

    // Give the adoption probe its full chance to run: fixed ticks first,
    // then a bounded wait that would exit early on a flip that must not
    // come.
    for _ in 0..20 {
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while mirror_reflow_mode(&ws, tab_id, pane) {
        assert!(
            Instant::now() < deadline,
            "the mirror flipped to reflow without any advertisement"
        );
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !mirror_reflow_mode(&ws, tab_id, pane),
        "a mirror on the default daemon must keep legacy reflow"
    );

    let _ = std::fs::remove_file(&path);
}
