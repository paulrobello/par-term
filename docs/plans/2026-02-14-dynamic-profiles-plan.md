# Dynamic Profiles from Remote URLs - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Load profile definitions from remote URLs with auto-refresh, caching, conflict resolution, and Settings UI controls.

**Architecture:** New `src/profile/dynamic.rs` module handles fetching, caching, and merging. Config gets a `dynamic_profile_sources` field. A tokio background task per source fetches on a timer and sends updates via `mpsc` channel. `WindowManager` checks the channel in `about_to_wait()` and propagates merged profiles to all windows.

**Tech Stack:** ureq (HTTP), sha2 (URL hashing), serde_yaml (parsing), tokio (async timer + mpsc channel), dirs (cache paths)

---

### Task 1: Add `ProfileSource` to Profile struct

**Files:**
- Modify: `src/profile/types.rs:8-14` (add ProfileSource enum before ProfileId)
- Modify: `src/profile/types.rs:11-144` (add `source` field to Profile struct)

**Step 1: Add the ProfileSource enum**

In `src/profile/types.rs`, after line 7 (after the use statements), add:

```rust
/// Tracks where a profile came from (runtime-only, not persisted)
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ProfileSource {
    #[default]
    Local,
    Dynamic {
        url: String,
        last_fetched: Option<std::time::SystemTime>,
    },
}
```

**Step 2: Add source field to Profile struct**

In the `Profile` struct (after the `ssh_extra_args` field at line ~143), add:

```rust
    /// Where this profile was loaded from (runtime-only, not persisted to YAML)
    #[serde(skip)]
    pub source: ProfileSource,
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles successfully (serde skip means no serialization impact)

**Step 4: Commit**

```bash
git add src/profile/types.rs
git commit -m "feat(profile): add ProfileSource enum to track profile origin"
```

---

### Task 2: Add DynamicProfileSource config types

**Files:**
- Create: `src/profile/dynamic.rs`
- Modify: `src/profile/mod.rs:11-14` (add module export)

**Step 1: Create the dynamic module with config types**

Create `src/profile/dynamic.rs`:

```rust
//! Dynamic profile loading from remote URLs
//!
//! Fetches profile definitions from remote URLs, caches them locally,
//! and merges them into the ProfileManager with configurable conflict resolution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How to resolve name conflicts between remote and local profiles
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Local profiles take precedence (default)
    #[default]
    LocalWins,
    /// Remote profiles overwrite local profiles with the same name
    RemoteWins,
}

impl ConflictResolution {
    /// All variants for UI dropdowns
    pub fn variants() -> &'static [ConflictResolution] {
        &[ConflictResolution::LocalWins, ConflictResolution::RemoteWins]
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ConflictResolution::LocalWins => "Local Wins",
            ConflictResolution::RemoteWins => "Remote Wins",
        }
    }
}

fn default_refresh_interval() -> u64 {
    1800 // 30 minutes
}

fn default_max_size() -> usize {
    1_048_576 // 1 MB
}

fn default_fetch_timeout() -> u64 {
    10 // seconds
}

/// Configuration for a single remote profile source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicProfileSource {
    /// URL to fetch profiles from (YAML format)
    pub url: String,

    /// Custom HTTP headers (e.g., Authorization)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// How often to refresh in seconds (default: 1800 = 30 min)
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,

    /// Maximum download size in bytes (default: 1 MB)
    #[serde(default = "default_max_size")]
    pub max_size_bytes: usize,

    /// HTTP timeout in seconds (default: 10)
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,

    /// Whether this source is active
    #[serde(default = "super::super::config::defaults::bool_true")]
    pub enabled: bool,

    /// How to handle name conflicts with local profiles
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}

impl Default for DynamicProfileSource {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: HashMap::new(),
            refresh_interval_secs: default_refresh_interval(),
            max_size_bytes: default_max_size(),
            fetch_timeout_secs: default_fetch_timeout(),
            enabled: true,
            conflict_resolution: ConflictResolution::default(),
        }
    }
}
```

**Step 2: Export from profile module**

In `src/profile/mod.rs`, add the module and re-export:

```rust
pub mod dynamic;
pub mod storage;
pub mod types;

pub use dynamic::{ConflictResolution, DynamicProfileSource};
pub use types::{Profile, ProfileId, ProfileManager, ProfileSource};
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles (no consumers yet)

