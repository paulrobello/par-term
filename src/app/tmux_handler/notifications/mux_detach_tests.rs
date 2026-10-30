//! par-mux detach, daemon-death, and par-mux wording tests: detach tears down
//! local state while the daemon session survives, and every surfaced
//! string says par-mux, never tmux.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    FailingTransport, attach_test, connect, manners_state, pump_daemon_layouts, socket_path,
    spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend};
use crate::app::tmux_handler::tmux_state::TmuxTransport;
use par_term_mux::{AgentEntry, AttachOutcome};
use std::io;
use std::time::{Duration, Instant};

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
        ws.last_toast_text().is_none(),
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
        last_input_at: None,
        stalled: None,
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
        ws.last_toast_text(),
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
        transport.send_settled("new-window").expect("new-window");
    }
    pump_daemon_layouts(&transport, 4);

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
    let deadline = Instant::now() + DAEMON_DEADLINE;
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

/// D7 (UX.md M4): closing a mux tab's last pane HIDES the tab — the
/// daemon window keeps running, the window↔tab mapping stays live, and
/// the typed undo entry re-shows it on Cmd+Z. The pre-D7 behavior (tab
/// dropped, invisible daemon window, ghost roster rows) is what this
/// test pins out of existence.
#[test]
fn last_pane_close_hides_mux_tab_and_undo_reshows_it() {
    let path = socket_path("hide-undo");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "hideundo", Some((80, 24)), &Default::default()).expect("attach");
    // A second window so hiding the first leaves a visible successor.
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
    ws.tmux_state.tmux_session_name = Some("hideundo".to_string());
    ws.tmux_state.tmux_sync.enable();

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

    // Focus a daemon window's tab, then run the last-pane close path
    // against it. A CREATED session reports its first window via the
    // daemon's own %window-add push, so the id comes from the mapping
    // the pump built, not from `existing_windows` (empty for new
    // sessions).
    let tab_id = ws.tab_manager.tabs()[0].id;
    let first_window = ws
        .tmux_state
        .tmux_sync
        .get_window(tab_id)
        .expect("tab maps to a daemon window");
    ws.tab_manager.switch_to(tab_id);

    assert!(!ws.hide_active_mux_tab(), "hiding keeps the window open");
    let tab = ws
        .tab_manager
        .get_tab(tab_id)
        .expect("hidden tab stays in the manager");
    assert!(tab.is_hidden, "last-pane close hides the tab, not drops it");
    assert_eq!(ws.tab_manager.visible_tab_count(), 1);
    assert_ne!(
        ws.tab_manager.active_tab_id(),
        Some(tab_id),
        "active moved to the visible successor"
    );
    assert_eq!(
        ws.tmux_state.tmux_sync.get_window(tab_id),
        Some(first_window),
        "the window↔tab mapping stays live"
    );
    let listing = ws
        .tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("list-windows")
        .expect("list-windows after hide");
    assert!(
        listing
            .iter()
            .any(|l| l.contains(&format!("@{first_window}"))),
        "the daemon window survived the last-pane close: {listing:?}"
    );
    let entry = ws
        .overlay_state
        .closed_tabs
        .front()
        .expect("a typed undo entry was pushed");
    let hidden = entry
        .hidden_mux_window
        .as_ref()
        .expect("entry is the re-show kind");
    assert_eq!(hidden.tab_id, tab_id);
    assert_eq!(hidden.window_id, first_window);

    // Cmd+Z re-shows the hidden tab.
    ws.reopen_closed_tab();
    assert!(
        ws.tab_manager.get_tab(tab_id).is_some_and(|t| !t.is_hidden),
        "undo re-shows the hidden tab"
    );
    assert_eq!(ws.tab_manager.active_tab_id(), Some(tab_id));
    assert!(
        ws.last_toast_text().unwrap_or("").contains("re-shown"),
        "the re-show names what happened: {:?}",
        ws.last_toast_text()
    );

    let _ = std::fs::remove_file(&path);
}

