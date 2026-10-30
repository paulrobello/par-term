//! par-mux attach, reattach, and attach-guard tests against a live in-process
//! daemon: the reattach replay, the version handshake, the off-loop attach
//! worker, the self-attach guard, and restored-session placeholders.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    adopt_panes, attach_test, connect, manners_state, pending_with_client, poll_actions,
    socket_path, spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, attach_until_installed};
use crate::tmux::TmuxSync;
use par_term_mux::{AttachOutcome, MuxSessionClient, VersionCheck, check_daemon_version};
use par_term_tmux::SyncAction;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// Marker echoed inside a pane before detach; the reattach screen
/// replay must carry it back as `PaneOutput`.
const MARKER: &str = "par-term-mux-wiring-marker";

fn wait_for(
    transport: &MuxTransport,
    sync: &mut TmuxSync,
    wanted: impl Fn(&SyncAction) -> bool,
) -> Vec<SyncAction> {
    let deadline = Instant::now() + DAEMON_DEADLINE;
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
    panic!("expected action never arrived within {DAEMON_DEADLINE:?}; saw: {seen:?}");
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
        let deadline = Instant::now() + DAEMON_DEADLINE;
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
        let deadline = Instant::now() + DAEMON_DEADLINE;
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
        let attach =
            attach_test(&transport, "keep", Some((80, 24)), &Default::default()).expect("attach");
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
    let toast = ws.last_toast_text().unwrap_or_default();
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
    assert!(ws.last_toast_text().is_none());
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
    let toast = ws.last_toast_text().unwrap_or_default();
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

    let mut client =
        par_term_mux::MuxSessionClient::connect_or_spawn_at(&path).expect("connect to test daemon");
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
        ws.last_toast_text().is_some_and(|t| t.contains("refused")),
        "the refusal must toast, got {:?}",
        ws.last_toast_text()
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
    let toast = ws.last_toast_text();
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
    let toast = ws.last_toast_text();
    assert!(
        toast.is_some_and(|t| t.contains("already attached") && t.contains("live")),
        "the second open must name the attached session, got {toast:?}"
    );
    let _ = std::fs::remove_file(&path);
}

/// The session-restore shape for a mux-attached window: restore spawns
/// ONE local shell tab (so a failed attach never leaves an empty
/// window) and marks it as the placeholder. These tests pin the
/// placeholder's lifecycle: closed by the first daemon window tab,
/// whichever road that tab arrives by, kept when the attach fails.
fn restore_placeholder_state() -> crate::app::window_state::WindowState {
    let mut ws = manners_state();
    let placeholder = ws
        .tab_manager
        .new_tab(
            &ws.config.load(),
            std::sync::Arc::clone(&ws.runtime),
            false,
            None,
        )
        .expect("placeholder tab");
    ws.tmux_state.mux_restore_placeholder_tab = Some(placeholder);
    ws
}