**Step 4: Commit**

```bash
git add src/profile/dynamic.rs src/profile/mod.rs
git commit -m "feat(profile): add DynamicProfileSource config types"
```

---

### Task 3: Write tests for DynamicProfileSource serialization

**Files:**
- Modify: `src/profile/dynamic.rs` (add tests module)

**Step 1: Add serialization roundtrip tests**

Append to `src/profile/dynamic.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_source() {
        let source = DynamicProfileSource::default();
        assert_eq!(source.refresh_interval_secs, 1800);
        assert_eq!(source.max_size_bytes, 1_048_576);
        assert_eq!(source.fetch_timeout_secs, 10);
        assert!(source.enabled);
        assert_eq!(source.conflict_resolution, ConflictResolution::LocalWins);
        assert!(source.headers.is_empty());
        assert!(source.url.is_empty());
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());

        let source = DynamicProfileSource {
            url: "https://example.com/profiles.yaml".to_string(),
            headers,
            refresh_interval_secs: 600,
            max_size_bytes: 512_000,
            fetch_timeout_secs: 15,
            enabled: true,
            conflict_resolution: ConflictResolution::RemoteWins,
        };

        let yaml = serde_yaml::to_string(&source).unwrap();
        let deserialized: DynamicProfileSource = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.url, "https://example.com/profiles.yaml");
        assert_eq!(
            deserialized.headers.get("Authorization").unwrap(),
            "Bearer token123"
        );
        assert_eq!(deserialized.refresh_interval_secs, 600);
        assert_eq!(deserialized.max_size_bytes, 512_000);
        assert_eq!(deserialized.fetch_timeout_secs, 15);
        assert!(deserialized.enabled);
        assert_eq!(
            deserialized.conflict_resolution,
            ConflictResolution::RemoteWins
        );
    }

    #[test]
    fn test_deserialize_minimal_yaml() {
        let yaml = r#"url: "https://example.com/profiles.yaml""#;
        let source: DynamicProfileSource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(source.url, "https://example.com/profiles.yaml");
        assert_eq!(source.refresh_interval_secs, 1800); // default
        assert_eq!(source.max_size_bytes, 1_048_576); // default
        assert!(source.enabled); // default true
        assert_eq!(source.conflict_resolution, ConflictResolution::LocalWins); // default
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
    }
}
```

**Step 2: Run tests**

Run: `cargo test profile::dynamic -- -v`
Expected: All 5 tests pass

**Step 3: Commit**

```bash
git add src/profile/dynamic.rs
git commit -m "test(profile): add DynamicProfileSource serialization tests"
```

---

### Task 4: Add `dynamic_profile_sources` to Config

**Files:**
- Modify: `src/config/mod.rs:1717` (add field before closing brace)
- Modify: `src/config/mod.rs:2033` (add to Default impl)

**Step 1: Add field to Config struct**

In `src/config/mod.rs`, before the closing `}` of the Config struct (line 1718), after the `collapsed_settings_sections` field, add:

```rust
    // ========================================================================
    // Dynamic Profile Sources
    // ========================================================================
    /// Remote URLs to fetch profile definitions from
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_profile_sources: Vec<crate::profile::DynamicProfileSource>,
```

**Step 2: Add to Default impl**

In the `Default` impl for Config, before the closing `}` (after `collapsed_settings_sections: Vec::new(),` at line ~2033), add:

```rust
            dynamic_profile_sources: Vec::new(),
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles successfully

**Step 4: Run existing config tests**

Run: `cargo test config -- -v`
Expected: All existing tests pass (empty default, serde skip_serializing_if means no YAML impact)

**Step 5: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add dynamic_profile_sources field"
```

---

### Task 5: Implement cache storage for dynamic profiles

**Files:**
- Modify: `src/profile/dynamic.rs` (add cache functions)

**Step 1: Add cache path helpers and read/write functions**

Add to `src/profile/dynamic.rs`, after the `DynamicProfileSource` impl block:

