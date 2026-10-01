//! Attached-tab (par-mux) tests for the UX.md P3 pane features: zoom
//! (A1, criterion 1), and keyboard focus / resize / tab switch reaching
//! the daemon so a SECOND client sees them (M7, criterion 4).
//!
//! The daemon's `list-panes` replies ids only (no active flag), so the
//! second client's evidence is its own notification stream:
//! `%window-pane-changed` for focus and window selection, `%layout-change`
//! for resizes and zoom — plus `pane-info` sizes read from the daemon.

use super::mux::MuxAttachPending;
use super::mux::tests::{manners_state, socket_path, spawn_daemon};
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, quiesce, wait_until};
use crate::app::window_state::WindowState;
use crate::pane::NavigationDirection;
use par_term_emu_core_rust::mux::MuxClient;
use par_term_emu_core_rust::tmux_control::TmuxNotification;
use std::time::Instant;

fn send(ws: &WindowState, cmd: &str) -> Vec<String> {
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled(cmd)
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

/// Drain until `done` holds (checked before each pump, so a condition
/// already true applies nothing new), then settle the send worker so the
/// next inline action does not fail fast behind the drain's queued work.
fn drain_until(ws: &mut WindowState, what: &str, done: impl Fn(&WindowState) -> bool) {
    wait_until(what, || {
        done(ws) || {
            ws.check_mux_notifications();
            done(ws)
        }
    });
    quiesce(ws);
}

/// An attached window state with `%0` split side by side into `%0 | %1`,
/// both mapped, plus a second raw client connected to the same daemon.
fn attached_split(tag: &str) -> (WindowState, MuxClient, std::path::PathBuf) {
    let path = socket_path(tag);
    spawn_daemon(&path);
    let core_client = MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: tag.to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");
    // The created session's %window-add waits in the client channel; the
    // drain's window adoption maps %0. A manual handle_tmux_window_add(0)
    // here would add a second tab for @0 and make window counts wrong.
    drain_until(&mut ws, "%0 mapped", |ws| {
        ws.tmux_state.tmux_pane_owners.contains_key(&0)
    });
    assert_eq!(ws.tab_manager.tab_count(), 1, "one tab for @0");
    assert!(ws.split_pane_via_mux(true), "daemon-side split");
    assert!(
        !ws.tmux_state
            .mux_last_error
            .as_deref()
            .is_some_and(|e| e.contains("split failed")),
        "the split reached the daemon: {:?}",
        ws.tmux_state.mux_last_error
    );
    drain_until(&mut ws, "split mapped", |ws| {
        ws.tmux_state.tmux_pane_owners.contains_key(&1)
            && ws
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager())
                .is_some_and(|pm| pm.pane_count() == 2)
    });
    // Lay the mirror out so directional lookups have real bounds.
    if let Some(pm) = ws
        .tab_manager
        .active_tab_mut()
        .and_then(|t| t.pane_manager_mut())
    {
        pm.set_bounds(crate::pane::PaneBounds::new(0.0, 0.0, 800.0, 480.0));
    }
    let mut second = MuxClient::connect(&path).expect("second client");
    // The daemon registers a connection for broadcasts on its first command.
    second
        .send("list-windows")
        .expect("second client registers");
    // Drop whatever the second client saw while connecting.
    while second.notifications().try_recv().is_ok() {}
    (ws, second, path)
}

/// Wait until the second client receives a notification matching `want`.
fn second_sees(second: &MuxClient, what: &str, want: impl Fn(&TmuxNotification) -> bool) {
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "the second client never saw {what}");
        match second.notifications().recv_timeout(left) {
            Ok(n) if want(&n) => return,
            Ok(_) => {}
            Err(e) => panic!("the second client never saw {what}: {e}"),
        }
    }
}

fn focused_daemon_pane(ws: &WindowState) -> Option<u64> {
    ws.focused_mux_pane_from_native()
}

fn zoomed_native(ws: &WindowState) -> Option<crate::pane::PaneId> {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.zoomed_pane_id())
}

