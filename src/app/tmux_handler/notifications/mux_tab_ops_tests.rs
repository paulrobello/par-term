//! par-mux tab-as-daemon-window, agent-launcher, client-metric, and
//! sessions-changed tests for an attached par-mux window.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{attach_test, connect, manners_state, socket_path, spawn_daemon};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, drain_send_worker, quiesce, wait_until};
use crate::app::tmux_handler::tmux_state::TmuxTransport;
use par_term_mux::MuxSessionClient;
use par_term_tmux::SyncAction;
use std::io;
use std::time::{Duration, Instant};

// =========================================================================
// Tab operations as daemon window operations (card 01a0d9b55e53)
// =========================================================================

/// The shared prefix of the tab-ops tests: a daemon whose session
/// exists, and a renderer-less `WindowState` attached through the REAL
/// `install_mux_transport`. The probe client answers daemon-side
/// queries without touching the app's transport.
fn attached_state(
    tag: &str,
    session: &str,
) -> (crate::app::window_state::WindowState, MuxSessionClient) {
    let path = socket_path(tag);
    spawn_daemon(&path);
    // Prime the session with a first client so the attach takes the
    // Attached (reattach) path and list-windows reports its windows.
    {
        let mut primer = MuxSessionClient::connect(&path).expect("primer connects");
        primer.create_or_attach(session).expect("primer creates");
        let deadline = Instant::now() + DAEMON_DEADLINE;
        loop {
            if primer
                .poll_actions()
                .iter()
                .any(|a| matches!(a, SyncAction::CreateTab { .. }))
            {
                break;
            }
            assert!(Instant::now() < deadline, "primer never saw the window");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    let probe = MuxSessionClient::connect(&path).expect("probe connects");
    let mut ws = manners_state();
    // Re-driven until it lands: an install is one-shot and its attach
    // round-trips ride the core client's fixed 10 s reply budget, so a
    // loaded machine can fail one attempt though nothing is broken.
    wait_until("install_mux_transport", || {
        let transport = connect(&path);
        ws.install_mux_transport(session, transport).is_ok()
    });
    (ws, probe)
}

/// Pump the app's notification loop until `cond` holds, panicking with
/// the waited-for thing otherwise.
fn pump_until(
    ws: &mut crate::app::window_state::WindowState,
    mut cond: impl FnMut(&crate::app::window_state::WindowState) -> bool,
    what: &str,
) {
    wait_until(what, || {
        ws.check_mux_notifications();
        cond(ws)
    });
}

/// Cmd+T while attached asks the daemon for a new window; the tab
/// arrives via %window-add (the shell-exit path in reverse) and is a
/// mirror tab, not a local shell tab.
#[test]
fn new_tab_creates_a_daemon_window() {
    let (mut ws, mut probe) = attached_state("tabops-new", "tabops-new");
    let before: Vec<_> = probe
        .list_windows()
        .expect("list-windows")
        .into_iter()
        .map(|w| w.id)
        .collect();
    let tabs_before = ws.tab_manager.tab_count();

    quiesce(&mut ws);
    ws.new_tab();

    pump_until(
        &mut ws,
        |ws| ws.tab_manager.tab_count() == tabs_before + 1,
        "the %window-add tab",
    );
    let after = probe.list_windows().expect("list-windows");
    assert_eq!(after.len(), before.len() + 1, "daemon gained a window");
    let new_window = after
        .iter()
        .map(|w| w.id)
        .find(|id| !before.contains(id))
        .expect("a new daemon window id");
    assert!(
        ws.tmux_state.tmux_sync.get_tab(new_window).is_some(),
        "the new tab maps to the new daemon window"
    );
}

// =========================================================================
// Agent launcher mux arm (card 01a0d8da6ec47b)
// =========================================================================

/// The daemon pane ids `%N` from a `list-panes` reply.
fn listed_panes(probe: &mut MuxSessionClient) -> Vec<u64> {
    probe
        .send("list-panes")
        .expect("list-panes")
        .iter()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('%')
                .and_then(|s| s.parse::<u64>().ok())
        })
        .collect()
}

