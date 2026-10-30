//! par-mux last-tab close, end-session, and last-pane close tests for an attached
//! par-mux session.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    attach_test, connect, manners_state, pump_daemon_layouts, socket_path, spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, quiesce, wait_until};
use par_term_mux::{AttachOutcome, MuxSessionClient};
use std::time::{Duration, Instant};

/// UX.md M1: closing the LAST attached tab no longer kills the
/// session silently — the close gates on the Detach / End session /
/// Cancel dialog, and Detach leaves the session listed by a fresh
/// `list-sessions` client (the criterion's survival half).
#[test]
fn last_tab_close_gates_on_dialog_and_detach_keeps_the_session() {
    let path = socket_path("last-tab");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "lasttab", Some((80, 24)), &Default::default()).expect("attach");

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
    ws.tmux_state.tmux_session_name = Some("lasttab".to_string());
    ws.tmux_state.mux_session_id = Some(match &attach.outcome {
        AttachOutcome::Created(s) | AttachOutcome::Attached(s) => s.id,
    });
    ws.tmux_state.tmux_sync.enable();

    // A CREATED session reports its only window via the daemon's own
    // %window-add push; pump until it became a tab with a mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() >= 1 && !ws.tmux_state.tmux_pane_owners.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created the tab: {} tabs, panes {:?}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // The close GATES: dialog up, tab intact, daemon window alive —
    // the pre-M1 behavior (kill-window, session deleted by the
    // daemon) is what this pins out of existence.
    assert!(!ws.close_current_tab(), "waiting on the dialog");
    assert!(
        ws.overlay_ui.mux_last_tab_ui.is_visible(),
        "the last-tab dialog must be showing"
    );
    assert_eq!(
        ws.tab_manager.tab_count(),
        1,
        "the tab stays until a choice"
    );
    let windows = ws
        .tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("list-windows")
        .expect("list-windows while gated");
    assert!(
        windows.iter().any(|l| l.contains('@')),
        "the daemon window survived the gated close: {windows:?}"
    );

    // The dialog hides itself when it returns an action; mirror that
    // ordering before dispatching the handler.
    ws.overlay_ui.mux_last_tab_ui.hide();
    ws.handle_mux_last_tab_action(crate::mux_last_tab_ui::MuxLastTabAction::Detach);
    assert_eq!(ws.tab_manager.tab_count(), 0, "the mux view tore down");
    assert!(ws.tmux_state.transport.is_none(), "the attach dropped");
    let mut checker = MuxSessionClient::connect(&path).expect("checker client connects");
    let sessions = checker.list_sessions().expect("list-sessions after detach");
    assert!(
        sessions.iter().any(|s| s.name == "lasttab"),
        "detach leaves the session listed: {sessions:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// UX.md M1, second half: End session kills the session on the daemon
/// (a fresh `list-sessions` sees it gone and the view tears down via
/// the kill notifications), and a close that does NOT empty the
/// session — another mux tab still maps a window — never gates.
#[test]
fn last_tab_close_end_session_kills_it_and_multi_window_close_never_gates() {
    let path = socket_path("end-session");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "endsess", Some((80, 24)), &Default::default()).expect("attach");
    // A second window so the first close leaves another mux tab
    // mapped; background layouts are not pushed on their own, so
    // force the %layout-change broadcast per pane.
    transport.send_settled("new-window").expect("new-window");
    pump_daemon_layouts(&transport, 2);

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
    ws.tmux_state.tmux_session_name = Some("endsess".to_string());
    ws.tmux_state.mux_session_id = Some(match &attach.outcome {
        AttachOutcome::Created(s) | AttachOutcome::Attached(s) => s.id,
    });
    ws.tmux_state.tmux_sync.enable();

    // Two windows = two mux tabs.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() >= 2 && ws.tmux_state.tmux_pane_owners.len() >= 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created 2 tabs: {} tabs, panes {:?}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // Not the last attached tab: the close kills the window directly,
    // no dialog. The kill-window is sent inline.
    quiesce(&mut ws);
    assert!(!ws.close_current_tab());
    assert!(
        !ws.overlay_ui.mux_last_tab_ui.is_visible(),
        "a close that leaves another mux tab must not gate"
    );
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the killed window's tab never closed: {} tabs",
            ws.tab_manager.tab_count()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let windows = ws
        .tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("list-windows")
        .expect("list-windows after first close");
    assert_eq!(
        windows.iter().filter(|l| l.contains('@')).count(),
        1,
        "one daemon window left: {windows:?}"
    );

    // Now the survivor IS the last attached tab: the close gates.
    assert!(!ws.close_current_tab());
    assert!(ws.overlay_ui.mux_last_tab_ui.is_visible());

    // End session: kill-session on the daemon, the broadcast
    // notifications tear the view down, and a fresh client sees the
    // session gone.
    ws.overlay_ui.mux_last_tab_ui.hide();
    quiesce(&mut ws);
    ws.handle_mux_last_tab_action(crate::mux_last_tab_ui::MuxLastTabAction::EndSession);
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tmux_state.transport.is_none() && ws.tab_manager.tab_count() == 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the view never tore down after kill-session: {} tabs, transport {}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.transport.is_some()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut checker = MuxSessionClient::connect(&path).expect("checker client connects");
    let sessions = checker.list_sessions().expect("list-sessions after kill");
    assert!(
        sessions.iter().all(|s| s.name != "endsess"),
        "end session removes it from the daemon: {sessions:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// UX.md M1 + M14: End session is one daemon `kill-session`, so it
/// kills windows the client never mapped — a daemon window dropped
/// by `max_tabs` overflow has no tab here, and the per-window
/// fallback left exactly those windows (and the session) alive.
#[test]
fn end_session_kills_windows_the_client_never_mapped() {
    let path = socket_path("end-session-overflow");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "endovf", Some((80, 24)), &Default::default()).expect("attach");

    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut config = crate::config::Config::default();
    config.tabs.max_tabs = 1;
    let mut ws = crate::app::window_state::WindowState::new(config, runtime);
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("endovf".to_string());
    ws.tmux_state.mux_session_id = Some(match &attach.outcome {
        AttachOutcome::Created(s) | AttachOutcome::Attached(s) => s.id,
    });
    ws.tmux_state.tmux_sync.enable();

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() == 1 && !ws.tmux_state.tmux_pane_owners.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created the first tab: {} tabs, panes {:?}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // A second daemon window the client cannot map (max_tabs=1).
    ws.tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("new-window")
        .expect("new-window");
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        let windows = ws
            .tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("list-windows")
            .expect("list-windows");
        if windows.iter().filter(|l| l.contains('@')).count() == 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the overflow window never appeared on the daemon"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    // The %window-add for the overflow window was queued on this
    // client ahead of the new-window reply. A drain skips while the
    // send worker holds the client, so settle the worker first: the
    // next drain then takes everything queued, and the max_tabs drop
    // is real when the count below is read.
    quiesce(&mut ws);
    ws.check_mux_notifications();
    assert_eq!(
        ws.tab_manager.tab_count(),
        1,
        "max_tabs=1 must leave the second daemon window unmapped"
    );

    // The only mapped tab is the last attached tab: the close gates.
    assert!(!ws.close_current_tab());
    assert!(ws.overlay_ui.mux_last_tab_ui.is_visible());

    ws.overlay_ui.mux_last_tab_ui.hide();
    quiesce(&mut ws);
    ws.handle_mux_last_tab_action(crate::mux_last_tab_ui::MuxLastTabAction::EndSession);
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tmux_state.transport.is_none() && ws.tab_manager.tab_count() == 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the view never tore down after kill-session: {} tabs, transport {}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.transport.is_some()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut checker = MuxSessionClient::connect(&path).expect("checker client connects");
    let sessions = checker.list_sessions().expect("list-sessions after kill");
    assert!(
        sessions.iter().all(|s| s.name != "endovf"),
        "end session must kill the unmapped overflow window too: {sessions:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// D7 fallback contract: hiding the ONLY tab closes the window (the
/// implicit-detach shape) — a window with zero visible tabs has no
/// surface to render — and the daemon window survives it.
#[test]
fn hiding_the_only_mux_tab_closes_the_window_and_the_daemon_window_survives() {
    let path = socket_path("hide-last");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "hidelast", Some((80, 24)), &Default::default()).expect("attach");

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
    ws.tmux_state.tmux_session_name = Some("hidelast".to_string());
    ws.tmux_state.tmux_sync.enable();

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() >= 1 && !ws.tmux_state.tmux_pane_owners.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created the tab: {} tabs",
            ws.tab_manager.tab_count()
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // A CREATED session's first window arrives via %window-add, so the
    // id comes from the mapping the pump built.
    let only_tab = ws.tab_manager.tabs()[0].id;
    let only_window = ws
        .tmux_state
        .tmux_sync
        .get_window(only_tab)
        .expect("tab maps to a daemon window");
    ws.tab_manager.switch_to(only_tab);
    assert!(
        ws.hide_active_mux_tab(),
        "hiding the only tab answers close-the-window"
    );
    assert_eq!(ws.tab_manager.tab_count(), 0, "no zombie tab remains");
    let listing = ws
        .tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("list-windows")
        .expect("list-windows after fallback close");
    assert!(
        listing
            .iter()
            .any(|l| l.contains(&format!("@{only_window}"))),
        "the daemon window survived the window close: {listing:?}"
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

    // Re-driven until it lands: an attach attempt's round-trips ride
    // the core client's fixed 10 s reply budget, so a loaded machine
    // can fail one attempt (the transport it consumed is dropped with
    // it) though nothing is broken.
    let mut built: Option<(MuxTransport, AttachSequence)> = None;
    wait_until("attach_sequence completes", || {
        let transport = connect(&path);
        match attach_test(&transport, tag, Some((80, 24)), &Default::default()) {
            Ok(seq) => {
                built = Some((transport, seq));
                true
            }
            Err(_) => false,
        }
    });
    let (transport, attach) = built.expect("attach_sequence");
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

    let deadline = Instant::now() + DAEMON_DEADLINE;
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

    quiesce(&mut ws);
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
        .send_settled("list-windows")
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
    // transport, the way another client would. The reply is never
    // read — the tab-close loop below is the teeth — and queueing
    // avoids standing in the core client's fixed 10 s reply budget,
    // which one slow daemon round-trip can blow on a loaded machine.
    ws.tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_command_no_wait("kill-pane -t %0")
        .expect("queueing must not block");

    let deadline = Instant::now() + DAEMON_DEADLINE;
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