/// Criterion 1 (attached): zoom goes daemon-side, the mirror shows the
/// zoomed pane alone with the tab badge, the zoomed mirror terminal takes
/// the full window grid, and a keyboard focus move unzooms (the daemon
/// clears the zoom on `select-pane`). A split also unzooms.
#[test]
fn attached_zoom_toggles_shows_its_badge_and_a_focus_move_or_split_unzooms() {
    let (mut ws, second, path) = attached_split("p3-zoom");
    let focused = focused_daemon_pane(&ws).expect("a mux pane is focused");

    ws.toggle_pane_zoom();
    drain_until(&mut ws, "zoom mirrored", |ws| zoomed_native(ws).is_some());
    second_sees(&second, "the zoomed layout", |n| {
        matches!(n, TmuxNotification::LayoutChange { window_raw_flags, .. }
            if window_raw_flags.contains('Z'))
    });
    let tab = ws.tab_manager.active_tab().expect("tab");
    assert_eq!(
        tab.pane_mode_badge(),
        Some(crate::tab::pane_badges::ZOOM_BADGE),
        "the tab shows the zoom badge"
    );
    let pm = tab.pane_manager().expect("pm");
    assert_eq!(pm.visible_panes().len(), 1, "only the zoomed pane is drawn");
    assert_eq!(
        ws.tmux_state
            .tmux_pane_owner(focused)
            .map(|(_, native)| native),
        zoomed_native(&ws),
        "the focused pane is the zoomed one"
    );
    let info = send(&ws, &format!("pane-info -t %{focused}")).join(" ");
    let zoomed_mirror = pm
        .get_pane(zoomed_native(&ws).unwrap())
        .and_then(|p| p.terminal.try_read().ok())
        .map(|t| t.dimensions())
        .expect("mirror dims");
    assert!(
        info.contains(&format!("{}x{}", zoomed_mirror.0, zoomed_mirror.1)),
        "the zoomed mirror has the daemon PTY's full-window size {zoomed_mirror:?}: {info}"
    );

    // Keyboard focus move → select-pane → the DAEMON unzooms (the second
    // client sees an unflagged layout and the zoomed pane's PTY is back to
    // its tree size) → the mirror follows.
    let zoomed_cols = daemon_size(&ws, focused).0;
    ws.navigate_pane(NavigationDirection::Left);
    ws.navigate_pane(NavigationDirection::Right);
    second_sees(&second, "the unzoomed layout", |n| {
        matches!(n, TmuxNotification::LayoutChange { window_raw_flags, .. }
            if !window_raw_flags.contains('Z'))
    });
    assert!(
        daemon_size(&ws, focused).0 < zoomed_cols,
        "the daemon unzoomed: the pane is back to its tree width"
    );
    drain_until(&mut ws, "unzoom on focus move", |ws| {
        zoomed_native(ws).is_none()
    });
    assert_eq!(ws.tab_manager.active_tab().unwrap().pane_mode_badge(), None);

    // Zoom again, then split: the daemon clears the zoom for the new pane.
    ws.toggle_pane_zoom();
    drain_until(&mut ws, "zoom again", |ws| zoomed_native(ws).is_some());
    assert!(ws.split_pane_via_mux(false), "split while zoomed");
    drain_until(&mut ws, "unzoom on split", |ws| {
        zoomed_native(ws).is_none()
            && ws
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager())
                .is_some_and(|pm| pm.pane_count() == 3)
    });

    let _ = std::fs::remove_file(&path);
}

/// Criterion 4 (focus): keyboard focus moves — directional, next/prev,
/// last pane — reach the daemon as `select-pane`, which a second client
/// sees as `%window-pane-changed` naming the newly focused pane.
#[test]
fn keyboard_focus_moves_are_visible_to_a_second_client() {
    let (mut ws, second, path) = attached_split("p3-focus");
    let start = focused_daemon_pane(&ws).expect("focused");

    ws.focus_pane_cycle(true);
    let after_next = focused_daemon_pane(&ws).expect("focused");
    assert_ne!(after_next, start, "next pane moved focus");
    second_sees(&second, "select-pane after next_pane", |n| {
        matches!(n, TmuxNotification::WindowPaneChanged { pane_id, .. }
            if *pane_id == format!("%{after_next}"))
    });

    ws.focus_last_pane();
    assert_eq!(
        focused_daemon_pane(&ws),
        Some(start),
        "last pane toggles back"
    );
    second_sees(&second, "select-pane after last_pane", |n| {
        matches!(n, TmuxNotification::WindowPaneChanged { pane_id, .. }
            if *pane_id == format!("%{start}"))
    });

    let dir = if ws
        .tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.neighbor_in_direction(pm.focused_pane_id()?, NavigationDirection::Left))
        .is_some()
    {
        NavigationDirection::Left
    } else {
        NavigationDirection::Right
    };
    ws.navigate_pane(dir);
    let after_nav = focused_daemon_pane(&ws).expect("focused");
    assert_ne!(after_nav, start);
    second_sees(&second, "select-pane after directional focus", |n| {
        matches!(n, TmuxNotification::WindowPaneChanged { pane_id, .. }
            if *pane_id == format!("%{after_nav}"))
    });

    let _ = std::fs::remove_file(&path);
}

