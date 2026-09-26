//! The par-mux daemon transport: a [`MuxSessionClient`] behind the
//! [`TmuxTransport`] seam, with the send worker and liveness tracking the
//! poll loop reports. Split from `mux.rs`, which owns the app wiring
//! (attach/detach lifecycle, input routing, client capability pushes).

use crate::app::tmux_handler::tmux_state::TmuxTransport;
use par_term_mux::MuxSessionClient;
use std::io;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
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

/// The daemon transport: a par-mux client behind the [`TmuxTransport`]
/// seam. Interior mutability because the routing hooks that reach it
/// (`send_input_via_tmux`, `notify_tmux_of_resize`) hold `&WindowState`.
///
/// Sends split by need: commands whose reply nobody reads (keystrokes,
/// size pushes, pastes) are queued to the send worker, which alone waits
/// on daemon replies — the event loop never does. Reply-needing commands
/// still run inline under a bounded lock, and the worker shares the
/// client behind the same mutex, so replies stay strictly ordered.
pub(crate) struct MuxTransport {
    client: Arc<Mutex<MuxSessionClient>>,
    outbox: SyncSender<String>,
    health: Arc<MuxHealth>,
}

impl MuxTransport {
    pub(crate) fn new(client: MuxSessionClient) -> Self {
        let client = Arc::new(Mutex::new(client));
        let health = Arc::new(MuxHealth::default());
        let (outbox, inbox) = sync_channel::<String>(OUTBOX_CAPACITY);
        spawn_send_worker(Arc::clone(&client), inbox, Arc::clone(&health));
        Self {
            client,
            outbox,
            health,
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

/// The send worker: the only thread that waits on a daemon reply for
/// fire-and-forget commands. Each queued command still runs the full
/// `send` (write + reply wait), so reply blocks stay consumed and paired
/// in order — the queue changes WHERE the 10 s wait happens, never the
/// protocol. Exits when the transport drops and closes the outbox.
fn spawn_send_worker(
    client: Arc<Mutex<MuxSessionClient>>,
    inbox: Receiver<String>,
    health: Arc<MuxHealth>,
) {
    std::thread::spawn(move || {
        while let Ok(command) = inbox.recv() {
            let mut client = client.lock().unwrap_or_else(|p| p.into_inner());
            health.send_started();
            let result = client.send(&command);
            health.send_finished(&result);
        }
    });
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
        match self.outbox.try_send(command.to_string()) {
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
}