/// A transport already dead — drain reports the disconnect, the T8
/// daemon-death tests. The drain's `true` is what synthesizes the
/// `SessionEnded` that tears the mux view down.
struct DeadTransport;

impl TmuxTransport for DeadTransport {
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
        (Vec::new(), true)
    }

    fn send_command(&self, _command: &str) -> io::Result<Vec<String>> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "daemon is dead",
        ))
    }

    fn send_command_no_wait(&self, _command: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "daemon is dead",
        ))
    }
}

/// UX.md T8: when the daemon dies, the drain synthesizes a
/// `SessionEnded` AFTER the transport is dropped, so the wording must
/// come from the remembered mux session — "tmux: Session ended" on a
/// par-mux path is the exact defect the criterion bans.
#[test]
fn daemon_death_toasts_par_mux_wording_not_tmux() {
    let path = socket_path("t8-death");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "t8death", Some((80, 24)), &Default::default()).expect("attach");

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
    ws.tmux_state.tmux_session_name = Some("t8death".to_string());
    ws.tmux_state.mux_session_id = Some(match &attach.outcome {
        AttachOutcome::Created(s) | AttachOutcome::Attached(s) => s.id,
    });
    ws.tmux_state.tmux_sync.enable();

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.focused_mux_pane_from_native().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never focused a daemon-mirrored pane"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // Kill the transport: drain reports the abrupt disconnect, which
    // synthesizes the SessionEnded that runs the shared teardown.
    ws.tmux_state.transport = Some(Box::new(DeadTransport));
    ws.check_mux_notifications();

    let toast = ws.last_toast_text().expect("daemon death surfaces a toast");
    assert!(
        toast.contains("par-mux"),
        "the death toast must say par-mux, got: {toast}"
    );
    assert!(
        !toast.contains("tmux"),
        "T8 — no tmux wording on a par-mux path, got: {toast}"
    );

    let _ = std::fs::remove_file(&path);
}

/// UX.md T8/M11: a tab created for a daemon window is titled
/// "par-mux @N" under a transport and "tmux @N" without one — the
/// real-tmux gateway keeps its wording, the mux path never shows it.
#[test]
fn window_add_titles_discriminate_par_mux_from_tmux() {
    let mut mux_ws = manners_state();
    mux_ws.tmux_state.transport = Some(Box::new(FailingTransport));
    mux_ws.handle_tmux_window_add(7);
    let tab = mux_ws
        .tab_manager
        .active_tab()
        .expect("daemon window becomes a tab");
    assert_eq!(tab.title, "par-mux @7");

    let mut tmux_ws = manners_state();
    tmux_ws.handle_tmux_window_add(3);
    let tab = tmux_ws
        .tab_manager
        .active_tab()
        .expect("tmux window becomes a tab");
    assert_eq!(tab.title, "tmux @3");
}

/// UX.md T8 end-to-end: a real attach (attach closure included — it
/// creates the display tabs BEFORE the transport is installed) leaves
/// no "tmux"-worded tab title behind, named or not.
#[test]
fn real_attach_leaves_no_tmux_worded_tab_titles() {
    let path = socket_path("t8-titles");
    spawn_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);

    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "t8titles".to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(
        ws.tmux_state.transport.is_some(),
        "the attach must have succeeded"
    );

    // No renderer in a headless test → attach carries no client size →
    // the daemon never pushes the layout that mirrors panes. The
    // wording claim under test is about TITLES: waiting for the
    // %window-add push to create its mapped tab is enough.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws
            .tab_manager
            .tabs()
            .iter()
            .any(|t| ws.tmux_state.tmux_sync.get_window(t.id).is_some())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the daemon window never became a mapped tab"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    for tab in ws.tab_manager.tabs() {
        assert!(
            !tab.title.contains("tmux"),
            "T8 — tab title must not say tmux, got: {}",
            tab.title
        );
    }

    let _ = std::fs::remove_file(&path);
}
