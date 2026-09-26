//! Directory-watched snapshot store for agent-usage records.
//!
//! One [`UsageStore`] owns the records directory: it watches the directory
//! for changes (notify, native backend with a poll fallback — the same
//! two-backend shape as the config watcher in `par-term-config`), rescans on
//! event, and keeps the last [`UsageSnapshot`] for the status-bar widget and
//! the popup panel to read.
//!
//! The store is deliberately unkillable by filesystem state: a missing
//! directory yields an empty snapshot, a directory that appears late is
//! picked up on the next refresh tick, and a garbage `.json` file becomes an
//! entry in `snapshot.errors` while every other file still parses.

use crate::agent_usage::records::{self, AgentUsageRecord};
use notify::{Config as NotifyConfig, Event, PollWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// How long successive watcher events are coalesced before one signal.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Poll interval for the fallback watcher backend.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// The store's view of the records directory at the last rescan.
#[derive(Debug, Default)]
pub(crate) struct UsageSnapshot {
    /// Displayable records (parse-ok, ready, not hidden), filename order.
    pub records: Vec<AgentUsageRecord>,
    /// One line per unreadable file, e.g. `claude.json: parse failed`.
    pub errors: Vec<String>,
}

/// Watches the records directories and serves the last snapshot.
pub(crate) struct UsageStore {
    /// The records directories: the primary plus any configured extra dirs
    /// (e.g. a synced folder — v2 merge). None may exist yet.
    dirs: Vec<PathBuf>,
    /// Directory watcher; `None` until some directory exists.
    watcher: Option<Box<dyn Watcher + Send>>,
    /// Directories already added to the watcher.
    watched: HashSet<PathBuf>,
    /// Signals "something in a watched directory changed" after debouncing.
    event_rx: Option<Receiver<()>>,
    /// Debounce state shared with the watcher callbacks.
    last_event: Arc<Mutex<Option<Instant>>>,
    /// Latest rescan result.
    snapshot: UsageSnapshot,
    /// Agent ids the user hid (Task 5's config; empty until then).
    hidden: HashSet<String>,
    /// When the last rescan ran; `None` before the first.
    last_rescan: Option<Instant>,
}

impl UsageStore {
    /// Create a store over `dir`. Watches immediately when the directory
    /// exists; otherwise starts empty and attaches on a later
    /// [`UsageStore::tick`]/[`UsageStore::refresh_now`] once it appears.
    pub(crate) fn new(dir: PathBuf) -> Self {
        let mut store = Self {
            dirs: vec![dir],
            watcher: None,
            watched: HashSet::new(),
            event_rx: None,
            last_event: Arc::new(Mutex::new(None)),
            snapshot: UsageSnapshot::default(),
            hidden: HashSet::new(),
            last_rescan: None,
        };
        store.attach_watcher_for_new_dirs();
        store.rescan();
        store
    }

    /// Replace the extra records directories (after the primary). Same-set
    /// calls are a no-op, like `set_hidden` — this runs every frame with the
    /// config's list. A change re-arms the watcher and rescans.
    pub(crate) fn set_extra_dirs(&mut self, extra: Vec<PathBuf>) {
        let mut dirs = Vec::with_capacity(self.dirs.len().max(extra.len() + 1));
        dirs.push(self.dirs[0].clone());
        dirs.extend(extra);
        if dirs == self.dirs {
            return;
        }
        self.dirs = dirs;
        // A dropped dir's watch dies with the rebuilt watcher; a kept dir is
        // re-added below, so the stale `watched` set is simply forgotten.
        self.watcher = None;
        self.watched.clear();
        self.attach_watcher_for_new_dirs();
        self.rescan();
    }

    /// Drain pending watcher events; rescans once if any arrived.
    /// Returns whether a rescan ran.
    pub(crate) fn poll(&mut self) -> bool {
        let changed = self
            .event_rx
            .as_ref()
            .is_some_and(|rx| rx.try_recv().is_ok());
        if changed {
            self.rescan();
        }
        changed
    }

    /// Refresh path: attach late-appearing directories' watchers when needed,
    /// and rescan when `min_interval` has elapsed since the last one. Returns
    /// whether a rescan ran.
    pub(crate) fn tick(&mut self, min_interval: Duration) -> bool {
        if self.attach_watcher_for_new_dirs() {
            self.rescan();
            return true;
        }
        let due = self
            .last_rescan
            .is_none_or(|last| last.elapsed() >= min_interval);
        if due {
            self.rescan();
            true
        } else {
            false
        }
    }

    /// Manual refresh (panel `r`, and Task 5's update-command tick): rescan
    /// now regardless of the interval, attaching the watcher first if the
    /// directory has appeared.
    pub(crate) fn refresh_now(&mut self) -> bool {
        self.attach_watcher_for_new_dirs();
        self.rescan();
        true
    }

    /// Last rescan result.
    pub(crate) fn snapshot(&self) -> &UsageSnapshot {
        &self.snapshot
    }

    /// The widget's one line: the tightest subscription limit across all
    /// displayable records, else the first prepaid balance, else an
    /// "active" marker. `None` when nothing is displayable — the widget
    /// self-hides on `None`.
    pub(crate) fn summary_line(&self) -> Option<String> {
        let tightest = self
            .snapshot
            .records
            .iter()
            .flat_map(|r| r.limits.iter().filter_map(|l| l.percent))
            .fold(None::<f64>, |acc: Option<f64>, p| {
                Some(acc.map_or(p, |a| a.max(p)))
            });
        if let Some(percent) = tightest {
            return Some(format!("\u{25c6} {percent:.0}%"));
        }
        if let Some((remaining, currency)) = self.snapshot.records.iter().find_map(|r| {
            r.balance
                .as_ref()
                .map(|b| (b.remaining, b.currency.clone()))
        }) {
            return Some(if currency == "USD" {
                format!("\u{25c6} ${remaining:.2}")
            } else {
                format!("\u{25c6} {remaining:.2} {currency}")
            });
        }
        if !self.snapshot.records.is_empty() {
            return Some("\u{25c6} active".to_string());
        }
        None
    }

    /// Replace the hidden-agent set and refilter. Called every frame from
    /// the status-bar render with the config's set, so an unchanged set is a
    /// no-op — an unconditional rescan here would re-read the records
    /// directory sixty times a second.
    pub(crate) fn set_hidden(&mut self, hidden: HashSet<String>) {
        if self.hidden == hidden {
            return;
        }
        self.hidden = hidden;
        self.rescan();
    }

    /// Read every `*.json` across the records directories into a fresh
    /// snapshot, merging records that share an agent id (one account synced
    /// from two machines — widest value, never summed). When no directory
    /// exists the snapshot clears — a deleted records dir hides the widget,
    /// it does not freeze the last data on screen.
    fn rescan(&mut self) {
        self.last_rescan = Some(Instant::now());
        // (filename, path) across every dir, filename order for
        // deterministic snapshots and stable first-appearance merge order.
        let mut entries: Vec<(String, PathBuf)> = Vec::new();
        let mut any_dir = false;
        for dir in &self.dirs {
            if let Ok(iter) = fs::read_dir(dir) {
                any_dir = true;
                entries.extend(
                    iter.filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
                        .map(|p| {
                            let filename = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            (filename, p)
                        }),
                );
            }
        }
        if !any_dir {
            self.snapshot = UsageSnapshot::default();
            return;
        }
        entries.sort();

        let mut records: Vec<AgentUsageRecord> = Vec::new();
        let mut errors = Vec::new();
        for (filename, path) in entries {
            match fs::read_to_string(&path)
                .ok()
                .and_then(|json| records::parse_record(&json))
            {
                Some(record) => match records.iter().position(|r| r.id == record.id) {
                    // Same account seen again (a synced second copy or an
                    // extra dir): widest-value merge into the first
                    // appearance.
                    Some(index) => {
                        records[index] = records::merge_records(records[index].clone(), record);
                    }
                    None => {
                        if records::should_display(&record, &self.hidden) {
                            records.push(record);
                        }
                    }
                },
                None => errors.push(format!("{filename}: parse failed")),
            }
        }
        self.snapshot = UsageSnapshot { records, errors };
    }

    /// Watch every records directory not yet watched, creating the watcher
    /// on the first. Returns whether anything new was attached (the caller
    /// rescans then — a dir appearing late means its files were missed).
    fn attach_watcher_for_new_dirs(&mut self) -> bool {
        let pending: Vec<PathBuf> = self
            .dirs
            .iter()
            .filter(|d| d.is_dir() && !self.watched.contains(*d))
            .cloned()
            .collect();
        if pending.is_empty() {
            return false;
        }
        let mut attached_any = false;
        for dir in pending {
            if self.watcher.is_none() && !self.create_watcher() {
                // No backend at all — retrying per dir cannot help.
                return false;
            }
            let Some(watcher) = self.watcher.as_mut() else {
                return false;
            };
            match watcher.watch(&dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.watched.insert(dir);
                    attached_any = true;
                }
                Err(e) => {
                    log::warn!("agent-usage watcher: cannot watch {}: {e}", dir.display());
                }
            }
        }
        attached_any
    }

    /// Build the watcher backend (native, poll fallback). Returns whether a
    /// watcher now exists; the event channel is installed either way so a
    /// later `poll()` drains harmlessly.
    fn create_watcher(&mut self) -> bool {
        let (tx, rx) = channel::<()>();
        let watcher: Box<dyn Watcher + Send> = {
            let handler = make_event_handler(tx.clone(), Arc::clone(&self.last_event));
            match notify::recommended_watcher(handler) {
                Ok(w) => Box::new(w),
                Err(e) => {
                    log::warn!("agent-usage watcher: native backend unavailable ({e}); polling");
                    let fallback = make_event_handler(tx, Arc::clone(&self.last_event));
                    match PollWatcher::new(
                        fallback,
                        NotifyConfig::default().with_poll_interval(POLL_INTERVAL),
                    ) {
                        Ok(p) => Box::new(p),
                        Err(e2) => {
                            log::warn!(
                                "agent-usage watcher: poll fallback also failed ({e2}); \
                                 events disabled until the next attach attempt"
                            );
                            self.event_rx = Some(rx);
                            return false;
                        }
                    }
                }
            }
        };
        self.watcher = Some(watcher);
        self.event_rx = Some(rx);
        true
    }
}

