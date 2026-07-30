//! A panic boundary that preserves the user's tabs.
//!
//! # Why this exists
//!
//! par-term ships reachable panics. Six of them — column indices used as UTF-8
//! byte offsets — were fixed the same day this module was written, and they were
//! reachable with an emoji, a CJK character or an accented letter. When one
//! fires, the process dies and takes every window's tab list, working directories
//! and split layout with it, because the only session save runs from
//! `WindowManager::close_window` on a clean exit.
//!
//! This module narrows that loss. It does **not** make par-term survive a panic.
//!
//! # Design: snapshot, not live serialization
//!
//! A panic hook runs on the panicking thread, before unwinding starts, with the
//! program in an unknown state. Serializing the live `WindowManager` from there
//! is not an option and would not be safe if it were:
//!
//! - The hook is a `'static` closure. It cannot hold a reference to the
//!   `WindowManager`, which lives on the event-loop thread's stack; reaching it
//!   would require a global raw pointer aliasing `&mut self`.
//! - The panic may have happened *mid-mutation* of the very state we would walk.
//!   A `Vec<Tab>` observed between a `set_len` and its element writes serializes
//!   garbage or faults.
//! - `capture_session` takes locks and calls into winit. Both can block or
//!   re-panic on a broken thread.
//!
//! So the event loop **publishes** a snapshot at a known-good point — a point
//! where no par-term data structure is mid-update — and the snapshot is already
//! serialized to YAML at that moment. The hook's entire job is then to copy a
//! `String` that is guaranteed self-consistent to a file. It runs no serde code,
//! walks no live structure and takes no par-term lock.
//!
//! Nothing is preserved until something calls
//! `WindowManager::publish_crash_snapshot` — the hook can only write what has
//! been published, and reports "no snapshot to write" otherwise. See that
//! method's own documentation for where the call belongs.
//!
//! The cost is staleness: the recovered session is as old as the last publish
//! (see `WindowManager::publish_crash_snapshot`). Session state changes rarely —
//! opening a tab, changing directory, moving a window — so a few seconds of
//! staleness costs at most one tab. Losing every tab is the alternative.
//!
//! # What this covers, and what it does not
//!
//! Covered — the crash file is written:
//!
//! - An ordinary `panic!`, `unwrap`, `expect`, slice-index or integer-overflow
//!   panic on the event-loop thread, including one raised deep inside a callback.
//!   This is the class the audit found reachable from keyboard input.
//! - A panic that will subsequently `abort` because it tries to unwind through a
//!   foreign (Objective-C, C) frame. The hook runs *before* unwinding begins, so
//!   it still gets its save. A `catch_unwind` boundary would not.
//! - A build compiled with `panic = "abort"`, for the same reason.
//!
//! **Not** covered — the crash file is not written, or may be incomplete:
//!
//! - **Aborts that never run a hook**: `SIGSEGV`, `SIGBUS`, stack overflow,
//!   `SIGKILL`, a failed allocation, or a double panic (a panic raised while a
//!   panic is already being processed aborts inside the runtime before the hook
//!   is re-entered).
//! - **Non-main-thread panics.** A panicking spawned thread does not end the
//!   process under `panic = "unwind"`; tokio catches worker panics as a
//!   `JoinError` and the app keeps running. Writing a crash file for each of
//!   those would manufacture crash recoveries out of survivable faults. Such
//!   panics are logged only — which is itself new: today they vanish silently.
//! - **A panic taken inside the debug logger.** The save runs first and
//!   completes; the *report* step then re-enters the logger's mutex, which
//!   `parking_lot` does not make reentrant, and hangs. The session is on disk and
//!   the default hook has already written the panic to stderr by that point, but
//!   the process hangs instead of exiting. Removing this needs a `try_lock`
//!   logging entry point in `src/debug.rs`.
//! - **State that is not in the snapshot.** Scrollback, shell history, running
//!   processes and anything typed but not yet run are gone. What survives is
//!   window geometry, the tab list, per-tab working directories and titles, and
//!   split-pane layout — i.e. exactly what `SessionState` holds.
//! - **A panic before the first publish.** Nothing has been captured yet, so
//!   there is nothing to write. Startup panics lose the (empty) session.
//! - **The panic report does not outlive the next launch.** `src/debug.rs` opens
//!   the debug log with `truncate(true)`, so the report is wiped when par-term
//!   next starts — which after a crash is usually seconds later. The recovered
//!   *session* survives that; the diagnostic only survives if the user reads it,
//!   or still has the stderr the default hook wrote, before restarting.
//!
//! # The panic is not swallowed
//!
//! There is deliberately no `catch_unwind`. After a panic mid-render the GPU
//! queue, the terminal grid and every lock invariant are indeterminate;
//! `AssertUnwindSafe` would assert a property that is false. The hook saves,
//! chains to the previous hook so the panic is still reported, and returns — and
//! the process dies exactly as it would have.

