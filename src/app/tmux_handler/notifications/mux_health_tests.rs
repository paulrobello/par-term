//! par-mux daemon health tests: fire-and-forget sends never block on a silent
//! daemon, which surfaces an unresponsive toast once per episode.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{connect, manners_state, socket_path, spawn_daemon, spawn_silent_daemon};
use super::mux_test_seams::drain_send_worker;
use super::mux_transport::{DaemonHealthSignal, MuxHealth};
use crate::app::tmux_handler::tmux_state::TmuxTransport;
use std::io;
use std::time::{Duration, Instant};

/// The hung-daemon card's freeze repro, inverted: with the daemon
/// silent, fire-and-forget sends (the per-keystroke seam) must return
/// immediately instead of each burning the core client's 10 s reply
/// timeout on the event loop.
#[test]
fn fire_and_forget_sends_never_block_on_a_silent_daemon() {
    let path = socket_path("silent-no-block");
    spawn_silent_daemon(&path);
    let transport = connect(&path);

    let started = Instant::now();
    for _ in 0..20 {
        transport
            .send_command_no_wait("send-keys -t %0 x")
            .expect("queueing must not block or fail");
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "20 queued sends took {:?} — the event loop would freeze",
        started.elapsed()
    );

    let _ = std::fs::remove_file(&path);
}

/// The hung-daemon card's toast criterion: a keystroke queued to a
/// silent daemon must surface the unresponsive toast through the real
/// per-frame drain (`check_mux_notifications`) within the 1 s
/// deadline — health check to `show_toast`, not a unit shortcut.
#[test]
fn silent_daemon_surfaces_unresponsive_toast_within_1s() {
    let path = socket_path("silent-toast");
    spawn_silent_daemon(&path);
    let transport = connect(&path);

    let mut ws = manners_state();
    ws.tmux_state.transport = Some(Box::new(transport));

    // The user's first keystroke into the mux pane.
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_command_no_wait("send-keys -t %0 x")
        .expect("queueing must not block");

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut toast_seen = None;
    while Instant::now() < deadline {
        ws.check_mux_notifications();
        if let Some(message) = ws.last_toast_text().map(str::to_string) {
            toast_seen = Some(message);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let message = toast_seen.expect("unresponsive toast within 1 s");
    assert!(
        message.contains("not responding"),
        "toast must say the daemon is not responding, got: {message}"
    );

    let _ = std::fs::remove_file(&path);
}

/// A healthy daemon never trips the health toast: the in-flight
/// window closes in milliseconds and `poll_event` stays `None`.
#[test]
fn healthy_daemon_reports_no_health_events() {
    let path = socket_path("healthy-quiet");
    spawn_daemon(&path);
    let transport = connect(&path);

    let queued = Instant::now();
    transport
        .send_command_no_wait("send-keys -t %0 x")
        .expect("queue");
    // Wait for the reply first (a loaded machine can keep an answered
    // send in flight past the 800 ms threshold; a reply that never
    // comes fails the drain at its deadline). Then hold until 1.2 s
    // after the send: an answered send whose in-flight mark lingered
    // would have crossed the threshold by then. Nothing else is queued,
    // so the hold cannot trip on load.
    drain_send_worker(&transport);
    if let Some(rest) = Duration::from_millis(1200).checked_sub(queued.elapsed()) {
        std::thread::sleep(rest);
    }
    for _ in 0..3 {
        assert_eq!(
            transport.daemon_health_event(),
            None,
            "a daemon that answers must not be flagged unresponsive"
        );
    }

    let _ = std::fs::remove_file(&path);
}

/// The health state machine's transitions, driven directly: threshold
/// crossing fires once (not per poll), success clears the episode,
/// and an observed reply timeout flags without needing a second send.
#[test]
fn mux_health_fires_once_per_episode_and_recovers() {
    let health = MuxHealth::default();
    let threshold = Duration::from_millis(50);

    health.send_started();
    assert_eq!(health.poll_event(threshold), None, "under threshold");
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(
        health.poll_event(threshold),
        Some(DaemonHealthSignal::Unresponsive)
    );
    assert_eq!(
        health.poll_event(threshold),
        None,
        "the toast must not re-fire every poll"
    );

    health.send_finished(&Ok(Vec::new()));
    assert_eq!(
        health.poll_event(threshold),
        Some(DaemonHealthSignal::Recovered)
    );
    assert_eq!(health.poll_event(threshold), None);

    let timeout = io::Error::new(io::ErrorKind::TimedOut, "no reply block within 10s");
    health.send_finished(&Err(timeout));
    assert_eq!(
        health.poll_event(threshold),
        Some(DaemonHealthSignal::Unresponsive),
        "an observed reply timeout flags without a new send"
    );
}
