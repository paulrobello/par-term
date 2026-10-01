//! End-to-end close confirmation for attached (par-mux) panes: the running
//! foreground job lives daemon-side, so the gate asks the daemon.

use crate::app::tmux_handler::notifications::mux::{MuxAttachPending, tests as mux_tests};
use crate::app::tmux_handler::notifications::mux_test_seams::{SettledSend, quiesce, wait_until};
use crate::app::window_state::WindowState;
use std::time::{Duration, Instant};

fn attached_two_pane_state(
    tag: &str,
    jobs_to_ignore: Vec<String>,
) -> (WindowState, std::path::PathBuf) {
    let path = mux_tests::socket_path(tag);
    mux_tests::spawn_daemon(&path);
    attach_two_panes(tag, &path, &path, jobs_to_ignore)
}

/// Attach a window through `connect_to` (the daemon itself, or a proxy in
/// front of it) to the daemon serving `daemon_socket`, and split it to two
/// mapped panes. Returns the window and `daemon_socket`.
fn attach_two_panes(
    tag: &str,
    connect_to: &std::path::Path,
    daemon_socket: &std::path::Path,
    jobs_to_ignore: Vec<String>,
) -> (WindowState, std::path::PathBuf) {
    let path = daemon_socket.to_path_buf();
    let core_client = par_term_emu_core_rust::mux::MuxClient::connect(connect_to).expect("connect");
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
    // What every production attach records: the socket it connects through.
    ws.tmux_state.mux_attach_socket = Some(connect_to.to_path_buf());
    ws.poll_mux_attach();
    assert!(ws.tmux_state.transport.is_some(), "attach must install");
    // The created session reports its window through the daemon's own
    // %window-add push, drained below. Adding @0 by hand as well left a
    // stale duplicate tab mapped to @0, which reads as a second attached
    // tab (and hides the last-tab gate).

    wait_until("%0 mapped", || {
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            let _ = t.send_command_no_wait("refresh-client -t %0 -C 80x24");
        }
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&0)
    });
    quiesce(&mut ws);
    assert!(ws.split_pane_via_mux(true), "split gives a second pane");
    wait_until("%1 mapped", || {
        ws.check_mux_notifications();
        ws.tmux_state.tmux_pane_owners.contains_key(&1)
    });
    (ws, path)
}

fn send(ws: &WindowState, command: &str) -> Vec<String> {
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_settled(command)
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
    wait_until(what, || {
        ws.check_mux_notifications();
        daemon_pane_count(ws) == want
    });
}

/// Poll `pane-info` until the daemon reports `want` as the foreground
/// command (the spawned job is a process the daemon must first observe).
fn wait_for_foreground(ws: &WindowState, pane: u64, want: &str) {
    let mut reply = Vec::new();
    let seen = super::notifications::mux_test_seams::poll_until(|| {
        reply = send(ws, &format!("pane-info -t %{pane}"));
        par_term_mux::pane_foreground_command(&reply, pane).as_deref() == Some(want)
    });
    assert!(
        seen,
        "daemon never reported {want:?} as the foreground of %{pane}: {reply:?}"
    );
}