#[test]
fn restored_mux_placeholder_closes_when_a_daemon_window_installs() {
    let path = socket_path("restore-placeholder");
    spawn_daemon(&path);

    // Seed the session with one live window so the reattach reports it
    // in existing_windows (a created session reports its window later
    // via %window-add instead — the sibling test below).
    {
        let mut seed = MuxSessionClient::connect(&path).expect("seed client");
        seed.create_or_attach("restoretest").expect("create");
        let deadline = Instant::now() + DAEMON_DEADLINE;
        let mut created = false;
        while Instant::now() < deadline && !created {
            created = seed
                .poll_actions()
                .iter()
                .any(|a| matches!(a, SyncAction::CreateTab { .. }));
            if !created {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(created, "seed client never saw the session's window");
    }

    let mut ws = restore_placeholder_state();
    let placeholder = ws.tmux_state.mux_restore_placeholder_tab;
    attach_until_installed(&mut ws, || pending_with_client("restoretest", &path), &path);

    assert!(
        ws.tab_manager
            .get_tab(placeholder.expect("placeholder marked"))
            .is_none(),
        "the restore placeholder must close once a daemon window tab exists"
    );
    assert_eq!(
        ws.tab_manager.tabs().len(),
        1,
        "exactly the daemon window's tab remains"
    );
}

#[test]
fn restored_mux_placeholder_closes_on_a_late_window_add_too() {
    let path = socket_path("restore-late-add");
    spawn_daemon(&path);

    // Fresh daemon: attach CREATES the session, so no existing windows
    // install — the window arrives later via %window-add.
    let mut ws = restore_placeholder_state();
    let placeholder = ws.tmux_state.mux_restore_placeholder_tab;
    attach_until_installed(&mut ws, || pending_with_client("lateadd", &path), &path);
    assert!(
        ws.tab_manager
            .get_tab(placeholder.expect("placeholder marked"))
            .is_some(),
        "no daemon tab yet — the placeholder must stay"
    );

    ws.handle_tmux_window_add(0);
    assert!(
        ws.tmux_state.mux_restore_placeholder_tab.is_none(),
        "the placeholder mark is consumed either way"
    );
    assert!(
        ws.tab_manager
            .get_tab(placeholder.expect("placeholder marked"))
            .is_none(),
        "the late %window-add must close the placeholder too"
    );
    assert_eq!(ws.tab_manager.tabs().len(), 1);
}

#[test]
fn restored_mux_placeholder_survives_a_failed_attach() {
    let mut ws = restore_placeholder_state();
    let placeholder = ws.tmux_state.mux_restore_placeholder_tab;
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Err(std::io::Error::other("attach boom"))).unwrap();
    drop(tx);
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "restorefail".to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(
        ws.tmux_state.transport.is_none(),
        "the attach must have failed"
    );
    assert!(
        ws.tab_manager
            .get_tab(placeholder.expect("placeholder marked"))
            .is_some(),
        "a failed attach must keep the placeholder so the window stays usable"
    );
    assert!(
        ws.tmux_state.mux_restore_placeholder_tab.is_none(),
        "the mark must retire with the failed attach — the tab is an \
             ordinary tab now, not closable by a later window-add"
    );
    assert_eq!(ws.tab_manager.tabs().len(), 1);
}

/// A daemon window that dies before its first %layout-change never
/// set `tab.tmux.tmux_pane_id`, so the session-ended cleanup's
/// pane-id filter skipped its tab — an immortal dead tab after the
/// session ended (found while writing the emptied-session E2E test:
/// kill-before-layout left tabs=1 with the sync table cleared).
/// The sync mapping identifies the tab: every window-add maps,
/// layout or not.
#[test]
fn a_layout_less_mux_tab_closes_when_the_session_ends() {
    let path = socket_path("zombie-tab");
    spawn_daemon(&path);

    let mut ws = manners_state();
    attach_until_installed(&mut ws, || pending_with_client("zombie", &path), &path);

    // The window arrives via %window-add, but NO layout is pumped: the
    // tab has no panes and no tmux_pane_id — the pre-layout shape.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while ws.tab_manager.tabs().is_empty() && Instant::now() < deadline {
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(!ws.tab_manager.tabs().is_empty(), "the window must arrive");
    assert!(
        ws.tab_manager
            .tabs()
            .iter()
            .all(|t| t.tmux.tmux_pane_id.is_none()),
        "precondition: no layout has set tmux_pane_id"
    );

    let tab_id = ws.tab_manager.tabs()[0].id;
    let window = ws
        .tmux_state
        .tmux_sync
        .get_window(tab_id)
        .expect("daemon window mapped");
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled(&format!("kill-window -t @{window}"))
        .expect("kill-window");
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !ws.tab_manager.tabs().is_empty() && Instant::now() < deadline {
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        ws.tab_manager.tabs().is_empty(),
        "the layout-less tab must close with the session"
    );
    assert!(ws.tmux_state.tmux_session_name.is_none());

    let _ = std::fs::remove_file(&path);
}

/// The par-mux session rule: attaching to a session whose name matches a
/// profile's `mux_session_patterns` applies that profile to the attached
/// (active) tab, announced with the same Undo toast as the other rules.
#[test]
fn a_matching_mux_session_pattern_switches_the_active_tab() {
    let mut ws = manners_state();
    let tab_id = ws
        .tab_manager
        .new_tab(
            &ws.config.load(),
            std::sync::Arc::clone(&ws.runtime),
            false,
            None,
        )
        .expect("local tab");
    ws.tab_manager.switch_to(tab_id);
    let profile_id = uuid::Uuid::from_u128(0xA1);
    let mut profile = crate::profile::Profile::new("MuxProf");
    profile.id = profile_id;
    profile.mux_session_patterns = vec!["PARMux".to_string()];
    ws.overlay_ui.profile_manager.add(profile);

    ws.apply_mux_session_profile("PARMux");

    let tab = ws.tab_manager.get_tab(tab_id).expect("tab");
    assert_eq!(tab.profile.auto_applied_profile_id, Some(profile_id));
    let toast = ws.last_toast_text().unwrap_or_default();
    assert!(
        toast.contains("par-mux session PARMux"),
        "the switch must announce its reason, got: {toast}"
    );
}

/// A pinned active tab keeps its profile — and so does a tab whose
/// profile was just set explicitly (opening a profile pins the tab).
#[test]
fn a_pinned_active_tab_keeps_its_profile_on_mux_attach() {
    let mut ws = manners_state();
    let tab_id = ws
        .tab_manager
        .new_tab(
            &ws.config.load(),
            std::sync::Arc::clone(&ws.runtime),
            false,
            None,
        )
        .expect("local tab");
    ws.tab_manager.switch_to(tab_id);
    {
        let tab = ws.tab_manager.get_tab_mut(tab_id).expect("tab");
        tab.profile.pinned = true;
    }
    let mut profile = crate::profile::Profile::new("MuxProf");
    profile.mux_session_patterns = vec!["PARMux".to_string()];
    ws.overlay_ui.profile_manager.add(profile);

    ws.apply_mux_session_profile("PARMux");

    let tab = ws.tab_manager.get_tab(tab_id).expect("tab");
    assert_eq!(tab.profile.auto_applied_profile_id, None);
    assert!(ws.last_toast_text().is_none());
}