use super::SessionState;
use arc_swap::ArcSwapOption;
use par_term_config::Config;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

/// Name of the file the hook writes.
///
/// Deliberately **not** `last_session.yaml`. A process that has just panicked
/// must never overwrite the good session file: if the snapshot were somehow bad,
/// doing so would destroy the state it is trying to rescue. A separate name also
/// makes "this session came back after a crash" detectable at startup without
/// storing a flag inside the file.
const CRASH_SESSION_FILE: &str = "crash_session.yaml";

/// Debug-log category for everything this module emits.
const CATEGORY: &str = "PANIC";

/// The last known-good session, already serialized to YAML.
///
/// `ArcSwapOption` rather than a `Mutex`: the hook must never block. A mutex
/// here could be held by the very thread that panicked, and the hook would
/// deadlock instead of saving — turning a crash into a hang.
static SNAPSHOT: ArcSwapOption<String> = ArcSwapOption::const_empty();

/// The thread that called [`install`] — the winit event-loop thread.
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Crash-file path, resolved once at install time so the hook never has to.
static CRASH_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Latch so the event-loop thread gets exactly one preservation attempt.
static SAVE_CLAIMED: AtomicBool = AtomicBool::new(false);

/// Reference point for [`LAST_PUBLISH_MS`]; set on the first publish.
static PUBLISH_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since [`PUBLISH_EPOCH`] at the last publish; `u64::MAX` = never.
static LAST_PUBLISH_MS: AtomicU64 = AtomicU64::new(u64::MAX);

/// What the hook managed to do with the snapshot. Reported to the debug log so a
/// crash report says whether the user's tabs were rescued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// The crash session file was written.
    Saved,
    /// No snapshot had been published yet (panic before the first publish, or
    /// `restore_session` is disabled so nothing is ever captured).
    NoSnapshot,
    /// An earlier panic already preserved the session; this one left it alone.
    AlreadySaved,
    /// A snapshot existed but the write failed (disk full, read-only config dir).
    WriteFailed,
}

impl std::fmt::Display for SaveOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Saved => "session snapshot written",
            Self::NoSnapshot => "no snapshot to write",
            Self::AlreadySaved => "session already preserved by an earlier panic",
            Self::WriteFailed => "snapshot write FAILED",
        })
    }
}

/// Path of the crash session file.
pub fn crash_session_path() -> PathBuf {
    CRASH_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| Config::config_dir().join(CRASH_SESSION_FILE))
}