/// Card 01a0ea7b3beb749085938c8fba01d9f2: closing an attached pane that is
/// running `sleep 100` holds the close behind the confirmation dialog, and
/// confirming kills the pane daemon-side. Where the daemon cannot read a
/// pane's foreground command (Windows: no process-table access, so
/// `pane-info` carries no `cmd=` token) the close must degrade to the
/// unconfirmed daemon-side kill, never block or fail. The arm is chosen by
/// platform: a timing probe would let a broken foreground report pass as
/// the degraded arm.
#[test]
fn closing_an_attached_pane_running_a_job_asks_first() {
    // `sleep` is on the default ignore list; this test IS the sleep-100
    // scenario, so ignore only shells.
    let shells = ["bash", "zsh", "fish", "sh"].map(String::from).to_vec();
    let (mut ws, path) = attached_two_pane_state("mux-close-confirm", shells);
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");

    if !daemon_reports_foreground(&ws, target) {
        eprintln!("old daemon (no pane-info cmd token): asserting the unconfirmed close");
        send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
        send(&ws, &format!("send-keys -t %{target} Enter"));
        quiesce(&mut ws);
        assert!(!ws.close_focused_pane());
        assert!(!ws.overlay_ui.close_confirmation_ui.is_visible());
        wait_for_pane_count(&mut ws, 1, "unconfirmed close never killed the pane");
        let _ = std::fs::remove_file(&path);
        return;
    }

    // The idle shell must not raise the dialog (covered on the sibling pane
    // by killing it first would change focus, so the idle case is the pure
    // unit test on the ignore filter); here the focused pane runs the job.
    send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
    send(&ws, &format!("send-keys -t %{target} Enter"));
    wait_for_foreground(&ws, target, "sleep");

    // The confirmed close sends `kill-pane` inline, which fails fast while
    // the send worker is busy; settle the worker so the kill reaches the
    // daemon.
    quiesce(&mut ws);
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

type Stream = std::sync::Arc<par_term_emu_core_rust::mux::LocalStream>;
type Trigger = std::sync::Arc<std::sync::Mutex<Option<(String, Duration)>>>;

/// A byte-level proxy in front of the daemon at `daemon` that can hold one
/// chosen command. Every connection made to `listen` is relayed to its own
/// daemon connection. Once armed with `(substring, hold)`, the FIRST command
/// line containing `substring` on any connection is forwarded only after
/// `hold` — the shape of one slow daemon command already in flight — and the
/// trigger disarms itself.
struct StallProxy {
    trigger: Trigger,
}

impl StallProxy {
    fn spawn(listen: &std::path::Path, daemon: &std::path::Path) -> Self {
        use par_term_emu_core_rust::mux::{
            accept_connection, bind_local_listener, connect_local_stream,
        };
        let trigger: Trigger = std::sync::Arc::default();
        let listener = bind_local_listener(listen).expect("proxy binds");
        let daemon = daemon.to_path_buf();
        let accept_trigger = std::sync::Arc::clone(&trigger);
        std::thread::spawn(move || {
            while let Ok((client, _abort)) = accept_connection(&listener) {
                let Ok(upstream) = connect_local_stream(&daemon) else {
                    continue;
                };
                let (client, upstream) = (Stream::new(client), Stream::new(upstream));
                let trigger = std::sync::Arc::clone(&accept_trigger);
                let (c, u) = (Stream::clone(&client), Stream::clone(&upstream));
                std::thread::spawn(move || relay_commands(&c, &u, &trigger));
                std::thread::spawn(move || relay_bytes(&upstream, &client));
            }
        });
        Self { trigger }
    }

    /// Hold the next command containing `substring` for `hold`.
    fn arm(&self, substring: &str, hold: Duration) {
        *self.trigger.lock().unwrap() = Some((substring.to_string(), hold));
    }

    /// Whether the armed command has arrived and is being held.
    fn fired(&self) -> bool {
        self.trigger.lock().unwrap().is_none()
    }
}

/// Client to daemon, line by line, so a command can be held whole.
fn relay_commands(from: &Stream, to: &Stream, trigger: &Trigger) {
    use std::io::{BufRead, Write};
    let mut reader = std::io::BufReader::new(&**from);
    let mut writer = &**to;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let hold = {
            let mut armed = trigger.lock().unwrap();
            match armed.as_ref() {
                Some((substring, hold)) if line.contains(substring.as_str()) => {
                    let hold = *hold;
                    *armed = None;
                    Some(hold)
                }
                _ => None,
            }
        };
        if let Some(hold) = hold {
            std::thread::sleep(hold);
        }
        if writer.write_all(line.as_bytes()).is_err() {
            return;
        }
    }
}

/// Daemon to client, raw bytes.
fn relay_bytes(from: &Stream, to: &Stream) {
    use std::io::{Read, Write};
    let (mut reader, mut writer) = (&**from, &**to);
    let mut buf = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if writer.write_all(&buf[..n]).is_err() {
                    return;
                }
            }
        }
    }
}

/// How long a stalled command is held: far past both the transport's inline
/// wait (250 ms) and any bound the close check may take, short of the core
/// client's 10 s reply timeout.
const STALL: Duration = Duration::from_secs(4);

