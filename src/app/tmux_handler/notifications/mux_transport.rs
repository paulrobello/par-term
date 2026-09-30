//! The par-mux daemon transport: a [`MuxSessionClient`] behind the
//! [`TmuxTransport`] seam, with the send worker and liveness tracking the
//! poll loop reports. Split from `mux.rs`, which owns the app wiring
//! (attach/detach lifecycle, input routing, client capability pushes).

use crate::app::tmux_handler::tmux_state::{MuxJob, MuxJobResult, TmuxTransport};
use par_term_mux::MuxSessionClient;
use par_term_tmux::TmuxPaneId;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long an in-flight fire-and-forget send may stay unanswered before
/// the poll loop reports the daemon unresponsive. Must stay comfortably
/// under the toast deadline the hung-daemon card requires (1 s) and well
/// under the core client's 10 s reply timeout it exists to pre-empt.
const DAEMON_UNRESPONSIVE_AFTER: Duration = Duration::from_millis(800);

/// How long a reply-needing `send_command` waits for the send worker to
/// release the client before failing fast. A worker that holds the client
/// this long is itself stuck on a daemon reply — queueing behind it would
/// freeze the caller for the worker's remaining timeout.
const SEND_LOCK_WAIT: Duration = Duration::from_millis(250);

/// The fire-and-forget outbox depth. Bounded so a wedged daemon cannot
/// accrue an unbounded backlog (typing and paste chunks enqueue far faster
/// than a 10 s reply timeout drains); overflow drops commands and counts
/// them for the health event.
const OUTBOX_CAPACITY: usize = 512;

/// One unit of send-worker work: a fire-and-forget command, or a
/// multi-command [`MuxJob`] whose result the event loop collects later.
enum WorkItem {
    Send(String),
    Job(MuxJob),
}

/// The daemon transport: a par-mux client behind the [`TmuxTransport`]
/// seam. Interior mutability because the routing hooks that reach it
/// (`send_input_via_tmux`, `notify_tmux_of_resize`) hold `&WindowState`.
///
/// Sends split by need: commands whose reply nobody reads (keystrokes,
/// size pushes, pastes) are queued to the send worker, which alone waits
/// on daemon replies — the event loop never does. Reply-needing commands
/// still run inline under a bounded lock, and the worker shares the
/// client behind the same mutex, so replies stay strictly ordered.
/// Discovery queries ([`MuxJob`]) also run on the worker; their results
/// wait in `job_results` for the event loop's drain.
pub(crate) struct MuxTransport {
    client: Arc<Mutex<MuxSessionClient>>,
    outbox: SyncSender<WorkItem>,
    health: Arc<MuxHealth>,
    job_results: Mutex<Receiver<MuxJobResult>>,
    /// Inline sends currently waiting for the client lock. A job yields to
    /// them before each of its commands (see [`run_job`]).
    inline_waiters: Arc<AtomicUsize>,
}

impl MuxTransport {
    pub(crate) fn new(client: MuxSessionClient) -> Self {
        let client = Arc::new(Mutex::new(client));
        let health = Arc::new(MuxHealth::default());
        let inline_waiters = Arc::new(AtomicUsize::new(0));
        let (outbox, inbox) = sync_channel::<WorkItem>(OUTBOX_CAPACITY);
        let (results_tx, results_rx) = channel();
        spawn_send_worker(
            Arc::clone(&client),
            inbox,
            Arc::clone(&health),
            results_tx,
            Arc::clone(&inline_waiters),
        );
        Self {
            client,
            outbox,
            health,
            job_results: Mutex::new(results_rx),
            inline_waiters,
        }
    }

    /// Connect to a daemon at `path`, spawning one when no live server
    /// owns it (losing the spawn race talks to the winner's daemon).
    /// Test entry: the app runtime reaches the daemon through
    /// [`Self::connect_or_spawn`] by session name; the wiring test binds
    /// an in-process server at an explicit path.
    #[cfg(test)]
    pub(crate) fn connect_or_spawn_at(path: &std::path::Path) -> io::Result<Self> {
        Ok(Self::new(MuxSessionClient::connect_or_spawn_at(path)?))
    }