```rust
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::SystemTime;

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
    pub url: String,
    pub last_fetched: SystemTime,
    pub etag: Option<String>,
    pub profile_count: usize,
}

/// Read cached profiles for a given URL
pub fn read_cache(url: &str) -> anyhow::Result<(Vec<super::types::Profile>, CacheMeta)> {
    let dir = cache_dir();
    let hash = url_to_cache_filename(url);

    let data_path = dir.join(format!("{}.yaml", hash));
    let meta_path = dir.join(format!("{}.meta", hash));

    let data = std::fs::read_to_string(&data_path)
        .with_context(|| format!("Failed to read cache data from {:?}", data_path))?;
    let meta_str = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read cache meta from {:?}", meta_path))?;

    let profiles: Vec<super::types::Profile> = serde_yaml::from_str(&data)
        .with_context(|| "Failed to parse cached profiles")?;
    let meta: CacheMeta = serde_json::from_str(&meta_str)
        .with_context(|| "Failed to parse cache metadata")?;

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
        .with_context(|| format!("Failed to create cache directory {:?}", dir))?;

    let hash = url_to_cache_filename(url);
    let data_path = dir.join(format!("{}.yaml", hash));
    let meta_path = dir.join(format!("{}.meta", hash));

    let data = serde_yaml::to_string(profiles)
        .with_context(|| "Failed to serialize profiles for cache")?;
    std::fs::write(&data_path, data)
        .with_context(|| format!("Failed to write cache data to {:?}", data_path))?;

    let meta = CacheMeta {
        url: url.to_string(),
        last_fetched: SystemTime::now(),
        etag,
        profile_count: profiles.len(),
    };
    let meta_str = serde_json::to_string_pretty(&meta)
        .with_context(|| "Failed to serialize cache metadata")?;
    std::fs::write(&meta_path, meta_str)
        .with_context(|| format!("Failed to write cache meta to {:?}", meta_path))?;

    Ok(())
}

use anyhow::Context;
```

**Step 2: Add cache tests**

Append to the existing `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn test_url_to_cache_filename_deterministic() {
        let url = "https://example.com/profiles.yaml";
        let a = url_to_cache_filename(url);
        let b = url_to_cache_filename(url);
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn test_url_to_cache_filename_different_urls() {
        let a = url_to_cache_filename("https://example.com/a.yaml");
        let b = url_to_cache_filename("https://example.com/b.yaml");
        assert_ne!(a, b);
    }

    #[test]
    fn test_cache_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        // Override cache dir by using write/read directly with temp paths
        let url = "https://test.example.com/profiles.yaml";
        let profiles = vec![
            super::super::types::Profile::new("Remote Profile 1"),
            super::super::types::Profile::new("Remote Profile 2"),
        ];

        // Write to temp dir manually (since cache_dir uses real XDG path)
        let hash = url_to_cache_filename(url);
        let data_path = temp.path().join(format!("{}.yaml", hash));
        let meta_path = temp.path().join(format!("{}.meta", hash));

        let data = serde_yaml::to_string(&profiles).unwrap();
        std::fs::write(&data_path, &data).unwrap();

        let meta = CacheMeta {
            url: url.to_string(),
            last_fetched: SystemTime::now(),
            etag: Some("abc123".to_string()),
            profile_count: 2,
        };
        let meta_str = serde_json::to_string_pretty(&meta).unwrap();
        std::fs::write(&meta_path, &meta_str).unwrap();

        // Read back
        let read_data = std::fs::read_to_string(&data_path).unwrap();
        let read_profiles: Vec<super::super::types::Profile> =
            serde_yaml::from_str(&read_data).unwrap();
        assert_eq!(read_profiles.len(), 2);
        assert_eq!(read_profiles[0].name, "Remote Profile 1");

        let read_meta_str = std::fs::read_to_string(&meta_path).unwrap();
        let read_meta: CacheMeta = serde_json::from_str(&read_meta_str).unwrap();
        assert_eq!(read_meta.url, url);
        assert_eq!(read_meta.profile_count, 2);
        assert_eq!(read_meta.etag, Some("abc123".to_string()));
    }
```

**Step 3: Run tests**

Run: `cargo test profile::dynamic -- -v`
Expected: All tests pass (including the new cache tests)

**Step 4: Commit**

```bash
git add src/profile/dynamic.rs
git commit -m "feat(profile): add cache storage for dynamic profiles"
```

---

### Task 6: Implement HTTP fetch and profile parsing

**Files:**
- Modify: `src/profile/dynamic.rs` (add fetch_profiles function)

**Step 1: Add fetch function**