/// A window attached THROUGH a [`StallProxy`] to an in-process daemon, two
/// panes mapped, the focused one running `sleep 100` as the daemon sees it.
/// `None` against a daemon without the `pane-info` foreground token (the
/// published 0.56 line), which has nothing to confirm on.
fn proxied_pane_running_sleep(
    tag: &str,
) -> Option<(
    WindowState,
    StallProxy,
    std::path::PathBuf,
    std::path::PathBuf,
)> {
    let (ws, proxy, daemon, proxy_path) = proxied_attach(tag)?;
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");
    send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
    send(&ws, &format!("send-keys -t %{target} Enter"));
    wait_for_foreground(&ws, target, "sleep");
    Some((ws, proxy, daemon, proxy_path))
}

/// Pump the window until `deadline` so the stalled command drains.
fn pump_until(ws: &mut WindowState, deadline: Instant) {
    while Instant::now() < deadline {
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Card 01a0f3af7c157451b1053dd6070ecd7b, criterion 1: one slow daemon
/// command already in flight on the attached connection (an off-loop
/// discovery or seed query) must not make a close skip the running-job
/// confirmation. The old check queued its `pane-info` behind the stalled
/// command on the one connection, gave up at the inline wait, and failed
/// open: no dialog, and the close went straight on to `kill-pane` with
/// `sleep 100` still running.
#[cfg(unix)]
#[test]
fn a_slow_in_flight_daemon_command_does_not_skip_the_job_confirmation() {
    use crate::app::tmux_handler::tmux_state::MuxJob;
    let Some((mut ws, proxy, daemon, proxy_path)) = proxied_pane_running_sleep("mux-close-stall")
    else {
        return;
    };

    // Settle queued size pushes first, so the held command is the seed's.
    quiesce(&mut ws);
    // A seed query from the off-loop worker, held at the proxy: it is in
    // flight on the attached connection, holding the client lock.
    proxy.arm("refresh-client -t %0", STALL);
    let transport = ws.tmux_state.transport.as_ref().expect("transport");
    assert!(transport.submit_job(MuxJob::SeedPanes(vec![0])));
    // The close must race a command already in flight, or it proves nothing.
    wait_until("the seed query is held at the proxy", || proxy.fired());
    let stalled_at = Instant::now();

    let started = Instant::now();
    assert!(
        !ws.close_focused_pane(),
        "the close must not end the window"
    );
    let took = started.elapsed();
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "a running job behind a slow in-flight command must still raise the confirmation"
    );
    assert_eq!(ws.overlay_ui.close_confirmation_ui.command_name(), "sleep");
    assert!(
        took < STALL / 2,
        "the close check waited out the in-flight command ({took:?})"
    );

    pump_until(&mut ws, stalled_at + STALL + Duration::from_millis(500));
    assert_eq!(
        daemon_pane_count(&ws),
        2,
        "the job's pane must survive the close"
    );
    let _ = std::fs::remove_file(&daemon);
    let _ = std::fs::remove_file(&proxy_path);
}

/// The hung-daemon half: when the running-job check itself cannot get an
/// answer in time, the close fails CLOSED (the confirmation opens) and the
/// event loop is back well before the daemon would have answered. The old
/// check waited on the daemon's reply for as long as it took (here the whole
/// stall; a hung daemon cost the core client's 10 s timeout) on the event
/// loop.
#[cfg(unix)]
#[test]
fn a_job_check_the_daemon_cannot_answer_in_time_holds_the_close() {
    let Some((mut ws, proxy, daemon, proxy_path)) = proxied_pane_running_sleep("mux-close-hung")
    else {
        return;
    };
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");

    // Settle queued jobs first, so the held `pane-info` is the close check's.
    quiesce(&mut ws);
    proxy.arm(&format!("pane-info -t %{target}"), STALL);
    let stalled_at = Instant::now();
    assert!(
        !ws.close_focused_pane(),
        "the close must not end the window"
    );
    let took = stalled_at.elapsed();
    assert!(
        took < STALL / 2,
        "the close blocked the event loop on an unanswered check ({took:?})"
    );
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "a check that could not answer must hold the close for confirmation"
    );

    pump_until(&mut ws, stalled_at + STALL + Duration::from_millis(500));
    assert_eq!(
        daemon_pane_count(&ws),
        2,
        "nothing is killed while the check is unanswered"
    );
    let _ = std::fs::remove_file(&daemon);
    let _ = std::fs::remove_file(&proxy_path);
}

fn daemon_window_count(ws: &WindowState) -> usize {
    send(ws, "list-windows")
        .iter()
        .filter(|l| l.contains('@'))
        .count()
}

