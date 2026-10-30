//! par-mux agent roster, pane title, and activity-indicator tests for attached
//! par-mux panes, including the roster manners rules (REPORT.md E3).
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    attach_test, connect, manners_state, pump_daemon_layouts, socket_path, spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, quiesce, wait_until};
use crate::app::window_state::WindowState;
use par_term_mux::AttachOutcome;
use std::time::{Duration, Instant};

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
        .send_settled("select-pane -t %0 -T 'build box'")
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
        .send_settled("select-pane -t %0 -T ''")
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
    let native = ws.tmux_state.tmux_pane_owners[&0].1;
    let local_title = |ws: &WindowState| -> Option<String> {
        ws.tab_manager.tabs().iter().find_map(|tab| {
            tab.pane_manager()
                .and_then(|pm| pm.get_pane(native))
                .map(|p| p.title.clone())
        })
    };

    // Rename through the app's write path: the local pane is named
    // immediately AND the daemon holds the title (queried back). The
    // rename sends inline, so the send worker is settled first.
    quiesce(&mut ws);
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
        .send_settled("pane-title -t %0")
        .expect("pane-title query");
    assert!(
        reply.iter().any(|l| l.trim() == "build box"),
        "the daemon owns the name: {reply:?}"
    );

    // Blank rename clears the daemon-side title too.
    quiesce(&mut ws);
    ws.rename_pane(native, "");
    let reply = ws
        .tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled("pane-title -t %0")
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
        .send_settled("split-window -h -t %0")
        .expect("split");
    transport
        .send_settled("select-pane -t %0 -T 'renamed pane'")
        .expect("user rename");
    transport
        // The sleep holds the prompt back: distro bashrcs set the title
        // from PS1 on every prompt, which would overwrite the OSC title.
        .send_settled("send-keys -t %1 -l 'printf \"\\033]0;osc pane\\007\"; sleep 60'")
        .expect("send printf");
    transport
        .send_settled("send-keys -t %1 Enter")
        .expect("enter");
    let deadline = Instant::now() + DAEMON_DEADLINE;
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
    assert!(ws.split_pane_via_mux(true), "split gives %1 to close");
    wait_until("the layout consumer maps the split pane %1", || {
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&1)
    });

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
    quiesce(&mut ws);
    assert!(
        ws.close_pane_via_mux(),
        "close must be consumed daemon-side"
    );
    let deadline = Instant::now() + DAEMON_DEADLINE;
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
    assert_eq!(rows.len(), 1, "the restart row while attached");
    assert_eq!(rows[0].action_id, "mux-restart-pane");
    // Detach is a built-in catalog row now (UX.md A23), not a runtime
    // one — the palette must not list it twice.
    assert!(
        crate::command_palette::catalog::build_catalog()
            .iter()
            .any(|e| e.action_id == "detach")
    );

    let _ = std::fs::remove_file(&path);
}
/// D7 (UX.md M4): a roster row whose pane lives in a HIDDEN mux tab
/// re-shows the tab. The pre-D7 code could not reach a hidden tab at
/// all — and computed its index over all tabs, 0-based, so visible
/// rows landed one tab to the left (UX.md M13/B5).
#[test]
fn roster_row_reshows_hidden_mux_tab() {
    let path = socket_path("hide-roster");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "hiderost", Some((80, 24)), &Default::default()).expect("attach");
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
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("hiderost".to_string());
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

    let tab_id = ws.tab_manager.tabs()[0].id;
    ws.tab_manager.switch_to(tab_id);
    assert!(!ws.hide_active_mux_tab(), "hiding keeps the window open");
    assert!(ws.tab_manager.get_tab(tab_id).unwrap().is_hidden);

    let pane_id = *ws
        .tmux_state
        .tmux_pane_owners
        .iter()
        .find(|(_, (owner, _))| *owner == tab_id)
        .map(|(pane, _)| pane)
        .expect("the hidden tab still owns its daemon pane");
    assert!(
        ws.focus_agent_roster_pane(pane_id),
        "the roster row resolves to the hidden tab's pane"
    );
    assert!(
        ws.tab_manager.get_tab(tab_id).is_some_and(|t| !t.is_hidden),
        "the roster row re-shows the hidden tab"
    );
    assert_eq!(ws.tab_manager.active_tab_id(), Some(tab_id));

    let _ = std::fs::remove_file(&path);
}