Add to `src/profile/dynamic.rs`:

```rust
/// Result of fetching profiles from a remote source
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub profiles: Vec<super::types::Profile>,
    pub etag: Option<String>,
    pub error: Option<String>,
}

/// Fetch profiles from a remote URL
///
/// Returns parsed profiles or an error. On success, also writes to cache.
pub fn fetch_profiles(source: &DynamicProfileSource) -> FetchResult {
    let url = &source.url;
    crate::debug_info!("DYNAMIC_PROFILE", "Fetching profiles from {}", url);

    let result = fetch_profiles_inner(source);

    match result {
        Ok((profiles, etag)) => {
            crate::debug_info!(
                "DYNAMIC_PROFILE",
                "Successfully fetched {} profiles from {}",
                profiles.len(),
                url
            );

            // Write to cache
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
            crate::debug_error!(
                "DYNAMIC_PROFILE",
                "Failed to fetch profiles from {}: {}",
                url,
                e
            );

            FetchResult {
                url: url.clone(),
                profiles: Vec::new(),
                etag: None,
                error: Some(e.to_string()),
            }
        }
    }
}

fn fetch_profiles_inner(
    source: &DynamicProfileSource,
) -> anyhow::Result<(Vec<super::types::Profile>, Option<String>)> {
    use std::io::Read;

    // Build request with custom headers
    let mut request = ureq::get(&source.url)
        .timeout(std::time::Duration::from_secs(source.fetch_timeout_secs));

    for (key, value) in &source.headers {
        request = request.header(key, value);
    }

    let response = request.call()
        .with_context(|| format!("HTTP request failed for {}", source.url))?;

    let etag = response.headers().get("etag").map(|s| s.to_string());

    // Read body with size limit
    let mut body = String::new();
    response
        .into_body()
        .as_reader()
        .take(source.max_size_bytes as u64)
        .read_to_string(&mut body)
        .with_context(|| format!("Failed to read response body from {}", source.url))?;

    if body.len() >= source.max_size_bytes {
        anyhow::bail!(
            "Response from {} exceeds max size of {} bytes",
            source.url,
            source.max_size_bytes
        );
    }

    // Parse YAML
    let profiles: Vec<super::types::Profile> = serde_yaml::from_str(&body)
        .with_context(|| format!("Failed to parse YAML from {}", source.url))?;

    Ok((profiles, etag))
}
```

**Step 2: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles (ureq API may need adjustment based on exact ureq 3.x API — check and adjust)

**Step 3: Commit**

```bash
git add src/profile/dynamic.rs
git commit -m "feat(profile): implement HTTP fetch for dynamic profiles"
```

---

### Task 7: Implement profile merge logic

**Files:**
- Modify: `src/profile/dynamic.rs` (add merge function)

**Step 1: Write failing merge test first**

Add to the test module in `src/profile/dynamic.rs`:

```rust
    #[test]
    fn test_merge_local_wins() {
        let mut manager = super::super::types::ProfileManager::new();
        manager.add(super::super::types::Profile::new("Shared Profile"));
        manager.add(super::super::types::Profile::new("Local Only"));

        let remote_profiles = vec![
            super::super::types::Profile::new("Shared Profile"),
            super::super::types::Profile::new("Remote Only"),
        ];

        let url = "https://example.com/profiles.yaml";
        merge_dynamic_profiles(
            &mut manager,
            &remote_profiles,
            url,
            &ConflictResolution::LocalWins,
        );

        // Local "Shared Profile" should still be there (local wins)
        // "Remote Only" should be added
        // "Local Only" should still be there
        let names: Vec<String> = manager
            .profiles_ordered()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert!(names.contains(&"Shared Profile".to_string()));
        assert!(names.contains(&"Local Only".to_string()));
        assert!(names.contains(&"Remote Only".to_string()));

        // The "Shared Profile" should be Local source (local won)
        let shared = manager.find_by_name("Shared Profile").unwrap();
        assert_eq!(shared.source, super::super::types::ProfileSource::Local);

        // The "Remote Only" should be Dynamic source
        let remote = manager.find_by_name("Remote Only").unwrap();
        assert!(matches!(
            remote.source,
            super::super::types::ProfileSource::Dynamic { .. }
        ));
    }

    #[test]
    fn test_merge_remote_wins() {
        let mut manager = super::super::types::ProfileManager::new();
        manager.add(super::super::types::Profile::new("Shared Profile"));

        let remote_profiles = vec![
            super::super::types::Profile::new("Shared Profile"),
        ];

        let url = "https://example.com/profiles.yaml";
        merge_dynamic_profiles(
            &mut manager,
            &remote_profiles,
            url,
            &ConflictResolution::RemoteWins,
        );

        // The "Shared Profile" should now be Dynamic source (remote won)
        let shared = manager.find_by_name("Shared Profile").unwrap();
        assert!(matches!(
            shared.source,
            super::super::types::ProfileSource::Dynamic { .. }
        ));
    }

    #[test]
    fn test_merge_removes_stale_dynamic_profiles() {
        let mut manager = super::super::types::ProfileManager::new();
        // Simulate a previously-fetched dynamic profile
        let mut old_remote = super::super::types::Profile::new("Old Remote");
        old_remote.source = super::super::types::ProfileSource::Dynamic {
            url: "https://example.com/profiles.yaml".to_string(),
            last_fetched: None,
        };
        manager.add(old_remote);

        // New fetch returns a different set
        let remote_profiles = vec![
            super::super::types::Profile::new("New Remote"),
        ];

        let url = "https://example.com/profiles.yaml";
        merge_dynamic_profiles(
            &mut manager,
            &remote_profiles,
            url,
            &ConflictResolution::LocalWins,
        );

        let names: Vec<String> = manager
            .profiles_ordered()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert!(!names.contains(&"Old Remote".to_string()));
        assert!(names.contains(&"New Remote".to_string()));
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test profile::dynamic::tests::test_merge -- -v`
Expected: FAIL (merge function doesn't exist yet)

**Step 3: Implement merge function**

Add to `src/profile/dynamic.rs`:

```rust
/// Merge dynamic profiles into a ProfileManager
///
/// 1. Remove all existing dynamic profiles from this URL
/// 2. For each remote profile, check for name conflicts with local profiles
/// 3. Apply conflict resolution strategy
/// 4. Mark merged profiles with Dynamic source
pub fn merge_dynamic_profiles(
    manager: &mut super::types::ProfileManager,
    remote_profiles: &[super::types::Profile],
    url: &str,
    conflict_resolution: &ConflictResolution,
) {
    // Step 1: Remove existing dynamic profiles from this URL
    let to_remove: Vec<super::types::ProfileId> = manager
        .profiles_ordered()
        .iter()
        .filter(|p| matches!(&p.source, super::types::ProfileSource::Dynamic { url: u, .. } if u == url))
        .map(|p| p.id)
        .collect();

    for id in &to_remove {
        manager.remove(id);
    }

    // Step 2: Merge remote profiles
    let now = std::time::SystemTime::now();
    for remote in remote_profiles {
        let existing = manager.find_by_name(&remote.name);

        match (existing, conflict_resolution) {
            // Name conflict, local wins: skip remote profile
            (Some(_), ConflictResolution::LocalWins) => {
                crate::debug_info!(
                    "DYNAMIC_PROFILE",
                    "Skipping remote profile '{}' (local wins)",
                    remote.name
                );
            }
            // Name conflict, remote wins: remove local, add remote
            (Some(local), ConflictResolution::RemoteWins) => {
                let local_id = local.id;
                manager.remove(&local_id);
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4(); // new ID for remote
                profile.source = super::types::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!(
                    "DYNAMIC_PROFILE",
                    "Remote profile '{}' overwrites local (remote wins)",
                    remote.name
                );
            }
            // No conflict: add remote profile
            (None, _) => {
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4();
                profile.source = super::types::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!(
                    "DYNAMIC_PROFILE",
                    "Added remote profile '{}' from {}",
                    remote.name,
                    url
                );
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test profile::dynamic -- -v`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/profile/dynamic.rs
git commit -m "feat(profile): implement merge logic for dynamic profiles"
```

---

### Task 8: Implement background fetch manager

**Files:**
- Modify: `src/profile/dynamic.rs` (add DynamicProfileManager)

**Step 1: Add the background fetch manager**

Add to `src/profile/dynamic.rs`:

```rust
use std::sync::Arc;
use tokio::sync::mpsc;

/// Message sent from background fetch tasks to the main thread
#[derive(Debug, Clone)]
pub struct DynamicProfileUpdate {
    pub url: String,
    pub profiles: Vec<super::types::Profile>,
    pub conflict_resolution: ConflictResolution,
    pub error: Option<String>,
}

/// Status of a dynamic profile source
#[derive(Debug, Clone)]
pub struct SourceStatus {
    pub url: String,
    pub enabled: bool,
    pub last_fetch: Option<SystemTime>,
    pub last_error: Option<String>,
    pub profile_count: usize,
    pub fetching: bool,
}

/// Manages background fetching of dynamic profiles
pub struct DynamicProfileManager {
    /// Channel receiver for updates from background tasks
    pub update_rx: mpsc::UnboundedReceiver<DynamicProfileUpdate>,
    /// Channel sender (cloned to background tasks)
    update_tx: mpsc::UnboundedSender<DynamicProfileUpdate>,
    /// Status of each source
    pub statuses: HashMap<String, SourceStatus>,
    /// Handles to cancel background tasks
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl DynamicProfileManager {
    /// Create a new DynamicProfileManager
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
    /// Also does an immediate fetch from cache for each source.
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
            let handle = runtime.spawn(async move {
                // Initial fetch
                let result = fetch_profiles(&source_clone);
                let _ = tx.send(DynamicProfileUpdate {
                    url: result.url.clone(),
                    profiles: result.profiles,
                    conflict_resolution: source_clone.conflict_resolution.clone(),
                    error: result.error,
                });

                // Periodic refresh
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                    source_clone.refresh_interval_secs,
                ));
                interval.tick().await; // Skip first immediate tick
                loop {
                    interval.tick().await;
                    let result = fetch_profiles(&source_clone);
                    if tx
                        .send(DynamicProfileUpdate {
                            url: result.url.clone(),
                            profiles: result.profiles,
                            conflict_resolution: source_clone.conflict_resolution.clone(),
                            error: result.error,
                        })
                        .is_err()
                    {
                        break; // Receiver dropped, stop task
                    }
                }
            });

            self.task_handles.push(handle);
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
        &self,
        sources: &[DynamicProfileSource],
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        for source in sources {
            if !source.enabled || source.url.is_empty() {
                continue;
            }
            let tx = self.update_tx.clone();
            let source_clone = source.clone();
            runtime.spawn(async move {
                let result = fetch_profiles(&source_clone);
                let _ = tx.send(DynamicProfileUpdate {
                    url: result.url.clone(),
                    profiles: result.profiles,
                    conflict_resolution: source_clone.conflict_resolution.clone(),
                    error: result.error,
                });
            });
        }
    }

    /// Trigger an immediate refresh of a specific source
    pub fn refresh_source(
        &self,
        source: &DynamicProfileSource,
        runtime: &Arc<tokio::runtime::Runtime>,
    ) {
        let tx = self.update_tx.clone();
        let source_clone = source.clone();
        runtime.spawn(async move {
            let result = fetch_profiles(&source_clone);
            let _ = tx.send(DynamicProfileUpdate {
                url: result.url.clone(),
                profiles: result.profiles,
                conflict_resolution: source_clone.conflict_resolution.clone(),
                error: result.error,
            });
        });
    }

    /// Check for pending updates (non-blocking)
    pub fn try_recv(&mut self) -> Option<DynamicProfileUpdate> {
        self.update_rx.try_recv().ok()
    }

    /// Update status after receiving an update
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

