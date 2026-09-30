//! UX.md M10 (card P4 criterion 5): reattach shows scrollback up to the
//! configured limit.
//!
//! The reattach seed is the daemon's `refresh-client -t %N` reply —
//! `export_screen_restore_sequence`, which replays the pane's main-screen
//! history before the visible screen — and the mirror pane is created with
//! `scrollback_lines` capacity (`Pane::new_for_tmux`). These tests drive
//! the real attach (`poll_mux_attach` → `install_mux_transport` →
//! `attach_sequence`), the layout consumer, and seed delivery, then read
//! the mirror's own scrollback.

use super::mux::MuxAttachPending;
use super::mux::tests::{socket_path, spawn_daemon};
use crate::app::window_state::WindowState;
use par_term_emu_core_rust::mux::MuxClient;
use std::time::{Duration, Instant};

/// Lines of output the first client leaves in the pane's history.
const ROWS: usize = 400;

fn window_state_with_scrollback(lines: usize) -> WindowState {
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut config = crate::config::Config::default();
    config.scrollback.scrollback_lines = lines;
    WindowState::new(config, runtime)
}

/// Create the session and print `ROWS` numbered lines into `%0`, waiting
/// until the daemon's own history holds the last one. The marker is a
/// transformation of the typed command (`ROW` → `row`), so the kernel
/// echo of the command line can never satisfy the wait.
fn seed_history(path: &std::path::Path, session: &str) {
    let mut first = MuxClient::connect(path).expect("first client");
    first
        .send(&format!("new-session -s {session}"))
        .expect("new-session");
    let command = format!("seq -f ROW%g 1 {ROWS} | tr A-Z a-z");
    let deadline = Instant::now() + Duration::from_secs(20);
    // The shell may still be starting; retype until the output lands.
    let mut typed = false;
    loop {
        if !typed {
            first
                .send(&format!("send-keys -t %0 \"{command}\" Enter"))
                .expect("send-keys");
            typed = true;
        }
        let history = first
            .send("capture-pane -t %0 -p -S -2000")
            .expect("capture-pane")
            .join("\n");
        if history.lines().any(|l| l.trim() == format!("row{ROWS}")) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the pane never printed its rows: {history:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // Dropping the client is the detach; the session keeps its history.
}

/// Attach `ws` to `session` through the production attach path and pump
/// until `%0`'s seed has been delivered to its mirror pane.
fn reattach(ws: &mut WindowState, path: &std::path::Path, session: &str) {
    let core_client = MuxClient::connect(path).expect("reattach client");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: session.to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");
    assert!(
        ws.tmux_state.mux_screen_seeds.contains_key(&0),
        "the attach collected a seed for %0"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while ws.tmux_state.mux_screen_seeds.contains_key(&0) {
        assert!(Instant::now() < deadline, "the %0 seed was never delivered");
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            // Force a layout push so the consumer creates the mirror pane.
            let _ = t.send_command("refresh-client -t %0 -C 80x24");
        }
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// `(scrollback_len, scrollback lines)` of the mirror pane showing `%0`.
fn mirror_scrollback(ws: &WindowState) -> (usize, Vec<String>) {
    let (tab_id, native) = ws.tmux_state.tmux_pane_owner(0).expect("%0 is mapped");
    let tab = ws.tab_manager.get_tab(tab_id).expect("owning tab");
    let pane = tab
        .pane_manager()
        .and_then(|pm| pm.get_pane(native))
        .expect("mirror pane");
    let term = pane.terminal.try_read().expect("mirror terminal");
    (term.scrollback_len(), term.scrollback())
}

/// A limit smaller than the daemon's history caps the mirror at the limit
/// and keeps the NEWEST lines; a generous limit receives the whole
/// history. Both from one daemon session, so the difference is the
/// configured limit alone.
#[test]
fn reattach_shows_scrollback_up_to_the_configured_limit() {
    let path = socket_path("m10-scrollback");
    spawn_daemon(&path);
    seed_history(&path, "m10");

    // Small limit: capped, newest kept.
    const LIMIT: usize = 100;
    let mut capped = window_state_with_scrollback(LIMIT);
    reattach(&mut capped, &path, "m10");
    let (len, lines) = mirror_scrollback(&capped);
    assert_eq!(
        len, LIMIT,
        "the mirror holds exactly the configured limit of history"
    );
    assert!(
        !lines.iter().any(|l| l.trim() == "row1"),
        "the oldest line is dropped at the limit"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.trim() == format!("row{}", ROWS - 30)),
        "recent history survives the cap: {:?}",
        &lines[lines.len().saturating_sub(5)..]
    );
    // UX.md U13: clear scrollback in a par-mux tab clears the mirror the
    // user sees, not the tab's hidden local shell. (The daemon has no
    // clear-history command, so its own history returns on reattach.)
    assert!(capped.execute_keybinding_action("clear_scrollback"));
    assert_eq!(
        mirror_scrollback(&capped).0,
        0,
        "the focused pane's mirror scrollback is cleared"
    );
    assert!(capped.detach_mux_session(), "detach the capped client");

    // Generous limit: the whole history arrives.
    let mut full = window_state_with_scrollback(10_000);
    reattach(&mut full, &path, "m10");
    let (len, lines) = mirror_scrollback(&full);
    assert!(
        len >= ROWS - 30,
        "a limit above the history receives all of it: {len} lines"
    );
    assert!(
        lines.iter().any(|l| l.trim() == "row1"),
        "the first history line reaches the mirror"
    );

    let _ = std::fs::remove_file(&path);
}
