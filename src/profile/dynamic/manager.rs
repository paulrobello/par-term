//! Background fetch manager for dynamic profile sources.
//!
//! [`DynamicProfileManager`] spawns tokio tasks that periodically fetch profiles
//! from remote URLs and sends [`DynamicProfileUpdate`] messages via an mpsc
//! channel for the main thread to process.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

use par_term_config::{ConflictResolution, DynamicProfileSource};

use crate::profile::dynamic::fetch::fetch_profiles;

/// Message sent from background fetch tasks to the main thread.
#[derive(Debug, Clone)]
pub struct DynamicProfileUpdate {
    /// The source URL that was fetched.
    pub url: String,
    /// Successfully parsed profiles (empty on error).
    pub profiles: Vec<par_term_config::Profile>,
    /// How to resolve conflicts with local profiles.
    pub conflict_resolution: ConflictResolution,
    /// Error message if the fetch failed.
    pub error: Option<String>,
}

/// Status of a dynamic profile source.
#[derive(Debug, Clone)]
pub struct SourceStatus {
    /// The source URL.
    pub url: String,
    /// Whether this source is enabled.
    pub enabled: bool,
    /// When profiles were last successfully fetched.
    pub last_fetch: Option<SystemTime>,
    /// Last error message (if any).
    pub last_error: Option<String>,
    /// Number of profiles from this source.
    pub profile_count: usize,
    /// Whether a fetch is currently in progress.
    pub fetching: bool,
}

/// Manages background fetching of dynamic profiles.
///
/// Spawns tokio tasks that periodically fetch profiles from remote URLs
/// and sends updates via an mpsc channel for the main thread to process.
pub struct DynamicProfileManager {
    /// Channel receiver for updates from background tasks.
    pub update_rx: mpsc::UnboundedReceiver<DynamicProfileUpdate>,
    /// Channel sender (cloned to background tasks).
    update_tx: mpsc::UnboundedSender<DynamicProfileUpdate>,
    /// Status of each source, keyed by URL.
    pub statuses: HashMap<String, SourceStatus>,
    /// Handles to cancel background tasks.
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl DynamicProfileManager {
    /// Create a new DynamicProfileManager with fresh channels.
    pub fn new() -> Self {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        Self {
            update_rx,
            update_tx,
            statuses: HashMap::new(),
            task_handles: Vec::new(),
        }
    }

    /// Start background fetch tasks for all enabled sources.
    ///
    /// Stops any existing tasks first. For each enabled source:
    /// 1. Initializes the source status
    /// 2. Loads cached profiles and sends them via the channel immediately
    /// 3. Spawns a tokio task that does an initial fetch, then periodic refreshes
    pub fn start(
        &mut self,
        sources: &[DynamicProfileSource],
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        use crate::profile::dynamic::cache::read_cache;

        // Cancel existing tasks
        self.stop();

        for source in sources {
            if !source.enabled || source.url.is_empty() {
                continue;
            }

            // Initialize status
            self.statuses.insert(
                source.url.clone(),
                SourceStatus {
                    url: source.url.clone(),
                    enabled: source.enabled,
                    last_fetch: None,
                    last_error: None,
                    profile_count: 0,
                    fetching: false,
                },
            );

            // Load from cache immediately
            if let Ok((profiles, meta)) = read_cache(&source.url) {
                let update = DynamicProfileUpdate {
                    url: source.url.clone(),
                    profiles,
                    conflict_resolution: source.conflict_resolution.clone(),
                    error: None,
                };
                let _ = self.update_tx.send(update);

                if let Some(status) = self.statuses.get_mut(&source.url) {
                    status.last_fetch = Some(meta.last_fetched);
                    status.profile_count = meta.profile_count;
                }
            }

            // Spawn background fetch task
            let tx = self.update_tx.clone();
            let source_clone = source.clone();
            let url_for_log = source.url.clone();
            let handle = runtime.spawn(async move {
                // Initial fetch using spawn_blocking since ureq is synchronous
                let src = source_clone.clone();
                let conflict = source_clone.conflict_resolution.clone();
                match tokio::task::spawn_blocking(move || fetch_profiles(&src)).await {
                    Ok(result) => {
                        if tx
                            .send(DynamicProfileUpdate {
                                url: result.url.clone(),
                                profiles: result.profiles,
                                conflict_resolution: conflict,
                                error: result.error,
                            })
                            .is_err()
                        {
                            return; // Receiver dropped
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "Dynamic profile fetch task panicked for {}: {}",
                            url_for_log,
                            e
                        );
                    }
                }

                // Periodic refresh
                let mut interval =
                    tokio::time::interval(Duration::from_secs(source_clone.refresh_interval_secs));
                interval.tick().await; // Skip first immediate tick
                loop {
                    interval.tick().await;
                    let src = source_clone.clone();
                    let source_clone2 = source_clone.clone();
                    let tx_clone = tx.clone();
                    match tokio::task::spawn_blocking(move || fetch_profiles(&src)).await {
                        Ok(result) => {
                            if tx_clone
                                .send(DynamicProfileUpdate {
                                    url: result.url.clone(),
                                    profiles: result.profiles,
                                    conflict_resolution: source_clone2.conflict_resolution.clone(),
                                    error: result.error,
                                })
                                .is_err()
                            {
                                break; // Receiver dropped
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Dynamic profile fetch task panicked for {}: {}",
                                url_for_log,
                                e
                            );
                        }
                    }
                }
            });

            self.task_handles.push(handle);

            if let Some(status) = self.statuses.get_mut(&source.url) {
                status.fetching = true;
            }
        }
    }

    /// Stop all background fetch tasks.
    pub fn stop(&mut self) {
        for handle in self.task_handles.drain(..) {
            handle.abort();
        }
    }

    /// Trigger an immediate refresh of all enabled sources.
    pub fn refresh_all(
        &mut self,
        sources: &[DynamicProfileSource],
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        for source in sources {
            if !source.enabled || source.url.is_empty() {
                continue;
            }
            self.refresh_source(source, runtime);
        }
    }

    /// Trigger an immediate refresh of a specific source.
    pub fn refresh_source(
        &mut self,
        source: &DynamicProfileSource,
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        let tx = self.update_tx.clone();
        let source_clone = source.clone();
        let url_for_log = source.url.clone();
        runtime.spawn(async move {
            let conflict = source_clone.conflict_resolution.clone();
            match tokio::task::spawn_blocking(move || fetch_profiles(&source_clone)).await {
                Ok(result) => {
                    let _ = tx.send(DynamicProfileUpdate {
                        url: result.url.clone(),
                        profiles: result.profiles,
                        conflict_resolution: conflict,
                        error: result.error,
                    });
                }
                Err(e) => {
                    log::error!(
                        "Dynamic profile fetch task panicked for {}: {}",
                        url_for_log,
                        e
                    );
                }
            }
        });

        if let Some(status) = self.statuses.get_mut(&source.url) {
            status.fetching = true;
        }
    }

    /// Check for pending updates (non-blocking).
    pub fn try_recv(&mut self) -> Option<DynamicProfileUpdate> {
        self.update_rx.try_recv().ok()
    }

    /// Update source status after receiving an update.
    pub fn update_status(&mut self, update: &DynamicProfileUpdate) {
        if let Some(status) = self.statuses.get_mut(&update.url) {
            status.fetching = false;
            status.last_error = update.error.clone();
            if update.error.is_none() {
                status.last_fetch = Some(SystemTime::now());
                status.profile_count = update.profiles.len();
            }
        }
    }
}

impl Default for DynamicProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DynamicProfileManager {
    fn drop(&mut self) {
        self.stop();
    }
}
