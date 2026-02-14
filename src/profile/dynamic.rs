//! Dynamic profile source configuration types
//!
//! Defines the configuration for fetching profiles from remote URLs,
//! caching fetched profiles, HTTP fetch logic, merge strategies,
//! and background fetch management via tokio tasks.
//! These types are serialized/deserialized as part of the main config file.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

/// How to resolve conflicts when a remote profile has the same ID as a local one
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Local profile takes precedence over remote
    #[default]
    LocalWins,
    /// Remote profile takes precedence over local
    RemoteWins,
}

impl ConflictResolution {
    /// Returns all variants of `ConflictResolution`
    pub fn variants() -> &'static [ConflictResolution] {
        &[
            ConflictResolution::LocalWins,
            ConflictResolution::RemoteWins,
        ]
    }

    /// Returns a human-readable display name for this variant
    pub fn display_name(&self) -> &'static str {
        match self {
            ConflictResolution::LocalWins => "Local Wins",
            ConflictResolution::RemoteWins => "Remote Wins",
        }
    }
}

// ── Serde default helpers ──────────────────────────────────────────────

fn default_refresh_interval_secs() -> u64 {
    1800
}

fn default_max_size_bytes() -> usize {
    1_048_576
}

fn default_fetch_timeout_secs() -> u64 {
    10
}

/// A remote profile source configuration stored in the main config file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DynamicProfileSource {
    /// URL to fetch profiles YAML from
    pub url: String,

    /// Custom HTTP headers to include in the fetch request (e.g., Authorization)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// How often to re-fetch profiles, in seconds (default: 1800 = 30 min)
    #[serde(default = "default_refresh_interval_secs")]
    pub refresh_interval_secs: u64,

    /// Maximum allowed response size in bytes (default: 1 MB)
    #[serde(default = "default_max_size_bytes")]
    pub max_size_bytes: usize,

    /// Timeout for the HTTP fetch request, in seconds (default: 10)
    #[serde(default = "default_fetch_timeout_secs")]
    pub fetch_timeout_secs: u64,

    /// Whether this source is enabled (default: true)
    #[serde(default = "crate::config::defaults::bool_true")]
    pub enabled: bool,

    /// How to resolve conflicts when a remote profile ID matches a local one
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}

impl Default for DynamicProfileSource {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: HashMap::new(),
            refresh_interval_secs: default_refresh_interval_secs(),
            max_size_bytes: default_max_size_bytes(),
            fetch_timeout_secs: default_fetch_timeout_secs(),
            enabled: true,
            conflict_resolution: ConflictResolution::default(),
        }
    }
}

// ── Cache storage ──────────────────────────────────────────────────────

/// Get the cache directory for dynamic profiles
pub fn cache_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("par-term")
        .join("cache")
        .join("dynamic_profiles")
}

/// Generate a deterministic filename from a URL
pub fn url_to_cache_filename(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)
}

/// Cache metadata stored alongside profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    /// The source URL this cache entry corresponds to
    pub url: String,
    /// When the profiles were last fetched
    pub last_fetched: SystemTime,
    /// HTTP ETag header from the server (for conditional requests)
    pub etag: Option<String>,
    /// Number of profiles in the cached data
    pub profile_count: usize,
}

/// Read cached profiles for a given URL
pub fn read_cache(url: &str) -> anyhow::Result<(Vec<super::types::Profile>, CacheMeta)> {
    let dir = cache_dir();
    let hash = url_to_cache_filename(url);
    let data_path = dir.join(format!("{hash}.yaml"));
    let meta_path = dir.join(format!("{hash}.meta"));

    let data = std::fs::read_to_string(&data_path)
        .with_context(|| format!("Failed to read cache data from {data_path:?}"))?;
    let meta_str = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read cache meta from {meta_path:?}"))?;

    let profiles: Vec<super::types::Profile> =
        serde_yaml::from_str(&data).with_context(|| "Failed to parse cached profiles")?;
    let meta: CacheMeta =
        serde_json::from_str(&meta_str).with_context(|| "Failed to parse cache metadata")?;

    Ok((profiles, meta))
}

/// Write profiles and metadata to cache
pub fn write_cache(
    url: &str,
    profiles: &[super::types::Profile],
    etag: Option<String>,
) -> anyhow::Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache directory {dir:?}"))?;

    let hash = url_to_cache_filename(url);
    let data_path = dir.join(format!("{hash}.yaml"));
    let meta_path = dir.join(format!("{hash}.meta"));

    let data = serde_yaml::to_string(profiles)
        .with_context(|| "Failed to serialize profiles for cache")?;
    std::fs::write(&data_path, data)
        .with_context(|| format!("Failed to write cache data to {data_path:?}"))?;

    let meta = CacheMeta {
        url: url.to_string(),
        last_fetched: SystemTime::now(),
        etag,
        profile_count: profiles.len(),
    };
    let meta_str = serde_json::to_string_pretty(&meta)
        .with_context(|| "Failed to serialize cache metadata")?;
    std::fs::write(&meta_path, meta_str)
        .with_context(|| format!("Failed to write cache meta to {meta_path:?}"))?;

    Ok(())
}