impl Drop for DynamicProfileManager {
    fn drop(&mut self) {
        self.stop();
    }
}
```

**Step 2: Update module re-exports**

In `src/profile/mod.rs`, add to re-exports:

```rust
pub use dynamic::{
    ConflictResolution, DynamicProfileManager, DynamicProfileSource, DynamicProfileUpdate,
    SourceStatus,
};
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add src/profile/dynamic.rs src/profile/mod.rs
git commit -m "feat(profile): implement background fetch manager for dynamic profiles"
```

---

### Task 9: Wire DynamicProfileManager into WindowManager

**Files:**
- Modify: `src/app/window_manager.rs:23-56` (add field to struct)
- Modify: `src/app/window_manager.rs:58-82` (add to constructor)
- Modify: `src/app/handler.rs:1327-1380` (check channel in about_to_wait)

**Step 1: Add DynamicProfileManager field to WindowManager**

In `src/app/window_manager.rs`, add to the struct:

```rust
    /// Dynamic profile manager for fetching remote profiles
    pub(crate) dynamic_profile_manager: crate::profile::DynamicProfileManager,
```

In the constructor `new()`, initialize it and start it:

```rust
        let mut dynamic_profile_manager = crate::profile::DynamicProfileManager::new();
        if !config.dynamic_profile_sources.is_empty() {
            dynamic_profile_manager.start(&config.dynamic_profile_sources, &runtime);
        }
