//! par-mux pane environment and session-persistence tests: daemon panes get the
//! attach env with local-tab parity, and only a live mux session persists
//! its name.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{
    attach_test, capture_until, connect, manners_state, pane_var, pending_with_client, socket_path,
    spawn_daemon,
};
use super::mux::*;
use super::mux_attach::stamp_pane_session_id;
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend};
use par_term_mux::AttachOutcome;
use std::time::{Duration, Instant};

/// `len:first8:last8` of a value — the form [`pane_var_summary`] prints.
fn summarize(value: &str) -> String {
    let head: String = value.chars().take(8).collect();
    let tail: String = value[value.len().saturating_sub(8)..].to_string();
    format!("{}:{head}:{tail}", value.len())
}

/// [`pane_var`] for values too long for one screen row: the shell
/// prints [`summarize`]'s form instead of the value.
fn pane_var_summary(transport: &MuxTransport, pane: u64, name: &str) -> String {
    let tag = format!("S{pane}{name}");
    let script = format!(
        "v=\"${name}\"; printf '<{tag}''=%s:%s:%s>\\n' \"${{#v}}\" \"${{v:0:8}}\" \"$(printf %s \"$v\" | tail -c 8)\""
    );
    transport
        .send_settled(&format!(
            "send-keys -t %{pane} -l {}",
            par_term_mux::quote_env_value(&script)
        ))
        .expect("send-keys");
    transport
        .send_settled(&format!("send-keys -t %{pane} Enter"))
        .expect("enter");
    let view = capture_until(transport, pane, &format!("<{tag}="));
    let start = view
        .find(&format!("<{tag}="))
        .unwrap_or_else(|| panic!("{name} never printed in %{pane}: {view:?}"));
    let rest = &view[start + tag.len() + 2..];
    rest[..rest.find('>').expect("closing marker")].to_string()
}

