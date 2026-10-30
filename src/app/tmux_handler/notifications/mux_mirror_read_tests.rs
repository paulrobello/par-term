//! par-mux tests proving reads of an attached par-mux pane (search, copy mode,
//! the inspector snapshot, exit polling, TUI replay) go through the
//! daemon-fed mirror, never the tab's hidden login shell.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    attach_test, connect, manners_state, pane_text, socket_path, spawn_daemon,
};
use super::mux::*;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, quiesce};
use crate::app::tmux_handler::tmux_state::TmuxTransport;
use crate::app::window_state::WindowState;
use par_term_mux::MuxSessionClient;
use par_term_tmux::SyncAction;
use std::time::{Duration, Instant};

/// Search and copy mode must read the focused pane's terminal — the
/// daemon mirror — in a mux tab. `tab.terminal` there is a hidden
/// local login shell, so Cmd+F, copy-mode entry/search, and line
/// motions looked at a screen the user cannot see. Proven red against
/// the tab-terminal reads before the fix.
#[test]
fn search_and_copy_mode_read_the_mux_mirror_not_the_hidden_shell() {
    let path = socket_path("mux-search-mirror");
    spawn_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "findme".to_string(),
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

    // Seed ONLY the mirror: daemon output routes to the mapped mirror,
    // never to the tab's hidden shell.
    ws.handle_tmux_output(0, b"mux-mirror-needle 4999\r\n");
    let deadline = Instant::now() + DAEMON_DEADLINE;
    while !pane_text(&ws, tab_id, pane).contains("mux-mirror-needle") {
        assert!(
            Instant::now() < deadline,
            "the mirror never showed the marker"
        );
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }

    // The mirror's own cursor state, for the entry assertions below.
    let (mirror_col, mirror_row, mirror_sb) = ws
        .tab_manager
        .get_tab(tab_id)
        .and_then(|tab| tab.pane_manager())
        .and_then(|pm| pm.get_pane(pane))
        .and_then(|p| p.terminal.try_read().ok())
        .map(|term| {
            let (col, row) = term.cursor_position();
            (col, row, term.scrollback_len())
        })
        .expect("mirror terminal");

    // Cmd+F's searchable lines come from the focused pane's terminal.
    let searchable = ws
        .tab_manager
        .get_tab(tab_id)
        .and_then(|tab| {
            tab.try_with_read_terminal(|term| {
                crate::app::window_state::search_highlight::get_all_searchable_lines(
                    term,
                    mirror_row + 1,
                )
                .map(|(_, line)| line)
                .collect::<Vec<_>>()
            })
        })
        .expect("a readable terminal");
    assert!(
        searchable.iter().any(|l| l.contains("mux-mirror-needle")),
        "Cmd+F must search the mirror's screen, got {searchable:?}"
    );

    // Copy-mode entry anchors on the mirror's cursor, not the hidden
    // shell's.
    ws.enter_copy_mode();
    assert!(
        ws.copy_mode.active,
        "copy mode must enter (copy_mode_enabled defaults on)"
    );
    assert_eq!(
        (ws.copy_mode.cursor_col, ws.copy_mode.cursor_absolute_line),
        (mirror_col, mirror_sb + mirror_row),
        "copy-mode entry anchors on the mirror's cursor"
    );

    // Copy-mode search finds the marker through the mirror.
    ws.copy_mode.search_query = "mux-mirror-needle".to_string();
    ws.copy_mode.search_direction = crate::copy_mode::SearchDirection::Forward;
    ws.execute_copy_mode_search(false);
    let line = ws.get_copy_mode_line_text().expect("line under the cursor");
    assert!(
        line.contains("mux-mirror-needle"),
        "copy-mode search must land on the mirror's marker line, got {line:?}"
    );

    // And the yank of that line reads the same mirror, so every read
    // path agrees on one terminal. The daemon pane's shell prompt shares
    // the marker's row (the raw write landed at the pane cursor), so the
    // selection spans the marker's columns, not the whole row.
    let (needle_line, needle_col) = (ws.copy_mode.cursor_absolute_line, ws.copy_mode.cursor_col);
    ws.copy_mode.visual_mode = crate::copy_mode::VisualMode::Char;
    ws.copy_mode.selection_anchor = Some((needle_line, needle_col));
    ws.copy_mode.cursor_col = needle_col + "mux-mirror-needle".len();
    ws.sync_copy_mode_selection();
    let yanked = ws.get_selected_text_for_copy().expect("selection text");
    assert!(
        yanked.contains("mux-mirror-needle"),
        "yank must read the mirror, got {yanked:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// The inspector's snapshot and command history must read the focused
/// pane's terminal — the daemon mirror — in a mux tab. `tab.terminal`
/// there is a hidden local login shell, so the agent reasoned about a
/// screen and history it cannot see. Seeds a full OSC 133 command
/// lifecycle into the mirror the way daemon `%output` delivers it, then
/// asserts the snapshot carries that command.
#[test]
fn inspector_snapshot_reads_the_mux_mirror_not_the_hidden_shell() {
    let path = socket_path("mux-inspector-read");
    spawn_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "inspect".to_string(),
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

    // Seed ONLY the mirror with a complete shell-integration command
    // lifecycle, exactly as the daemon pane's shell would emit it.
    ws.handle_tmux_output(
        0,
        b"\x1b]133;C;echo mirror-cmd\x07mirror-output\r\n\x1b]133;D;7\x07",
    );
    // Bridge the queued OSC events into command history — the same
    // update the render pipeline's pane gather performs.
    {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        let p = tab
            .pane_manager()
            .and_then(|pm| pm.get_pane(pane))
            .expect("mirror pane");
        let mut term = p.terminal.try_write().expect("mirror write lock");
        let sb_len = term.scrollback_len();
        let (_, cursor_row) = term.cursor_position();
        term.update_scrollback_metadata(sb_len, cursor_row);
    }
    let mirror_history = {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        tab.try_with_read_terminal(|term| term.core_command_history())
    }
    .expect("readable mirror terminal");
    assert!(
        mirror_history
            .iter()
            .any(|(cmd, ..)| cmd == "echo mirror-cmd"),
        "seed must land in the mirror's command history, got {mirror_history:?}"
    );

    // The inspector snapshot must carry the mirror's command, not the
    // hidden shell's (empty) history.
    ws.overlay_ui.ai_inspector.open = true;
    ws.overlay_ui.ai_inspector.needs_refresh = true;
    ws.refresh_inspector_snapshot();
    let snapshot = ws
        .overlay_ui
        .ai_inspector
        .snapshot
        .as_ref()
        .expect("snapshot must gather");
    assert!(
        !ws.overlay_ui.ai_inspector.needs_refresh,
        "a successful gather clears the refresh flag"
    );
    let commands: Vec<_> = snapshot
        .commands
        .iter()
        .map(|c| c.command.as_str())
        .collect();
    assert!(
        commands.contains(&"echo mirror-cmd"),
        "the snapshot must read the mirror's command history, got {commands:?}"
    );
    let exit = snapshot
        .commands
        .iter()
        .find(|c| c.command == "echo mirror-cmd")
        .and_then(|c| c.exit_code);
    assert_eq!(
        exit,
        Some(7),
        "the mirror's exit code must survive the gather"
    );

    let _ = std::fs::remove_file(&path);
}

/// Run-and-notify's exit-status poll must watch the focused mirror in
/// a mux tab: the command runs in the daemon pane, so only the
/// mirror's shell-integration history ever records its exit code. The
/// differential half pins the defect — polling the hidden local shell
/// (`tab.terminal`, the pre-fix wiring) never sees the command and the
/// agent is told "unknown exit code".
#[test]
fn run_and_notify_exit_poll_watches_the_mux_mirror() {
    let path = socket_path("mux-inspector-poll");
    spawn_daemon(&path);

    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: "polltest".to_string(),
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

    // Baseline recorded before the command lands, as run-and-notify
    // does, plus the two handles the poll could be wired to.
    let (baseline, mirror_handle, hidden_shell) = {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        (
            tab.try_with_read_terminal(|term| term.core_command_history().len())
                .expect("readable mirror terminal"),
            tab.read_terminal_handle(),
            std::sync::Arc::clone(&tab.terminal),
        )
    };

    // The command completes in the daemon pane: its OSC 133 lifecycle
    // reaches the mirror through %output, events bridged as the pane
    // gather does.
    ws.handle_tmux_output(0, b"\x1b]133;C;pwd\x07/home/user\r\n\x1b]133;D;0\x07");
    {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        let p = tab
            .pane_manager()
            .and_then(|pm| pm.get_pane(pane))
            .expect("mirror pane");
        let mut term = p.terminal.try_write().expect("mirror write lock");
        let sb_len = term.scrollback_len();
        let (_, cursor_row) = term.cursor_position();
        term.update_scrollback_metadata(sb_len, cursor_row);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("poll runtime");
    let exit = runtime.block_on(
        crate::app::window_state::action_handlers::inspector::poll_command_exit(
            Some(mirror_handle),
            baseline,
            300,
        ),
    );
    assert_eq!(exit, Some(0), "the poll must read the mirror's exit code");

    // Differential: the hidden local shell never saw the command — the
    // pre-fix wiring times out with no exit code.
    let none = runtime.block_on(
        crate::app::window_state::action_handlers::inspector::poll_command_exit(
            Some(hidden_shell),
            baseline,
            2,
        ),
    );
    assert_eq!(none, None, "polling the hidden shell must find nothing");

    let _ = std::fs::remove_file(&path);
}

/// A full-screen TUI's reattach seed and live redraws must both render
/// the real screen in a client emulator fed exactly as the app feeds
/// its pane terminals: the seed through `process_data` at the pane's
/// size, live `%output` payloads through `process` on the same
/// terminal.
///
/// Root-cause probe for the blank/partial TUI render card: the emitter
/// alt-screens, paints a bordered grid, then pushes cursor-addressed
/// incremental updates. If this fails, the transport or the seed is
/// dropping real screen content, not the app renderer.
// The pane runs a POSIX shell TUI (printf/read/python3); Windows panes run PowerShell.
#[cfg(unix)]
#[test]
fn tui_replay_and_live_output_render_in_a_client_emulator() {
    let path = socket_path("tui-live");
    spawn_daemon(&path);

    // Client A creates the session and starts a TUI emitter in its pane.
    {
        let mut first = MuxSessionClient::connect(&path).expect("first client");
        first.create_or_attach("tui").expect("create");
        std::thread::sleep(Duration::from_millis(300));

        // Alt-screen, bordered grid, then cursor-addressed single-cell
        // updates. Plain ANSI, no terminfo dependency.
        let script_path =
            std::env::temp_dir().join(format!("par-term-tui-probe-{}.py", std::process::id()));
        std::fs::write(
            &script_path,
            r##"import sys, time
out = sys.stdout
out.write("\x1b[?1049h\x1b[2J\x1b[H")
out.flush()
out.write("\x1b[41m")
out.flush()
for r in range(24):
    line = "".join("#" if r in (0, 23) or c in (0, 79) else "o" for c in range(80))
    out.write("\x1b[%d;1H%s" % (r + 1, line))
out.write("\x1b[0m")
out.flush()
for i in range(40):
    time.sleep(0.15)
    out.write("\x1b[12;40H%d" % (i % 10))
    out.flush()
out.write("\x1b[?1049l")
out.flush()
"##,
        )
        .expect("write emitter script");
        first
            .send_keys_literal(0, &format!("python3 {}", script_path.display()))
            .expect("send command");
        first.send_keys(0, b"\r").expect("send enter");

        // Wait for the TUI to actually paint (a fixed sleep raced shell
        // startup under load: the seed caught only the typed command).
        let deadline = Instant::now() + DAEMON_DEADLINE;
        loop {
            let screen = first
                .send("capture-pane -t %0 -p")
                .expect("capture-pane")
                .join("\n");
            if screen.contains("####") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the TUI never painted: {screen:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    } // client A drops — detach; the TUI keeps running daemon-side

    // The app's exact attach: create-or-attach, size push, per-pane replay.
    let transport = connect(&path);
    let attach = attach_test(&transport, "tui", Some((80, 24)), &Default::default())
        .expect("attach_sequence");

    let seed = attach
        .screens
        .iter()
        .find(|(pane, _)| *pane == 0)
        .map(|(_, data)| data.clone())
        .expect("seed for %0");

    // The replay must carry the TUI's alt-screen grid — frame and fill.
    let seed_text = String::from_utf8_lossy(&seed);
    assert!(
        seed_text.contains("####"),
        "the seed carries the TUI frame: {seed_text:?}"
    );
    assert!(
        seed_text.contains("ooo"),
        "the seed carries the TUI fill: {seed_text:?}"
    );

    // The client emulator: fresh core Terminal at the pane's size, fed
    // the seed exactly as `apply_pending_mux_screen_seeds` feeds it.
    let mut term = par_term_emu_core_rust::terminal::Terminal::with_scrollback(80, 24, 1000);
    term.process(&seed);

    let after_seed = term.content();
    assert!(
        after_seed.contains("####") && after_seed.contains("ooo"),
        "the seed renders the TUI screen after emulator processing: {after_seed:?}"
    );

    // Attribute fidelity: the fill was painted on a red background;
    // the styled replay must restore it (the old plain-text seed
    // dropped every color — the "partial restore" symptom).
    let fill_bg = term.active_grid().get(40, 5).map(|c| c.bg());
    assert!(
        fill_bg
            == Some(par_term_emu_core_rust::color::Color::Named(
                par_term_emu_core_rust::color::NamedColor::Red,
            )),
        "the replayed seed must restore the fill's background color: {fill_bg:?}"
    );

    // Live output drains into the same terminal — cursor-addressed
    // incremental updates must land on the seeded screen without a
    // full redraw.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let mut saw_update = false;
    while Instant::now() < deadline && !saw_update {
        let (notes, _) = transport.drain();
        for note in notes {
            if let par_term_emu_core_rust::tmux_control::TmuxNotification::Output { pane_id, data } =
                note
                && pane_id == "%0"
            {
                term.process(&data);
            }
        }
        saw_update = term
            .content()
            .lines()
            .any(|line| line.as_bytes().get(39).is_some_and(|b| b.is_ascii_digit()));
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        saw_update,
        "an incremental TUI update must land on the seeded baseline: {:?}",
        term.content()
    );

    let _ = std::fs::remove_file(&path);
}

/// The app-layer reattach repro: a running TUI, a detach, then a
/// reattach driven through the REAL `WindowState` path — tab creation
/// (`handle_tmux_window_add`), the `%layout-change` consumer creating
/// native panes, `check_mux_notifications` routing, seed delivery, and
/// live `%output` landing on the seeded pane. The transport-level
/// wiring tests bypass all of that; this is the path the GUI runs, and
/// the one that was green twice at transport level while panes still
/// rendered blank live.
///
/// The TUI gates its second frame on `read` (the core seed-state
/// pattern): frame 1 must arrive via the SEED, frame 2 via LIVE
/// `%output` after the reattached side releases it — so the test
/// cannot pass on either path alone.
// The pane runs a POSIX shell TUI (printf/read/python3); Windows panes run PowerShell.
#[cfg(unix)]
#[test]
fn reattach_renders_and_updates_a_tui_through_the_window_state_path() {
    let path = socket_path("ws-tui");
    spawn_daemon(&path);

    let tui = "printf \"\\033[?1049h\\033[2;3H\\033[1;44m\"; echo SEED | tr A-Z a-z; \
                   printf \"\\033[0m\\033[?25l\"; read -r x; \
                   printf \"\\033[4;5H\\033[1;42m\"; echo LATE | tr A-Z a-z; \
                   printf \"\\033[0m\"; sleep 60";

    // First client: create the session, start the TUI, wait for its
    // first frame, then drop the connection (detach, D5).
    {
        let mut first = MuxSessionClient::connect(&path).expect("first client");
        first.create_or_attach("wstui").expect("create");
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
        // PaneOutput actions only flow for MAPPED panes (sync.rs), so
        // adopt the daemon's panes before waiting on frame 1 — the
        // adopt_panes pattern from the reattach wiring test.
        for pane in first.list_panes().expect("list-panes") {
            first.sync().map_pane(pane, 10_000 + pane);
        }
        first
            .send(&format!("send-keys -t %0 '{tui}' Enter"))
            .expect("start the TUI");
        let mut framed = false;
        while Instant::now() < deadline && !framed {
            framed = first.poll_actions().iter().any(|a| {
                matches!(a, SyncAction::PaneOutput { data, .. }
                    if String::from_utf8_lossy(data).contains("seed"))
            });
            if !framed {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        assert!(framed, "TUI frame 1 never reached the first client");
    }

    // The app attach: the real sequence, then manual installation into
    // a real (renderer-less) WindowState — the same steps
    // `install_mux_transport` performs minus the renderer-derived size.
    let transport = connect(&path);
    let attach = attach_test(&transport, "wstui", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
    assert!(
        attach
            .screens
            .iter()
            .any(|(_, data)| String::from_utf8_lossy(data).contains("seed")),
        "the reattach seed carries the TUI's first frame: {:?}",
        attach.screens
    );

    let mut ws = manners_state();
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wstui".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump the app's own notification loop — the layout consumer must
    // create the native pane, the seed must render in it, and both must
    // happen through the production dispatch path.
    let mut existing_windows = attach.existing_windows.clone();
    if existing_windows.is_empty() {
        // A created session reports its window via %window-add instead.
        existing_windows = vec![0];
    }
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let native_of = |ws: &crate::app::window_state::WindowState| {
        ws.tmux_state
            .tmux_pane_owners
            .get(&0)
            .map(|&(_, native)| native)
            .or_else(|| ws.tmux_state.tmux_sync.get_native_pane(0))
    };
    let native = loop {
        ws.check_mux_notifications();
        if let Some(native) = native_of(&ws) {
            break native;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the native pane\n\
                 window→tab: {:?}\n\
                 pane map: {:?}\n\
                 tab count: {}",
            existing_windows
                .iter()
                .map(|w| (*w, ws.tmux_state.tmux_sync.get_tab(*w)))
                .collect::<Vec<_>>(),
            ws.tmux_state.tmux_pane_owners,
            ws.tab_manager.tab_count()
        );
        std::thread::sleep(Duration::from_millis(25));
    };

    let pane_state = |ws: &WindowState| -> Option<(bool, String)> {
        for tab in ws.tab_manager.tabs() {
            let Some(pm) = tab.pane_manager() else {
                continue;
            };
            let Some(pane) = pm.get_pane(native) else {
                continue;
            };
            let Ok(term) = pane.terminal.try_read() else {
                continue;
            };
            return Some((
                term.is_alt_screen_active(),
                term.content().unwrap_or_default(),
            ));
        }
        None
    };

    let mut seeded = false;
    while Instant::now() < deadline && !seeded {
        ws.check_mux_notifications();
        seeded = matches!(pane_state(&ws), Some((alt, text)) if alt && text.contains("seed"));
        if !seeded {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    assert!(
        seeded,
        "the seed must render the TUI frame on the native pane's ALT screen: \
             {:?}",
        pane_state(&ws)
    );

    // Release the TUI's second frame through the REAL input path with
    // mux_focused_pane unset — the fresh-attach frozen-panes scenario:
    // keystrokes must resolve their target from the focused native pane
    // (the daemon rejects untargeted send-keys, and nothing sets
    // mux_focused_pane until a click or a daemon focus push).
    // A daemon focus push can land during the seed poll; clear it so
    // the keystroke still has to resolve its target without it.
    ws.tmux_state.mux_focused_pane = None;
    assert!(
        ws.send_input_via_tmux(b"g\r"),
        "the mux transport must consume input"
    );
    let mut updated = false;
    while Instant::now() < deadline && !updated {
        ws.check_mux_notifications();
        updated = matches!(pane_state(&ws), Some((_, text)) if text.contains("late"));
        if !updated {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    assert!(
        updated,
        "input through the real path must reach the daemon and its output \
             land on the seeded native pane: {:?}",
        pane_state(&ws)
    );

    // Split through the real action with mux_focused_pane STILL unset —
    // the fresh-attach silent-no-split scenario: the daemon requires -t
    // on split-window, and the untargeted form used to be sent and its
    // %error reply silently counted as success.
    quiesce(&mut ws);
    ws.split_pane_vertical();
    let mut split_landed = false;
    while Instant::now() < deadline && !split_landed {
        ws.check_mux_notifications();
        split_landed = ws.tmux_state.tmux_pane_owners.contains_key(&1);
        if !split_landed {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    assert!(
        split_landed,
        "the daemon-side split must create the second native pane via \
             %layout-change: map = {:?}",
        ws.tmux_state.tmux_pane_owners
    );

    // The daemon's focus push for the new pane must move NATIVE focus
    // (the only source of truth for routing). It arrives in the same
    // batch as the layout that creates the pane, so it must be applied
    // after the layout consumer, or the new pane is not yet mapped and
    // the push is lost (reported live: split did not focus the new pane).
    let (owner_tab, new_native) = ws.tmux_state.tmux_pane_owners[&1];
    assert_eq!(
        ws.tmux_state.tmux_pane_in_tab(owner_tab, new_native),
        Some(1),
        "the reverse map routes the new pane's input"
    );
    let focused_native = || {
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.focused_pane())
            .map(|p| p.id)
    };
    assert_eq!(
        focused_native(),
        Some(new_native),
        "native focus must move to the split's new pane"
    );

    // Input now targets the new pane and its output lands there.
    assert!(ws.send_input_via_tmux(b"echo SPLIT-OK | tr A-Z a-z\r"));
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let mut new_pane_text = String::new();
    while Instant::now() < deadline && !new_pane_text.contains("split-ok") {
        ws.check_mux_notifications();
        for tab in ws.tab_manager.tabs() {
            if let Some(pane) = tab.pane_manager().and_then(|pm| pm.get_pane(new_native))
                && let Ok(term) = pane.terminal.try_read()
            {
                new_pane_text = term.content().unwrap_or_default();
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let daemon_view = ws
        .tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled("capture-pane -t %1 -p")
        .expect("capture")
        .join("|");
    assert!(
        new_pane_text.contains("split-ok"),
        "the new pane must render its own output: app={new_pane_text:?} daemon={daemon_view:?} \
             map={:?}",
        ws.tmux_state.tmux_pane_owners
    );

    // Mouse reports for a focused mux pane go to the DAEMON pane — the
    // local mirror has no PTY, so a local write dropped every click and
    // wheel event (mouse-aware TUIs ignored the mouse entirely). The
    // bytes land in the pane's input, so a shell echoes a raw marker.
    assert!(
        ws.route_mouse_report_to_mux(b"MOUSE-ROUTE-OK"),
        "a focused mux pane must take the mouse report"
    );
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let mut daemon_view = String::new();
    while Instant::now() < deadline && !daemon_view.contains("MOUSE-ROUTE-OK") {
        daemon_view = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("capture-pane -t %1 -p")
            .expect("capture")
            .join("|");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        daemon_view.contains("MOUSE-ROUTE-OK"),
        "the mouse report must reach the daemon pane: {daemon_view:?}"
    );

    let _ = std::fs::remove_file(&path);
}