/// Install the panic hook. Call once from `main`, after logging is initialized.
///
/// The calling thread is recorded as the event-loop thread: only a panic on that
/// thread triggers a save (see the module docs for why).
pub fn install() {
    // Idempotent. A second call would chain our own hook to itself and report
    // every panic twice.
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if !claim(&INSTALLED) {
        return;
    }

    MAIN_THREAD.get_or_init(|| std::thread::current().id());
    // Resolve the path now so the hook does no directory lookup and no `format!`
    // on a broken thread.
    CRASH_PATH.get_or_init(|| Config::config_dir().join(CRASH_SESSION_FILE));

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let on_event_loop = MAIN_THREAD.get() == Some(&std::thread::current().id());

        // Step 1 — preserve, before anything that can block. If the report step
        // below hangs (see the module docs on the logger mutex), the user's tabs
        // are already on disk.
        let outcome = on_event_loop.then(save_crash_session);

        // Step 2 — chain to the hook we replaced. The default hook writes the
        // panic message and, when `RUST_BACKTRACE` is set, a backtrace to stderr.
        // It runs before our own logging so the user always sees the panic even
        // if the debug log is unreachable.
        previous(info);

        // Step 3 — the same report into the debug log, which is where crash
        // reports are expected to be found and which survives the terminal
        // window closing.
        report(info, outcome);
    }));

    // Deliberately not "armed": whether a panic can actually preserve anything
    // depends on a snapshot having been published, which this function cannot
    // know. The hook reports the real outcome per panic instead.
    log::info!(
        "Panic hook installed; crash session file is {:?}",
        crash_session_path()
    );
}

/// Claim a one-shot latch. `true` for the first caller only.
///
/// The re-entrancy guard. It stops a *second* event-loop panic from overwriting
/// the crash file — on some winit backends a panic is caught out of the platform
/// run loop and the loop keeps dispatching, so the hook can genuinely run twice.
/// The first snapshot is the one to keep: everything after the first panic was
/// produced by a program in an unknown state.
///
/// It cannot guard a panic raised *inside* the hook. The runtime aborts on a
/// double panic before the hook is entered again, which is why every step of the
/// hook is kept infallible rather than relying on this.
///
/// Split out as a free function over an explicit latch so the behaviour is
/// testable without provoking a real panic.
fn claim(latch: &AtomicBool) -> bool {
    !latch.swap(true, Ordering::AcqRel)
}

/// Record a known-good session snapshot.
///
/// Serializes on the caller's thread — the whole point is that no serde code
/// runs inside the hook. Call only from a point where no par-term structure is
/// mid-update.
pub fn publish(state: &SessionState) {
    let yaml = match serde_yaml_ng::to_string(state) {
        Ok(yaml) => yaml,
        Err(e) => {
            // Keep the previous snapshot: a stale one beats none.
            log::warn!("Crash guard: failed to serialize session snapshot: {e}");
            return;
        }
    };
    SNAPSHOT.store(Some(std::sync::Arc::new(yaml)));

    let epoch = PUBLISH_EPOCH.get_or_init(Instant::now);
    LAST_PUBLISH_MS.store(epoch.elapsed().as_millis() as u64, Ordering::Release);
}

/// Whether `min_interval` has elapsed since the last [`publish`].
///
/// `true` when nothing has been published yet.
pub fn snapshot_is_due(min_interval: Duration) -> bool {
    let last = LAST_PUBLISH_MS.load(Ordering::Acquire);
    if last == u64::MAX {
        return true;
    }
    let Some(epoch) = PUBLISH_EPOCH.get() else {
        return true;
    };
    epoch.elapsed().as_millis() as u64 >= last.saturating_add(min_interval.as_millis() as u64)
}

/// Write the published snapshot to the crash file. Called only from the hook.
fn save_crash_session() -> SaveOutcome {
    // Check for a snapshot *before* claiming the latch. A panic that has nothing
    // to save must not consume the one attempt: winit catches and re-raises
    // panics out of the platform run loop on some backends, so an early panic —
    // one raised before the event loop ever published — can precede the panic
    // that actually has the user's tabs open. Burning the latch there would lose
    // exactly the case this module exists for.
    let Some(yaml) = SNAPSHOT.load_full() else {
        return SaveOutcome::NoSnapshot;
    };
    if !claim(&SAVE_CLAIMED) {
        return SaveOutcome::AlreadySaved;
    }
    write_crash_session(&crash_session_path(), &yaml)
}

