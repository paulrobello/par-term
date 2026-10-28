//! Wire-shape proof for the session picker's named-layout and exit-action
//! surfaces (cards "Expose named mux layouts (save/load/list/delete) in the
//! UI" and "Expose daemon workspace exit-action (set-exit-action) in mux
//! UI"). The picker's scan and executor send plain verbs through
//! [`MuxSessionClient::send_checked`]; this drives those verbs through a
//! live in-process daemon (`MuxServer::bind` on a temp socket, the
//! `mux_paste_buffer` lifecycle) so the reply shapes the picker parses —
//! newline-joined layout names, the bare `set-exit-action` token reply,
//! the `-f`-less load refusal — are validated against the real daemon,
//! not a mock.
//!
//! Gated on `mux`, which is on by default. Run directly:
//!
//! ```sh
//! cargo test -p par-term --test mux_layouts_picker -- --nocapture
//! ```

#![cfg(feature = "mux")]

use par_term::session_picker_mux::{MuxExitAction, layout_names_from_reply};
use par_term_emu_core_rust::mux::MuxServer;
use par_term_mux::MuxSessionClient;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn socket_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "par-term-mux-layouts-picker-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn spawn_daemon(path: &std::path::Path) {
    let server = MuxServer::bind(path).expect("daemon binds");
    // run() serves until the process ends — the core's own e2e lifecycle.
    std::thread::spawn(move || server.run());
}

fn connect(path: &std::path::Path) -> MuxSessionClient {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match MuxSessionClient::connect(path) {
            Ok(client) => return client,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => panic!("daemon never accepted a connection: {e}"),
        }
    }
}

fn body_lines(reply: &[String]) -> Vec<String> {
    reply
        .iter()
        .filter(|l| !l.trim().is_empty())
        .cloned()
        .collect()
}

#[test]
fn layout_and_exit_action_verbs_answer_the_shapes_the_picker_parses() {
    let path = socket_path();
    spawn_daemon(&path);
    let mut client = connect(&path);

    // Make a live pane first, so the saved layout carries a workspace and
    // the -f-less load below has something it must refuse to kill.
    client
        .send_checked("new-session -s picker-probe")
        .expect("new-session answers");

    // save-layout: OK reply, then list-layouts carries the name in the
    // newline-joined body the scan parses.
    let saved = client
        .send_checked("save-layout picker-test")
        .expect("save-layout answers");
    assert!(
        !saved.body.iter().any(|l| l.starts_with("%error")),
        "save-layout refused: {saved:?}"
    );
    let listed = client
        .send_checked("list-layouts")
        .expect("list-layouts answers");
    assert_eq!(
        layout_names_from_reply(&listed.body),
        vec!["picker-test".to_string()],
        "list-layouts body must be one name per line"
    );

    // load-layout without -f refuses while live panes exist — the guard
    // the picker's Confirm Replace flow leans on; the reply names the fix.
    let refused = client
        .send_checked("load-layout picker-test")
        .expect("load-layout answers");
    let refusal = refused.body.join(" ");
    assert!(
        refusal.contains("-f") && refusal.contains("killed"),
        "the -f-less refusal must name the flag and the kill: {refusal:?}"
    );

    // load-layout -f swaps the tree: OK, and the daemon still serves.
    let loaded = client
        .send_checked("load-layout -f picker-test")
        .expect("load-layout -f answers");
    assert!(
        !loaded.body.iter().any(|l| l.starts_with("%error")),
        "load-layout -f refused: {loaded:?}"
    );

    // The bare set-exit-action form is a pure query replying the
    // effective token; the scan decodes it with MuxExitAction.
    let workspaces = client.list_workspaces().expect("list-workspaces answers");
    let first_ws = workspaces.first().expect("a workspace row").id;
    let queried = client
        .send_checked(&format!("set-exit-action -t +{first_ws}"))
        .expect("bare set-exit-action answers");
    let token = body_lines(&queried.body)
        .first()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        MuxExitAction::from_wire(&token),
        Some(MuxExitAction::Hold),
        "a fresh daemon folds remain-on-exit to hold: {token:?}"
    );

    // kill-layout removes the row, leaving an empty store.
    let killed = client
        .send_checked("kill-layout picker-test")
        .expect("kill-layout answers");
    assert!(
        !killed.body.iter().any(|l| l.starts_with("%error")),
        "kill-layout refused: {killed:?}"
    );
    let listed = client
        .send_checked("list-layouts")
        .expect("list-layouts answers");
    assert!(
        layout_names_from_reply(&listed.body).is_empty(),
        "the store must be empty after the delete: {listed:?}"
    );
}
