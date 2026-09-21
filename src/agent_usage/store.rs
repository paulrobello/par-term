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

// The popup panel (Task 4) reads snapshot errors and record detail beyond
// summary_line; until it lands, non-test builds see those as unread. Remove
// this attribute when the panel ships.
#![cfg_attr(not(test), allow(dead_code))]

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

/// Watches the records directory and serves the last snapshot.
pub(crate) struct UsageStore {
    /// The records directory (may not exist yet).
    dir: PathBuf,
    /// Directory watcher; `None` until the directory exists.
    watcher: Option<Box<dyn Watcher + Send>>,
    /// Signals "something in the directory changed" after debouncing.
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
            dir,
            watcher: None,
            event_rx: None,
            last_event: Arc::new(Mutex::new(None)),
            snapshot: UsageSnapshot::default(),
            hidden: HashSet::new(),
            last_rescan: None,
        };
        store.attach_watcher_if_dir_exists();
        store.rescan();
        store
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

    /// Refresh path: attach a late-appearing directory's watcher when needed,
    /// and rescan when `min_interval` has elapsed since the last one. Returns
    /// whether a rescan ran.
    pub(crate) fn tick(&mut self, min_interval: Duration) -> bool {
        if self.watcher.is_none() && self.attach_watcher_if_dir_exists() {
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
        if self.watcher.is_none() {
            self.attach_watcher_if_dir_exists();
        }
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

    /// Replace the hidden-agent set (Task 5 wiring) and refilter.
    pub(crate) fn set_hidden(&mut self, hidden: HashSet<String>) {
        self.hidden = hidden;
        self.rescan();
    }

    /// Read every `*.json` in the directory into a fresh snapshot. A missing
    /// directory clears the snapshot — a deleted records dir hides the
    /// widget, it does not freeze the last data on screen.
    fn rescan(&mut self) {
        self.last_rescan = Some(Instant::now());
        if !self.dir.is_dir() {
            self.snapshot = UsageSnapshot::default();
            return;
        }
        let mut entries: Vec<PathBuf> = match fs::read_dir(&self.dir) {
            Ok(iter) => iter
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
                .collect(),
            Err(_) => {
                self.snapshot = UsageSnapshot::default();
                return;
            }
        };
        // Filename order, so the snapshot is deterministic across rescans.
        entries.sort();

        let mut records = Vec::new();
        let mut errors = Vec::new();
        for path in entries {
            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            match fs::read_to_string(&path)
                .ok()
                .and_then(|json| records::parse_record(&json))
            {
                Some(record) => {
                    if records::should_display(&record, &self.hidden) {
                        records.push(record);
                    }
                }
                None => errors.push(format!("{filename}: parse failed")),
            }
        }
        self.snapshot = UsageSnapshot { records, errors };
    }

    /// Try to watch the directory. Returns whether a watcher is now attached.
    fn attach_watcher_if_dir_exists(&mut self) -> bool {
        if self.watcher.is_some() || !self.dir.is_dir() {
            return self.watcher.is_some();
        }
        let (tx, rx) = channel::<()>();
        let mut watcher: Box<dyn Watcher + Send> = {
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
        if let Err(e) = watcher.watch(&self.dir, RecursiveMode::NonRecursive) {
            log::warn!(
                "agent-usage watcher: cannot watch {}: {e}",
                self.dir.display()
            );
            self.event_rx = Some(rx);
            return false;
        }
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
        assert!(
            poll_until_event(&mut store, Duration::from_secs(3)),
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
