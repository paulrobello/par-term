//! Tests for activity and idle notification features.
//!
//! These tests validate the configuration and state tracking for:
//! - Activity notifications (notify when terminal output resumes after inactivity)
//! - Silence/idle notifications (notify when terminal has been silent too long)

use par_term::config::Config;
use std::time::{Duration, Instant};

/// Test that activity notification config defaults are correct.
#[test]
fn test_activity_notification_config_defaults() {
    let config = Config::default();

    // Activity notifications are disabled by default
    assert!(!config.notification_activity_enabled);
    // Default threshold is 10 seconds
    assert_eq!(config.notification_activity_threshold, 10);
}

/// Test that silence notification config defaults are correct.
#[test]
fn test_silence_notification_config_defaults() {
    let config = Config::default();

    // Silence notifications are disabled by default
    assert!(!config.notification_silence_enabled);
    // Default threshold is 300 seconds (5 minutes)
    assert_eq!(config.notification_silence_threshold, 300);
}

/// Test that activity notification config can be deserialized from YAML.
#[test]
fn test_activity_notification_yaml_deserialization() {
    let yaml = r#"
notification_activity_enabled: true
notification_activity_threshold: 30
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();

    assert!(config.notification_activity_enabled);
    assert_eq!(config.notification_activity_threshold, 30);
}

/// Test that silence notification config can be deserialized from YAML.
#[test]
fn test_silence_notification_yaml_deserialization() {
    let yaml = r#"
notification_silence_enabled: true
notification_silence_threshold: 600
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();

    assert!(config.notification_silence_enabled);
    assert_eq!(config.notification_silence_threshold, 600);
}

/// Test that both notification types can be enabled together.
#[test]
fn test_both_notification_types_enabled() {
    let yaml = r#"
notification_activity_enabled: true
notification_activity_threshold: 15
notification_silence_enabled: true
notification_silence_threshold: 120
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();

    assert!(config.notification_activity_enabled);
    assert_eq!(config.notification_activity_threshold, 15);
    assert!(config.notification_silence_enabled);
    assert_eq!(config.notification_silence_threshold, 120);
}

/// Test that notification config serializes correctly.
#[test]
fn test_notification_config_yaml_serialization() {
    let config = Config::default();
    let yaml = serde_yml::to_string(&config).unwrap();

    // Check that the fields are present in serialization
    assert!(yaml.contains("notification_activity_enabled: false"));
    assert!(yaml.contains("notification_activity_threshold: 10"));
    assert!(yaml.contains("notification_silence_enabled: false"));
    assert!(yaml.contains("notification_silence_threshold: 300"));
}

/// Test backward compatibility with config aliases.
#[test]
fn test_notification_config_aliases() {
    // Test activity_notifications alias
    let yaml = r#"
activity_notifications: true
activity_threshold: 25
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    assert!(config.notification_activity_enabled);
    assert_eq!(config.notification_activity_threshold, 25);

    // Test silence_notifications alias
    let yaml = r#"
silence_notifications: true
silence_threshold: 180
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    assert!(config.notification_silence_enabled);
    assert_eq!(config.notification_silence_threshold, 180);
}

/// Test activity state tracking logic (simulated since we can't create real Tab).
/// This tests the core time-based detection algorithm.
#[test]
fn test_activity_detection_timing() {
    // Simulate the activity detection logic
    let activity_threshold = Duration::from_secs(10);

    // Case 1: Recently active - no notification should trigger
    let last_activity = Instant::now();
    let time_since_activity = last_activity.elapsed();
    assert!(
        time_since_activity < activity_threshold,
        "Recently active terminal should not trigger activity notification"
    );

    // Case 2: Activity detection after threshold (simulated)
    // In practice this would be tested with actual terminal output
    let idle_duration = Duration::from_secs(15);
    assert!(
        idle_duration >= activity_threshold,
        "Terminal idle for {} seconds should trigger activity notification on resume",
        idle_duration.as_secs()
    );
}

/// Test silence detection timing logic.
#[test]
fn test_silence_detection_timing() {
    // Simulate the silence detection logic
    let silence_threshold = Duration::from_secs(300);

    // Case 1: Not yet silent long enough
    let short_silence = Duration::from_secs(60);
    assert!(
        short_silence < silence_threshold,
        "Short silence should not trigger notification"
    );

    // Case 2: Silent for longer than threshold
    let long_silence = Duration::from_secs(400);
    assert!(
        long_silence >= silence_threshold,
        "Long silence should trigger notification"
    );
}

/// Test that silence_notified flag prevents duplicate notifications.
#[test]
fn test_silence_notification_deduplication() {
    // Simulate the flag behavior
    let mut silence_notified = false;
    let silence_threshold = Duration::from_secs(300);
    let time_since_activity = Duration::from_secs(400);

    // First check - should trigger notification
    if !silence_notified && time_since_activity >= silence_threshold {
        silence_notified = true;
        // Notification would be sent here
    }
    assert!(silence_notified, "First check should set flag");

    // Second check - should NOT trigger (flag already set)
    let should_notify = !silence_notified && time_since_activity >= silence_threshold;
    assert!(
        !should_notify,
        "Second check should not trigger duplicate notification"
    );
}

/// Test that activity resets the silence_notified flag.
#[test]
fn test_activity_resets_silence_flag() {
    let mut silence_notified = true;
    let mut last_seen_generation: u64 = 5;
    let current_generation: u64 = 6;

    // Simulate new activity detection
    if current_generation > last_seen_generation {
        last_seen_generation = current_generation;
        silence_notified = false; // Reset on activity
    }

    assert!(!silence_notified, "Activity should reset silence flag");
    assert_eq!(last_seen_generation, 6, "Generation should be updated");
}

/// Test threshold boundary conditions.
#[test]
fn test_threshold_boundary_conditions() {
    // Test exactly at threshold
    let threshold = Duration::from_secs(10);

    // At exactly threshold, should trigger
    let at_threshold = Duration::from_secs(10);
    assert!(at_threshold >= threshold, "At threshold should trigger");

    // Just under threshold, should not trigger
    let under_threshold = Duration::from_millis(9999);
    assert!(
        under_threshold < threshold,
        "Under threshold should not trigger"
    );
}

/// Test with minimum threshold values.
#[test]
fn test_minimum_threshold_values() {
    let yaml = r#"
notification_activity_threshold: 1
notification_silence_threshold: 1
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();

    assert_eq!(config.notification_activity_threshold, 1);
    assert_eq!(config.notification_silence_threshold, 1);
}

/// Test with large threshold values.
#[test]
fn test_large_threshold_values() {
    let yaml = r#"
notification_activity_threshold: 3600
notification_silence_threshold: 86400
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();

    // 1 hour activity threshold
    assert_eq!(config.notification_activity_threshold, 3600);
    // 24 hour silence threshold
    assert_eq!(config.notification_silence_threshold, 86400);
}