/// A window attached THROUGH a [`StallProxy`] with two daemon windows: the
/// active tab mirrors a two-pane window whose UNFOCUSED pane runs `sleep
/// 100` as the daemon sees it, and a second mux tab keeps the session
/// alive past that tab's close (no last-tab dialog). Returns the window,
/// the proxy, both socket paths, and the job's daemon pane. `None` against
/// a daemon without the `pane-info` foreground token.
fn proxied_tab_with_a_background_job(
    tag: &str,
) -> Option<(
    WindowState,
    StallProxy,
    std::path::PathBuf,
    std::path::PathBuf,
    u64,
)> {
    let (mut ws, proxy, daemon, proxy_path) = proxied_attach(tag)?;
    let tab = ws.tab_manager.active_tab_id().expect("active tab");

    // The second daemon window: a mux tab of its own once its pane maps.
    send(&ws, "new-window");
    let deadline = Instant::now() + Duration::from_secs(10);
    while ws.tmux_state.tmux_pane_owners.len() < 3 {
        assert!(Instant::now() < deadline, "the second window never mapped");
        for line in send(&ws, "list-panes") {
            if let Some(pane) = line.strip_prefix('%').and_then(|p| p.parse::<u64>().ok())
                && !ws.tmux_state.tmux_pane_owners.contains_key(&pane)
            {
                send(&ws, &format!("refresh-client -t %{pane} -C 80x24"));
            }
        }
        ws.check_mux_notifications();
        std::thread::sleep(Duration::from_millis(50));
    }
    ws.tab_manager.switch_to(tab);

    let focused = ws.focused_mux_pane_from_native().expect("focused mux pane");
    let job = ws
        .tmux_state
        .tab_tmux_pane_ids(tab)
        .into_iter()
        .find(|p| *p != focused)
        .expect("the tab's other pane");
    send(&ws, &format!("send-keys -t %{job} -l 'sleep 100'"));
    send(&ws, &format!("send-keys -t %{job} Enter"));
    wait_for_foreground(&ws, job, "sleep");
    Some((ws, proxy, daemon, proxy_path, job))
}

/// Whether the daemon reports `pane`'s foreground command (`pane-info`'s
/// `cmd=` token, absent before the 0.57 core). Polled: the token is also
/// absent until the pane's shell process is up, so one early probe would
/// skip the test on a daemon that has it.
fn daemon_reports_foreground(ws: &WindowState, pane: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let reply = send(ws, &format!("pane-info -t %{pane}"));
        if reply.iter().any(|l| l.contains(" cmd=")) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// [`attach_two_panes`] through a fresh [`StallProxy`]; `None` (sockets
/// removed) against a daemon without the `pane-info` foreground token.
fn proxied_attach(
    tag: &str,
) -> Option<(
    WindowState,
    StallProxy,
    std::path::PathBuf,
    std::path::PathBuf,
)> {
    let shells = ["bash", "zsh", "fish", "sh"].map(String::from).to_vec();
    let daemon = mux_tests::socket_path(&format!("{tag}-d"));
    let proxy_path = mux_tests::socket_path(&format!("{tag}-p"));
    mux_tests::spawn_daemon(&daemon);
    let proxy = StallProxy::spawn(&proxy_path, &daemon);
    let (ws, _) = attach_two_panes(tag, &proxy_path, &daemon, shells);
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");
    if !daemon_reports_foreground(&ws, target) {
        eprintln!("old daemon (no pane-info cmd token): nothing to confirm on");
        let _ = std::fs::remove_file(&daemon);
        let _ = std::fs::remove_file(&proxy_path);
        return None;
    }
    Some((ws, proxy, daemon, proxy_path))
}

/// Card 01a0f6bc76a576f09dbc3cd9d3bb640e: closing an attached TAB whose
/// daemon pane runs a job must raise the running-job confirmation, through
/// the same separate-connection check as a pane close. The old check read
/// the tab's own terminal — for an attached tab, the hidden local shell,
/// which never runs the job — so the close went straight on to
/// `kill-window` and ended `sleep 100` unasked. The job sits in the tab's
/// unfocused pane (a tab close ends every pane in it), and one slow daemon
/// command is in flight on the attached connection.
#[cfg(unix)]
#[test]
fn closing_an_attached_tab_with_a_daemon_job_asks_first_past_a_slow_command() {
    use crate::app::tmux_handler::tmux_state::MuxJob;
    let Some((mut ws, proxy, daemon, proxy_path, _job)) =
        proxied_tab_with_a_background_job("mux-tabclose-stall")
    else {
        return;
    };

    proxy.arm("refresh-client -t %0", STALL);
    let stalled_at = Instant::now();
    let transport = ws.tmux_state.transport.as_ref().expect("transport");
    assert!(transport.submit_job(MuxJob::SeedPanes(vec![0])));
    std::thread::sleep(Duration::from_millis(150));

    let started = Instant::now();
    assert!(!ws.close_current_tab(), "the close must not end the window");
    let took = started.elapsed();
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "a daemon job in the tab must raise the confirmation"
    );
    assert_eq!(ws.overlay_ui.close_confirmation_ui.command_name(), "sleep");
    assert!(
        took < STALL / 2,
        "the close check waited out the in-flight command ({took:?})"
    );

    pump_until(&mut ws, stalled_at + STALL + Duration::from_millis(500));
    assert_eq!(
        daemon_window_count(&ws),
        2,
        "the job's window must survive the close"
    );
    let _ = std::fs::remove_file(&daemon);
    let _ = std::fs::remove_file(&proxy_path);
}