    /// The wrapped client, locked. Callers run short command sequences
    /// (attach, resync, probes); the send worker interleaves between
    /// calls, never inside one, because each call releases its guard
    /// before the next borrows.
    pub(crate) fn client(&self) -> MutexGuard<'_, MuxSessionClient> {
        self.client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Lock the client for an inline reply-needing send, giving up (and
    /// failing fast) once the send worker has held it past
    /// [`SEND_LOCK_WAIT`] — the queue-behind-a-hung-worker freeze guard.
    fn lock_for_send(&self) -> io::Result<MutexGuard<'_, MuxSessionClient>> {
        let deadline = Instant::now() + SEND_LOCK_WAIT;
        self.inline_waiters.fetch_add(1, Ordering::SeqCst);
        let _waiting = WaiterGuard(&self.inline_waiters);
        loop {
            match self.client.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    return Ok(poisoned.into_inner());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::other(
                            "mux send worker still holds the client — daemon unresponsive?",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

/// Decrements the inline-waiter count when an inline lock attempt ends,
/// whether it got the lock or gave up.
struct WaiterGuard<'a>(&'a AtomicUsize);

impl Drop for WaiterGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The send worker: the only thread that waits on a daemon reply for
/// fire-and-forget commands and discovery jobs. Each command still runs
/// the full `send` (write + reply wait), so reply blocks stay consumed and
/// paired in order — the queue changes WHERE the 10 s wait happens, never
/// the protocol. Exits when the transport drops and closes the outbox.
fn spawn_send_worker(
    client: Arc<Mutex<MuxSessionClient>>,
    inbox: Receiver<WorkItem>,
    health: Arc<MuxHealth>,
    results: Sender<MuxJobResult>,
    inline_waiters: Arc<AtomicUsize>,
) {
    std::thread::spawn(move || {
        let send = |command: &str| worker_send(&client, &health, command);
        // A job's commands are read-only queries, so letting an inline
        // send run between two of them reorders nothing that matters.
        // Without this yield the loop's 5 ms lock poll rarely lands in the
        // gap between two job commands, and a long job starves it past
        // SEND_LOCK_WAIT. The yield is bounded by the same wait: an inline
        // send gives up at that point anyway.
        let job_send = |command: &str| {
            let deadline = Instant::now() + SEND_LOCK_WAIT;
            while inline_waiters.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            send(command)
        };
        while let Ok(item) = inbox.recv() {
            match item {
                WorkItem::Send(command) => {
                    let _ = send(&command);
                }
                WorkItem::Job(job) => {
                    if results.send(run_job(job, &job_send)).is_err() {
                        return;
                    }
                }
            }
        }
    });
}

/// One worker-side command. The client lock is held for this command only,
/// never across a job: the event loop's inline sends wait at most
/// `SEND_LOCK_WAIT`, so a job holding the lock between its commands would
/// make them fail for the job's whole duration.
fn worker_send(
    client: &Mutex<MuxSessionClient>,
    health: &MuxHealth,
    command: &str,
) -> io::Result<Vec<String>> {
    let mut client = client.lock().unwrap_or_else(|p| p.into_inner());
    health.send_started();
    let result = client.send(command);
    health.send_finished(&result);
    result
}

/// Run a [`MuxJob`] against `send` and package what it learned. Every
/// command a job sends is a read-only query; anything that changes daemon
/// state goes through the ordered `Send` queue instead.
fn run_job(job: MuxJob, send: &dyn Fn(&str) -> io::Result<Vec<String>>) -> MuxJobResult {
    match job {
        MuxJob::DiscoverWindow(window) => {
            let mut panes = Vec::new();
            if let Ok(listed) = send("list-panes") {
                for pane in listed
                    .iter()
                    .filter_map(|l| l.trim().strip_prefix('%')?.parse::<TmuxPaneId>().ok())
                {
                    if let Ok(info) = send(&format!("pane-info -t %{pane}"))
                        && let Some((w, size)) = info.first().and_then(|l| parse_pane_info(l))
                        && w == window
                    {
                        panes.push((pane, size));
                    }
                }
            }
            MuxJobResult::WindowPanes { window, panes }
        }
        MuxJob::SeedPanes(panes) => MuxJobResult::PaneSeeds(
            panes
                .into_iter()
                .map(|pane| (pane, send(&format!("refresh-client -t %{pane}"))))
                .collect(),
        ),
    }
}

/// The window a `pane-info` reply (`%N @W COLSxROWS [cmd=…]`) places the
/// pane in, with the pane's grid size.
pub(super) fn parse_pane_info(line: &str) -> Option<(par_term_tmux::TmuxWindowId, (u16, u16))> {
    let mut fields = line.split_whitespace().skip(1);
    let window = fields.next()?.strip_prefix('@')?.parse().ok()?;
    let (cols, rows) = fields.next()?.split_once('x')?;
    Some((window, (cols.parse().ok()?, rows.parse().ok()?)))
}

/// Daemon liveness as the poll loop needs it: whether the send currently
/// in flight has gone unanswered past the threshold (or the worker saw a
/// reply timeout), with per-episode toast dedup so the signal fires once
/// per hang and once per recovery — never per frame.
#[derive(Default)]
pub(crate) struct MuxHealth {
    state: Mutex<MuxHealthState>,
}

#[derive(Default)]
struct MuxHealthState {
    /// When the worker started the send still in flight, if one is.
    started: Option<Instant>,
    /// The last finished send timed out (any success clears it).
    timed_out: bool,
    /// A toast is already on screen for this episode.
    toast_shown: bool,
    /// Commands dropped because the outbox was full.
    dropped: u64,
}

/// The transition [`MuxHealth::poll_event`] reports.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DaemonHealthSignal {
    Unresponsive,
    Recovered,
}

impl MuxHealth {
    /// Worker hook: a send just left the event loop toward the daemon.
    pub(crate) fn send_started(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.started = Some(Instant::now());
    }

    /// Worker hook: the send completed. A timeout marks the daemon
    /// unresponsive until some later send succeeds; anything else (reply
    /// or non-timeout error) proves liveness.
    pub(crate) fn send_finished(&self, result: &io::Result<Vec<String>>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.started = None;
        state.timed_out = matches!(
            result,
            Err(e) if e.kind() == io::ErrorKind::TimedOut
        );
    }

    /// Outbox overflow bookkeeping for the unresponsive toast.
    pub(crate) fn count_dropped(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.dropped += 1;
    }

    /// The poll-loop view: `Some(Unresponsive)` on the first frame a send
    /// has been unanswered past `threshold` (or a timeout was observed),
    /// `Some(Recovered)` on the first frame a flagged daemon answers
    /// again, `None` otherwise.
    pub(crate) fn poll_event(&self, threshold: Duration) -> Option<DaemonHealthSignal> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let hung = state.timed_out
            || state
                .started
                .is_some_and(|started| started.elapsed() >= threshold);
        match (hung, state.toast_shown) {
            (true, false) => {
                state.toast_shown = true;
                Some(DaemonHealthSignal::Unresponsive)
            }
            (false, true) => {
                state.toast_shown = false;
                Some(DaemonHealthSignal::Recovered)
            }
            _ => None,
        }
    }
}

impl TmuxTransport for MuxTransport {
    fn drain(
        &self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
        // A worker send mid-reply-wait holds the lock; that is exactly the
        // hung case, where no notifications exist to drain anyway. Skip
        // the poll rather than queue behind it on the event loop.
        let Ok(mut client) = self.client.try_lock() else {
            return (Vec::new(), false);
        };
        client.drain_core_notifications()
    }