/// Build the shared watcher callback: keep create/modify/remove events that
/// touch a `.json` file, debounce them, and signal the store's channel.
///
/// Built as a factory (the `par-term-config` watcher's `make_event_handler`
/// pattern) because both backends need their own closure — the native
/// backend consumes the first one whether or not it initializes.
fn make_event_handler(
    tx: Sender<()>,
    last_event: Arc<Mutex<Option<Instant>>>,
) -> impl Fn(std::result::Result<Event, notify::Error>) + Send + 'static {
    move |result: std::result::Result<Event, notify::Error>| {
        let Ok(event) = result else { return };
        let relevant = matches!(
            event.kind,
            notify::EventKind::Modify(_)
                | notify::EventKind::Create(_)
                | notify::EventKind::Remove(_)
        ) && event
            .paths
            .iter()
            .any(|p| p.extension().is_some_and(|ext| ext == "json"));
        if !relevant {
            return;
        }
        // Debounce: coalesce editor save storms into one signal.
        let should_send = {
            let now = Instant::now();
            let mut last = last_event.lock();
            if last.is_some_and(|t| now.duration_since(t) < DEBOUNCE) {
                false
            } else {
                *last = Some(now);
                true
            }
        };
        if should_send && tx.send(()).is_err() {
            log::trace!("agent-usage watcher: receiver gone");
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const READY_RECORD: &str = r#"{
        "schemaVersion": 1, "id": "claude", "name": "Claude",
        "updatedAt": "2026-09-20T17:06:14Z", "ready": true,
        "limits": [{"label": "Session (5-hour)", "percent": 42.0}]
    }"#;

    fn write(dir: &TempDir, name: &str, contents: &str) {
        fs::write(dir.path().join(name), contents).expect("write fixture");
    }

    /// Poll for up to `deadline`, returning true the moment `store.poll()`
    /// reports a rescan. Keeps the watcher test off a fixed sleep (native
    /// backends fire in milliseconds; the poll fallback takes up to 500 ms).
    fn poll_until_event(store: &mut UsageStore, deadline: Duration) -> bool {
        let start = Instant::now();
        loop {
            if store.poll() {
                return true;
            }
            if start.elapsed() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// One account synced from two machines: the primary dir and a synced
    /// extra dir each hold a `claude.json`; the snapshot carries ONE merged
    /// record (dates union by date, counts widen, limits keep the wider
    /// set), and dropping the extra dir drops back to the primary alone.
    #[test]
    fn extra_dirs_merge_one_account_synced_from_two_machines() {
        let dir = TempDir::new().expect("tempdir");
        write(
            &dir,
            "claude.json",
            r#"{"id":"claude","ready":true,"updatedAt":"2026-09-20T18:00:00Z",
                "limits":[{"label":"Weekly","percent":70.0}],
                "activeDays":2,"activeDates":["2026-09-18","2026-09-19"],
                "totalPrompts":900}"#,
        );
        let synced = TempDir::new().expect("tempdir");
        write(
            &synced,
            "claude.json",
            r#"{"id":"claude","ready":true,"updatedAt":"2026-09-20T17:00:00Z",
                "limits":[{"label":"Weekly","percent":70.0},
                           {"label":"Session (5-hour)","percent":12.0}],
                "activeDays":2,"activeDates":["2026-09-19","2026-09-20"],
                "totalPrompts":1200}"#,
        );
        let mut store = UsageStore::new(dir.path().to_path_buf());
        assert_eq!(store.snapshot().records.len(), 1, "primary alone");

        store.set_extra_dirs(vec![synced.path().to_path_buf()]);
        let records = &store.snapshot().records;
        assert_eq!(records.len(), 1, "the synced copy merges, not duplicates");
        let merged = &records[0];
        assert_eq!(merged.active_days, Some(3), "18, 19, 20 — union, never sum");
        assert_eq!(merged.total_prompts, Some(1200), "widest count wins");
        assert_eq!(merged.limits.len(), 2, "the wider limit set survives");

        // Dropping the extra dir returns to the primary's record alone.
        store.set_extra_dirs(vec![]);
        let records = &store.snapshot().records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].active_days, Some(2));
        assert_eq!(records[0].total_prompts, Some(900));
    }

    /// A missing extra dir is tolerated (empty, no errors), and one that
    /// appears later is picked up by `refresh_now`.
    #[test]
    fn late_appearing_extra_dir_is_picked_up_by_refresh() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        let outer = TempDir::new().expect("tempdir");
        let synced = outer.path().join("synced");
        fs::create_dir_all(&synced).expect("create synced root");

        let mut store = UsageStore::new(dir.path().to_path_buf());
        let missing = outer.path().join("not-yet");
        store.set_extra_dirs(vec![missing.clone()]);
        assert_eq!(
            store.snapshot().records.len(),
            1,
            "missing extra dir tolerated"
        );
        assert!(
            store.snapshot().errors.is_empty(),
            "a missing dir is not an error: {:?}",
            store.snapshot().errors
        );

        fs::create_dir_all(&missing).expect("create late dir");
        fs::write(
            missing.join("codex.json"),
            r#"{"id":"codex","ready":true,"updatedAt":"2026-09-20T17:00:00Z"}"#,
        )
        .expect("write synced record");
        store.refresh_now();
        assert_eq!(
            store.snapshot().records.len(),
            2,
            "late dir's record arrives"
        );
    }

    #[test]
    fn new_loads_existing_records() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        let store = UsageStore::new(dir.path().to_path_buf());
        assert_eq!(store.snapshot().records.len(), 1);
        assert_eq!(store.snapshot().records[0].id, "claude");
        assert!(store.snapshot().errors.is_empty());
    }

    #[test]
    fn tick_rescans_on_interval_and_skips_when_fresh() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        let mut store = UsageStore::new(dir.path().to_path_buf());
        // The constructor already rescanned, so a long interval is not due.
        assert!(!store.tick(Duration::from_secs(300)));
        // A zero interval is always due.
        assert!(store.tick(Duration::ZERO));

        // The rescan reloads current file contents, not a stale snapshot.
        write(&dir, "claude.json", &READY_RECORD.replace("42.0", "55.0"));
        assert!(store.tick(Duration::ZERO));
        assert_eq!(store.snapshot().records[0].limits[0].percent, Some(55.0));
    }

    #[test]
    fn watcher_event_triggers_poll() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        let mut store = UsageStore::new(dir.path().to_path_buf());

        // Update the file; the watcher must surface it via poll().
        write(&dir, "claude.json", &READY_RECORD.replace("42.0", "90.0"));
        // 15s, not 3s: FSEvents delivery plus the 300ms debounce can exceed a
        // tight tolerance when the full suite has the machine loaded; the
        // helper returns on first event, so a healthy run still costs ~0.3s.
        assert!(
            poll_until_event(&mut store, Duration::from_secs(15)),
            "watcher event should arrive within tolerance"
        );
        assert_eq!(store.snapshot().records[0].limits[0].percent, Some(90.0));
    }

    #[test]
    fn garbage_file_records_error_others_still_parse() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        write(&dir, "broken.json", "{not json");
        let store = UsageStore::new(dir.path().to_path_buf());
        assert_eq!(store.snapshot().records.len(), 1, "good file survives");
        assert_eq!(store.snapshot().errors, vec!["broken.json: parse failed"]);
    }

    #[test]
    fn absent_dir_is_empty_without_panicking() {
        let dir = TempDir::new().expect("tempdir");
        let missing = dir.path().join("never-created");
        let store = UsageStore::new(missing);
        assert!(store.snapshot().records.is_empty());
        assert!(store.snapshot().errors.is_empty());
        assert_eq!(store.summary_line(), None);
    }

    #[test]
    fn late_created_dir_is_picked_up_by_refresh() {
        let outer = TempDir::new().expect("tempdir");
        let records_dir = outer.path().join("usage");
        let mut store = UsageStore::new(records_dir.clone());
        assert!(store.snapshot().records.is_empty());

        fs::create_dir_all(&records_dir).expect("create records dir late");
        fs::write(records_dir.join("claude.json"), READY_RECORD).expect("write record");

        store.refresh_now();
        assert_eq!(store.snapshot().records.len(), 1, "late dir must load");
    }

    #[test]
    fn removed_dir_clears_snapshot() {
        let outer = TempDir::new().expect("tempdir");
        let records_dir = outer.path().join("usage");
        fs::create_dir_all(&records_dir).expect("create dir");
        fs::write(records_dir.join("claude.json"), READY_RECORD).expect("write record");
        let mut store = UsageStore::new(records_dir.clone());
        assert_eq!(store.snapshot().records.len(), 1);

        fs::remove_dir_all(&records_dir).expect("remove dir");
        store.refresh_now();
        assert!(
            store.snapshot().records.is_empty(),
            "a deleted records dir hides the widget, it does not freeze old data"
        );
    }

    #[test]
    fn summary_line_tracks_tightest_limit_then_balance_then_active() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "a.json", READY_RECORD);
        let store = UsageStore::new(dir.path().to_path_buf());
        assert_eq!(store.summary_line().as_deref(), Some("\u{25c6} 42%"));

        // A second record with a tighter limit wins.
        let dir2 = TempDir::new().expect("tempdir");
        write(&dir2, "a.json", READY_RECORD);
        write(
            &dir2,
            "b.json",
            r#"{"id":"codex","ready":true,
                "limits":[{"label":"Weekly","percent":91.0}]}"#,
        );
        let store2 = UsageStore::new(dir2.path().to_path_buf());
        assert_eq!(store2.summary_line().as_deref(), Some("\u{25c6} 91%"));

        // No limits but a prepaid balance.
        let dir3 = TempDir::new().expect("tempdir");
        write(
            &dir3,
            "fw.json",
            r#"{"id":"fireworks","ready":true,
                "balance":{"remaining":12.5,"funded":50.0,"spent":37.5,
                           "currency":"USD","estimated":false}}"#,
        );
        let store3 = UsageStore::new(dir3.path().to_path_buf());
        assert_eq!(store3.summary_line().as_deref(), Some("\u{25c6} $12.50"));

        // Ready with neither limits nor balance.
        let dir4 = TempDir::new().expect("tempdir");
        write(&dir4, "bare.json", r#"{"id":"x","ready":true}"#);
        let store4 = UsageStore::new(dir4.path().to_path_buf());
        assert_eq!(store4.summary_line().as_deref(), Some("\u{25c6} active"));
    }

    #[test]
    fn hidden_agent_is_excluded_after_set_hidden() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir, "claude.json", READY_RECORD);
        let mut store = UsageStore::new(dir.path().to_path_buf());
        assert_eq!(store.snapshot().records.len(), 1);

        store.set_hidden(["claude".to_string()].into());
        assert!(store.snapshot().records.is_empty());
        assert_eq!(store.summary_line(), None);
    }
}