/// The hung-daemon half for a tab close: a job check the daemon cannot
/// answer in time holds the close (fail-closed) instead of killing the
/// window, and the event loop is back well before the daemon answers.
#[cfg(unix)]
#[test]
fn a_tab_job_check_the_daemon_cannot_answer_in_time_holds_the_close() {
    let Some((mut ws, proxy, daemon, proxy_path, _job)) =
        proxied_tab_with_a_background_job("mux-tabclose-hung")
    else {
        return;
    };

    proxy.arm("pane-info", STALL);
    let stalled_at = Instant::now();
    assert!(!ws.close_current_tab(), "the close must not end the window");
    let took = stalled_at.elapsed();
    assert!(
        took < STALL / 2,
        "the close blocked the event loop on an unanswered check ({took:?})"
    );
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "a check that could not answer must hold the close for confirmation"
    );

    pump_until(&mut ws, stalled_at + STALL + Duration::from_millis(500));
    assert_eq!(
        daemon_window_count(&ws),
        2,
        "nothing is killed while the check is unanswered"
    );
    let _ = std::fs::remove_file(&daemon);
    let _ = std::fs::remove_file(&proxy_path);
}

/// The confirmed close of the session's LAST attached tab still takes the
/// last-tab gate (UX.md M1): "Close Anyway" on the running-job dialog
/// answers for the job, not for the session. The confirmed tab close used
/// to go straight to the immediate close, which for the only daemon window
/// is a `kill-window` that empties and deletes the session.
#[test]
fn confirming_a_job_on_the_last_attached_tab_still_asks_about_the_session() {
    let shells = ["bash", "zsh", "fish", "sh"].map(String::from).to_vec();
    let (mut ws, path) = attached_two_pane_state("mux-tabclose-last", shells);
    let target = ws.focused_mux_pane_from_native().expect("focused mux pane");
    if !daemon_reports_foreground(&ws, target) {
        eprintln!("old daemon (no pane-info cmd token): nothing to confirm on");
        let _ = std::fs::remove_file(&path);
        return;
    }
    send(&ws, &format!("send-keys -t %{target} -l 'sleep 100'"));
    send(&ws, &format!("send-keys -t %{target} Enter"));
    wait_for_foreground(&ws, target, "sleep");

    assert!(
        !ws.close_current_tab(),
        "the close must wait for the dialog"
    );
    assert!(
        ws.overlay_ui.close_confirmation_ui.is_visible(),
        "the job is asked about first"
    );
    assert!(!ws.overlay_ui.mux_last_tab_ui.is_visible());

    ws.overlay_ui.close_confirmation_ui.hide();
    assert!(!ws.close_current_tab_confirmed());
    assert!(
        ws.overlay_ui.mux_last_tab_ui.is_visible(),
        "confirming the job must still ask before ending the session"
    );
    assert_eq!(
        daemon_window_count(&ws),
        1,
        "the session's only window survives"
    );
    let _ = std::fs::remove_file(&path);
}