// ── HTTP fetch and profile parsing ─────────────────────────────────────

/// Result of fetching profiles from a remote source
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The source URL that was fetched
    pub url: String,
    /// Successfully parsed profiles (empty on error)
    pub profiles: Vec<super::types::Profile>,
    /// HTTP ETag header from the response
    pub etag: Option<String>,
    /// Error message if the fetch failed
    pub error: Option<String>,
}

/// Fetch profiles from a remote URL
pub fn fetch_profiles(source: &DynamicProfileSource) -> FetchResult {
    let url = &source.url;
    crate::debug_info!("DYNAMIC_PROFILE", "Fetching profiles from {}", url);

    match fetch_profiles_inner(source) {
        Ok((profiles, etag)) => {
            crate::debug_info!(
                "DYNAMIC_PROFILE",
                "Fetched {} profiles from {}",
                profiles.len(),
                url
            );
            if let Err(e) = write_cache(url, &profiles, etag.clone()) {
                crate::debug_error!(
                    "DYNAMIC_PROFILE",
                    "Failed to cache profiles from {}: {}",
                    url,
                    e
                );
            }
            FetchResult {
                url: url.clone(),
                profiles,
                etag,
                error: None,
            }
        }
        Err(e) => {
            crate::debug_error!("DYNAMIC_PROFILE", "Failed to fetch from {}: {}", url, e);
            FetchResult {
                url: url.clone(),
                profiles: Vec::new(),
                etag: None,
                error: Some(e.to_string()),
            }
        }
    }
}

/// Internal fetch implementation
fn fetch_profiles_inner(
    source: &DynamicProfileSource,
) -> anyhow::Result<(Vec<super::types::Profile>, Option<String>)> {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    // Warn if using HTTP with auth headers (credential leaking risk)
    if !source.url.starts_with("https://") && !source.url.starts_with("file://") {
        if source.headers.keys().any(|k| {
            let lower = k.to_lowercase();
            lower == "authorization" || lower.contains("token") || lower.contains("secret")
        }) {
            anyhow::bail!(
                "Refusing to send authentication headers over insecure HTTP for {}. Use HTTPS.",
                source.url
            );
        }
        crate::debug_info!(
            "DYNAMIC_PROFILE",
            "Warning: {} uses insecure HTTP. Consider using HTTPS.",
            source.url
        );
    }

    // Create an agent with the source-specific timeout
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(std::time::Duration::from_secs(
            source.fetch_timeout_secs,
        )))
        .build()
        .into();

    let mut request = agent.get(&source.url);

    for (key, value) in &source.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    let mut response = request
        .call()
        .with_context(|| format!("HTTP request failed for {}", source.url))?;

    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = response
        .body_mut()
        .with_config()
        .limit(source.max_size_bytes as u64)
        .read_to_string()
        .with_context(|| format!("Failed to read response body from {}", source.url))?;

    let profiles: Vec<super::types::Profile> = serde_yaml::from_str(&body)
        .with_context(|| format!("Failed to parse YAML from {}", source.url))?;

    Ok((profiles, etag))
}

// ── Profile merge logic ────────────────────────────────────────────────

/// Merge dynamic profiles into a ProfileManager
///
/// 1. Remove existing dynamic profiles from this URL
/// 2. For each remote profile, check for name conflicts with local profiles
/// 3. Apply conflict resolution strategy
/// 4. Mark merged profiles with Dynamic source
pub fn merge_dynamic_profiles(
    manager: &mut super::types::ProfileManager,
    remote_profiles: &[super::types::Profile],
    url: &str,
    conflict_resolution: &ConflictResolution,
) {
    // Remove existing dynamic profiles from this URL
    let to_remove: Vec<super::types::ProfileId> = manager
        .profiles_ordered()
        .iter()
        .filter(
            |p| matches!(&p.source, super::types::ProfileSource::Dynamic { url: u, .. } if u == url),
        )
        .map(|p| p.id)
        .collect();
    for id in &to_remove {
        manager.remove(id);
    }

    // Merge remote profiles
    let now = SystemTime::now();
    for remote in remote_profiles {
        let existing = manager.find_by_name(&remote.name);
        match (existing, conflict_resolution) {
            (Some(_), ConflictResolution::LocalWins) => {
                crate::debug_info!("DYNAMIC_PROFILE", "Skipping '{}' (local wins)", remote.name);
            }
            (Some(local), ConflictResolution::RemoteWins) => {
                let local_id = local.id;
                manager.remove(&local_id);
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4();
                profile.source = super::types::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!(
                    "DYNAMIC_PROFILE",
                    "Remote '{}' overwrites local",
                    remote.name
                );
            }
            (None, _) => {
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4();
                profile.source = super::types::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!("DYNAMIC_PROFILE", "Added remote '{}'", remote.name);
            }
        }
    }
}

