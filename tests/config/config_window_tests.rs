//! Integration tests for window management, session logging, and startup directory config.
//!
//! Covers: window type, target monitor/space, session logging, startup directory modes,
//! and effective startup directory resolution.

use par_term::config::{Config, SessionLogFormat, StartupDirectoryMode, WindowType};

// ============================================================================
// Window Management Features Tests
// ============================================================================

#[test]
fn test_config_window_management_defaults() {
    let config = Config::default();
    // Window type should default to Normal
    assert_eq!(config.window_type, WindowType::Normal);
    // Target monitor should be None (OS decides)
    assert!(config.target_monitor.is_none());
    // Target Space should be None (OS decides)
    assert!(config.target_space.is_none());
    // Lock window size should be disabled by default
    assert!(!config.lock_window_size);
    // Show window number should be disabled by default
    assert!(!config.show_window_number);
}

#[test]
fn test_config_window_type_variants() {
    // Test all window type variants
    let yaml = r#"window_type: normal"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::Normal);

    let yaml = r#"window_type: fullscreen"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::Fullscreen);

    let yaml = r#"window_type: edge_top"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeTop);

    let yaml = r#"window_type: edge_bottom"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeBottom);

    let yaml = r#"window_type: edge_left"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeLeft);

    let yaml = r#"window_type: edge_right"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeRight);
}

#[test]
fn test_config_window_type_is_edge() {
    assert!(!WindowType::Normal.is_edge());
    assert!(!WindowType::Fullscreen.is_edge());
    assert!(WindowType::EdgeTop.is_edge());
    assert!(WindowType::EdgeBottom.is_edge());
    assert!(WindowType::EdgeLeft.is_edge());
    assert!(WindowType::EdgeRight.is_edge());
}

#[test]
fn test_config_window_management_yaml_deserialization() {
    let yaml = r#"
window_type: edge_top
target_monitor: 1
lock_window_size: true
show_window_number: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeTop);
    assert_eq!(config.target_monitor, Some(1));
    assert!(config.lock_window_size);
    assert!(config.show_window_number);
}

#[test]
fn test_config_window_management_yaml_serialization() {
    let config = Config {
        window_type: WindowType::Fullscreen,
        target_monitor: Some(2),
        lock_window_size: true,
        show_window_number: true,
        ..Config::default()
    };

    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(yaml.contains("window_type: fullscreen"));
    assert!(yaml.contains("target_monitor: 2"));
    assert!(yaml.contains("lock_window_size: true"));
    assert!(yaml.contains("show_window_number: true"));
}

#[test]
fn test_config_window_management_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
lock_window_size: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.lock_window_size);
    // Other fields should have defaults
    assert_eq!(config.window_type, WindowType::Normal);
    assert!(config.target_monitor.is_none());
    assert!(!config.show_window_number);
}

#[test]
fn test_config_target_monitor_none_yaml() {
    // Test that target_monitor can be explicitly null/none
    let yaml = r#"
target_monitor: null
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.target_monitor.is_none());
}

#[test]
fn test_config_target_space_default() {
    let config = Config::default();
    assert!(config.target_space.is_none());
}

#[test]
fn test_config_target_space_yaml_deserialization() {
    let yaml = r#"
target_space: 3
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.target_space, Some(3));
}

#[test]
fn test_config_target_space_null_yaml() {
    let yaml = r#"
target_space: null
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.target_space.is_none());
}

#[test]
fn test_config_target_space_yaml_serialization() {
    let config = Config {
        target_space: Some(5),
        ..Config::default()
    };

    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(yaml.contains("target_space: 5"));
}

#[test]
fn test_config_target_space_missing_defaults_to_none() {
    // When target_space is not in YAML, it should default to None
    let yaml = r#"
font_size: 14.0
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.target_space.is_none());
}

#[test]
fn test_window_type_display_names() {
    assert_eq!(WindowType::Normal.display_name(), "Normal");
    assert_eq!(WindowType::Fullscreen.display_name(), "Fullscreen");
    assert_eq!(WindowType::EdgeTop.display_name(), "Edge (Top)");
    assert_eq!(WindowType::EdgeBottom.display_name(), "Edge (Bottom)");
    assert_eq!(WindowType::EdgeLeft.display_name(), "Edge (Left)");
    assert_eq!(WindowType::EdgeRight.display_name(), "Edge (Right)");
}