```

Add the field to the struct init.

**Step 2: Check for updates in about_to_wait**

In `src/app/handler.rs`, inside the `about_to_wait` method on `WindowManager` (after the existing per-window loop at line ~1379), add:

```rust
        // Check for dynamic profile updates
        while let Some(update) = self.dynamic_profile_manager.try_recv() {
            self.dynamic_profile_manager.update_status(&update);

            // Merge into all window profile managers
            for window_state in self.windows.values_mut() {
                crate::profile::dynamic::merge_dynamic_profiles(
                    &mut window_state.profile_manager,
                    &update.profiles,
                    &update.url,
                    &update.conflict_resolution,
                );
                window_state.profiles_menu_needs_update = true;
            }

            log::info!(
                "Dynamic profiles updated from {}: {} profiles{}",
                update.url,
                update.profiles.len(),
                update.error.as_ref().map_or(String::new(), |e| format!(" (error: {})", e))
            );
        }
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add src/app/window_manager.rs src/app/handler.rs
git commit -m "feat(app): wire dynamic profile manager into event loop"
```

---

### Task 10: Add `reload_dynamic_profiles` keybinding action

**Files:**
- Modify: `src/app/input_events.rs:1136` (add to execute_keybinding_action match)

**Step 1: Add the action**

In `src/app/input_events.rs`, in the `execute_keybinding_action` match (after an existing action), add:

```rust
            "reload_dynamic_profiles" => {
                // Set a flag that WindowManager checks in about_to_wait
                self.reload_dynamic_profiles_requested = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Dynamic profiles reload requested via keybinding");
                true
            }
```

**Step 2: Add the flag to WindowState**

In `src/app/window_state.rs`, add the field:

```rust
    pub(crate) reload_dynamic_profiles_requested: bool,
```

Initialize to `false` in the constructor.

**Step 3: Handle the flag in WindowManager's about_to_wait**

In `src/app/handler.rs` about_to_wait, in the per-window loop, add:

```rust
            if window_state.reload_dynamic_profiles_requested {
                window_state.reload_dynamic_profiles_requested = false;
                self.dynamic_profile_manager.refresh_all(
                    &self.config.dynamic_profile_sources,
                    &self.runtime,
                );
            }
```

**Step 4: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles

**Step 5: Commit**

```bash
git add src/app/input_events.rs src/app/window_state.rs src/app/handler.rs
git commit -m "feat(keybinding): add reload_dynamic_profiles action"
```

---

### Task 11: Add Settings UI for dynamic profile sources

**Files:**
- Modify: `src/settings_ui/profiles_tab.rs` (add dynamic sources section)
- Modify: `src/settings_ui/sidebar.rs` (add search keywords)

**Step 1: Add dynamic sources UI section to profiles_tab.rs**

In `src/settings_ui/profiles_tab.rs`, after the existing profile management section, add a "Dynamic Profile Sources" collapsible section. The UI should include:

- A list of configured sources showing URL, status, profile count
- "Add Source" button
- For each source: URL text field, headers key-value editor, interval slider (5-60 min), max size input, enabled toggle, conflict resolution dropdown, "Refresh Now" button, "Remove" button
- "Refresh All" button

This section needs access to `config.dynamic_profile_sources` (via `settings.config`). Use the same `egui::CollapsingHeader` and `egui::Grid` patterns as existing sections.

For the headers editor, use a simple key-value list with add/remove buttons.

Note: The settings UI communicates changes via `settings.has_changes = true` and `*changes_this_frame = true`. The dynamic profile manager restart happens when config is applied.

**Step 2: Update search keywords**

In `src/settings_ui/sidebar.rs`, in the `SettingsTab::Profiles` arm of `tab_search_keywords()`, add:

```rust
            "dynamic",
            "remote url",
            "fetch",
            "refresh",
            "team",
            "shared",
            "download",
            "sync",
            "dynamic profiles",
