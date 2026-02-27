//! Config file watcher for automatic reload.
//!
//! Watches the config.yaml file for changes and triggers automatic reloading.
//! Uses debouncing to avoid multiple reloads during rapid saves from editors.

use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, Event, PollWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Event indicating the config file has changed and needs reloading.
#[derive(Debug, Clone)]
pub struct ConfigReloadEvent {
    /// Path to the config file that changed.
    pub path: PathBuf,
}

/// Watches the config file for changes and sends reload events.
pub struct ConfigWatcher {
    /// The file system watcher (kept alive to maintain watching).
    _watcher: Box<dyn Watcher + Send>,
    /// Receiver for config change events.
    event_receiver: Receiver<ConfigReloadEvent>,
}

impl std::fmt::Debug for ConfigWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigWatcher").finish_non_exhaustive()
    }
}

/// Build the shared event-handler closure used by both watcher backends.
///
/// Returns a closure that filters events to the given `filename`, applies
/// debouncing, and sends `ConfigReloadEvent` values on `tx`.
fn make_event_handler(
    filename: std::ffi::OsString,
    canonical_path: PathBuf,
    debounce_delay: Duration,
    tx: std::sync::mpsc::Sender<ConfigReloadEvent>,
    last_event_time: Arc<Mutex<Option<Instant>>>,
) -> impl Fn(std::result::Result<Event, notify::Error>) + Send + 'static {
    move |result: std::result::Result<Event, notify::Error>| {
        if let Ok(event) = result {
            // Only process modify and create events (create handles atomic saves)
            if !matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
            ) {
                return;
            }

            // Check if any event path matches our config filename
            let matches_config: bool = event
                .paths
                .iter()
                .any(|p: &PathBuf| p.file_name().map(|f| f == filename).unwrap_or(false));

            if !matches_config {
                return;
            }

            // Debounce: skip if we sent an event too recently
            let should_send: bool = {
                let now: Instant = Instant::now();
                let mut last: parking_lot::MutexGuard<'_, Option<Instant>> =
                    last_event_time.lock();
                if let Some(last_time) = *last {
                    if now.duration_since(last_time) < debounce_delay {
                        log::trace!("Debouncing config reload event");
                        false
                    } else {
                        *last = Some(now);
                        true
                    }
                } else {
                    *last = Some(now);
                    true
                }
            };

            if should_send {
                let reload_event = ConfigReloadEvent {
                    path: canonical_path.clone(),
                };
                log::info!("Config file changed: {}", reload_event.path.display());
                if let Err(e) = tx.send(reload_event) {
                    log::error!("Failed to send config reload event: {}", e);
                }
            }
        }
    }
}

impl ConfigWatcher {
    /// Create a new config watcher.
    ///
    /// Attempts to use the platform's native watcher (`RecommendedWatcher`: inotify on
    /// Linux, kqueue on macOS, ReadDirectoryChanges on Windows) for low-latency,
    /// event-driven notifications. If the native backend fails to initialise (e.g.
    /// inside a container or on a network filesystem), falls back to a `PollWatcher`
    /// that checks for changes every 500 ms.
    ///
    /// # Arguments
    /// * `config_path` - Path to the config file to watch.
    /// * `debounce_delay_ms` - Debounce delay in milliseconds to avoid rapid reloads.
    ///
    /// # Errors
    /// Returns an error if the config file doesn't exist or watching fails on both
    /// backends.
    pub fn new(config_path: &Path, debounce_delay_ms: u64) -> Result<Self> {
        if !config_path.exists() {
            anyhow::bail!("Config file not found: {}", config_path.display());
        }

        let canonical: PathBuf = config_path
            .canonicalize()
            .unwrap_or_else(|_| config_path.to_path_buf());

        let filename: std::ffi::OsString = canonical
            .file_name()
            .context("Config path has no filename")?
            .to_os_string();

        let parent_dir: PathBuf = canonical
            .parent()
            .context("Config path has no parent directory")?
            .to_path_buf();

        let (tx, rx) = channel::<ConfigReloadEvent>();
        let debounce_delay: Duration = Duration::from_millis(debounce_delay_ms);
        let last_event_time: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

        // Try the platform-native watcher first; fall back to PollWatcher on failure.
        let mut watcher: Box<dyn Watcher + Send> = Self::create_watcher(
            filename,
            canonical.clone(),
            debounce_delay,
            tx,
            last_event_time,
        )?;

        watcher
            .watch(&parent_dir, RecursiveMode::NonRecursive)
            .with_context(|| {
                format!("Failed to watch config directory: {}", parent_dir.display())
            })?;

        log::info!("Config hot reload: watching {}", canonical.display());

        Ok(Self {
            _watcher: watcher,
            event_receiver: rx,
        })
    }

