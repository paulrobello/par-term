//! Shared test seams for par-mux tests outside this module: a window
//! attached to an in-process daemon with N mapped daemon tabs, and the
//! condition waits every daemon-driven test uses.
//!
//! Daemon-driven tests wait on observable state, never on the clock: a
//! loaded machine (full-suite runs beside other builds) stretches every
//! daemon round-trip and shell start, so a wall-clock window that is ample
//! idle fails at random under load. The deadline below exists only to fail
//! a genuinely hung daemon.

use super::mux::tests::{manners_state, socket_path, spawn_daemon};
use super::mux_transport::SEND_LOCK_BUSY;
use crate::app::tmux_handler::tmux_state::{MuxJob, MuxJobResult, TmuxTransport};
use crate::app::window_state::WindowState;
use par_term_emu_core_rust::mux::MuxClient;
use std::time::{Duration, Instant};

/// How long a daemon-driven wait may take before the test fails. Generous
/// on purpose: it bounds a hung daemon, not a slow one.
pub(crate) const DAEMON_DEADLINE: Duration = Duration::from_secs(60);

/// Poll `cond` until it holds or [`DAEMON_DEADLINE`] passes; returns
/// whether it held. Callers that dump state on failure assert on this.
pub(crate) fn poll_until(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// [`poll_until`], failing the test with `what` at the deadline.
pub(crate) fn wait_until(what: &str, cond: impl FnMut() -> bool) {
    assert!(
        poll_until(cond),
        "{what}: not observed within {DAEMON_DEADLINE:?}"
    );
}

/// Inline sends that ride out the transport's lock-busy fail-fast.
pub(crate) trait SettledSend {
    /// `send_command`, retried while (and only while) it fails with
    /// [`SEND_LOCK_BUSY`]: that error is returned before anything is
    /// written, so a retry cannot repeat a command. Every other error,
    /// and a worker still busy at [`DAEMON_DEADLINE`], is returned.
    fn send_settled(&self, command: &str) -> std::io::Result<Vec<String>>;
}

impl<T: TmuxTransport + ?Sized> SettledSend for T {
    fn send_settled(&self, command: &str) -> std::io::Result<Vec<String>> {
        let mut result = None;
        poll_until(|| match self.send_command(command) {
            Err(e) if e.to_string() == SEND_LOCK_BUSY => {
                result = Some(Err(e));
                false
            }
            other => {
                result = Some(other);
                true
            }
        });
        result.expect("polled at least once")
    }
}

/// Wait until the transport's send worker is idle. Production actions that
/// send inline (split, the close-confirm probe, rename, new-window, kill)
/// fail fast while the worker still runs queued discovery, seeds, or
/// fire-and-forget sends, so a test settles the worker before one.
///
/// Off-loop jobs in flight are pumped to completion first (only then: a
/// pump also applies pending layouts, which a caller may not want yet); an
/// empty seed job then goes through the worker's FIFO behind every queued
/// send as a barrier. Its result is taken directly — the pump would
/// consume it — which is safe because no other job is in flight by then.
pub(crate) fn quiesce(ws: &mut WindowState) {
    wait_until("off-loop mux jobs land", || {
        !ws.tmux_state.mux_jobs_in_flight() || {
            ws.check_mux_notifications();
            !ws.tmux_state.mux_jobs_in_flight()
        }
    });
    drain_send_worker(ws.tmux_state.transport.as_deref().expect("transport"));
}

/// Wait until every command queued on `transport`'s send worker has been
/// answered: an empty seed job queued now completes only after them. The
/// caller must have no other job in flight (its result would be taken).
pub(crate) fn drain_send_worker(transport: &dyn TmuxTransport) {
    assert!(
        transport.submit_job(MuxJob::SeedPanes(Vec::new())),
        "the barrier job is queued"
    );
    let mut results = Vec::new();
    wait_until("the send worker drains its queue", || {
        results.extend(transport.take_job_results());
        !results.is_empty()
    });
    assert!(
        matches!(results.as_slice(), [MuxJobResult::PaneSeeds(seeds)] if seeds.is_empty()),
        "only the barrier was in flight ({} results)",
        results.len()
    );
}

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
            .send_settled("new-window")
            .expect("new-window");
    }
    wait_until("tabs mapped", || {
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            for pane in 0..tabs as u64 {
                let _ = t.send_command_no_wait(&format!("refresh-client -t %{pane} -C 80x24"));
            }
        }
        ws.check_tmux_notifications();
        ws.tab_manager.tab_count() >= tabs && ws.tmux_state.tmux_pane_owners.len() >= tabs
    });
    quiesce(&mut ws);
    (ws, path)
}
