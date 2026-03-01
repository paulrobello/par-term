//! Cache storage for dynamic profiles.
//!
//! Provides functions to read and write fetched remote profiles to the local
//! filesystem cache, keyed by a SHA-256 hash of the source URL.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::SystemTime;

/// Get the cache directory for dynamic profiles.
pub fn cache_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("par-term")
        .join("cache")
        .join("dynamic_profiles")
}

/// Generate a deterministic filename from a URL.
pub fn url_to_cache_filename(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)
}

/// Cache metadata stored alongside profile data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    /// The source URL this cache entry corresponds to.
    pub url: String,
    /// When the profiles were last fetched.
    pub last_fetched: SystemTime,
    /// HTTP ETag header from the server (for conditional requests).
    pub etag: Option<String>,
    /// Number of profiles in the cached data.
    pub profile_count: usize,
}

/// Read cached profiles for a given URL.
pub fn read_cache(url: &str) -> anyhow::Result<(Vec<par_term_config::Profile>, CacheMeta)> {
    let dir = cache_dir();
    let hash = url_to_cache_filename(url);
    let data_path = dir.join(format!("{hash}.yaml"));
    let meta_path = dir.join(format!("{hash}.meta"));

    let data = std::fs::read_to_string(&data_path)
        .with_context(|| format!("Failed to read cache data from {data_path:?}"))?;
    let meta_str = std::fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read cache meta from {meta_path:?}"))?;

    let profiles: Vec<par_term_config::Profile> =
        serde_yaml_ng::from_str(&data).with_context(|| "Failed to parse cached profiles")?;
    let meta: CacheMeta =
        serde_json::from_str(&meta_str).with_context(|| "Failed to parse cache metadata")?;

    Ok((profiles, meta))
}

/// Write profiles and metadata to cache.
pub fn write_cache(
    url: &str,
    profiles: &[par_term_config::Profile],
    etag: Option<String>,
) -> anyhow::Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache directory {dir:?}"))?;

    let hash = url_to_cache_filename(url);
    let data_path = dir.join(format!("{hash}.yaml"));
    let meta_path = dir.join(format!("{hash}.meta"));

    let data = serde_yaml_ng::to_string(profiles)
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
