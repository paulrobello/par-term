//! par-mux pane operation tests for an attached par-mux window: split, swap,
//! close, two windows' pane maps, and a failed split's error path.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    FailingTransport, attach_test, connect, manners_state, pane_text, pane_var, socket_path,
    spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, poll_until, quiesce, wait_until};
use crate::pane::NavigationDirection;
use std::time::{Duration, Instant};

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
    // Push a layout for window @0 so the consumer maps %0.
    poll_until(|| {
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %0 -C 80x24")
            .expect("size push broadcasts %layout-change");
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&0)
    });
    assert!(
        ws.tmux_state.tmux_pane_owners.contains_key(&0),
        "the layout consumer never mapped %0"
    );

    quiesce(&mut ws);
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

/// A mux-mode pane swap goes DAEMON-side: `swap_pane_via_mux` sends
/// targeted `swap-pane -t %f -s %n`, the daemon broadcasts the new
/// layout, and the %layout-change consumer re-lays-out the local
/// mirror — the native pane owning daemon %0 ends up on the side the
/// focused pane held before. A local tree swap here would fight the
/// daemon's next layout push.
#[test]
fn swap_pane_via_mux_swaps_daemon_side_and_the_mirror_follows() {
    let path = socket_path("mux-swap");
    spawn_daemon(&path);

    // Attach through the app's own install path (poll arm), then map
    // %0 — the split test's pump.
    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "swapme".to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");
    ws.handle_tmux_window_add(0);
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
        assert!(
            Instant::now() < deadline,
            "the layout consumer never mapped %0"
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

    // Split to a second pane; the reply's new daemon pane %1 becomes
    // focused, and the layout consumer maps it into the mirror.
    quiesce(&mut ws);
    assert!(ws.split_pane_via_mux(true), "daemon-side split");
    wait_until("the layout consumer maps %1", || {
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&1)
    });
    quiesce(&mut ws);

    // The focused native pane maps to a daemon pane; its LEFT neighbor
    // maps to another (do not assume which id the split put where).
    let tab_id = ws.tab_manager.active_tab().unwrap().id;
    let (focused_native, neighbor_native) = {
        let tab = ws.tab_manager.active_tab().expect("active mux tab");
        let pm = tab.pane_manager().expect("pane manager");
        let focused = pm.focused_pane_id().expect("focused native pane");
        (
            focused,
            pm.neighbor_in_direction(focused, NavigationDirection::Left),
        )
    };
    let neighbor_native = neighbor_native.expect("left neighbor exists");
    let daemon_of = |native: crate::pane::PaneId| -> u64 {
        ws.tmux_state
            .tmux_pane_owners
            .iter()
            .find(|(_, (owner, pane))| *owner == tab_id && *pane == native)
            .map(|(&tmux, _)| tmux)
            .expect("daemon id for native pane")
    };
    let f_daemon = daemon_of(focused_native);
    let n_daemon = daemon_of(neighbor_native);
    assert_ne!(f_daemon, n_daemon, "two distinct daemon panes");

    // Swap focused with its left neighbor, daemon-side. The %layout-
    // change broadcast re-lays-out the mirror; native pane ids may be
    // recreated by that relayout, so each poll re-resolves them from
    // the pane map before reading bounds.
    let mut probe = par_term_mux::MuxSessionClient::connect(&path).expect("probe client");
    let before = probe.list_panes().expect("list-panes");
    assert!(
        ws.swap_pane_via_mux(NavigationDirection::Left),
        "daemon-side swap must be consumed"
    );
    let after = probe.list_panes().expect("list-panes");
    assert_ne!(
        before, after,
        "the daemon must swap the two panes (list-panes order {before:?} -> {after:?})"
    );
    let deadline = Instant::now() + DAEMON_DEADLINE;
    // The mirror's PRE-swap orientation has the focused pane right of
    // its neighbor (the split put it there); the consumer applying the
    // swapped layout flips that. Poll until the flip is visible.
    let flipped = loop {
        let native_of = |daemon: u64| -> Option<crate::pane::PaneId> {
            ws.tmux_state
                .tmux_pane_owners
                .iter()
                .find(|(tmux, (owner, _))| **tmux == daemon && *owner == tab_id)
                .map(|(_, (_, pane))| *pane)
        };
        let xf = native_of(f_daemon)
            .and_then(|n| ws.tab_manager.active_tab()?.pane_manager()?.get_pane(n))
            .map(|p| p.bounds.x);
        let xn = native_of(n_daemon)
            .and_then(|n| ws.tab_manager.active_tab()?.pane_manager()?.get_pane(n))
            .map(|p| p.bounds.x);
        if let (Some(xf), Some(xn)) = (xf, xn)
            && xf < xn
        {
            break true;
        }
        // The known test pump: a refresh-client size push forces the
        // consumer to apply the pending daemon layout.
        let _ = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %0 -C 80x24");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
        assert!(Instant::now() < deadline, "mirror never re-laid-out");
    };
    assert!(
        flipped,
        "the focused pane's mirror must end left of its neighbor's"
    );

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

    // Window @1 -> tab B. A fresh session numbers its panes %0, %1 in
    // creation order, so the second window owns %1.
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled("new-window")
        .expect("new-window");
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.tmux_pane_owners.contains_key(&1) {
        assert!(
            Instant::now() < deadline,
            "window @1 never got a mapped pane"
        );
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %1 -C 80x24")
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
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
        assert!(
            Instant::now() < deadline,
            "the layout consumer never mapped %0"
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
    quiesce(&mut ws);
    assert!(
        ws.split_pane_via_mux(true),
        "split gives the window a second pane to close"
    );
    // The split pane's mapping arrives with the %layout-change push,
    // not the command reply — pump until the consumer has mapped %1.
    wait_until("the layout consumer maps the split pane %1", || {
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&1)
    });

    // Close the focused pane (%1, the split's new pane) daemon-side.
    quiesce(&mut ws);
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
    let deadline = Instant::now() + DAEMON_DEADLINE;
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

/// UX.md M6: a failed par-mux split must not fall through to a LOCAL
/// split — the transport error consumes the call (no stray local
/// pane inside the attached tab) and surfaces a persistent error.
#[test]
fn failed_mux_split_creates_no_local_pane_and_shows_a_persistent_error() {
    let path = socket_path("split-fail");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "splitfail", Some((80, 24)), &Default::default()).expect("attach");

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
    ws.tmux_state.tmux_session_name = Some("splitfail".to_string());
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

    // Swap in the failing transport, then force the error.
    ws.tmux_state.transport = Some(Box::new(FailingTransport));
    ws.split_pane_vertical();

    let tab = ws.tab_manager.active_tab().expect("tab survives");
    assert!(
        !tab.has_multiple_panes(),
        "a failed mux split must not create a stray local pane"
    );
    let toast = ws
        .last_toast_text()
        .expect("split failure surfaces an error");
    assert!(toast.contains("split failed"), "toast says so: {toast}");
    assert!(
        ws.overlay_state
            .toasts
            .newest()
            .is_some_and(|t| t.kind == crate::app::overlay::toast::ToastKind::Error),
        "the error is persistent (no auto-hide): {toast}"
    );

    let _ = std::fs::remove_file(&path);
}