/// The daemon pane's grid size, from `pane-info` (`%N @W COLSxROWS ...`).
fn daemon_size(ws: &WindowState, pane: u64) -> (u16, u16) {
    let info = send(ws, &format!("pane-info -t %{pane}")).join(" ");
    let size = info.split_whitespace().nth(2).expect("size field");
    let (c, r) = size.split_once('x').expect("COLSxROWS");
    (c.parse().expect("cols"), r.parse().expect("rows"))
}

/// Criterion 4 (resize): a keyboard arrow resize on an attached tab goes
/// daemon-side (`resize-pane -L/-R`), so the daemon pane's size changes in
/// the arrow's direction and a second client receives the `%layout-change`.
#[test]
fn keyboard_resize_is_visible_to_a_second_client() {
    let (mut ws, second, path) = attached_split("p3-resize");
    let focused = focused_daemon_pane(&ws).expect("focused");
    let left_is_focused = ws
        .tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.neighbor_in_direction(pm.focused_pane_id()?, NavigationDirection::Right))
        .is_some();
    // A same-panes layout push makes the mirror adopt the daemon grid (the
    // headless test has no renderer to size it), so pixel bounds and
    // cells agree before the resize is computed.
    send(&ws, "refresh-client -t %0 -C 80x24");
    drain_until(&mut ws, "mirror adopts the daemon grid", |ws| {
        ws.tmux_state
            .tmux_pane_owner(focused)
            .is_some_and(|(tab, native)| {
                ws.tab_manager
                    .get_tab(tab)
                    .and_then(|t| t.pane_manager())
                    .and_then(|pm| pm.get_pane(native))
                    .and_then(|p| p.terminal.try_read().ok().map(|t| t.dimensions().0))
                    == Some(daemon_size(ws, focused).0 as usize)
            })
    });
    while second.notifications().try_recv().is_ok() {}
    let (cols_before, rows_before) = daemon_size(&ws, focused);

    // Right on the left pane grows it; Right on the right pane moves its
    // left divider right, shrinking it — either way the divider moves right.
    ws.resize_pane(NavigationDirection::Right);
    second_sees(&second, "the resize layout", |n| {
        matches!(n, TmuxNotification::LayoutChange { .. })
    });
    let (cols_after, rows_after) = daemon_size(&ws, focused);
    assert_eq!(rows_after, rows_before, "a Right resize keeps the height");
    if left_is_focused {
        assert!(cols_after > cols_before, "{cols_before} -> {cols_after}");
    } else {
        assert!(cols_after < cols_before, "{cols_before} -> {cols_after}");
    }
    // The mirror adopts the daemon geometry from the layout push.
    drain_until(&mut ws, "mirror resized", |ws| {
        ws.tmux_state
            .tmux_pane_owner(focused)
            .and_then(|(tab, native)| {
                ws.tab_manager
                    .get_tab(tab)?
                    .pane_manager()?
                    .get_pane(native)?
                    .terminal
                    .try_read()
                    .ok()
                    .map(|t| t.dimensions().0 as u16 == cols_after)
            })
            .unwrap_or(false)
    });

    let _ = std::fs::remove_file(&path);
}

/// Criterion 4 (tab switch): switching tabs from the keyboard on an
/// attached window sends `select-window`, which a second client sees as
/// `%window-pane-changed` for the newly selected window.
#[test]
fn keyboard_tab_switch_is_visible_to_a_second_client() {
    let (mut ws, second, path) = attached_split("p3-tabs");
    send(&ws, "new-window");
    drain_until(&mut ws, "second window mapped", |ws| {
        ws.tab_manager
            .tabs()
            .iter()
            .filter(|t| ws.mux_window_for_tab(t.id).is_some())
            .count()
            == 2
    });
    while second.notifications().try_recv().is_ok() {}

    for step in 0..2 {
        ws.next_tab();
        let window = ws
            .tab_manager
            .active_tab_id()
            .and_then(|id| ws.mux_window_for_tab(id))
            .expect("the active tab mirrors a daemon window");
        second_sees(&second, &format!("select-window step {step}"), |n| {
            matches!(n, TmuxNotification::WindowPaneChanged { window_id, .. }
                if *window_id == format!("@{window}"))
        });
    }

    let _ = std::fs::remove_file(&path);
}