    fn send_command(&self, command: &str) -> io::Result<Vec<String>> {
        let mut client = self.lock_for_send()?;
        client.send(command)
    }

    fn send_command_no_wait(&self, command: &str) -> io::Result<()> {
        match self.outbox.try_send(WorkItem::Send(command.to_string())) {
            Ok(()) => Ok(()),
            // A full outbox means the daemon wedged while input kept
            // arriving: drop the command (the unresponsive toast says why)
            // rather than block the event loop or grow without bound.
            Err(TrySendError::Full(_)) => {
                self.health.count_dropped();
                Ok(())
            }
            // Worker gone (transport being dropped): fall back inline.
            Err(TrySendError::Disconnected(_)) => self.send_command(command).map(|_| ()),
        }
    }

    fn daemon_health_event(&self) -> Option<String> {
        let dropped = self
            .health
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .dropped;
        self.health
            .poll_event(DAEMON_UNRESPONSIVE_AFTER)
            .map(|signal| match signal {
                DaemonHealthSignal::Unresponsive => {
                    if dropped > 0 {
                        format!(
                            "par-mux daemon not responding — input dropped ({dropped} commands)"
                        )
                    } else {
                        "par-mux daemon not responding — input queued until it recovers".into()
                    }
                }
                DaemonHealthSignal::Recovered => "par-mux daemon responding again".into(),
            })
    }

    fn submit_job(&self, job: MuxJob) -> bool {
        self.outbox.try_send(WorkItem::Job(job)).is_ok()
    }

    fn take_job_results(&self) -> Vec<MuxJobResult> {
        let rx = self
            .job_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rx.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::mux::tests::{connect, socket_path, spawn_daemon};
    use crate::app::tmux_handler::tmux_state::{MuxJob, TmuxTransport};

    /// Inline reply-needing sends must not wait out a whole off-loop job.
    /// A job's commands run back to back on the worker; without priority
    /// the event loop's bounded lock wait (`SEND_LOCK_WAIT`) rarely lands
    /// in the gap between two of them, and the send fails as if the daemon
    /// were hung — which silently skips the close-confirm `pane-info` probe
    /// and fails a split made right after a new window arrives.
    #[test]
    fn inline_sends_overtake_a_long_off_loop_job() {
        let path = socket_path("job-priority");
        spawn_daemon(&path);
        let transport = connect(&path);
        // Far longer than SEND_LOCK_WAIT even on a fast machine: each
        // command is a full daemon round-trip.
        assert!(transport.submit_job(MuxJob::SeedPanes(vec![0; 200_000])));
        std::thread::sleep(std::time::Duration::from_millis(20));
        let started = std::time::Instant::now();
        for i in 0..40 {
            transport
                .send_command("list-sessions")
                .unwrap_or_else(|e| panic!("inline send {i} starved behind the job: {e}"));
        }
        assert!(
            transport.take_job_results().is_empty(),
            "the job must still be running, or the test proves nothing (took {:?})",
            started.elapsed()
        );
        let _ = std::fs::remove_file(&path);
    }
}