// ── Background fetch manager ────────────────────────────────────────

/// Message sent from background fetch tasks to the main thread
#[derive(Debug, Clone)]
pub struct DynamicProfileUpdate {
    /// The source URL that was fetched
    pub url: String,
    /// Successfully parsed profiles (empty on error)
    pub profiles: Vec<super::types::Profile>,
    /// How to resolve conflicts with local profiles
    pub conflict_resolution: ConflictResolution,
    /// Error message if the fetch failed
    pub error: Option<String>,
}

/// Status of a dynamic profile source
#[derive(Debug, Clone)]
pub struct SourceStatus {
    /// The source URL
    pub url: String,
    /// Whether this source is enabled
    pub enabled: bool,
    /// When profiles were last successfully fetched
    pub last_fetch: Option<SystemTime>,
    /// Last error message (if any)
    pub last_error: Option<String>,
    /// Number of profiles from this source
    pub profile_count: usize,
    /// Whether a fetch is currently in progress
    pub fetching: bool,
}

/// Manages background fetching of dynamic profiles
///
/// Spawns tokio tasks that periodically fetch profiles from remote URLs
/// and sends updates via an mpsc channel for the main thread to process.
pub struct DynamicProfileManager {
    /// Channel receiver for updates from background tasks
    pub update_rx: mpsc::UnboundedReceiver<DynamicProfileUpdate>,
    /// Channel sender (cloned to background tasks)
    update_tx: mpsc::UnboundedSender<DynamicProfileUpdate>,
    /// Status of each source, keyed by URL
    pub statuses: HashMap<String, SourceStatus>,
    /// Handles to cancel background tasks
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl DynamicProfileManager {
    /// Create a new DynamicProfileManager with fresh channels
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

    /// Stop all background fetch tasks
    pub fn stop(&mut self) {
        for handle in self.task_handles.drain(..) {
            handle.abort();
        }
    }

    /// Trigger an immediate refresh of all enabled sources
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

    /// Trigger an immediate refresh of a specific source
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

    /// Check for pending updates (non-blocking)
    pub fn try_recv(&mut self) -> Option<DynamicProfileUpdate> {
        self.update_rx.try_recv().ok()
    }

    /// Update source status after receiving an update
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_source() {
        let source = DynamicProfileSource::default();

        assert_eq!(source.url, "");
        assert!(source.headers.is_empty());
        assert_eq!(source.refresh_interval_secs, 1800);
        assert_eq!(source.max_size_bytes, 1_048_576);
        assert_eq!(source.fetch_timeout_secs, 10);
        assert!(source.enabled);
        assert_eq!(source.conflict_resolution, ConflictResolution::LocalWins);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok123".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());

        let source = DynamicProfileSource {
            url: "https://example.com/profiles.yaml".to_string(),
            headers,
            refresh_interval_secs: 900,
            max_size_bytes: 512_000,
            fetch_timeout_secs: 15,
            enabled: false,
            conflict_resolution: ConflictResolution::RemoteWins,
        };

        let yaml = serde_yaml::to_string(&source).expect("serialize");
        let deserialized: DynamicProfileSource = serde_yaml::from_str(&yaml).expect("deserialize");

        assert_eq!(deserialized.url, source.url);
        assert_eq!(deserialized.headers, source.headers);
        assert_eq!(
            deserialized.refresh_interval_secs,
            source.refresh_interval_secs
        );
        assert_eq!(deserialized.max_size_bytes, source.max_size_bytes);
        assert_eq!(deserialized.fetch_timeout_secs, source.fetch_timeout_secs);
        assert_eq!(deserialized.enabled, source.enabled);
        assert_eq!(deserialized.conflict_resolution, source.conflict_resolution);
    }

    #[test]
    fn test_deserialize_minimal_yaml() {
        let yaml = "url: https://example.com/profiles.yaml\n";
        let source: DynamicProfileSource = serde_yaml::from_str(yaml).expect("deserialize minimal");

        assert_eq!(source.url, "https://example.com/profiles.yaml");
        assert!(source.headers.is_empty());
        assert_eq!(source.refresh_interval_secs, 1800);
        assert_eq!(source.max_size_bytes, 1_048_576);
        assert_eq!(source.fetch_timeout_secs, 10);
        assert!(source.enabled);
        assert_eq!(source.conflict_resolution, ConflictResolution::LocalWins);
    }

    #[test]
    fn test_conflict_resolution_display() {
        assert_eq!(ConflictResolution::LocalWins.display_name(), "Local Wins");
        assert_eq!(ConflictResolution::RemoteWins.display_name(), "Remote Wins");
    }

    #[test]
    fn test_conflict_resolution_variants() {
        let variants = ConflictResolution::variants();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], ConflictResolution::LocalWins);
        assert_eq!(variants[1], ConflictResolution::RemoteWins);
    }

    // ── Cache tests ────────────────────────────────────────────────────

    #[test]
    fn test_url_to_cache_filename_deterministic() {
        let url = "https://example.com/profiles.yaml";
        let a = super::url_to_cache_filename(url);
        let b = super::url_to_cache_filename(url);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn test_url_to_cache_filename_different_urls() {
        let a = super::url_to_cache_filename("https://example.com/a.yaml");
        let b = super::url_to_cache_filename("https://example.com/b.yaml");
        assert_ne!(a, b);
    }

    #[test]
    fn test_cache_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let url = "https://test.example.com/profiles.yaml";
        let profiles = vec![
            super::super::types::Profile::new("Remote Profile 1"),
            super::super::types::Profile::new("Remote Profile 2"),
        ];
        let hash = super::url_to_cache_filename(url);
        let data_path = temp.path().join(format!("{hash}.yaml"));
        let meta_path = temp.path().join(format!("{hash}.meta"));

        // Write
        let data = serde_yaml::to_string(&profiles).unwrap();
        std::fs::write(&data_path, &data).unwrap();
        let meta = super::CacheMeta {
            url: url.to_string(),
            last_fetched: std::time::SystemTime::now(),
            etag: Some("abc123".to_string()),
            profile_count: 2,
        };
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

        // Read back
        let read_profiles: Vec<super::super::types::Profile> =
            serde_yaml::from_str(&std::fs::read_to_string(&data_path).unwrap()).unwrap();
        assert_eq!(read_profiles.len(), 2);
        assert_eq!(read_profiles[0].name, "Remote Profile 1");

        let read_meta: super::CacheMeta =
            serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(read_meta.url, url);
        assert_eq!(read_meta.profile_count, 2);
        assert_eq!(read_meta.etag, Some("abc123".to_string()));
    }

    // ── Merge tests ────────────────────────────────────────────────────

    #[test]
    fn test_merge_local_wins() {
        use super::super::types::{Profile, ProfileManager, ProfileSource};
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Shared Profile"));
        manager.add(Profile::new("Local Only"));

        let remote = vec![Profile::new("Shared Profile"), Profile::new("Remote Only")];

        super::merge_dynamic_profiles(
            &mut manager,
            &remote,
            "https://example.com/p.yaml",
            &super::ConflictResolution::LocalWins,
        );

        let names: Vec<String> = manager
            .profiles_ordered()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert!(names.contains(&"Shared Profile".to_string()));
        assert!(names.contains(&"Local Only".to_string()));
        assert!(names.contains(&"Remote Only".to_string()));

        // Shared Profile should still be Local
        let shared = manager.find_by_name("Shared Profile").unwrap();
        assert_eq!(shared.source, ProfileSource::Local);

        // Remote Only should be Dynamic
        let remote_p = manager.find_by_name("Remote Only").unwrap();
        assert!(matches!(remote_p.source, ProfileSource::Dynamic { .. }));
    }

    #[test]
    fn test_merge_remote_wins() {
        use super::super::types::{Profile, ProfileManager, ProfileSource};
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Shared Profile"));

        let remote = vec![Profile::new("Shared Profile")];

        super::merge_dynamic_profiles(
            &mut manager,
            &remote,
            "https://example.com/p.yaml",
            &super::ConflictResolution::RemoteWins,
        );

        let shared = manager.find_by_name("Shared Profile").unwrap();
        assert!(matches!(shared.source, ProfileSource::Dynamic { .. }));
    }

    #[test]
    fn test_merge_removes_stale_dynamic_profiles() {
        use super::super::types::{Profile, ProfileManager, ProfileSource};
        let mut manager = ProfileManager::new();
        let mut old = Profile::new("Old Remote");
        old.source = ProfileSource::Dynamic {
            url: "https://example.com/p.yaml".to_string(),
            last_fetched: None,
        };
        manager.add(old);

        let remote = vec![Profile::new("New Remote")];

        super::merge_dynamic_profiles(
            &mut manager,
            &remote,
            "https://example.com/p.yaml",
            &super::ConflictResolution::LocalWins,
        );

        let names: Vec<String> = manager
            .profiles_ordered()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert!(!names.contains(&"Old Remote".to_string()));
        assert!(names.contains(&"New Remote".to_string()));
    }
}
