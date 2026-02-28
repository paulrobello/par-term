//! Tests for session ended notification and notification suppression features.
//!
//! These tests validate the configuration and behavior for:
//! - Session ended notifications (notify when a shell process exits)
//! - Suppress notifications when focused (skip desktop notifications when window is focused)

use par_term::config::Config;

/// Test that session ended notification config defaults are correct.
#[test]
fn test_session_ended_notification_config_defaults() {
    let config = Config::default();

    // Session ended notifications are disabled by default
    assert!(!config.notification_session_ended);
}

/// Test that suppress notifications when focused config defaults are correct.
#[test]
fn test_suppress_notifications_when_focused_config_defaults() {
    let config = Config::default();

    // Suppression when focused is enabled by default
    assert!(config.suppress_notifications_when_focused);
}

/// Test that session ended notification config can be deserialized from YAML.
#[test]
fn test_session_ended_notification_yaml_deserialization() {
    let yaml = r#"
notification_session_ended: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.notification_session_ended);

    let yaml = r#"
notification_session_ended: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(!config.notification_session_ended);
}

/// Test that suppress notifications when focused config can be deserialized from YAML.
#[test]
fn test_suppress_notifications_when_focused_yaml_deserialization() {
    let yaml = r#"
suppress_notifications_when_focused: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.suppress_notifications_when_focused);

    let yaml = r#"
suppress_notifications_when_focused: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(!config.suppress_notifications_when_focused);
}

/// Test backward compatibility with session_ended alias.
#[test]
fn test_session_ended_config_alias() {
    let yaml = r#"
session_ended: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.notification_session_ended);

    let yaml = r#"
session_ended: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(!config.notification_session_ended);
}

/// Test that notification config serializes correctly.
#[test]
fn test_session_notification_config_yaml_serialization() {
    let config = Config::default();
    let yaml = serde_yaml_ng::to_string(&config).unwrap();

    // Check that the fields are present in serialization
    assert!(yaml.contains("notification_session_ended: false"));
    assert!(yaml.contains("suppress_notifications_when_focused: true"));
}

/// Test that all notification settings can be configured together.
#[test]
fn test_all_notification_settings_together() {
    let yaml = r#"
notification_bell_desktop: true
notification_activity_enabled: true
notification_silence_enabled: true
notification_session_ended: true
suppress_notifications_when_focused: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();

    assert!(config.notification_bell_desktop);
    assert!(config.notification_activity_enabled);
    assert!(config.notification_silence_enabled);
    assert!(config.notification_session_ended);
    assert!(!config.suppress_notifications_when_focused);
}

/// Test that session ended and suppress can have opposite defaults.
#[test]
fn test_session_ended_with_suppress_disabled() {
    let yaml = r#"
notification_session_ended: true
suppress_notifications_when_focused: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();

    assert!(config.notification_session_ended);
    assert!(!config.suppress_notifications_when_focused);
}

/// Test exit_notified flag deduplication logic (simulated).
#[test]
fn test_exit_notification_deduplication() {
    // Simulate the flag behavior
    let mut exit_notified = false;
    let shell_exited = true;

    // First check - should trigger notification
    if !exit_notified && shell_exited {
        exit_notified = true;
        // Notification would be sent here
    }
    assert!(exit_notified, "First check should set flag");

    // Second check - should NOT trigger (flag already set)
    let should_notify = !exit_notified && shell_exited;
    assert!(
        !should_notify,
        "Second check should not trigger duplicate notification"
    );
}

/// Test suppress when focused logic (simulated).
#[test]
fn test_suppress_when_focused_logic() {
    // Simulate the suppression logic
    let suppress_enabled = true;
    let is_focused = true;

    // When focused and suppression enabled, should not send desktop notification
    let should_send_desktop = !(suppress_enabled && is_focused);
    assert!(
        !should_send_desktop,
        "Desktop notification should be suppressed when focused"
    );

    // When not focused, should send desktop notification
    let is_focused = false;
    let should_send_desktop = !(suppress_enabled && is_focused);
    assert!(
        should_send_desktop,
        "Desktop notification should be sent when not focused"
    );

    // When suppression disabled, should always send
    let suppress_enabled = false;
    let is_focused = true;
    let should_send_desktop = !(suppress_enabled && is_focused);
    assert!(
        should_send_desktop,
        "Desktop notification should be sent when suppression disabled"
    );
}

/// Test that visual and audio bells are NOT affected by suppress when focused.
/// (This is tested by ensuring the config fields are independent)
#[test]
fn test_bells_independent_of_suppress() {
    let yaml = r#"
notification_bell_visual: true
notification_bell_sound: 50
suppress_notifications_when_focused: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();

    // Bell settings should be independent of suppress setting
    assert!(config.notification_bell_visual);
    assert_eq!(config.notification_bell_sound, 50);
    assert!(config.suppress_notifications_when_focused);
    // Visual and audio bells are handled separately and are not suppressed
}

/// Test empty YAML uses defaults.
#[test]
fn test_empty_yaml_uses_defaults() {
    let yaml = "";
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();

    assert!(!config.notification_session_ended);
    assert!(config.suppress_notifications_when_focused);
}
