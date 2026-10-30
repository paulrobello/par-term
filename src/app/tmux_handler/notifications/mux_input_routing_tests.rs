//! par-mux input routing tests for attached par-mux panes: focus reports, paste
//! (bracketed, newline conversion, paste delay), IME commits, and literal
//! bytes reach the daemon pane, and a stale mux focus never captures a
//! local tab's input.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    attach_test, capture_until, connect, manners_state, socket_path, spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, poll_until, wait_until};
use std::time::{Duration, Instant};

/// Focus reporting (DECSET 1004) follows the same rule as every other
/// mux input: the report bytes belong in the DAEMON pane. The focus
/// handler wrote ESC[I / ESC[O into the mirror terminal, which has no
/// PTY — an app in a mux pane that enabled focus tracking never heard
/// the window focus change (card 01a0d9b55c2273a2be9d87a0718690c3).
#[cfg(unix)]
#[test]
fn focus_reports_reach_the_daemon_pane() {
    let path = socket_path("ws-focus");
    spawn_daemon(&path);

    // Created-session attach (the triggers-test pattern).
    let transport = connect(&path);
    let attach = attach_test(&transport, "wsfocus", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
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
    ws.tmux_state.tmux_session_name = Some("wsfocus".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer created the native mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let tab_id = loop {
        ws.check_mux_notifications();
        if let Some(&(tab_id, native)) = ws.tmux_state.tmux_pane_owners.get(&0) {
            ws.tab_manager.switch_to(tab_id);
            break (tab_id, native);
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    };

    // The app in the pane enables focus tracking (DECSET 1004) —
    // routed through the daemon's %output, so the MIRROR terminal
    // carries the mode.
    ws.handle_tmux_output(0, b"\x1b[?1004h");
    {
        let tab = ws.tab_manager.get_tab(tab_id.0).expect("mux tab");
        let pane = tab
            .pane_manager()
            .and_then(|pm| pm.get_pane(tab_id.1))
            .expect("mirror pane");
        let term = pane.terminal.try_read().expect("pane terminal free");
        assert!(
            term.focus_tracking_enabled(),
            "the mirror must carry the app's focus-tracking mode"
        );
    }

    // Make the daemon pane echo its input: `cat -v` renders ESC[I as
    // `^[[I`, so capture-pane is the oracle for delivery. Typed as
    // hex bytes — bare words are key names on the daemon's send-keys.
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled("send-keys -t %0 -H 63 61 74 20 2d 76 0d")
        .expect("start cat -v");
    // Reports sent before `cat` runs land in the shell's line editor,
    // which consumes them; wait until the daemon reports `cat` as the
    // pane's foreground command.
    wait_until("cat -v runs in %0", || {
        let info = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("pane-info -t %0")
            .expect("pane-info");
        par_term_mux::pane_foreground_command(&info, 0).as_deref() == Some("cat")
    });

    // Window blur then focus through the production entry. The first
    // call flips state (a fresh window reports focused), so both
    // ESC[O and ESC[I should be delivered.
    ws.handle_focus_change(false);
    ws.handle_focus_change(true);

    // The report bytes must have reached the daemon pane.
    let mut daemon_view = String::new();
    let mut saw_focus_out = false;
    let mut saw_focus_in = false;
    poll_until(|| {
        daemon_view = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        saw_focus_out = daemon_view.contains("^[[O");
        saw_focus_in = daemon_view.contains("^[[I");
        saw_focus_out && saw_focus_in
    });
    assert!(
        saw_focus_in && saw_focus_out,
        "the daemon pane must see ESC[O and ESC[I from the focus \
             changes: {daemon_view:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Paste is the one input stream that never got a mux branch: keyboard
/// input goes through `send_input_via_tmux`'s transport check, mouse
/// reports through `route_mouse_report_to_mux`, but the shared paste
/// entry (`paste_via_tmux`, used by Cmd+V, option-click paths, and
/// middle-click) only knew the real-tmux gateway — for a mux pane it
/// returned false and the local fallback wrote into a PTY-less mirror,
/// dropping the paste (card 01a0d099b83e7dc0a5725390a8c5e20c).
#[test]
fn paste_via_tmux_routes_to_the_daemon_pane() {
    let path = socket_path("ws-paste");
    spawn_daemon(&path);

    // Attach + install into a renderer-less WindowState — the same
    // steps `install_mux_transport` performs (the reattach pattern).
    let transport = connect(&path);
    let attach = attach_test(&transport, "wspaste", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
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
    ws.tmux_state.tmux_session_name = Some("wspaste".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer creates the native pane — the
    // paste router resolves its target from the focused native pane.
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

    // The shared paste entry must CONSUME for a focused mux pane (the
    // shell echoes the bytes, so capture-pane sees the marker).
    assert!(
        ws.paste_via_tmux("PASTE-ROUTE-OK"),
        "a focused mux pane must take the paste through the shared entry"
    );
    let mut daemon_view = String::new();
    while Instant::now() < deadline && !daemon_view.contains("PASTE-ROUTE-OK") {
        daemon_view = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        daemon_view.contains("PASTE-ROUTE-OK"),
        "the paste must reach the daemon pane: {daemon_view:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// The IME commit path must route through the daemon transport — the
/// `send_input_via_tmux` seam — when a mux pane is focused, not into the
/// PTY-less local mirror. The marker carries a multi-byte é so the test
/// also proves UTF-8 committed text survives the wire (card
/// 01a0d9e6e6167353b58822718f83b15d, criterion 1's mux clause).
#[test]
fn ime_commit_reaches_the_mux_pane_via_the_transport() {
    let path = socket_path("ws-ime");
    spawn_daemon(&path);

    // Attach + install into a renderer-less WindowState — the same
    // steps `install_mux_transport` performs (the reattach pattern).
    let transport = connect(&path);
    let attach = attach_test(&transport, "wsime", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
    let mut ws = manners_state();
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wsime".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer creates the native pane — the
    // input router resolves its target from the focused native pane.
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

    // The committed text must land in the daemon pane (the shell echoes
    // the bytes, so capture-pane sees the marker).
    ws.handle_ime_event(winit::event::Ime::Commit("é-IME-ROUTE-OK".into()));
    let mut daemon_view = String::new();
    while Instant::now() < deadline && !daemon_view.contains("é-IME-ROUTE-OK") {
        daemon_view = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        daemon_view.contains("é-IME-ROUTE-OK"),
        "the IME commit must reach the daemon pane: {daemon_view:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Attach a WindowState to a fresh daemon, pump until pane %0 has its
/// native mirror, then arm bracketed paste and run `cat -v` in the
/// daemon pane: `cat -v` renders the pane's raw input bytes in caret
/// notation (ESC → `^[`, CR → `^M`), so the exact wire form of a paste
/// is assertable from `capture-pane`.
fn paste_byte_mirror_state(
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

    // Pump until the layout consumer creates the native pane — the
    // paste router resolves its target from the focused native pane.
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

    // Run the byte mirror in the daemon pane. `stty raw -echo` runs
    // FIRST so a paste can never land on a still-canonical tty (where
    // echo and ICRNL would mangle it); `printf` emits DECSET 2004 so
    // the client mirror arms bracketed paste deterministically, with
    // no reliance on the shell's own init; `cat -v` then renders
    // exactly the bytes the paste sent, in caret notation
    // (ESC → `^[`, CR → `^M`). The readiness marker is emitted as
    // `MIRR""OR_READY` so the string the wait below matches can only
    // exist as the echo's OUTPUT, never as the zle echo of the typed
    // line itself.
    let line = b"stty raw -echo; printf '\\033[?2004h'; echo MIRR\"\"OR_READY; cat -v";
    let hex = line
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    ws.tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled(&format!("send-keys -t %0 -H {hex}"))
        .expect("type byte-mirror command");
    ws.tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("send-keys -t %0 Enter")
        .expect("run byte-mirror command");

    // Pump until BOTH the mirror reports bracketed paste armed AND
    // the daemon-side capture shows MIRROR_READY. Neither alone
    // suffices: zsh's line editor emits its own ?2004h at prompt
    // draw, so the mirror can arm before this line has executed at
    // all (the fixed 150ms settle that used to cover the gap was the
    // parallel-load flake), and MIRROR_READY without the printf's
    // DECSET would leave the paste unwrapped. Pumping notifications
    // before the capture means an armed mirror seen in the same
    // iteration as the marker reflects the printf's own emission,
    // which precedes the marker in the pane's output stream — so both
    // conditions together prove the tty is raw and the paste will be
    // wrapped. A paste that then beats the exec of `cat -v` queues in
    // the raw input buffer and is rendered when `cat -v` reads.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        let armed = focused_mirror_bracketed(&ws).is_some_and(|(start, _)| !start.is_empty());
        let view = ws
            .tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        if armed && view.contains("MIRROR_READY") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the byte mirror never became ready: armed={armed} view={view:?}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    (ws, path)
}

/// Pane %0's mirror-terminal bracketed-paste sequences, if the mirror
/// is resolvable and unlocked.
fn focused_mirror_bracketed(
    ws: &crate::app::window_state::WindowState,
) -> Option<(Vec<u8>, Vec<u8>)> {
    let native = ws.tmux_state.tmux_pane_owners.get(&0)?.1;
    for tab in ws.tab_manager.tabs() {
        if let Some(pane) = tab.pane_manager().and_then(|pm| pm.get_pane(native))
            && let Ok(term) = pane.terminal.try_read()
        {
            return Some(term.bracketed_paste_sequences());
        }
    }
    None
}

/// Card 01a0d9b551c57243b9888e1ac3be6b55, criterion 1: a paste into a
/// mux pane whose mirror has bracketed paste enabled must reach the
/// daemon pane wrapped in the bracketed-paste sequences with `\n`
/// converted to `\r` — asserted on the raw bytes the daemon pane
/// receives (`cat -v` caret notation), not the rendered screen.
#[cfg(unix)]
#[test]
fn mux_paste_wraps_bracketed_paste_and_converts_newlines() {
    let (ws, path) = paste_byte_mirror_state("ws-paste-bp");

    assert!(
        ws.paste_via_tmux("L1\nL2"),
        "a focused mux pane must take the paste"
    );
    let transport = ws.tmux_state.transport.as_ref().unwrap();
    let view = capture_until(&**transport, 0, "^[[201~");
    assert!(
        view.contains("^[[200~L1^ML2^[[201~"),
        "the daemon pane must receive the bracketed, CR-converted paste: {view:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Card 01a0d9b551c57243b9888e1ac3be6b55, criterion 2: a multi-line
/// mux paste under a configured `paste_delay_ms` is paced — the second
/// line is NOT in the daemon pane when the first arrives, and lands
/// once the delay elapses (the test drives the same poll tick the app
/// loop would).
#[cfg(unix)]
#[test]
fn mux_paste_honors_paste_delay_ms() {
    let (mut ws, path) = paste_byte_mirror_state("ws-paste-delay");

    let mut cfg = crate::config::Config::default();
    cfg.selection.paste_delay_ms = 400;
    ws.config.store(std::sync::Arc::new(cfg));

    let delay = Duration::from_millis(400);
    let pasted_at = Instant::now();
    assert!(
        ws.paste_via_tmux("D1\nD2"),
        "a focused mux pane must take the paste"
    );

    // First line goes out immediately; the second must wait out the
    // 400ms delay. Only the poll tick (`check_mux_notifications`) can
    // send it, and none runs until D1 is seen, so a stalled test thread
    // cannot let D2 land early. The delay itself is pinned two ways,
    // both lower bounds a loaded machine cannot violate: the queued
    // chunk is due no sooner than the delay after the paste, and D2
    // first appears no sooner than that either.
    let mut early = String::new();
    poll_until(|| {
        early = ws
            .tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        early.contains("D1^M")
    });
    assert!(
        early.contains("D1^M"),
        "the first line must arrive: {early:?}"
    );
    assert!(
        !early.contains("D2"),
        "the second line must wait out paste_delay_ms: {early:?}"
    );
    assert!(
        ws.tmux_state
            .pending_mux_paste
            .borrow()
            .as_ref()
            .is_some_and(|p| p.next_due >= pasted_at + delay),
        "the second line is queued behind the full delay"
    );

    let mut late = early.clone();
    poll_until(|| {
        ws.check_mux_notifications();
        late = ws
            .tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("capture-pane -t %0 -p")
            .expect("capture")
            .join("|");
        late.contains("D2")
    });
    let landed_after = pasted_at.elapsed();
    assert!(
        late.contains("D2"),
        "the second line must land after the delay: {late:?}"
    );
    assert!(
        landed_after >= delay,
        "the second line landed {landed_after:?} after the paste, inside the delay"
    );
    assert!(
        late.contains("^[[201~"),
        "the bracketed end must follow the last line: {late:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// A stale tracked focus must not capture a local tab's input. Before
/// the fix, `mux_focused_pane` won over the focused native pane and
/// outlived its tab: after the mux tab closed, every keystroke in the
/// remaining local tab was sent to a daemon pane that no longer existed
/// (reported live: "closed the par-mux panes, but the original native
/// tab is unresponsive").
#[test]
fn a_stale_mux_focus_does_not_capture_local_tab_input() {
    let path = socket_path("stale-focus");
    spawn_daemon(&path);
    let transport = connect(&path);
    attach_test(&transport, "stale", None, &Default::default()).expect("attach");

    let mut ws = manners_state();
    ws.tmux_state.transport = Some(Box::new(transport));
    // The mux tab is gone; only the tracked id survives.
    ws.tmux_state.mux_focused_pane = Some(0);
    assert!(
        ws.tmux_state.tmux_pane_owners.is_empty(),
        "no mux pane is mapped — the user is not on a mux pane"
    );

    assert!(
        !ws.send_input_via_tmux(b"x"),
        "input must fall through to the local PTY, not the daemon"
    );
    assert!(
        !ws.send_literal_bytes_via_tmux(b"\n"),
        "the literal-bytes path must fall through too"
    );

    let _ = std::fs::remove_file(&path);
}

/// The converse of the stale-focus test: ON a mapped mux pane the
/// literal-bytes path must route (Shift+Enter's raw LF reaches the
/// daemon as send-keys -H). Before the fix, the tmux-connected gate
/// refused first — a mux window has no tmux_session — so the branch
/// below it was dead code and the LF fell onto the hidden local shell.
#[test]
fn literal_bytes_route_to_the_daemon_on_a_mapped_mux_pane() {
    let path = socket_path("literal-route");
    spawn_daemon(&path);
    let transport = connect(&path);
    attach_test(&transport, "litroute", None, &Default::default()).expect("attach");

    let mut ws = manners_state();
    ws.tmux_state.transport = Some(Box::new(transport));
    // A local tab whose focused pane mirrors daemon pane %0 — the
    // minimum mapping focused_mux_pane_from_native needs.
    let grid = None;
    let tab_id = ws
        .tab_manager
        .new_tab(
            &ws.config.load(),
            std::sync::Arc::clone(&ws.runtime),
            false,
            grid,
        )
        .expect("local tab");
    ws.tab_manager.switch_to(tab_id);
    let pane_id = ws
        .tab_manager
        .active_tab()
        .and_then(|tab| tab.focused_pane_id())
        .expect("focused pane");
    ws.tmux_state
        .set_tab_pane_mappings(tab_id, &[(0, pane_id)].into_iter().collect());

    assert!(
        ws.send_literal_bytes_via_tmux(b"\n"),
        "Shift+Enter's literal LF must route through the daemon"
    );

    let _ = std::fs::remove_file(&path);
}