```

**Step 3: Verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiles

**Step 4: Commit**

```bash
git add src/settings_ui/profiles_tab.rs src/settings_ui/sidebar.rs
git commit -m "feat(settings): add dynamic profile sources UI"
```

---

### Task 12: Restart DynamicProfileManager on config change

**Files:**
- Modify: `src/app/handler.rs` or wherever config is applied after settings change

**Step 1: Find where config changes are applied**

Search for where the config is reloaded/applied after settings changes. Look for `reload_config` or settings window save handling. When the config changes, restart the dynamic profile manager:

```rust
self.dynamic_profile_manager.stop();
self.dynamic_profile_manager.start(
    &self.config.dynamic_profile_sources,
    &self.runtime,
);
```

**Step 2: Verify compilation**

Run: `cargo build 2>&1 | head -30`

**Step 3: Commit**

```bash
git add -A
git commit -m "feat(app): restart dynamic profile manager on config change"
```

---

### Task 13: Add visual indicator for dynamic profiles

**Files:**
- Modify: `src/profile_modal_ui.rs` (show "[dynamic]" badge in profile list)
- Modify: `src/profile_drawer_ui.rs` (show indicator in drawer)

**Step 1: Add dynamic indicator to profile list**

In the profile list rendering (wherever profile names are displayed in `profile_modal_ui.rs`), check `profile.source` and add a visual indicator:

```rust
if matches!(profile.source, ProfileSource::Dynamic { .. }) {
    ui.label(egui::RichText::new(" [dynamic]").color(egui::Color32::from_rgb(100, 180, 255)).small());
}
```

**Step 2: Make dynamic profiles read-only in the editor**

In the profile editor, if the profile is dynamic, show fields as read-only or disable the save button with a message "Dynamic profiles are managed remotely."

**Step 3: Add indicator in profile drawer**

In `src/profile_drawer_ui.rs`, add a similar visual indicator for dynamic profiles.

**Step 4: Verify compilation**

Run: `cargo build 2>&1 | head -30`

**Step 5: Commit**

```bash
git add src/profile_modal_ui.rs src/profile_drawer_ui.rs
git commit -m "feat(ui): add visual indicators for dynamic profiles"
```

---

### Task 14: Run full test suite and fix issues

**Files:** Various

**Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings

**Step 3: Check formatting**

Run: `cargo fmt -- --check`
Expected: No formatting issues

**Step 4: Fix any issues found**

Address any compilation errors, test failures, or lint warnings.

**Step 5: Commit fixes if needed**

```bash
git add -A
git commit -m "fix: address lint and test issues for dynamic profiles"
```

---

### Task 15: Manual integration test

**Step 1: Create a test profiles YAML file**

Create a file at a local HTTP-accessible path or use `file://` URL for testing:

```yaml
# ~/.config/par-term/test-remote-profiles.yaml
- name: "Remote Dev Profile"
  shell: "/bin/bash"
  working_directory: "/tmp"
  tags: ["remote", "dev"]
- name: "Remote Prod Profile"
  shell: "/bin/zsh"
  tags: ["remote", "prod"]
```

**Step 2: Configure a dynamic source**

Add to `~/.config/par-term/config.yaml`:

```yaml
dynamic_profile_sources:
  - url: "file:///Users/<you>/.config/par-term/test-remote-profiles.yaml"
    refresh_interval_secs: 60
    enabled: true
```

**Step 3: Build and run**

Run: `cargo build --release && cargo run --release`

**Step 4: Verify**

- Check profiles list shows the remote profiles with [dynamic] indicator
- Check Settings > Profiles shows the dynamic sources section
- Check manual refresh works
- Check cache files exist in `~/.config/par-term/cache/dynamic_profiles/`

**Step 5: Clean up test config**

Remove the test `dynamic_profile_sources` entry from config.

---

### Task 16: Final commit and verify

**Step 1: Run full checks**

Run: `make checkall` (or `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test`)

**Step 2: Verify all clean**

Expected: All checks pass

**Step 3: Final commit if needed**

```bash
git add -A
git commit -m "chore: final cleanup for dynamic profiles feature"
```