/// The file-writing half of [`save_crash_session`], with the path injected.
///
/// Goes through `atomic_save` — the same staged-write-then-rename path every
/// other par-term save uses. Writing from a panicking process is exactly the
/// case a truncating write gets wrong: an interrupted `write` leaves a partial
/// file, and every session loader reads a partial file back as "no saved
/// session", so the user loses the data with no diagnostic at all.
fn write_crash_session(path: &Path, yaml: &str) -> SaveOutcome {
    match crate::atomic_save::save_string_atomic(path, yaml) {
        Ok(()) => SaveOutcome::Saved,
        Err(_) => SaveOutcome::WriteFailed,
    }
}

/// Extract a panic payload without allocating.
fn payload_str<'a>(info: &'a PanicHookInfo<'_>) -> &'a str {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "<non-string panic payload>"
    }
}

/// Write the panic to the debug log.
///
/// `outcome` is `None` when no save was attempted (a panic off the event-loop
/// thread, or a second panic after the latch was claimed).
fn report(info: &PanicHookInfo<'_>, outcome: Option<SaveOutcome>) {
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>");
    let location = info.location();
    let (file, line, column) = location
        .map(|l| (l.file(), l.line(), l.column()))
        .unwrap_or(("<unknown>", 0, 0));

    // R-18: one call for the pair of debug-log + `log` crate destinations.
    crate::debug_and_log_error!(
        CATEGORY,
        "PANIC on thread '{}' at {}:{}:{}: {}",
        thread_name,
        file,
        line,
        column,
        payload_str(info)
    );

    match outcome {
        Some(outcome) => log::error!(
            "Panic boundary: {} ({:?}). Restart par-term to recover the session.",
            outcome,
            crash_session_path()
        ),
        None => log::error!(
            "Panic boundary: no save attempted — this panic is off the event-loop \
             thread, and a spawned thread's panic does not end the process"
        ),
    }

    // Opt-in via RUST_BACKTRACE, matching the default hook. `force_capture`
    // would allocate and symbolize on every panic, which is the opposite of what
    // a hook on a broken thread should do.
    let backtrace = std::backtrace::Backtrace::capture();
    if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
        log::error!("PANIC backtrace:\n{backtrace}");
    }
}

/// Load the crash session file and delete it.
///
/// Returns `None` when there is no crash file, when it holds no windows, or when
/// it cannot be parsed. The file is removed in every case: a crash file that
/// fails to parse must not be retried on every subsequent launch.
///
/// A `Some` always carries at least one window, so a caller can treat it as
/// strictly better than the normal session file and fall back cleanly otherwise.
pub fn take_crash_session() -> Option<SessionState> {
    take_crash_session_from(&crash_session_path())
}

/// [`take_crash_session`] with the path injected, for tests.
fn take_crash_session_from(path: &Path) -> Option<SessionState> {
    if !path.exists() {
        return None;
    }

    let loaded = super::storage::load_session_from(path.to_path_buf());

    if let Err(e) = std::fs::remove_file(path) {
        log::warn!("Failed to remove crash session file {path:?}: {e}");
    }

    match loaded {
        // A window-less crash file restores nothing, so report it as absent and
        // let the caller fall back to the normal session file. `publish` never
        // writes one; a hand-edited or truncated file still can.
        Ok(session) => session.filter(|s| !s.windows.is_empty()),
        Err(e) => {
            log::warn!("Crash session file {path:?} could not be parsed, discarding: {e:#}");
            None
        }
    }
}