/// Card 01a0d93b1c03: mux panes get par-term's shell environment. A
/// created session's first pane sees the attach env (`new-session -e`);
/// a split gets its own ITERM_SESSION_ID; a reattach with a changed
/// value refreshes it for panes created afterwards.
#[cfg(unix)]
#[test]
fn mux_panes_get_the_attach_env_and_a_unique_iterm_session_id() {
    let path = socket_path("session-env");
    spawn_daemon(&path);
    let env: std::collections::HashMap<String, String> = [
        ("TERM_PROGRAM", "iTerm.app"),
        ("__PAR_TERM", "1"),
        ("PAR_TERM_ENV_PROBE", "first value"),
        ("ITERM_SESSION_ID", "w0t0p0:attach"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    let transport = connect(&path);
    let attach = attach_test(&transport, "envs", None, &env).expect("attach");
    let AttachOutcome::Created(session) = attach.outcome else {
        panic!("expected a created session");
    };
    // The first pane spawned inside new-session, so -e reached it —
    // including over the core's own TERM_PROGRAM=kitty default.
    assert_eq!(pane_var(&transport, 0, "TERM_PROGRAM"), "iTerm.app");
    assert_eq!(pane_var(&transport, 0, "__PAR_TERM"), "1");
    assert_eq!(pane_var(&transport, 0, "PAR_TERM_ENV_PROBE"), "first value");

    stamp_pane_session_id(&transport, Some(session.id));
    let reply = transport
        .send_settled("split-window -h -t %0")
        .expect("split");
    assert!(
        reply.iter().any(|l| l.trim() == "%1"),
        "split reply: {reply:?}"
    );
    let first_id = pane_var(&transport, 0, "ITERM_SESSION_ID");
    let second_id = pane_var(&transport, 1, "ITERM_SESSION_ID");
    assert_eq!(first_id, "w0t0p0:attach");
    assert!(second_id.starts_with("w0t0p0:"), "{second_id:?}");
    assert_ne!(
        first_id, second_id,
        "each pane needs its own ITERM_SESSION_ID"
    );
    drop(transport);

    // Reattach with a changed value: panes created afterwards see it,
    // the pane already running keeps what it spawned with.
    let mut changed = env.clone();
    changed.insert("PAR_TERM_ENV_PROBE".to_string(), "it's changed".to_string());
    let second = connect(&path);
    let reattach = attach_test(&second, "envs", None, &changed).expect("reattach");
    assert!(matches!(reattach.outcome, AttachOutcome::Attached(_)));
    second
        .send_settled("split-window -v -t %1")
        .expect("split after reattach");
    assert_eq!(pane_var(&second, 2, "PAR_TERM_ENV_PROBE"), "it's changed");
    assert_eq!(pane_var(&second, 0, "PAR_TERM_ENV_PROBE"), "first value");

    let _ = std::fs::remove_file(&path);
}

/// Criterion 1 of card 01a0d93b1c03: a mux pane shows the same values
/// a local tab gets from `build_shell_env` for the parity keys, with a
/// `shell_env` entry and a UTF-8 locale in the mix. PATH is compared
/// whole: the daemon has the test process's PATH, and the session env
/// must replace it with par-term's augmented one.
#[cfg(unix)]
#[test]
fn mux_pane_env_matches_a_local_tab_for_the_parity_keys() {
    let path = socket_path("env-parity");
    spawn_daemon(&path);
    let shell_env: std::collections::HashMap<String, String> = [(
        "PAR_TERM_USER_VAR".to_string(),
        "from shell_env".to_string(),
    )]
    .into();
    let local = crate::tab::build_shell_env(Some(&shell_env)).expect("env");
    let mut sent = local.clone();
    sent.insert("PAR_TERM_PATH_PROBE".to_string(), local["PATH"].clone());
    let transport = connect(&path);
    attach_test(&transport, "parity", None, &sent).expect("attach");
    for key in [
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "LC_TERMINAL",
        "LC_TERMINAL_VERSION",
        "__PAR_TERM",
        "LANG",
        "PAR_TERM_USER_VAR",
        "ITERM_SESSION_ID",
    ] {
        // Long values wrap across screen rows; compare the byte length
        // plus the ends, which is what the capture can carry intact.
        let expected = local.get(key).cloned().unwrap_or_default();
        let got = pane_var_summary(&transport, 0, key);
        assert_eq!(got, summarize(&expected), "{key}");
    }
    // PATH: the pane's shell rc may add entries of its own (a local
    // tab runs the same rc), so parity means every entry of par-term's
    // augmented PATH is present. The expected value rides in as a probe
    // var and the shell counts the entries missing from $PATH.
    let script = r#"n=0; printf '%s\n' "$PAR_TERM_PATH_PROBE" | tr : '\n' > /tmp/.ptprobe.$$; while read -r d; do [[ ":$PATH:" == *":$d:"* ]] || n=$((n+1)); done < /tmp/.ptprobe.$$; rm -f /tmp/.ptprobe.$$; echo "<MISS""=$n>""#;
    transport
        .send_settled(&format!(
            "send-keys -t %0 -l {}",
            par_term_mux::quote_env_value(script)
        ))
        .expect("send-keys");
    transport
        .send_settled("send-keys -t %0 Enter")
        .expect("enter");
    let view = capture_until(&transport, 0, "<MISS=");
    assert!(view.contains("<MISS=0>"), "PATH entries missing: {view:?}");
    let _ = std::fs::remove_file(&path);
}

/// A window attached to a par-mux daemon must persist its session name
/// as a MUX name, never a tmux one: the next launch restored a
/// tmux-tagged name through the tmux gateway, which spawned a real
/// `tmux -CC new-session -A` of the same name (measured live
/// 2026-09-23: single pane, then a blank window, on successive
/// reopens). The session must have a live mapped window — that is the
/// quit-with-panes shape worth reattaching to on relaunch.
#[test]
fn an_attached_mux_session_persists_as_mux_not_tmux() {
    let path = socket_path("persist-kind");
    spawn_daemon(&path);
    let transport = connect(&path);
    attach_test(&transport, "kind", None, &Default::default()).expect("attach");

    let mut ws = manners_state();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("kind".to_string());
    ws.handle_tmux_window_add(0);
    assert_eq!(
        ws.tmux_state.persisted_session_names(),
        (None, Some("kind".to_string())),
        "a transport-attached name over live windows is a mux name"
    );

    let _ = std::fs::remove_file(&path);
}

/// The save-time shape of the emptied-session bug: the attach recorded
/// the name and the transport is still installed, but every daemon
/// window has closed (each close unmaps). Persisting the name made the
/// next launch create-or-attach to the emptied session and hand back a
/// fresh window the user had deliberately closed (observed live
/// 2026-09-26) — an emptied session persists nothing.
#[test]
fn an_emptied_mux_session_does_not_persist_its_name() {
    let path = socket_path("persist-emptied");
    spawn_daemon(&path);
    let transport = connect(&path);
    attach_test(&transport, "gone", None, &Default::default()).expect("attach");

    let mut ws = manners_state();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("gone".to_string());
    assert_eq!(
        ws.tmux_state.persisted_session_names(),
        (None, None),
        "a session with no live windows must not persist its name"
    );

    let _ = std::fs::remove_file(&path);
}

/// End to end: the real roads the user's shells take. A created
/// session's window arrives via %window-add and the live session
/// persists its name (the quit-with-panes case); killing that last
/// window — what a shell exit in the pane does — closes and unmaps the
/// tab, and the emptied session persists nothing.
#[test]
fn exiting_every_mux_window_stops_the_session_name_persisting() {
    let path = socket_path("persist-e2e");
    spawn_daemon(&path);

    let mut ws = manners_state();
    ws.tmux_state.mux_attach_pending = Some(pending_with_client("emptyme", &path));
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");

    let deadline = Instant::now() + DAEMON_DEADLINE;
    while (ws.tab_manager.tabs().is_empty() || ws.tmux_state.tmux_pane_owners.is_empty())
        && Instant::now() < deadline
    {
        // The layout pump from the recorder pattern: force the daemon
        // to broadcast %layout-change so the pane (and the tab's
        // tmux_pane_id) exists — the shape a live pane has before the
        // user exits its shell.
        let _ = ws
            .tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_settled("refresh-client -t %0 -C 80x24");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !ws.tab_manager.tabs().is_empty() && !ws.tmux_state.tmux_pane_owners.is_empty(),
        "the created session's window and pane must arrive"
    );

    assert_eq!(
        ws.tmux_state.persisted_session_names(),
        (None, Some("emptyme".to_string())),
        "a live session with a mapped window persists its name"
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
        "the killed window's tab must close"
    );
    assert_eq!(
        ws.tmux_state.persisted_session_names(),
        (None, None),
        "the emptied session must not persist its name"
    );

    let _ = std::fs::remove_file(&path);
}