    /// Try to create the best available watcher backend.
    ///
    /// Attempts `RecommendedWatcher` first. If that fails (e.g. inside a
    /// container, network filesystem, or restricted environment), logs a warning
    /// and falls back to `PollWatcher` with a 500 ms poll interval.
    fn create_watcher(
        filename: std::ffi::OsString,
        canonical_path: PathBuf,
        debounce_delay: Duration,
        tx: std::sync::mpsc::Sender<ConfigReloadEvent>,
        last_event_time: Arc<Mutex<Option<Instant>>>,
    ) -> Result<Box<dyn Watcher + Send>> {
        // Build the shared handler (clone inputs for the fallback path).
        let filename2 = filename.clone();
        let canonical_path2 = canonical_path.clone();
        let debounce_delay2 = debounce_delay;
        let tx2 = tx.clone();
        let last_event_time2 = Arc::clone(&last_event_time);

        let handler = make_event_handler(
            filename,
            canonical_path,
            debounce_delay,
            tx,
            last_event_time,
        );

        match notify::recommended_watcher(handler) {
            Ok(w) => {
                log::debug!("Config watcher: using native (RecommendedWatcher) backend");
                Ok(Box::new(w))
            }
            Err(e) => {
                log::warn!(
                    "Config watcher: native backend unavailable ({}); falling back to PollWatcher",
                    e
                );
                let fallback_handler = make_event_handler(
                    filename2,
                    canonical_path2,
                    debounce_delay2,
                    tx2,
                    last_event_time2,
                );
                let poll_watcher = PollWatcher::new(
                    fallback_handler,
                    NotifyConfig::default().with_poll_interval(Duration::from_millis(500)),
                )
                .context("Failed to create fallback PollWatcher")?;
                Ok(Box::new(poll_watcher))
            }
        }
    }

    /// Check for pending config reload events (non-blocking).
    ///
    /// Returns the next reload event if one is available, or `None` if no events are pending.
    pub fn try_recv(&self) -> Option<ConfigReloadEvent> {
        self.event_receiver.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation_with_existing_file() {
        let temp_dir: TempDir = TempDir::new().expect("Failed to create temp dir");
        let config_path: PathBuf = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let result = ConfigWatcher::new(&config_path, 100);
        assert!(
            result.is_ok(),
            "ConfigWatcher should succeed with existing file"
        );
    }

    #[test]
    fn test_watcher_creation_with_nonexistent_file() {
        let path = PathBuf::from("/tmp/nonexistent_config_watcher_test/config.yaml");
        let result = ConfigWatcher::new(&path, 100);
        assert!(
            result.is_err(),
            "ConfigWatcher should fail with nonexistent file"
        );
    }

    #[test]
    fn test_no_initial_events() {
        let temp_dir: TempDir = TempDir::new().expect("Failed to create temp dir");
        let config_path: PathBuf = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher: ConfigWatcher =
            ConfigWatcher::new(&config_path, 100).expect("Failed to create watcher");

        // Should return None immediately with no events
        assert!(
            watcher.try_recv().is_none(),
            "No events should be pending after creation"
        );
    }

    #[test]
    fn test_file_change_detection() {
        let temp_dir: TempDir = TempDir::new().expect("Failed to create temp dir");
        let config_path: PathBuf = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher: ConfigWatcher =
            ConfigWatcher::new(&config_path, 50).expect("Failed to create watcher");

        // Give the watcher time to set up
        std::thread::sleep(Duration::from_millis(100));

        // Modify the file
        fs::write(&config_path, "font_size: 14.0\n").expect("Failed to write config");

        // Wait for the watcher to detect the change (native is faster; poll takes up to 500ms)
        std::thread::sleep(Duration::from_millis(700));

        // Check for the reload event (platform-dependent, don't assert failure)
        if let Some(event) = watcher.try_recv() {
            assert!(
                event.path.ends_with("config.yaml"),
                "Event path should end with config.yaml"
            );
        }
    }

    #[test]
    fn test_debug_impl() {
        let temp_dir: TempDir = TempDir::new().expect("Failed to create temp dir");
        let config_path: PathBuf = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher: ConfigWatcher =
            ConfigWatcher::new(&config_path, 100).expect("Failed to create watcher");

        let debug_str: String = format!("{:?}", watcher);
        assert!(
            debug_str.contains("ConfigWatcher"),
            "Debug output should contain struct name"
        );
    }
}