/// Drop the snapshot and remove any crash file. Call on a clean exit.
///
/// Without this a crash file written during one run would still be on disk after
/// the next clean shutdown, and the launch after that would announce a crash
/// recovery that never happened.
///
/// Refuses to delete a crash file that *this* run's panic hook wrote. Reaching a
/// clean exit is not proof no panic happened: winit catches panics out of the
/// platform run loop on some backends, so `EventLoop::run_app` can return
/// normally after the hook has already preserved the session. Deleting the file
/// there would destroy the rescue on exactly the path this module exists for.
pub fn disarm() {
    SNAPSHOT.store(None);
    disarm_at(&crash_session_path(), SAVE_CLAIMED.load(Ordering::Acquire));
}

/// [`disarm`] with the path and the "a panic was preserved" flag injected.
fn disarm_at(path: &Path, panic_preserved: bool) {
    if panic_preserved {
        log::warn!(
            "Clean exit after a preserved panic: keeping {path:?} so the next \
             launch can recover the session"
        );
        return;
    }

    if path.exists()
        && let Err(e) = std::fs::remove_file(path)
    {
        log::warn!("Failed to remove crash session file {path:?} on clean exit: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionTab, SessionWindow};
    use par_term_config::snapshot_types::TabSnapshot;
    use tempfile::tempdir;

    fn sample_session(cwd: &str) -> SessionState {
        SessionState {
            saved_at: "2026-07-30T12:00:00Z".to_string(),
            windows: vec![SessionWindow {
                position: (10, 20),
                size: (1280, 800),
                tabs: vec![SessionTab {
                    snapshot: TabSnapshot {
                        cwd: Some(cwd.to_string()),
                        title: "work".to_string(),
                        custom_color: None,
                        user_title: None,
                        custom_icon: None,
                    },
                    pane_layout: None,
                }],
                active_tab_index: 0,
                tmux_session_name: None,
            }],
        }
    }

    /// The latch the hook uses: the first caller gets the save, nobody else.
    #[test]
    fn claim_admits_exactly_one_caller() {
        let latch = AtomicBool::new(false);
        assert!(claim(&latch), "first claim must succeed");
        assert!(!claim(&latch), "second claim must be refused");
        assert!(!claim(&latch), "and stay refused");
    }

    /// The latch is a real atomic swap, not a load-then-store that two racing
    /// callers could both win.
    #[test]
    fn claim_admits_one_caller_under_contention() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;

        let latch = Arc::new(AtomicBool::new(false));
        let winners = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let latch = Arc::clone(&latch);
                let winners = Arc::clone(&winners);
                std::thread::spawn(move || {
                    if claim(&latch) {
                        winners.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("worker thread");
        }

        assert_eq!(winners.load(Ordering::Relaxed), 1);
    }

    /// The whole preservation path with the path injected: what the hook writes
    /// is a real session file that the normal loader accepts.
    #[test]
    fn hook_write_produces_a_loadable_session_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);

        let state = sample_session("/home/user/project");
        let yaml = serde_yaml_ng::to_string(&state).expect("serialize");

        assert_eq!(write_crash_session(&path, &yaml), SaveOutcome::Saved);
        assert!(path.exists());

        let loaded = crate::session::storage::load_session_from(path)
            .expect("load")
            .expect("some session");
        assert_eq!(loaded.windows.len(), 1);
        assert_eq!(loaded.windows[0].size, (1280, 800));
        assert_eq!(
            loaded.windows[0].tabs[0].snapshot.cwd.as_deref(),
            Some("/home/user/project")
        );
    }

    /// The crash file records working directories, so it lands at 0600 like
    /// every other par-term save regardless of umask.
    #[cfg(unix)]
    #[test]
    fn crash_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        let yaml = serde_yaml_ng::to_string(&sample_session("/tmp")).expect("serialize");

        assert_eq!(write_crash_session(&path, &yaml), SaveOutcome::Saved);

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    /// A write to an unwritable path is reported, not panicked on — the hook
    /// must stay infallible.
    #[test]
    fn write_failure_is_reported_not_panicked() {
        let temp = tempdir().expect("tempdir");
        // A directory where the file should be makes the rename fail.
        let path = temp.path().join("blocked");
        std::fs::create_dir(&path).expect("blocking directory");

        assert_eq!(
            write_crash_session(&path, "saved_at: x\nwindows: []\n"),
            SaveOutcome::WriteFailed
        );
    }

    #[test]
    fn take_consumes_the_crash_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        let yaml = serde_yaml_ng::to_string(&sample_session("/home/user/x")).expect("serialize");
        write_crash_session(&path, &yaml);

        let taken = take_crash_session_from(&path).expect("crash session");
        assert_eq!(
            taken.windows[0].tabs[0].snapshot.cwd.as_deref(),
            Some("/home/user/x")
        );
        assert!(
            !path.exists(),
            "crash file must be consumed, not left behind"
        );
    }

    #[test]
    fn take_returns_none_when_there_is_no_crash_file() {
        let temp = tempdir().expect("tempdir");
        assert!(take_crash_session_from(&temp.path().join("absent.yaml")).is_none());
    }

    /// A window-less crash file must read as "no crash session" so the caller
    /// falls back to the normal session file instead of restoring nothing.
    #[test]
    fn take_treats_a_window_less_crash_file_as_absent() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        std::fs::write(&path, "saved_at: '2026-07-30T12:00:00Z'\nwindows: []\n").expect("write");

        assert!(take_crash_session_from(&path).is_none());
        assert!(!path.exists());
    }

    /// A crash file that cannot be parsed is discarded rather than retried on
    /// every launch for the rest of time.
    #[test]
    fn take_discards_and_removes_a_corrupt_crash_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        std::fs::write(&path, "not: valid: yaml: [[[").expect("write");

        assert!(take_crash_session_from(&path).is_none());
        assert!(!path.exists(), "a corrupt crash file must still be removed");
    }

    #[test]
    fn disarm_removes_a_crash_file_left_by_an_earlier_run() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        std::fs::write(&path, "saved_at: x\nwindows: []\n").expect("write");

        disarm_at(&path, false);
        assert!(!path.exists());

        // Idempotent: a clean exit with no crash file must not error.
        disarm_at(&path, false);
    }

    /// The case that would silently undo the whole feature: winit can return
    /// from `run_app` after a panic the hook already preserved, and the clean-exit
    /// path must not then delete the rescue.
    #[test]
    fn disarm_keeps_a_crash_file_this_run_wrote() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);
        std::fs::write(&path, "saved_at: x\nwindows: []\n").expect("write");

        disarm_at(&path, true);
        assert!(
            path.exists(),
            "a crash file written by this run's panic hook must survive a later clean exit"
        );
    }

    /// The publish → snapshot → write chain over the real globals, plus the
    /// throttle that gates it.
    ///
    /// The only test that touches the process-wide `SNAPSHOT` and
    /// `LAST_PUBLISH_MS`; keeping it to one test is what makes the others
    /// order-independent under the default parallel runner.
    #[test]
    fn publishing_arms_the_hook_and_throttles_the_next_capture() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(CRASH_SESSION_FILE);

        // Nothing captured yet is always due, whatever the interval — a long
        // interval must not starve the very first capture.
        assert!(snapshot_is_due(Duration::from_secs(3600)) || SNAPSHOT.load().is_some());

        publish(&sample_session("/home/user/published"));

        let yaml = SNAPSHOT.load_full().expect("a snapshot was published");
        assert_eq!(write_crash_session(&path, &yaml), SaveOutcome::Saved);

        let loaded = crate::session::storage::load_session_from(path)
            .expect("load")
            .expect("some session");
        assert_eq!(
            loaded.windows[0].tabs[0].snapshot.cwd.as_deref(),
            Some("/home/user/published")
        );

        assert!(
            !snapshot_is_due(Duration::from_secs(3600)),
            "an hour-long interval must suppress a capture taken just now"
        );
        assert!(
            snapshot_is_due(Duration::ZERO),
            "a zero interval is always due"
        );
    }
}
