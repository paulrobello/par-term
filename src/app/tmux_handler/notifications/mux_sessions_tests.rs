//! UX.md P4 criterion 1: without editing any profile, a user can attach,
//! create, switch, and detach a par-mux session from the picker and the
//! palette.
//!
//! Every test scans and attaches in a private socket directory
//! (`mux_socket_dir_override`) holding in-process daemons, so the user's
//! live daemons are never listed, attached, or ended. The daemons are bound
//! at `par-mux-<name>.sock` paths the way `par-mux <name>` binds them.

use super::mux::tests::manners_state;
use super::mux_test_seams::wait_until;
use crate::app::window_state::WindowState;
use crate::session_picker_mux::{MuxPickerAction, MuxSessionRow};
use par_term_emu_core_rust::mux::{MuxClient, MuxServer};
use std::path::{Path, PathBuf};

/// A private socket directory for one test. Short: macOS caps a socket
/// path at 104 bytes.
fn socket_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(format!("/tmp/ptm-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("socket dir");
    dir
}

/// Serve an EMPTY in-process daemon where a create for `name` will look
/// (`<dir>/par-mux-<name>.sock`), so the create's connect-or-spawn finds it
/// and never starts the external `par-mux` binary — a spawned daemon would
/// outlive the test with nothing to reach it. The create path is still
/// proven: the session appears on this daemon and the window attaches.
fn standby_daemon(dir: &Path, name: &str) -> PathBuf {
    daemon(dir, name, &[])
}

/// Serve a daemon at `<dir>/par-mux-<name>.sock` holding `sessions`.
fn daemon(dir: &Path, name: &str, sessions: &[&str]) -> PathBuf {
    let path = dir.join(format!("par-mux-{name}.sock"));
    let server = MuxServer::bind(&path).expect("daemon binds");
    std::thread::spawn(move || server.run());
    let mut client = MuxClient::connect(&path).expect("seed client");
    for session in sessions {
        client
            .send(&format!("new-session -s {session}"))
            .expect("new-session");
    }
    path
}

fn window_in(dir: &Path) -> WindowState {
    let mut ws = manners_state();
    ws.tmux_state.mux_socket_dir_override = Some(dir.to_path_buf());
    ws
}

/// Pump until `done` holds (attach completion, directory scan, layout).
fn pump(ws: &mut WindowState, what: &str, done: impl Fn(&WindowState) -> bool) {
    wait_until(what, || {
        done(ws) || {
            ws.check_tmux_notifications();
            done(ws)
        }
    });
}

/// A complete directory scan. The scan gives each daemon a fixed reply
/// window (`DAEMON_QUERY_DEADLINE`) and reports a slow one as "did not
/// answer"; under load an in-process daemon can miss it, so the test
/// rescans until every daemon answered. A daemon that never answers still
/// fails at the wait's deadline.
fn scan(ws: &mut WindowState) -> Vec<MuxSessionRow> {
    wait_until("a directory scan every daemon answered", || {
        ws.tmux_state.mux_directory = None;
        ws.refresh_mux_directory();
        pump(ws, "directory scan", |ws| {
            ws.tmux_state.mux_directory.is_some()
        });
        ws.tmux_state
            .mux_directory
            .as_ref()
            .is_some_and(|d| d.errors.is_empty())
    });
    ws.tmux_state
        .mux_directory
        .as_ref()
        .map(|d| d.sessions.clone())
        .unwrap_or_default()
}

fn attached(ws: &WindowState) -> Option<(String, String)> {
    ws.tmux_state.transport.as_ref()?;
    Some((
        ws.tmux_state.mux_daemon.clone().unwrap_or_default(),
        ws.tmux_state.tmux_session_name.clone().unwrap_or_default(),
    ))
}

/// The daemon's session names, sorted (its `list-sessions` order is not
/// part of the contract).
fn daemon_sessions(socket: &Path) -> Vec<String> {
    let mut client = par_term_mux::MuxSessionClient::connect(socket).expect("probe");
    let mut names: Vec<String> = client
        .list_sessions()
        .expect("list-sessions")
        .into_iter()
        .map(|s| s.name)
        .collect();
    names.sort();
    names
}

/// The picker lists every session of every daemon, including a second
/// session held by a daemon named otherwise — and attaching that one goes
/// through its own daemon, never a new empty namesake.
#[test]
fn the_picker_lists_every_session_and_attaches_through_its_daemon() {
    let dir = socket_dir("list");
    daemon(&dir, "work", &["work"]);
    let shared = daemon(&dir, "default", &["alpha", "beta"]);
    let mut ws = window_in(&dir);

    let rows = scan(&mut ws);
    let names: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r.daemon.as_str(), r.name.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![("default", "alpha"), ("default", "beta"), ("work", "work")],
        "one row per session, grouped by daemon"
    );

    let beta = rows.iter().find(|r| r.name == "beta").unwrap().clone();
    assert!(!beta.survives_restore(), "beta's daemon is named otherwise");
    ws.handle_mux_session_request(MuxPickerAction::Attach(beta));
    pump(&mut ws, "attach to beta", |ws| attached(ws).is_some());
    assert_eq!(
        attached(&ws),
        Some(("default".to_string(), "beta".to_string()))
    );
    assert!(
        !dir.join("par-mux-beta.sock").exists(),
        "no namesake daemon was spawned for beta"
    );
    assert_eq!(
        daemon_sessions(&shared),
        vec!["alpha", "beta"],
        "attaching created nothing on the shared daemon"
    );
    assert_eq!(
        ws.tmux_state.persisted_session_names(),
        (None, None),
        "a session restore could not find again is not persisted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Create from the picker: a new daemon named after the session, the
/// window attached to it, and the directory lists it afterwards.
#[test]
fn the_picker_creates_a_session_and_attaches_to_it() {
    let dir = socket_dir("create");
    let mut ws = window_in(&dir);
    assert!(scan(&mut ws).is_empty());

    standby_daemon(&dir, "fresh");
    ws.handle_mux_session_request(MuxPickerAction::Create("fresh".to_string()));
    pump(&mut ws, "attach to the new session", |ws| {
        attached(ws).is_some()
    });
    assert_eq!(
        attached(&ws),
        Some(("fresh".to_string(), "fresh".to_string()))
    );
    assert_eq!(
        daemon_sessions(&dir.join("par-mux-fresh.sock")),
        vec!["fresh"]
    );
    pump(&mut ws, "the new session's tab", |ws| {
        ws.tab_manager.tab_count() > 0
    });

    let rows = scan(&mut ws);
    assert!(
        rows.iter()
            .any(|r| r.name == "fresh" && r.survives_restore())
    );

    // Invalid names are refused before any connect or spawn.
    ws.handle_mux_session_request(MuxPickerAction::Create("bad/name".to_string()));
    assert!(ws.tmux_state.mux_attach_pending.is_none());
    assert!(!dir.join("par-mux-bad").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Switch (attach while attached) moves the window to the other session,
/// keeps the first session running, and never leaves the window without
/// a tab; detach then leaves the window local with both sessions alive.
#[test]
fn switching_and_detaching_keep_every_session_running() {
    let dir = socket_dir("switch");
    let one = daemon(&dir, "one", &["one"]);
    let two = daemon(&dir, "two", &["two"]);
    let mut ws = window_in(&dir);
    let rows = scan(&mut ws);
    let row = |name: &str| rows.iter().find(|r| r.name == name).unwrap().clone();

    ws.handle_mux_session_request(MuxPickerAction::Attach(row("one")));
    pump(&mut ws, "attach one", |ws| attached(ws).is_some());
    pump(&mut ws, "one's tab", |ws| ws.tab_manager.tab_count() > 0);
    let one_tabs = ws.tab_manager.tab_count();
    assert!(one_tabs > 0);

    ws.handle_mux_session_request(MuxPickerAction::Attach(row("two")));
    assert!(
        ws.tab_manager.tab_count() > 0,
        "mid-switch the window keeps a (placeholder) tab"
    );
    pump(&mut ws, "switch to two", |ws| {
        attached(ws).is_some_and(|(_, s)| s == "two")
    });
    pump(&mut ws, "two's tab replaces the placeholder", |ws| {
        ws.tmux_state.mux_restore_placeholder_tab.is_none() && ws.tab_manager.tab_count() > 0
    });
    assert_eq!(daemon_sessions(&one), vec!["one"], "one kept running");

    ws.handle_mux_session_request(MuxPickerAction::Detach);
    assert!(attached(&ws).is_none(), "detached");
    assert_eq!(daemon_sessions(&two), vec!["two"], "two kept running");
    assert_eq!(daemon_sessions(&one), vec!["one"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The palette offers every listed session except the attached one, plus
/// New par-mux Session and Detach as catalog rows; its attach and create
/// rows run through `execute_keybinding_action`, the palette's dispatch.
#[test]
fn the_palette_attaches_creates_and_detaches() {
    let dir = socket_dir("palette");
    daemon(&dir, "p1", &["p1"]);
    let mut ws = window_in(&dir);
    scan(&mut ws);

    let rows = WindowState::mux_session_palette_rows(&ws.tmux_state);
    let ids: Vec<&str> = rows.iter().map(|r| r.action_id.as_str()).collect();
    assert_eq!(ids, vec!["attach_mux_session:p1/p1"]);
    let catalog: Vec<String> = crate::command_palette::catalog::build_catalog()
        .into_iter()
        .map(|e| e.action_id)
        .collect();
    for id in ["new_mux_session", "detach", "toggle_session_picker"] {
        assert!(catalog.iter().any(|c| c == id), "{id} is a palette row");
    }

    assert!(ws.execute_keybinding_action("attach_mux_session:p1/p1"));
    pump(&mut ws, "palette attach", |ws| attached(ws).is_some());
    assert_eq!(attached(&ws), Some(("p1".into(), "p1".into())));
    scan(&mut ws);
    assert!(
        WindowState::mux_session_palette_rows(&ws.tmux_state).is_empty(),
        "the attached session is not offered again"
    );

    standby_daemon(&dir, "session-1");
    assert!(ws.execute_keybinding_action("new_mux_session"));
    pump(&mut ws, "palette create", |ws| {
        attached(ws).is_some_and(|(_, s)| s == "session-1")
    });
    assert!(dir.join("par-mux-session-1.sock").exists());

    assert!(ws.execute_keybinding_action("detach"));
    assert!(attached(&ws).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rename and End from the picker reach the daemon (UP1/UP2); End asks
/// twice in the picker, so the request here is the confirmed one.
#[test]
fn the_picker_renames_and_ends_sessions() {
    let dir = socket_dir("rename");
    let shared = daemon(&dir, "default", &["a", "b"]);
    let mut ws = window_in(&dir);
    let rows = scan(&mut ws);
    let a = rows.iter().find(|r| r.name == "a").unwrap().clone();
    let b = rows.iter().find(|r| r.name == "b").unwrap().clone();

    ws.handle_mux_session_request(MuxPickerAction::Rename(a, "renamed".to_string()));
    assert_eq!(daemon_sessions(&shared), vec!["b", "renamed"]);

    ws.handle_mux_session_request(MuxPickerAction::Kill(b));
    wait_until("b ends", || daemon_sessions(&shared) == vec!["renamed"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A hand-bound `attach_mux_session:<name>` works before the cached list
/// is filled (the first press after launch), and a name no daemon lists is
/// refused without creating anything.
#[test]
fn a_bound_attach_resolves_without_the_cache_and_never_creates() {
    let dir = socket_dir("bound");
    daemon(&dir, "default", &["listed"]);
    let mut ws = window_in(&dir);
    assert!(
        ws.tmux_state.mux_directory.is_none(),
        "cache not filled yet"
    );
    assert!(ws.execute_keybinding_action("attach_mux_session:listed"));
    pump(&mut ws, "bound attach", |ws| attached(ws).is_some());
    assert_eq!(attached(&ws), Some(("default".into(), "listed".into())));
    ws.detach_mux_session();

    let mut ws = window_in(&dir);
    ws.execute_keybinding_action("attach_mux_session:ghost");
    pump(&mut ws, "the lookup settles", |ws| {
        ws.tmux_state.mux_attach_pending.is_none()
    });
    assert!(attached(&ws).is_none(), "nothing attached");
    assert!(!dir.join("par-mux-ghost.sock").exists(), "nothing created");
    assert!(
        ws.tmux_state
            .mux_last_error
            .as_deref()
            .is_some_and(|e| e.contains("ghost")),
        "the miss is reported on the chip: {:?}",
        ws.tmux_state.mux_last_error
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// M15: `attach_mux_session_by_name` — the `--attach` / `mux_auto_attach`
/// entry — finds a listed session through its daemon and creates an
/// unknown one in its own.
#[test]
fn attach_by_name_finds_or_creates_without_a_profile() {
    let dir = socket_dir("byname");
    daemon(&dir, "default", &["listed"]);
    let mut ws = window_in(&dir);
    ws.attach_mux_session_by_name("listed");
    pump(&mut ws, "by-name attach", |ws| attached(ws).is_some());
    assert_eq!(attached(&ws), Some(("default".into(), "listed".into())));
    assert!(!dir.join("par-mux-listed.sock").exists());
    ws.detach_mux_session();

    let mut ws = window_in(&dir);
    standby_daemon(&dir, "brand-new");
    ws.attach_mux_session_by_name("brand-new");
    pump(&mut ws, "by-name create", |ws| attached(ws).is_some());
    assert_eq!(
        attached(&ws),
        Some(("brand-new".into(), "brand-new".into()))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
