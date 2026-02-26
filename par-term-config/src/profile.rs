//! Profile configuration types.
//!
//! Defines configuration types for profile management including
//! dynamic profile sources.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

fn default_true() -> bool {
    true
}

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
    #[serde(default = "default_true")]
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

        let yaml = serde_yml::to_string(&source).expect("serialize");
        let deserialized: DynamicProfileSource = serde_yml::from_str(&yaml).expect("deserialize");

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
        let source: DynamicProfileSource = serde_yml::from_str(yaml).expect("deserialize minimal");

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
}