/// Split left (A5) in an attached tab: `split-window -b` puts the new
/// daemon pane left of the focused one, and the mirror follows.
#[test]
fn attached_split_left_places_the_new_pane_before_the_focused_one() {
    let (mut ws, _second, path) = attached_split("p3-split-left");
    let focused = focused_daemon_pane(&ws).expect("focused");
    let before: std::collections::HashSet<u64> =
        ws.tmux_state.tmux_pane_owners.keys().copied().collect();
    ws.split_pane_before(crate::pane::SplitDirection::Vertical);
    drain_until(&mut ws, "split-left mapped", |ws| {
        ws.tmux_state.tmux_pane_owners.len() == before.len() + 1
    });
    let new = *ws
        .tmux_state
        .tmux_pane_owners
        .keys()
        .find(|p| !before.contains(p))
        .expect("new daemon pane");
    // Lay the mirror out and compare x positions.
    if let Some(pm) = ws
        .tab_manager
        .active_tab_mut()
        .and_then(|t| t.pane_manager_mut())
    {
        pm.set_bounds(crate::pane::PaneBounds::new(0.0, 0.0, 800.0, 480.0));
    }
    let x_of = |ws: &WindowState, daemon: u64| {
        let (tab, native) = ws.tmux_state.tmux_pane_owner(daemon).expect("mapped");
        ws.tab_manager
            .get_tab(tab)
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.get_pane(native))
            .map(|p| p.bounds.x)
            .expect("mirror pane")
    };
    assert!(
        x_of(&ws, new) < x_of(&ws, focused),
        "the new pane sits left of the pane that was focused"
    );
    let _ = std::fs::remove_file(&path);
}

/// Equalize (A3) in an attached tab: after a resize skews the split, the
/// mirror computes equal sizes and sends them as absolute `resize-pane`;
/// the daemon's panes come back within a cell of each other and a second
/// client sees the `%layout-change`.
#[test]
fn attached_equalize_evens_the_daemon_panes() {
    let (mut ws, second, path) = attached_split("p3b-equalize");
    send(&ws, "refresh-client -t %0 -C 80x24");
    drain_until(&mut ws, "mirror adopts the daemon grid", |ws| {
        ws.tmux_state
            .tmux_pane_owner(0)
            .is_some_and(|(tab, native)| {
                ws.tab_manager
                    .get_tab(tab)
                    .and_then(|t| t.pane_manager())
                    .and_then(|pm| pm.get_pane(native))
                    .and_then(|p| p.terminal.try_read().ok().map(|t| t.dimensions().0))
                    == Some(daemon_size(ws, 0).0 as usize)
            })
    });
    send(&ws, "resize-pane -t %0 -x 60");
    drain_until(&mut ws, "skewed layout mirrored", |ws| {
        daemon_size(ws, 0).0 >= 55
    });
    while second.notifications().try_recv().is_ok() {}

    ws.equalize_panes();
    second_sees(&second, "the equalize layout", |n| {
        matches!(n, TmuxNotification::LayoutChange { .. })
    });
    wait_until("the daemon panes even out", || {
        ws.check_mux_notifications();
        daemon_size(&ws, 0).0.abs_diff(daemon_size(&ws, 1).0) <= 1
    });
    let _ = std::fs::remove_file(&path);
}

/// Layout presets (A4) are refused in an attached tab with a toast, and
/// the daemon layout is untouched: core 0.57 has no `select-layout`, and
/// a structural preset needs panes moved daemon-side (documented bound).
#[test]
fn attached_layout_presets_are_refused_and_leave_the_daemon_layout() {
    let (mut ws, _second, path) = attached_split("p3b-preset");
    let before = (daemon_size(&ws, 0), daemon_size(&ws, 1));
    for preset in crate::pane::LayoutPreset::ALL {
        ws.overlay_state.toasts.clear();
        ws.apply_layout_preset(preset);
        let toast = ws.last_toast_text().map(str::to_string).unwrap_or_default();
        assert!(
            toast.contains("not available in par-mux tabs"),
            "{}: {toast:?}",
            preset.name()
        );
    }
    // Anything a preset queued has reached the daemon once the worker is
    // idle, so the sizes below are final.
    quiesce(&mut ws);
    assert_eq!((daemon_size(&ws, 0), daemon_size(&ws, 1)), before);
    let _ = std::fs::remove_file(&path);
}