/// UX.md M13/B5: selecting a roster agent must focus the OWNING tab
/// and pane, from both ends of the tab strip — the pre-812dd8c5 code
/// computed the index 0-based over all tabs and landed one tab to
/// the left (tab 3's row focused tab 2, tab 1's row switched nowhere).
#[test]
fn roster_jump_from_first_and_third_tab_lands_on_the_owner() {
    let path = socket_path("roster-ends");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "rostends", Some((80, 24)), &Default::default()).expect("attach");
    transport.send_settled("new-window").expect("new-window");
    transport.send_settled("new-window").expect("new-window");
    pump_daemon_layouts(&transport, 3);

    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws =
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("rostends".to_string());
    ws.tmux_state.tmux_sync.enable();

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() >= 3 && ws.tmux_state.tmux_pane_owners.len() >= 3 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created 3 tabs: {} tabs, panes {:?}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // Start from the middle tab so a wrong switch is observable in
    // both directions.
    let middle_tab_id = ws.tab_manager.tabs()[1].id;
    ws.tab_manager.switch_to(middle_tab_id);
    assert_eq!(ws.tab_manager.active_tab_id(), Some(middle_tab_id));

    let owner_tab_of_pane = |ws: &WindowState, tab_id: crate::tab::TabId| {
        *ws.tmux_state
            .tmux_pane_owners
            .iter()
            .find(|(_, (owner, _))| *owner == tab_id)
            .map(|(pane, _)| pane)
            .expect("each tab owns a daemon pane")
    };

    // The first tab's agent: focus moves LEFT to tab 1.
    let first_tab_id = ws.tab_manager.tabs()[0].id;
    let first_pane = owner_tab_of_pane(&ws, first_tab_id);
    assert!(ws.focus_agent_roster_pane(first_pane));
    assert_eq!(
        ws.tab_manager.active_tab_id(),
        Some(first_tab_id),
        "tab 1's roster row must focus tab 1"
    );

    // The third tab's agent: focus moves RIGHT past the middle.
    let third_tab_id = ws.tab_manager.tabs()[2].id;
    let third_pane = owner_tab_of_pane(&ws, third_tab_id);
    assert!(ws.focus_agent_roster_pane(third_pane));
    assert_eq!(
        ws.tab_manager.active_tab_id(),
        Some(third_tab_id),
        "tab 3's roster row must focus tab 3, not one to the left"
    );

    let _ = std::fs::remove_file(&path);
}

/// UX.md B2: a background tab receiving output lights its tab-bar
/// activity dot; focusing the tab clears it, and the active tab never
/// shows its own dot.
#[test]
fn background_tab_output_lights_activity_dot_and_focus_clears_it() {
    let path = socket_path("activity-dot");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "actdot", Some((80, 24)), &Default::default()).expect("attach");
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
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("actdot".to_string());
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

    let first_id = ws.tab_manager.tabs()[0].id;
    let second_id = ws.tab_manager.tabs()[1].id;
    ws.tab_manager.switch_to(first_id);
    // Baseline: quiesce before the explicit phase — 0.58.1's daemon can
    // push a late `%output` wave for the new pane after `%window-add`,
    // and a single consume races it (observed flaking only under full
    // suite load). Two consecutive passes with no dot at all mean no
    // output arrived, so the dot below derives only from the explicit
    // mark_updated calls.
    let mut quiet_passes = 0;
    let quiesce_deadline = Instant::now() + DAEMON_DEADLINE;
    while quiet_passes < 2 {
        for tab in ws.tab_manager.tabs_mut() {
            tab.activity.has_activity = false;
        }
        ws.check_mux_notifications();
        ws.check_activity_idle_notifications();
        if ws
            .tab_manager
            .tabs()
            .iter()
            .all(|t| !t.activity.has_activity)
        {
            quiet_passes += 1;
        } else {
            quiet_passes = 0;
        }
        assert!(
            Instant::now() < quiesce_deadline,
            "output waves never quiesced before the baseline"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    for tab in ws.tab_manager.tabs() {
        assert!(!tab.activity.has_activity, "no dot before new output");
    }

    // New output in BOTH tabs: only the background one may dot.
    for tab in ws.tab_manager.tabs_mut() {
        if let Ok(term) = tab.terminal.try_write() {
            term.process_data(b"x");
        }
    }
    ws.check_activity_idle_notifications();
    assert!(
        ws.tab_manager
            .get_tab(second_id)
            .is_some_and(|t| t.activity.has_activity),
        "the background tab's output must light its dot"
    );
    assert!(
        !ws.tab_manager
            .get_tab(first_id)
            .is_some_and(|t| t.activity.has_activity),
        "the active tab never shows its own dot"
    );

    // Focusing the background tab clears it.
    ws.tab_manager.switch_to(second_id);
    assert!(
        !ws.tab_manager
            .get_tab(second_id)
            .is_some_and(|t| t.activity.has_activity),
        "focusing the tab clears the dot"
    );

    let _ = std::fs::remove_file(&path);
}

/// UX.md B13/TW1: closing a background tab via its close button
/// leaves the active tab unchanged.
#[test]
fn closing_a_background_tab_keeps_the_active_tab() {
    let path = socket_path("close-background");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach =
        attach_test(&transport, "closebg", Some((80, 24)), &Default::default()).expect("attach");
    transport.send_settled("new-window").expect("new-window");
    transport.send_settled("new-window").expect("new-window");
    pump_daemon_layouts(&transport, 3);

    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws =
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("closebg".to_string());
    ws.tmux_state.tmux_sync.enable();

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.tab_count() >= 3 && ws.tmux_state.tmux_pane_owners.len() >= 3 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "never created 3 tabs: {} tabs, panes {:?}",
            ws.tab_manager.tab_count(),
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    let first_id = ws.tab_manager.tabs()[0].id;
    let third_id = ws.tab_manager.tabs()[2].id;
    ws.tab_manager.switch_to(first_id);

    quiesce(&mut ws);
    ws.handle_tab_bar_action_after_render(crate::tab_bar_ui::TabBarAction::Close(third_id));
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tab_manager.get_tab(third_id).is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the background tab never closed: {} tabs",
            ws.tab_manager.tab_count()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        ws.tab_manager.active_tab_id(),
        Some(first_id),
        "closing a background tab must not change the active tab"
    );
    assert!(!ws.is_shutting_down, "two tabs remain, no shutdown");

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
        ws: String::new(),
        msg: String::new(),
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
    assert!(ws.last_toast_text().is_none());
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
        ws.last_toast_text().is_none(),
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
    assert!(ws.last_toast_text().is_none());
    assert!(ws.tab_manager.active_tab_id().is_none());
    assert!(ws.focus_state.pending_focus_tab_switch.is_none());
}
