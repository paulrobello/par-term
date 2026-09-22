//! Phase 4 compatibility proof: a real par-mux daemon, driven through the
//! UNMODIFIED sync layer imported from par-term-tmux.
//!
//! The claim under test (par-mux.md Phase 4 "The compatibility test"):
//! `ParserBridge` and `TmuxSync` — written for tmux control mode — work
//! against par-mux as-is. Nothing here forks bridge/sync code; every
//! asserted action was produced by par-term-tmux's functions from the
//! daemon's live notification stream.
//!
//! The daemon runs in-process (`MuxServer::bind` + thread), the same way
//! the core's own end-to-end tests prove their exit criteria; the
//! binary-spawn path (`MuxClient::connect_or_spawn`) is the core's tested
//! surface and gets exercised for real by the app wiring (T4.4).

#![cfg(feature = "mux")]

use par_term_emu_core_rust::mux::MuxServer;
use par_term_mux::{AttachOutcome, MuxSessionClient};
use par_term_tmux::{SyncAction, TmuxLayout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Marker echoed back through the daemon's `%output` push.
const MARKER: &str = "par-term-mux-compat-marker";

fn socket_path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "par-term-mux-compat-{}-{tag}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn spawn_daemon(path: &Path) {
    let server = MuxServer::bind(path).expect("daemon binds");
    // run() serves until the process ends — the same detach-and-die
    // lifecycle the core's own end-to-end tests use.
    std::thread::spawn(move || server.run());
}

fn connect(path: &Path) -> MuxSessionClient {
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

/// Poll until an action matching `wanted` arrives; return everything seen.
fn wait_for(
    client: &mut MuxSessionClient,
    wanted: impl Fn(&SyncAction) -> bool,
) -> Vec<SyncAction> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        let actions = client.poll_actions();
        let hit = actions.iter().any(&wanted);
        seen.extend(actions);
        if hit {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("expected action never arrived within 10s; saw: {seen:?}");
}

/// Map every daemon pane to a native pane id (`1000 + tmux id`) — the
/// app's reattach adoption step, mirrored so unmapped panes drop no
/// output.
fn adopt_panes(client: &mut MuxSessionClient) {
    for pane in client.list_panes().expect("list-panes") {
        client.sync().map_pane(pane, 1000 + pane);
    }
}

#[test]
fn sync_layer_round_trips_a_live_daemon() {
    let path = socket_path("roundtrip");
    spawn_daemon(&path);
    let mut client = connect(&path);

    // 1. new-session broadcasts %window-add; the bridge converts it and
    //    TmuxSync emits CreateTab.
    let outcome = client.create_or_attach("compat").expect("create_or_attach");
    assert!(
        matches!(outcome, AttachOutcome::Created(_)),
        "fresh daemon has no session to attach to: {outcome:?}"
    );
    let seen = wait_for(&mut client, |a| matches!(a, SyncAction::CreateTab { .. }));
    let window_id = seen
        .iter()
        .find_map(|a| match a {
            SyncAction::CreateTab { window_id } => Some(*window_id),
            _ => None,
        })
        .expect("CreateTab arrived");

    // The app side of the contract: allocate a tab, map the window to it.
    let tab = 100;
    client.sync().map_window(window_id, tab);
    adopt_panes(&mut client);

    // 2. split-window broadcasts %layout-change; TmuxSync maps it to
    //    UpdateLayout for the mapped tab, and the daemon's real layout
    //    string parses with par-term-tmux's own parser.
    client.send("split-window -t %0 -h").expect("split-window");
    let seen = wait_for(&mut client, |a| {
        matches!(a, SyncAction::UpdateLayout { .. })
    });
    let layout = seen
        .iter()
        .find_map(|a| match a {
            SyncAction::UpdateLayout { layout, .. } => Some(layout.clone()),
            _ => None,
        })
        .expect("UpdateLayout arrived");
    let parsed = TmuxLayout::parse(&layout).expect("daemon layout parses with TmuxLayout::parse");
    assert_eq!(
        parsed.pane_ids().len(),
        2,
        "split produced two panes in the layout: {layout}"
    );
    adopt_panes(&mut client);

    // 3. send-keys in the gateway's exact form (key names from
    //    escape_keys_for_tmux, C-j for the newline) routes pane output
    //    back as PaneOutput with the mapped native pane id.
    client
        .send_keys(0, format!("echo {MARKER}\n").as_bytes())
        .expect("send-keys");
    wait_for(&mut client, |a| match a {
        SyncAction::PaneOutput { pane_id, data }
            if String::from_utf8_lossy(data).contains(MARKER) =>
        {
            *pane_id == 1000
        }
        _ => false,
    });

    // 4. rename-window broadcasts %window-renamed -> RenameTab for the tab.
    client
        .send(&format!("rename-window -t @{window_id} renamed"))
        .expect("rename-window");
    wait_for(
        &mut client,
        |a| matches!(a, SyncAction::RenameTab { tab_id: 100, name } if name == "renamed"),
    );

    // 5. kill-window broadcasts %window-close -> CloseTab for the tab.
    client
        .send(&format!("kill-window -t @{window_id}"))
        .expect("kill-window");
    wait_for(&mut client, |a| {
        matches!(a, SyncAction::CloseTab { tab_id: 100 })
    });

    let _ = std::fs::remove_file(&path);
}

#[test]
fn create_or_attach_reattaches_to_a_persisted_session() {
    let path = socket_path("reattach");
    spawn_daemon(&path);

    {
        let mut first = connect(&path);
        let outcome = first.create_or_attach("keep").expect("create_or_attach");
        assert!(matches!(outcome, AttachOutcome::Created(_)));
        // Daemon-outlives-client: dropping the connection leaves the tree.
    }

    let mut second = connect(&path);
    let outcome = second.create_or_attach("keep").expect("create_or_attach");
    assert!(
        matches!(outcome, AttachOutcome::Attached(ref s) if s.name == "keep"),
        "second client reattaches to the persisted session: {outcome:?}"
    );
    assert_eq!(
        second.list_panes().expect("list-panes").len(),
        1,
        "the surviving window still holds its pane"
    );

    let _ = std::fs::remove_file(&path);
}
