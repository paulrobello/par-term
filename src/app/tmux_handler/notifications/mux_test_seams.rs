//! Shared test seams for par-mux tests outside this module: a window
//! attached to an in-process daemon with N mapped daemon tabs.

use super::mux::tests::{manners_state, socket_path, spawn_daemon};
use crate::app::window_state::WindowState;
use par_term_emu_core_rust::mux::MuxClient;
use std::time::{Duration, Instant};

/// Attach a fresh window to a new in-process daemon and open `tabs` daemon
/// windows, pumping until every one is a mapped tab. Returns the window and
/// the daemon's socket (remove it when done).
pub(crate) fn attached_window_with_tabs(
    tag: &str,
    tabs: usize,
) -> (WindowState, std::path::PathBuf) {
    let path = socket_path(tag);
    spawn_daemon(&path);
    let core = MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core)).unwrap();
    drop(tx);
    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(super::mux::MuxAttachPending {
        name: tag.to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach installs");
    for _ in 1..tabs {
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command("new-window")
            .expect("new-window");
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while ws.tab_manager.tab_count() < tabs || ws.tmux_state.tmux_pane_owners.len() < tabs {
        assert!(Instant::now() < deadline, "tabs never mapped");
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            for pane in 0..tabs as u64 {
                let _ = t.send_command_no_wait(&format!("refresh-client -t %{pane} -C 80x24"));
            }
        }
        ws.check_tmux_notifications();
        std::thread::sleep(Duration::from_millis(30));
    }
    (ws, path)
}
