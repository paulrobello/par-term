//! End-to-end close confirmation for attached (par-mux) panes: the running
//! foreground job lives daemon-side, so the gate asks the daemon.

use crate::app::tmux_handler::notifications::mux::{MuxAttachPending, tests as mux_tests};
use crate::app::window_state::WindowState;
use std::time::{Duration, Instant};

fn attached_two_pane_state(
    tag: &str,
    jobs_to_ignore: Vec<String>,
) -> (WindowState, std::path::PathBuf) {
    let path = mux_tests::socket_path(tag);
    mux_tests::spawn_daemon(&path);
    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Ok(core_client)).unwrap();
    drop(tx);

    let mut config = crate::config::Config::default();
    config.shell.confirm_close_running_jobs = true;
    config.shell.jobs_to_ignore = jobs_to_ignore;
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime"),
    );
    let mut ws = WindowState::new(config, runtime);
    ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
        name: tag.to_string(),
        rx,
    });
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");
    ws.handle_tmux_window_add(0);

    let deadline = Instant::now() + Duration::from_secs(10);
    while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
        assert!(Instant::now() < deadline, "%0 never mapped");
        send(&ws, "refresh-client -t %0 -C 80x24");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(ws.split_pane_via_mux(true), "split gives a second pane");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ws.tmux_state.tmux_pane_owners.contains_key(&1) {
        assert!(Instant::now() < deadline, "%1 never mapped");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
    (ws, path)
}

fn send(ws: &WindowState, command: &str) -> Vec<String> {
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_command(command)
        .expect("daemon command")
}

fn daemon_pane_count(ws: &WindowState) -> usize {
    send(ws, "list-panes")
        .iter()
        .filter(|l| l.starts_with('%'))
        .count()
}

/// Pump notifications until the daemon holds `want` panes.
fn wait_for_pane_count(ws: &mut WindowState, want: usize, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while daemon_pane_count(ws) != want {
        assert!(Instant::now() < deadline, "{what}");
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll `pane-info` until the daemon reports `want` as the foreground
/// command (the spawned job is a process the daemon must first observe).
fn wait_for_foreground(ws: &WindowState, pane: u64, want: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let reply = send(ws, &format!("pane-info -t %{pane}"));
        if par_term_mux::pane_foreground_command(&reply, pane).as_deref() == Some(want) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon never reported {want:?} as the foreground of %{pane}: {reply:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Card 01a0ea7b3beb749085938c8fba01d9f2: closing an attached pane that is
/// running `sleep 100` holds the close behind the confirmation dialog, and
/// confirming kills the pane daemon-side. Against a daemon that predates
/// the `pane-info` foreground token (the published 0.56 line) the close
/// must degrade to the unconfirmed daemon-side kill, never block or fail.
/// Under the plain gate this test therefore exercises the old-daemon arm;
/// under `scripts/with-local-core.sh` it exercises the confirming arm.
#[test]
fn closing_an_attached_pane_running_a_job_asks_first() {
    // `sleep` is on the default ignore list; this test IS the sleep-100
    // scenario, so ignore only shells.
    let shells = ["bash", "zsh", "fish", "sh"].map(String::from).to_vec();
    let (mut ws, path) = attached_two_pane_state("mux-close-confirm", shells);
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");

    let idle = send(&ws, &format!("pane-info -t %{target}"));
    let daemon_reports_foreground = idle.iter().any(|l| l.contains(" cmd="));

    if !daemon_reports_foreground {
        eprintln!("old daemon (no pane-info cmd token): asserting the unconfirmed close");
        send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
        send(&ws, &format!("send-keys -t %{target} Enter"));
        assert!(!ws.close_focused_pane());
        assert!(!ws.overlay_ui.close_confirmation_ui.is_visible());
        wait_for_pane_count(&mut ws, 1, "old-daemon close never killed the pane");
        let _ = std::fs::remove_file(&path);
        return;
    }

    // The idle shell must not raise the dialog (covered on the sibling pane
    // by killing it first would change focus, so the idle case is the pure
    // unit test on the ignore filter); here the focused pane runs the job.
    send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
    send(&ws, &format!("send-keys -t %{target} Enter"));
    wait_for_foreground(&ws, target, "sleep");

    assert!(
        !ws.close_focused_pane(),
        "the close must wait for the dialog"
    );
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "a running foreground job must raise the confirmation"
    );
    assert_eq!(
        daemon_pane_count(&ws),
        2,
        "nothing is killed before confirming"
    );

    // Confirming closes the pane daemon-side.
    assert!(!ws.close_focused_pane_confirmed());
    wait_for_pane_count(&mut ws, 1, "confirmed close never killed the pane");
    let _ = std::fs::remove_file(&path);
}
