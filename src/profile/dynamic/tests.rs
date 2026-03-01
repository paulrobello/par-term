//! Tests for the dynamic profile module.

use std::collections::HashMap;

use super::cache::{CacheMeta, url_to_cache_filename};
use super::merge::merge_dynamic_profiles;
use par_term_config::{ConflictResolution, DynamicProfileSource};

// ── Source configuration tests ─────────────────────────────────────────

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
        allow_http: false,
    };

    let yaml = serde_yaml_ng::to_string(&source).expect("serialize");
    let deserialized: DynamicProfileSource = serde_yaml_ng::from_str(&yaml).expect("deserialize");

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
    let source: DynamicProfileSource = serde_yaml_ng::from_str(yaml).expect("deserialize minimal");

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

// ── Cache tests ────────────────────────────────────────────────────────

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
    let url = "https://test.example.com/profiles.yaml";
    let profiles = vec![
        par_term_config::Profile::new("Remote Profile 1"),
        par_term_config::Profile::new("Remote Profile 2"),
    ];
    let hash = url_to_cache_filename(url);
    let data_path = temp.path().join(format!("{hash}.yaml"));
    let meta_path = temp.path().join(format!("{hash}.meta"));

    // Write
    let data = serde_yaml_ng::to_string(&profiles).unwrap();
    std::fs::write(&data_path, &data).unwrap();
    let meta = CacheMeta {
        url: url.to_string(),
        last_fetched: std::time::SystemTime::now(),
        etag: Some("abc123".to_string()),
        profile_count: 2,
    };
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    // Read back
    let read_profiles: Vec<par_term_config::Profile> =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&data_path).unwrap()).unwrap();
    assert_eq!(read_profiles.len(), 2);
    assert_eq!(read_profiles[0].name, "Remote Profile 1");

    let read_meta: CacheMeta =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    assert_eq!(read_meta.url, url);
    assert_eq!(read_meta.profile_count, 2);
    assert_eq!(read_meta.etag, Some("abc123".to_string()));
}

// ── Merge tests ────────────────────────────────────────────────────────

#[test]
fn test_merge_local_wins() {
    use par_term_config::{Profile, ProfileManager, ProfileSource};
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Shared Profile"));
    manager.add(Profile::new("Local Only"));

    let remote = vec![Profile::new("Shared Profile"), Profile::new("Remote Only")];

    merge_dynamic_profiles(
        &mut manager,
        &remote,
        "https://example.com/p.yaml",
        &ConflictResolution::LocalWins,
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
    use par_term_config::{Profile, ProfileManager, ProfileSource};
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Shared Profile"));

    let remote = vec![Profile::new("Shared Profile")];

    merge_dynamic_profiles(
        &mut manager,
        &remote,
        "https://example.com/p.yaml",
        &ConflictResolution::RemoteWins,
    );

    let shared = manager.find_by_name("Shared Profile").unwrap();
    assert!(matches!(shared.source, ProfileSource::Dynamic { .. }));
}

#[test]
fn test_merge_removes_stale_dynamic_profiles() {
    use par_term_config::{Profile, ProfileManager, ProfileSource};
    let mut manager = ProfileManager::new();
    let mut old = Profile::new("Old Remote");
    old.source = ProfileSource::Dynamic {
        url: "https://example.com/p.yaml".to_string(),
        last_fetched: None,
    };
    manager.add(old);

    let remote = vec![Profile::new("New Remote")];

    merge_dynamic_profiles(
        &mut manager,
        &remote,
        "https://example.com/p.yaml",
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