/// A launch splits the resolved daemon pane and TYPES the command line
/// into the new pane's shell (the daemon spawns shells — typing IS the
/// launch), and the new pane is a daemon pane mapped client-side, the
/// eligibility the roster's pickers and hooks need.
#[test]
fn launch_agent_via_mux_types_into_a_fresh_daemon_pane() {
    let (mut ws, mut probe) = attached_state("agent-launch", "agent-launch");
    // 0.61 sizing contract (see closing_a_tab_kills_the_daemon_window):
    // the mapping is pumped by reporting a size that moves the window
    // through the app's own connection.
    let first_pane = listed_panes(&mut probe).remove(0);
    wait_until("no daemon pane ever mapped", || {
        let _ = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("attached")
            .send_settled(&format!("refresh-client -t %{first_pane} -C 60x20"));
        ws.check_mux_notifications();
        !ws.tmux_state.tmux_pane_owners.is_empty()
    });
    let before = listed_panes(&mut probe);

    quiesce(&mut ws);
    let outcome = ws.launch_agent_via_mux("echo PAR_TERM_AGENT_LAUNCHED");
    assert_eq!(outcome, MuxLaunchOutcome::Launched);

    // A new daemon pane exists; its shell ran the typed command.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let new_pane = loop {
        let fresh = listed_panes(&mut probe);
        if let Some(id) = fresh.iter().find(|id| !before.contains(id)) {
            break *id;
        }
        assert!(
            Instant::now() < deadline,
            "launch created no daemon pane; before={before:?} now={fresh:?}"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let view = loop {
        let captured = probe
            .send(&format!("capture-pane -t %{new_pane} -p"))
            .expect("capture-pane")
            .join("|");
        if captured.contains("PAR_TERM_AGENT_LAUNCHED") || Instant::now() >= deadline {
            break captured;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        view.contains("PAR_TERM_AGENT_LAUNCHED"),
        "typed command never ran in %{new_pane}: {view:?}"
    );
    // Client-side mapping (roster rows scope to mapped panes).
    pump_until(
        &mut ws,
        |ws| ws.tmux_state.tmux_pane_owners.contains_key(&new_pane),
        "the launched pane's client mapping",
    );
}

/// Dispatch guards: an unknown agent id and a missing default no-op
/// (toast + false) instead of launching anything.
#[test]
fn launch_dispatch_rejects_unknown_agent_and_missing_default() {
    let mut ws = manners_state();
    assert!(!ws.launch_agent_by_id("bogus", false));
    assert!(!ws.launch_default_agent());
    // And the action-name spellings reach them through the dispatcher.
    assert!(!ws.execute_keybinding_action("launch-agent:bogus"));
    assert!(!ws.execute_keybinding_action("launch-default-agent"));
}

/// The local arm (no transport): dispatching a configured agent opens a
/// new tab and the typed command runs in its shell — the snippet NewTab
/// path carrying the launcher's command line.
// Windows runners' ConPTY startup outlasts the 10s poll intermittently.
#[cfg(unix)]
#[test]
fn launch_agent_local_arm_opens_a_tab_and_runs_the_command() {
    // A multi-thread runtime: unlike the mirror-terminal tests, this
    // one pumps a REAL local shell, and the core's PTY reader needs
    // background task progress a dormant current-thread runtime
    // never makes.
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws =
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
    ws.config.store(std::sync::Arc::new(crate::config::Config {
        agents: vec![par_term_config::agent_launcher::AgentLaunchConfig {
            id: "probe".to_string(),
            name: "Probe".to_string(),
            command: "echo PAR_TERM_LOCAL_LAUNCH".to_string(),
            autonomy_args: String::new(),
            default: false,
        }],
        ..Default::default()
    }));
    assert!(ws.execute_keybinding_action("launch-agent:probe"));
    assert_eq!(ws.tab_manager.tab_count(), 1, "the launch opened a tab");

    // The delayed write lands after the shell initializes; poll the
    // tab's screen for the marker (Cmd+F's searchable-lines seam).
    let tab_id = ws.tab_manager.active_tab().unwrap().id;
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        let seen = ws.tab_manager.get_tab(tab_id).and_then(|tab| {
            tab.try_with_read_terminal(|term| {
                crate::app::window_state::search_highlight::get_all_searchable_lines(
                    term,
                    term.dimensions().1,
                )
                .any(|(_, line)| line.contains("PAR_TERM_LOCAL_LAUNCH"))
            })
        });
        if seen == Some(true) {
            return;
        }
        if Instant::now() >= deadline {
            let diag = ws
                .tab_manager
                .get_tab(tab_id)
                .and_then(|tab| {
                    tab.try_with_read_terminal(|term| {
                        (
                            term.dimensions(),
                            term.is_running(),
                            term.content().map(|c| c.chars().count()).unwrap_or(0),
                        )
                    })
                })
                .unwrap_or(((0, 0), false, 0));
            let lines = ws
                .tab_manager
                .get_tab(tab_id)
                .and_then(|tab| {
                    tab.try_with_read_terminal(|term| {
                        crate::app::window_state::search_highlight::get_all_searchable_lines(
                            term,
                            term.dimensions().1,
                        )
                        .map(|(_, line)| line)
                        .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_default();
            panic!(
                "typed command never ran in the local launch tab (dims={}x{} running={} content_chars={}): {lines:?}",
                diag.0.0, diag.0.1, diag.1, diag.2
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Closing a mux tab kills the daemon window; the tab tears down via
/// %window-close and leaves no stale pane mappings behind.
#[test]
fn closing_a_tab_kills_the_daemon_window() {
    let (mut ws, mut probe) = attached_state("tabops-close", "tabops-close");
    let before = probe.list_windows().expect("list-windows");
    assert_eq!(before.len(), 1, "one daemon window before the close");
    let window_id = before[0].id;
    let tab_id = ws
        .tmux_state
        .tmux_sync
        .get_tab(window_id)
        .expect("tab mapped to the window");
    // 0.61 sizing contract: a size report that does not move the
    // window broadcasts nothing to the other clients (the two-client
    // size-war fix — the geometry goes to the issuer's reply alone),
    // so the mapping is pumped by reporting a smaller size through
    // the app's own connection: the re-fit moves, and the broadcast
    // `%layout-change` maps the panes.
    let first_pane = probe.list_panes().expect("list panes").remove(0);
    wait_until("the window's pane mappings", || {
        let _ = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("attached")
            .send_settled(&format!("refresh-client -t %{first_pane} -C 60x20"));
        ws.check_mux_notifications();
        !ws.tmux_state.tab_tmux_pane_ids(tab_id).is_empty()
    });
    let panes: Vec<_> = ws
        .tmux_state
        .tab_tmux_pane_ids(tab_id)
        .into_iter()
        .collect();
    ws.tab_manager.switch_to(tab_id);

    quiesce(&mut ws);
    let was_last = ws.close_current_tab_immediately();

    assert!(!was_last, "the close returns before the notification lands");
    pump_until(
        &mut ws,
        |ws| ws.tmux_state.tmux_sync.get_tab(window_id).is_none(),
        "the %window-close teardown",
    );
    let after = probe.list_windows().expect("list-windows");
    assert!(after.is_empty(), "daemon window died with the tab");
    assert!(
        panes
            .iter()
            .all(|p| !ws.tmux_state.tmux_pane_owners.contains_key(p)),
        "no stale pane mapping for the killed window"
    );
}

/// Move-tab is blocked for mux tabs at every entry point: the request
/// never queues and the user sees why.
#[test]
fn moving_a_mux_tab_to_another_window_is_blocked() {
    let (mut ws, mut probe) = attached_state("tabops-move", "tabops-move");
    // A second daemon window so has_multiple_tabs would allow a move.
    probe.send("new-window").expect("new-window");
    pump_until(
        &mut ws,
        |ws| ws.tab_manager.tab_count() == 2,
        "the second daemon window's tab",
    );
    let tab_id = ws
        .tmux_state
        .tmux_sync
        .get_tab(0)
        .expect("tab for window @0");
    ws.tab_manager.switch_to(tab_id);

    use crate::tab_bar_ui::TabBarAction;
    ws.handle_tab_bar_action_after_render(TabBarAction::MoveTabToNewWindow(tab_id));
    assert!(
        ws.overlay_ui.pending_move_tab_request.is_none(),
        "no move request queued for a mux tab"
    );
    assert!(
        ws.last_toast_text().is_some(),
        "the block is explained to the user"
    );
}

/// Attach carries the daemon's window names onto the display tabs, and
/// renaming a mux tab forwards rename-window to the daemon.
#[test]
fn attach_carries_window_names_and_rename_reaches_the_daemon() {
    let path = socket_path("tabops-names");
    spawn_daemon(&path);
    {
        let mut primer = MuxSessionClient::connect(&path).expect("primer connects");
        primer.create_or_attach("tabops-names").expect("primer");
        let deadline = Instant::now() + DAEMON_DEADLINE;
        loop {
            if primer
                .poll_actions()
                .iter()
                .any(|a| matches!(a, SyncAction::CreateTab { .. }))
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(25));
        }
        primer
            .send("rename-window -t @0 build")
            .expect("name the window before attach");
    }

    let transport = connect(&path);
    let mut probe = MuxSessionClient::connect(&path).expect("probe connects");
    let mut ws = manners_state();
    ws.install_mux_transport("tabops-names", transport)
        .expect("install_mux_transport");

    let tab_id = ws.tmux_state.tmux_sync.get_tab(0).expect("tab for @0");
    let title = ws
        .tab_manager
        .get_tab(tab_id)
        .expect("tab exists")
        .title
        .clone();
    assert_eq!(title, "build", "attach carries the daemon window name");

    use crate::tab_bar_ui::TabBarAction;
    quiesce(&mut ws);
    ws.handle_tab_bar_action_after_render(TabBarAction::RenameTab(tab_id, "it's docs".to_string()));
    let named = probe.list_windows().expect("list-windows");
    assert_eq!(
        named[0].name, "it's docs",
        "rename-window reached the daemon, quoting intact"
    );
    let title = ws
        .tab_manager
        .get_tab(tab_id)
        .expect("tab exists")
        .title
        .clone();
    assert_eq!(title, "it's docs", "local title follows the rename");
}

// =========================================================================
// Client metric reports: -p cell pixels, set-client-colors (card
// 01a0db77d7a2)
// =========================================================================

/// A transport that records every command and answers nothing — the
/// wire-shape oracle for the fire-and-forget pushes.
#[derive(Default)]
struct RecordingTransport {
    commands: std::cell::RefCell<Vec<String>>,
}

impl TmuxTransport for RecordingTransport {
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
        (Vec::new(), false)
    }

    fn send_command(&self, command: &str) -> io::Result<Vec<String>> {
        self.commands.borrow_mut().push(command.to_string());
        Ok(Vec::new())
    }

    fn send_command_no_wait(&self, command: &str) -> io::Result<()> {
        self.commands.borrow_mut().push(command.to_string());
        Ok(())
    }
}

/// The resize/font pushes carry the cell pixels beside the grid
/// (`-C WxH -p WxH` in one command), the theme push carries both
/// halves of the color pair, and the theme hex the app derives is the
/// configured theme's fg/bg as lowercase `rrggbb`.
#[test]
fn client_metric_pushes_carry_cells_and_colors() {
    let transport = RecordingTransport::default();
    push_client_size(&transport, Some(7), 80, 24, (12, 24));
    // No target pane: the push is dropped, not sent untargeted.
    push_client_size(&transport, None, 80, 24, (12, 24));
    push_client_colors(&transport, "c0c0c0", "1e1e2e");
    let commands = transport.commands.borrow();
    assert_eq!(
        &*commands,
        &[
            "refresh-client -t %7 -C 80x24 -p 12x24".to_string(),
            "set-client-colors -f c0c0c0 -b 1e1e2e".to_string(),
        ],
        "one -C/-p command per size push, one set-client-colors per \
             theme push, and nothing for a targetless push"
    );

    let config = crate::config::Config::default();
    let (fg, bg) = mux_client_colors_hex(&config);
    let theme = config.load_theme();
    let hex = |c: crate::config::Color| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    assert_eq!(fg, hex(theme.foreground), "fg is the theme foreground");
    assert_eq!(bg, hex(theme.background), "bg is the theme background");
}

/// The attach reports reach the daemon and land in the pane terminal:
/// after an attach carrying `-p 12x24` and bg `121212`, a CSI 16t
/// query in the pane answers `6;12;24t` (not the 10x20 construction
/// default) and an OSC 11 query answers the client bg. The pane's own
/// program proves the reports live daemon-side, not just on the wire.
#[cfg(unix)]
#[test]
fn attach_reports_cells_and_theme_to_the_daemon() {
    let path = socket_path("attach-metrics");
    spawn_daemon(&path);

    let transport = connect(&path);
    attach_sequence(
        &transport,
        "metrics",
        Some((90, 30)),
        Some((12, 24)),
        &("c0c0c0".to_string(), "121212".to_string()),
        &Default::default(),
    )
    .expect("attach with metrics");
    transport
        .client()
        // The marker is quote-split so the ECHOED command text never
        // matches it — only the executed echo's output does.
        .send_keys_literal(
            0,
            "printf \"\\033[16t\\033]11;?\\007\"; echo MET\"RIC\"DONE",
        )
        .expect("send metric query");
    transport.client().send_keys(0, b"\r").expect("enter");

    // The replies are written to the pane's pty INPUT and only become
    // visible when the tty echoes them, which can land after the marker
    // line. Poll for the replies themselves, not just the marker.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let screen = loop {
        let screen = transport.client().refresh_pane(0).expect("replay");
        let screen = screen.join("\n");
        let replied = screen.contains("24;12t") && screen.contains("rgb:1212/1212/1212");
        if (screen.contains("METRICDONE") && replied) || Instant::now() >= deadline {
            break screen;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        screen.contains("METRICDONE"),
        "the metric program never printed: {screen:?}"
    );
    assert!(
        // xterm's XTWINOPS replies are height-first: 6;height;width. The
        // `ESC[6;` prefix is omitted: readline consumes a varying amount
        // of it as a key-sequence prefix before the tty echoes the rest.
        screen.contains("24;12t"),
        "CSI 16t answers with the reported 12x24 cells, not the default: {screen:?}"
    );
    assert!(
        screen.contains("rgb:1212/1212/1212"),
        "OSC 11 answers with the client bg, not the core theme: {screen:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// `%sessions-changed` ends the view when the attached session is what
/// died: killing the session's last window from a second client makes
/// the next drain re-query `list-sessions`, find the session gone, and
/// run the full end-of-view teardown (transport dropped, identity
/// cleared, toast naming the daemon-side end) — while the daemon
/// itself keeps running.
#[test]
fn sessions_changed_ends_views_whose_session_is_gone() {
    sessions_changed_ends_view(None, "gone-session");
}

/// The per-fact half of the capability gate (ENH-043): a daemon whose
/// `list-commands` does not advertise `client-snapshot v1` gets the
/// plain `list-sessions` re-query and never a `client-snapshot` line.
#[test]
fn sessions_changed_per_fact_fallback_still_ends_the_view() {
    // A distinct socket tag: `socket_path` UNLINKS on every call, so a
    // tag shared with the sibling variant would delete the other's
    // live socket when the suite runs the two in parallel.
    sessions_changed_ends_view(Some(false), "gone-session-perfact");
}

/// The shared body: kill the attached session from a second client and
/// pump until the view ends. `force_snapshot_supported` pre-seeds the
/// capability cache (`None` = let the drain probe the real daemon,
/// which advertises `client-snapshot v1` and takes the snapshot path).
fn sessions_changed_ends_view(force_snapshot_supported: Option<bool>, tag: &str) {
    // Capture the path once: `socket_path` UNLINKS on every call (it
    // is a spawn helper, not a getter), so re-calling it after
    // `attached_state` bound the daemon would delete the live socket.
    let path = socket_path(tag);
    let (mut ws, mut probe) = attached_state(tag, "doomed");
    ws.tmux_state.mux_snapshot_supported = force_snapshot_supported;
    assert!(
        ws.tmux_state.transport.is_some() && ws.tmux_state.mux_session_id.is_some(),
        "precondition: the view is attached"
    );
    // The doomed session's only window, captured before the keeper
    // exists — list-windows is global, so this ordering is what names
    // it unambiguously.
    let window = probe
        .list_windows()
        .expect("probe lists windows")
        .first()
        .expect("the doomed session holds a window")
        .id;
    // A second session keeps the daemon non-empty: an emptied daemon
    // exits after its grace (EXIT_EMPTY_GRACE), and that exit is the
    // DISCONNECT path's scenario, not this one — here the daemon must
    // outlive the killed session.
    probe
        .create_or_attach("keeper")
        .expect("keeper session exists");
    quiesce(&mut ws);
    probe
        .send(&format!("kill-window -t @{window}"))
        .expect("kill-window removes the session's last window");

    // The pushes reach this client's channel on its own reader thread,
    // and the drain re-queries `list-sessions` inline (failing open if
    // the send worker holds the client), so pump with the worker idle
    // until the view ends. `redrawn` is the answer of the drain that
    // ended it.
    let mut redrawn = false;
    wait_until("the view ends", || {
        if let Some(t) = ws.tmux_state.transport.as_deref()
            && !ws.tmux_state.mux_jobs_in_flight()
        {
            drain_send_worker(t);
        }
        redrawn = ws.check_mux_notifications();
        ws.tmux_state.transport.is_none()
    });
    assert!(redrawn, "the ended view must request a redraw");
    assert!(
        ws.tmux_state.transport.is_none(),
        "the view ended — the transport is dropped"
    );
    assert!(ws.tmux_state.mux_session_id.is_none());
    assert!(ws.tmux_state.tmux_session_name.is_none());
    let toast = ws.last_toast_text().unwrap_or_default();
    assert!(
        toast.contains("session ended on the daemon"),
        "the toast names the daemon-side end, got: {toast}"
    );
    // The daemon survives the view: a fresh client still connects, the
    // keeper session remains, the doomed one is gone.
    let mut late = MuxSessionClient::connect(&path).expect("daemon lives");
    let names: Vec<String> = late
        .list_sessions()
        .expect("list after teardown")
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(
        names.contains(&"keeper".to_string()) && !names.contains(&"doomed".to_string()),
        "the daemon kept running with exactly the doomed session removed: {names:?}"
    );
}

/// A `%layout-change` born of ANOTHER client's resize
/// (`refresh-client -C`, the exact form ParDeck's streamer sends) must
/// re-fit the local mirror grid: the daemon-side leaf geometry is the
/// authority, or the desktop keeps rendering the pane at its own
/// window's size with wrapping drift (pardeck P2 proof, card
/// 01a0e8c150ae7c02ab588e2365d80eaf).
#[cfg(unix)]
#[test]
fn layout_change_from_another_clients_resize_refits_the_mirror() {
    let path = socket_path("layout-adopt");
    spawn_daemon(&path);

    let transport = connect(&path);
    let attach = attach_test(
        &transport,
        "layout-adopt",
        Some((80, 24)),
        &Default::default(),
    )
    .expect("attach_sequence");
    let mut ws = manners_state();
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("layout-adopt".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer creates the native mirror for %0.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the pane"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // The deliberate resize: another viewer shrinks the pane under us.
    ws.tmux_state
        .transport
        .as_ref()
        .unwrap()
        .send_settled("refresh-client -t %0 -C 61x17")
        .expect("deliberate resize");

    let mirror_dims = |ws: &crate::app::window_state::WindowState| {
        let (tab_id, pane_id) = *ws.tmux_state.tmux_pane_owners.get(&0).expect("%0 owned");
        let tab = ws.tab_manager.get_tab(tab_id).expect("tab");
        let pm = tab.pane_manager().expect("panes");
        let pane = pm.get_pane(pane_id).expect("pane");
        pane.terminal
            .try_read()
            .expect("terminal lock")
            .dimensions()
    };

    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if mirror_dims(&ws) == (61, 17) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the mirror never adopted the deliberate resize (last dims {:?})",
            mirror_dims(&ws)
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    let _ = std::fs::remove_file(&path);
}