#[test]
fn test_window_type_all() {
    let all_types = WindowType::all();
    assert_eq!(all_types.len(), 6);
    assert!(all_types.contains(&WindowType::Normal));
    assert!(all_types.contains(&WindowType::Fullscreen));
    assert!(all_types.contains(&WindowType::EdgeTop));
    assert!(all_types.contains(&WindowType::EdgeBottom));
    assert!(all_types.contains(&WindowType::EdgeLeft));
    assert!(all_types.contains(&WindowType::EdgeRight));
}

// ============================================================================
// Session Logging Tests
// ============================================================================

#[test]
fn test_session_logging_defaults() {
    let config = Config::default();
    // Session logging should be disabled by default
    assert!(!config.auto_log_sessions);
    // Default format should be asciicast
    assert_eq!(config.session_log_format, SessionLogFormat::Asciicast);
    // Archive on close should be enabled by default
    assert!(config.archive_on_close);
    // Log directory should contain par-term/logs
    assert!(config.session_log_directory.contains("par-term"));
    assert!(config.session_log_directory.contains("logs"));
}

#[test]
fn test_session_log_format_enum() {
    // Test all variants
    assert_eq!(SessionLogFormat::Plain.display_name(), "Plain Text");
    assert_eq!(SessionLogFormat::Html.display_name(), "HTML");
    assert_eq!(
        SessionLogFormat::Asciicast.display_name(),
        "Asciicast (asciinema)"
    );

    // Test file extensions
    assert_eq!(SessionLogFormat::Plain.extension(), "txt");
    assert_eq!(SessionLogFormat::Html.extension(), "html");
    assert_eq!(SessionLogFormat::Asciicast.extension(), "cast");

    // Test all() method
    let all_formats = SessionLogFormat::all();
    assert_eq!(all_formats.len(), 3);
}

#[test]
fn test_session_logging_yaml_deserialization() {
    let yaml = r#"
auto_log_sessions: true
session_log_format: plain
session_log_directory: "/tmp/test-logs"
archive_on_close: false
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.auto_log_sessions);
    assert_eq!(config.session_log_format, SessionLogFormat::Plain);
    assert_eq!(config.session_log_directory, "/tmp/test-logs");
    assert!(!config.archive_on_close);
}

#[test]
fn test_session_logging_yaml_serialization() {
    let config = Config {
        auto_log_sessions: true,
        session_log_format: SessionLogFormat::Html,
        session_log_directory: "/var/log/terminal".to_string(),
        ..Config::default()
    };

    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(yaml.contains("auto_log_sessions: true"));
    assert!(yaml.contains("session_log_format: html"));
    assert!(yaml.contains("session_log_directory: /var/log/terminal"));
}

#[test]
fn test_session_log_format_yaml_variants() {
    // Test all format variants
    let yaml = r#"session_log_format: plain"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Plain);

    let yaml = r#"session_log_format: html"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Html);

    let yaml = r#"session_log_format: asciicast"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Asciicast);
}

#[test]
fn test_session_logging_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
auto_log_sessions: true
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.auto_log_sessions);
    // Other fields should have defaults
    assert_eq!(config.session_log_format, SessionLogFormat::Asciicast);
    assert!(config.archive_on_close);
}

// ============================================================================
// Startup Directory Configuration Tests
// ============================================================================

#[test]
fn test_startup_directory_mode_defaults() {
    let config = Config::default();
    assert_eq!(
        config.startup_directory_mode,
        StartupDirectoryMode::Home,
        "Default startup directory mode should be Home"
    );
    assert!(
        config.startup_directory.is_none(),
        "Default startup_directory should be None"
    );
    assert!(
        config.last_working_directory.is_none(),
        "Default last_working_directory should be None"
    );
}

#[test]
fn test_startup_directory_mode_yaml_parsing() {
    // Test home mode
    let yaml = r#"startup_directory_mode: home"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.startup_directory_mode, StartupDirectoryMode::Home);

    // Test previous mode
    let yaml = r#"startup_directory_mode: previous"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(
        config.startup_directory_mode,
        StartupDirectoryMode::Previous
    );

    // Test custom mode
    let yaml = r#"startup_directory_mode: custom"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.startup_directory_mode, StartupDirectoryMode::Custom);
}

#[test]
fn test_startup_directory_custom_path() {
    let yaml = r#"
startup_directory_mode: custom
startup_directory: "/tmp/test-dir"
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.startup_directory_mode, StartupDirectoryMode::Custom);
    assert_eq!(config.startup_directory, Some("/tmp/test-dir".to_string()));
}

#[test]
fn test_startup_directory_mode_display_names() {
    assert_eq!(StartupDirectoryMode::Home.display_name(), "Home Directory");
    assert_eq!(
        StartupDirectoryMode::Previous.display_name(),
        "Previous Session"
    );
    assert_eq!(
        StartupDirectoryMode::Custom.display_name(),
        "Custom Directory"
    );
}

#[test]
fn test_startup_directory_mode_all() {
    let all_modes = StartupDirectoryMode::all();
    assert_eq!(all_modes.len(), 3);
    assert!(all_modes.contains(&StartupDirectoryMode::Home));
    assert!(all_modes.contains(&StartupDirectoryMode::Previous));
    assert!(all_modes.contains(&StartupDirectoryMode::Custom));
}

#[test]
fn test_get_effective_startup_directory_home_mode() {
    let config = Config::default();
    // Home mode should return the home directory
    let effective_dir = config.get_effective_startup_directory();
    assert!(effective_dir.is_some(), "Should return home directory");
    let dir = effective_dir.unwrap();
    assert!(
        std::path::Path::new(&dir).exists(),
        "Home directory should exist"
    );
}

#[test]
fn test_get_effective_startup_directory_custom_mode_nonexistent() {
    let yaml = r#"
startup_directory_mode: custom
startup_directory: "/nonexistent/path/that/does/not/exist"
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    // Should fall back to home directory when custom path doesn't exist
    let effective_dir = config.get_effective_startup_directory();
    assert!(
        effective_dir.is_some(),
        "Should fall back to home directory"
    );
    let dir = effective_dir.unwrap();
    assert!(
        std::path::Path::new(&dir).exists(),
        "Fallback directory should exist"
    );
}

#[test]
fn test_get_effective_startup_directory_previous_mode_nonexistent() {
    let yaml = r#"
startup_directory_mode: previous
last_working_directory: "/nonexistent/path/that/does/not/exist"
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    // Should fall back to home directory when previous path doesn't exist
    let effective_dir = config.get_effective_startup_directory();
    assert!(
        effective_dir.is_some(),
        "Should fall back to home directory"
    );
    let dir = effective_dir.unwrap();
    assert!(
        std::path::Path::new(&dir).exists(),
        "Fallback directory should exist"
    );
}

#[test]
fn test_get_effective_startup_directory_custom_mode_with_tilde() {
    let yaml = r#"
startup_directory_mode: custom
startup_directory: "~"
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    // Tilde should expand to home directory
    let effective_dir = config.get_effective_startup_directory();
    assert!(
        effective_dir.is_some(),
        "Should return expanded home directory"
    );
    let dir = effective_dir.unwrap();
    assert!(
        std::path::Path::new(&dir).exists(),
        "Expanded home directory should exist"
    );
}

#[test]
fn test_get_effective_startup_directory_legacy_working_directory() {
    // Legacy working_directory should take precedence
    let yaml = r#"
working_directory: "/tmp"
startup_directory_mode: custom
startup_directory: "~"
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    let effective_dir = config.get_effective_startup_directory();
    assert!(effective_dir.is_some());
    assert_eq!(
        effective_dir.unwrap(),
        "/tmp",
        "Legacy working_directory should take precedence"
    );
}

#[test]
fn test_startup_directory_yaml_serialization() {
    let config = Config {
        startup_directory_mode: StartupDirectoryMode::Custom,
        startup_directory: Some("~/Projects".to_string()),
        ..Config::default()
    };

    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(yaml.contains("startup_directory_mode: custom"));
    assert!(yaml.contains("startup_directory: ~/Projects"));
}
