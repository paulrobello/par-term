#![allow(clippy::field_reassign_with_default)]

use par_term::config::{
    Config, TabBarPosition, UnfocusedCursorStyle, WindowType, substitute_variables,
};

#[test]
fn test_config_defaults() {
    let config = Config::default();
    assert_eq!(config.cols, 80);
    assert_eq!(config.rows, 24);
    assert_eq!(config.font_size, 12.0);
    assert_eq!(config.font_family, "JetBrains Mono");
    assert_eq!(config.line_spacing, 1.0);
    assert_eq!(config.char_spacing, 1.0);
    assert_eq!(config.scrollback_lines, 10000);
    assert_eq!(config.window_title, "par-term");
    assert_eq!(config.theme, "dark-background");
    assert!(config.auto_copy_selection);
    assert!(config.middle_click_paste);
    assert!(!config.copy_trailing_newline); // Inverted logic: false means strip trailing newline
    assert_eq!(config.screenshot_format, "png");
    // Session undo defaults
    assert_eq!(config.session_undo_timeout_secs, 5);
    assert_eq!(config.session_undo_max_entries, 10);
    assert!(!config.session_undo_preserve_shell);
}

#[test]
fn test_config_with_title() {
    let config = Config::default().with_title("My Terminal");
    assert_eq!(config.window_title, "My Terminal");
}

#[test]
fn test_config_yaml_serialization() {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("cols: 80"));
    assert!(yaml.contains("rows: 24"));
    assert!(yaml.contains("font_size: 12.0"));
}

#[test]
fn test_config_yaml_deserialization() {
    let yaml = r#"
cols: 100
rows: 30
font_size: 16.0
font_family: "Consolas"
scrollback_size: 5000
window_title: "Test Terminal"
theme: "light-background"
auto_copy_selection: true
middle_click_paste: false
screenshot_format: "svg"
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.cols, 100);
    assert_eq!(config.rows, 30);
    assert_eq!(config.font_size, 16.0);
    assert_eq!(config.font_family, "Consolas");
    assert_eq!(config.scrollback_lines, 5000); // Tests backward compatibility via alias
    assert_eq!(config.window_title, "Test Terminal");
    assert_eq!(config.theme, "light-background");
    assert!(config.auto_copy_selection);
    assert!(!config.middle_click_paste);
    assert_eq!(config.screenshot_format, "svg");
}

#[test]
fn test_config_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
cols: 100
font_size: 16.0
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.cols, 100);
    assert_eq!(config.rows, 24); // default
    assert_eq!(config.font_size, 16.0);
    assert_eq!(config.font_family, "JetBrains Mono"); // default
}

#[test]
fn test_config_builder_chain() {
    let config = Config::default().with_title("Custom Terminal");
    assert_eq!(config.window_title, "Custom Terminal");
    // Defaults should still be intact
    assert_eq!(config.cols, 80);
    assert_eq!(config.rows, 24);
}

#[test]
fn test_config_power_saving_defaults() {
    let config = Config::default();
    // Default: pause shaders on blur is enabled for power savings
    assert!(config.pause_shaders_on_blur);
    // Default: pause refresh on blur is disabled (maintain responsiveness)
    assert!(!config.pause_refresh_on_blur);
    // Default unfocused FPS is 30
    assert_eq!(config.unfocused_fps, 30);

    // Initial text defaults
    assert!(config.initial_text.is_empty());
    assert_eq!(config.initial_text_delay_ms, 100);
    assert!(config.initial_text_send_newline);
}

#[test]
fn test_config_power_saving_yaml_deserialization() {
    let yaml = r#"
pause_shaders_on_blur: false
pause_refresh_on_blur: true
unfocused_fps: 5
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(!config.pause_shaders_on_blur);
    assert!(config.pause_refresh_on_blur);
    assert_eq!(config.unfocused_fps, 5);
}

#[test]
fn test_config_initial_text_yaml_deserialization() {
    let yaml = r#"
initial_text: "ssh server"
initial_text_delay_ms: 250
initial_text_send_newline: false
"#;

    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.initial_text, "ssh server");
    assert_eq!(config.initial_text_delay_ms, 250);
    assert!(!config.initial_text_send_newline);
}

#[test]
fn test_config_initial_text_yaml_serialization() {
    let mut config = Config::default();
    config.initial_text = "echo ready".to_string();
    config.initial_text_delay_ms = 10;
    config.initial_text_send_newline = false;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("initial_text: echo ready"));
    assert!(yaml.contains("initial_text_delay_ms: 10"));
    assert!(yaml.contains("initial_text_send_newline: false"));
}

#[test]
fn test_config_power_saving_yaml_serialization() {
    let mut config = Config::default();
    config.pause_shaders_on_blur = false;
    config.pause_refresh_on_blur = true;
    config.unfocused_fps = 15;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("pause_shaders_on_blur: false"));
    assert!(yaml.contains("pause_refresh_on_blur: true"));
    assert!(yaml.contains("unfocused_fps: 15"));
}

#[test]
fn test_config_tab_bar_color_defaults() {
    let config = Config::default();
    // Tab bar background colors
    assert_eq!(config.tab_bar_background, [40, 40, 40]);
    assert_eq!(config.tab_active_background, [60, 60, 60]);
    assert_eq!(config.tab_inactive_background, [40, 40, 40]);
    assert_eq!(config.tab_hover_background, [50, 50, 50]);
    // Tab text colors
    assert_eq!(config.tab_active_text, [255, 255, 255]);
    assert_eq!(config.tab_inactive_text, [180, 180, 180]);
    // Tab indicator colors
    assert_eq!(config.tab_active_indicator, [100, 150, 255]);
    assert_eq!(config.tab_activity_indicator, [100, 180, 255]);
    assert_eq!(config.tab_bell_indicator, [255, 200, 100]);
    // Close button colors
    assert_eq!(config.tab_close_button, [150, 150, 150]);
    assert_eq!(config.tab_close_button_hover, [255, 100, 100]);
}

#[test]
fn test_config_tab_bar_color_yaml_deserialization() {
    let yaml = r#"
tab_bar_background: [30, 30, 30]
tab_active_background: [80, 80, 80]
tab_inactive_background: [35, 35, 35]
tab_hover_background: [55, 55, 55]
tab_active_text: [240, 240, 240]
tab_inactive_text: [160, 160, 160]
tab_active_indicator: [120, 170, 255]
tab_activity_indicator: [80, 200, 255]
tab_bell_indicator: [255, 180, 80]
tab_close_button: [130, 130, 130]
tab_close_button_hover: [255, 80, 80]
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_background, [30, 30, 30]);
    assert_eq!(config.tab_active_background, [80, 80, 80]);
    assert_eq!(config.tab_inactive_background, [35, 35, 35]);
    assert_eq!(config.tab_hover_background, [55, 55, 55]);
    assert_eq!(config.tab_active_text, [240, 240, 240]);
    assert_eq!(config.tab_inactive_text, [160, 160, 160]);
    assert_eq!(config.tab_active_indicator, [120, 170, 255]);
    assert_eq!(config.tab_activity_indicator, [80, 200, 255]);
    assert_eq!(config.tab_bell_indicator, [255, 180, 80]);
    assert_eq!(config.tab_close_button, [130, 130, 130]);
    assert_eq!(config.tab_close_button_hover, [255, 80, 80]);
}

#[test]
fn test_config_tab_bar_color_yaml_serialization() {
    let mut config = Config::default();
    config.tab_bar_background = [50, 50, 50];
    config.tab_active_indicator = [200, 100, 50];

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("tab_bar_background:"));
    assert!(yaml.contains("- 50"));
    assert!(yaml.contains("tab_active_indicator:"));
    assert!(yaml.contains("- 200"));
    assert!(yaml.contains("- 100"));
}

#[test]
fn test_config_tab_bar_color_partial_yaml() {
    // Test that default values are used for missing tab bar color fields
    let yaml = r#"
tab_bar_background: [25, 25, 25]
tab_active_text: [200, 200, 200]
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_background, [25, 25, 25]);
    assert_eq!(config.tab_active_text, [200, 200, 200]);
    // Other fields should have defaults
    assert_eq!(config.tab_active_background, [60, 60, 60]);
    assert_eq!(config.tab_inactive_background, [40, 40, 40]);
    assert_eq!(config.tab_close_button, [150, 150, 150]);
}

#[test]
fn test_config_inactive_tab_dimming_defaults() {
    let config = Config::default();
    // Default: dimming is enabled
    assert!(config.dim_inactive_tabs);
    // Default: 60% opacity for inactive tabs
    assert!((config.inactive_tab_opacity - 0.6).abs() < f32::EPSILON);
}

#[test]
fn test_config_inactive_tab_dimming_yaml_deserialization() {
    let yaml = r#"
dim_inactive_tabs: false
inactive_tab_opacity: 0.8
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(!config.dim_inactive_tabs);
    assert!((config.inactive_tab_opacity - 0.8).abs() < f32::EPSILON);
}

#[test]
fn test_config_inactive_tab_dimming_yaml_serialization() {
    let mut config = Config::default();
    config.dim_inactive_tabs = false;
    config.inactive_tab_opacity = 0.5;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("dim_inactive_tabs: false"));
    assert!(yaml.contains("inactive_tab_opacity: 0.5"));
}

#[test]
fn test_config_inactive_tab_dimming_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
dim_inactive_tabs: true
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.dim_inactive_tabs);
    // Default opacity should be used
    assert!((config.inactive_tab_opacity - 0.6).abs() < f32::EPSILON);
}

#[test]
fn test_config_inactive_tab_opacity_bounds() {
    // Test various opacity values that might be configured
    let yaml = r#"
inactive_tab_opacity: 0.0
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!((config.inactive_tab_opacity).abs() < f32::EPSILON);

    let yaml = r#"
inactive_tab_opacity: 1.0
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!((config.inactive_tab_opacity - 1.0).abs() < f32::EPSILON);

    let yaml = r#"
inactive_tab_opacity: 0.3
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!((config.inactive_tab_opacity - 0.3).abs() < f32::EPSILON);
}

#[test]
fn test_config_cursor_enhancement_defaults() {
    let config = Config::default();
    // Unfocused cursor style defaults to Hollow
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Hollow);
    // Cursor guide disabled by default
    assert!(!config.cursor_guide_enabled);
    // Default guide color: white with low alpha
    assert_eq!(config.cursor_guide_color, [255, 255, 255, 20]);
    // Cursor shadow disabled by default
    assert!(!config.cursor_shadow_enabled);
    // Default shadow color: black with 50% alpha
    assert_eq!(config.cursor_shadow_color, [0, 0, 0, 128]);
    // Default shadow offset
    assert!((config.cursor_shadow_offset[0] - 2.0).abs() < f32::EPSILON);
    assert!((config.cursor_shadow_offset[1] - 2.0).abs() < f32::EPSILON);
    // Default shadow blur
    assert!((config.cursor_shadow_blur - 3.0).abs() < f32::EPSILON);
    // Cursor boost (glow) disabled by default (0.0)
    assert!((config.cursor_boost).abs() < f32::EPSILON);
    // Default boost color: white
    assert_eq!(config.cursor_boost_color, [255, 255, 255]);
}

#[test]
fn test_config_cursor_enhancement_yaml_deserialization() {
    let yaml = r#"
unfocused_cursor_style: hidden
cursor_guide_enabled: true
cursor_guide_color: [200, 200, 255, 40]
cursor_shadow_enabled: true
cursor_shadow_color: [0, 0, 0, 200]
cursor_shadow_offset: [3.0, 3.0]
cursor_shadow_blur: 5.0
cursor_boost: 0.5
cursor_boost_color: [255, 200, 100]
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Hidden);
    assert!(config.cursor_guide_enabled);
    assert_eq!(config.cursor_guide_color, [200, 200, 255, 40]);
    assert!(config.cursor_shadow_enabled);
    assert_eq!(config.cursor_shadow_color, [0, 0, 0, 200]);
    assert!((config.cursor_shadow_offset[0] - 3.0).abs() < f32::EPSILON);
    assert!((config.cursor_shadow_offset[1] - 3.0).abs() < f32::EPSILON);
    assert!((config.cursor_shadow_blur - 5.0).abs() < f32::EPSILON);
    assert!((config.cursor_boost - 0.5).abs() < f32::EPSILON);
    assert_eq!(config.cursor_boost_color, [255, 200, 100]);
}

#[test]
fn test_config_unfocused_cursor_style_variants() {
    // Test hollow variant
    let yaml = r#"unfocused_cursor_style: hollow"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Hollow);

    // Test same variant
    let yaml = r#"unfocused_cursor_style: same"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Same);

    // Test hidden variant
    let yaml = r#"unfocused_cursor_style: hidden"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Hidden);
}

#[test]
fn test_config_cursor_enhancement_yaml_serialization() {
    let mut config = Config::default();
    config.unfocused_cursor_style = UnfocusedCursorStyle::Same;
    config.cursor_guide_enabled = true;
    config.cursor_guide_color = [100, 150, 200, 50];
    config.cursor_shadow_enabled = true;
    config.cursor_boost = 0.7;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("unfocused_cursor_style: same"));
    assert!(yaml.contains("cursor_guide_enabled: true"));
    assert!(yaml.contains("cursor_shadow_enabled: true"));
    assert!(yaml.contains("cursor_boost: 0.7"));
}

#[test]
fn test_config_cursor_enhancement_partial_yaml() {
    // Test that default values are used for missing cursor enhancement fields
    let yaml = r#"
cursor_guide_enabled: true
cursor_boost: 0.3
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.cursor_guide_enabled);
    assert!((config.cursor_boost - 0.3).abs() < f32::EPSILON);
    // Other fields should have defaults
    assert_eq!(config.unfocused_cursor_style, UnfocusedCursorStyle::Hollow);
    assert!(!config.cursor_shadow_enabled);
    assert_eq!(config.cursor_guide_color, [255, 255, 255, 20]);
}

// ============================================================================
// Answerback String Tests
// ============================================================================

#[test]
fn test_config_answerback_string_default() {
    let config = Config::default();
    // Answerback should be empty by default for security
    assert!(config.answerback_string.is_empty());
}

#[test]
fn test_config_answerback_string_yaml_deserialization() {
    let yaml = r#"
answerback_string: "par-term"
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.answerback_string, "par-term");
}

#[test]
fn test_config_answerback_string_yaml_serialization() {
    let mut config = Config::default();
    config.answerback_string = "vt100".to_string();

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("answerback_string: vt100"));
}

#[test]
fn test_config_answerback_string_empty_by_default_yaml() {
    // Empty string should deserialize correctly
    let yaml = r#"
answerback_string: ""
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.answerback_string.is_empty());
}

#[test]
fn test_config_answerback_string_partial_yaml() {
    // Test that default value is used when field is missing
    let yaml = r#"
cols: 120
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    // Answerback should use default (empty)
    assert!(config.answerback_string.is_empty());
    assert_eq!(config.cols, 120);
}

// ============================================================================
// Advanced Mouse Features Tests
// ============================================================================

#[test]
fn test_config_advanced_mouse_defaults() {
    let config = Config::default();
    // Option+Click moves cursor should be enabled by default
    assert!(config.option_click_moves_cursor);
    // Focus follows mouse should be disabled by default (opt-in)
    assert!(!config.focus_follows_mouse);
    // Horizontal scroll reporting should be enabled by default
    assert!(config.report_horizontal_scroll);
}

#[test]
fn test_config_advanced_mouse_yaml_deserialization() {
    let yaml = r#"
option_click_moves_cursor: false
focus_follows_mouse: true
report_horizontal_scroll: false
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(!config.option_click_moves_cursor);
    assert!(config.focus_follows_mouse);
    assert!(!config.report_horizontal_scroll);
}

#[test]
fn test_config_advanced_mouse_yaml_serialization() {
    let mut config = Config::default();
    config.option_click_moves_cursor = false;
    config.focus_follows_mouse = true;
    config.report_horizontal_scroll = false;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("option_click_moves_cursor: false"));
    assert!(yaml.contains("focus_follows_mouse: true"));
    assert!(yaml.contains("report_horizontal_scroll: false"));
}

#[test]
fn test_config_advanced_mouse_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
focus_follows_mouse: true
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.focus_follows_mouse);
    // Other fields should have defaults
    assert!(config.option_click_moves_cursor);
    assert!(config.report_horizontal_scroll);
}

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
    // Lock window size should be disabled by default
    assert!(!config.lock_window_size);
    // Show window number should be disabled by default
    assert!(!config.show_window_number);
}

#[test]
fn test_config_window_type_variants() {
    // Test all window type variants
    let yaml = r#"window_type: normal"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::Normal);

    let yaml = r#"window_type: fullscreen"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::Fullscreen);

    let yaml = r#"window_type: edge_top"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeTop);

    let yaml = r#"window_type: edge_bottom"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeBottom);

    let yaml = r#"window_type: edge_left"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeLeft);

    let yaml = r#"window_type: edge_right"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.window_type, WindowType::EdgeTop);
    assert_eq!(config.target_monitor, Some(1));
    assert!(config.lock_window_size);
    assert!(config.show_window_number);
}

#[test]
fn test_config_window_management_yaml_serialization() {
    let mut config = Config::default();
    config.window_type = WindowType::Fullscreen;
    config.target_monitor = Some(2);
    config.lock_window_size = true;
    config.show_window_number = true;

    let yaml = serde_yaml::to_string(&config).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.target_monitor.is_none());
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

use par_term::config::SessionLogFormat;

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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.auto_log_sessions);
    assert_eq!(config.session_log_format, SessionLogFormat::Plain);
    assert_eq!(config.session_log_directory, "/tmp/test-logs");
    assert!(!config.archive_on_close);
}

#[test]
fn test_session_logging_yaml_serialization() {
    let mut config = Config::default();
    config.auto_log_sessions = true;
    config.session_log_format = SessionLogFormat::Html;
    config.session_log_directory = "/var/log/terminal".to_string();

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("auto_log_sessions: true"));
    assert!(yaml.contains("session_log_format: html"));
    assert!(yaml.contains("session_log_directory: /var/log/terminal"));
}

#[test]
fn test_session_log_format_yaml_variants() {
    // Test all format variants
    let yaml = r#"session_log_format: plain"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Plain);

    let yaml = r#"session_log_format: html"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Html);

    let yaml = r#"session_log_format: asciicast"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.session_log_format, SessionLogFormat::Asciicast);
}

#[test]
fn test_session_logging_partial_yaml() {
    // Test that default values are used for missing fields
    let yaml = r#"
auto_log_sessions: true
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.auto_log_sessions);
    // Other fields should have defaults
    assert_eq!(config.session_log_format, SessionLogFormat::Asciicast);
    assert!(config.archive_on_close);
}

// ============================================================================
// Startup Directory Configuration Tests
// ============================================================================

use par_term::config::StartupDirectoryMode;

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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.startup_directory_mode, StartupDirectoryMode::Home);

    // Test previous mode
    let yaml = r#"startup_directory_mode: previous"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        config.startup_directory_mode,
        StartupDirectoryMode::Previous
    );

    // Test custom mode
    let yaml = r#"startup_directory_mode: custom"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.startup_directory_mode, StartupDirectoryMode::Custom);
}

#[test]
fn test_startup_directory_custom_path() {
    let yaml = r#"
startup_directory_mode: custom
startup_directory: "/tmp/test-dir"
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let config: Config = serde_yaml::from_str(yaml).unwrap();
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
    let mut config = Config::default();
    config.startup_directory_mode = StartupDirectoryMode::Custom;
    config.startup_directory = Some("~/Projects".to_string());

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("startup_directory_mode: custom"));
    assert!(yaml.contains("startup_directory: ~/Projects"));
}

// ============================================================================
// Variable Substitution Tests
// ============================================================================

/// Helper to safely set an env var in tests (unsafe in Rust 2024 edition).
unsafe fn set_test_var(key: &str, val: &str) {
    unsafe { std::env::set_var(key, val) };
}

/// Helper to safely remove an env var in tests.
unsafe fn remove_test_var(key: &str) {
    unsafe { std::env::remove_var(key) };
}

#[test]
fn test_substitute_variables_basic_env_var() {
    unsafe { set_test_var("PAR_TEST_VAR", "hello_world") };
    let result = substitute_variables("value: ${PAR_TEST_VAR}");
    assert_eq!(result, "value: hello_world");
    unsafe { remove_test_var("PAR_TEST_VAR") };
}

#[test]
fn test_substitute_variables_home_and_user() {
    // HOME should be set on all Unix-like systems
    let home = std::env::var("HOME").unwrap_or_default();
    let result = substitute_variables("path: ${HOME}/Pictures/bg.png");
    assert_eq!(result, format!("path: {home}/Pictures/bg.png"));
}

#[test]
fn test_substitute_variables_multiple_vars() {
    unsafe { set_test_var("PAR_TEST_A", "alpha") };
    unsafe { set_test_var("PAR_TEST_B", "beta") };
    let result = substitute_variables("${PAR_TEST_A} and ${PAR_TEST_B}");
    assert_eq!(result, "alpha and beta");
    unsafe { remove_test_var("PAR_TEST_A") };
    unsafe { remove_test_var("PAR_TEST_B") };
}

#[test]
fn test_substitute_variables_missing_var_unchanged() {
    // Unset vars should remain as-is
    unsafe { remove_test_var("PAR_NONEXISTENT_VAR_12345") };
    let result = substitute_variables("value: ${PAR_NONEXISTENT_VAR_12345}");
    assert_eq!(result, "value: ${PAR_NONEXISTENT_VAR_12345}");
}

#[test]
fn test_substitute_variables_default_value() {
    unsafe { remove_test_var("PAR_MISSING_WITH_DEFAULT") };
    let result = substitute_variables("shell: ${PAR_MISSING_WITH_DEFAULT:-/bin/bash}");
    assert_eq!(result, "shell: /bin/bash");
}

#[test]
fn test_substitute_variables_default_value_not_used_when_set() {
    unsafe { set_test_var("PAR_SET_WITH_DEFAULT", "/bin/zsh") };
    let result = substitute_variables("shell: ${PAR_SET_WITH_DEFAULT:-/bin/bash}");
    assert_eq!(result, "shell: /bin/zsh");
    unsafe { remove_test_var("PAR_SET_WITH_DEFAULT") };
}

#[test]
fn test_substitute_variables_escaped_dollar() {
    // $${VAR} should produce the literal ${VAR}
    unsafe { set_test_var("PAR_TEST_ESC", "should_not_appear") };
    let result = substitute_variables("literal: $${PAR_TEST_ESC}");
    assert_eq!(result, "literal: ${PAR_TEST_ESC}");
    unsafe { remove_test_var("PAR_TEST_ESC") };
}

#[test]
fn test_substitute_variables_no_vars() {
    let input = "cols: 80\nrows: 24\nfont_size: 12.0";
    let result = substitute_variables(input);
    assert_eq!(result, input);
}

#[test]
fn test_substitute_variables_adjacent_vars() {
    unsafe { set_test_var("PAR_TEST_X", "foo") };
    unsafe { set_test_var("PAR_TEST_Y", "bar") };
    let result = substitute_variables("${PAR_TEST_X}${PAR_TEST_Y}");
    assert_eq!(result, "foobar");
    unsafe { remove_test_var("PAR_TEST_X") };
    unsafe { remove_test_var("PAR_TEST_Y") };
}

#[test]
fn test_substitute_variables_in_yaml_config() {
    unsafe { set_test_var("PAR_TEST_FONT", "Fira Code") };
    unsafe { set_test_var("PAR_TEST_TITLE", "My Terminal") };
    let yaml = r#"
font_family: "${PAR_TEST_FONT}"
window_title: "${PAR_TEST_TITLE}"
cols: 120
"#;
    let substituted = substitute_variables(yaml);
    let config: Config = serde_yaml::from_str(&substituted).unwrap();
    assert_eq!(config.font_family, "Fira Code");
    assert_eq!(config.window_title, "My Terminal");
    assert_eq!(config.cols, 120);
    unsafe { remove_test_var("PAR_TEST_FONT") };
    unsafe { remove_test_var("PAR_TEST_TITLE") };
}

#[test]
fn test_substitute_variables_partial_string() {
    unsafe { set_test_var("PAR_TEST_USER", "testuser") };
    let result = substitute_variables("badge: ${PAR_TEST_USER}@myhost");
    assert_eq!(result, "badge: testuser@myhost");
    unsafe { remove_test_var("PAR_TEST_USER") };
}

#[test]
fn test_substitute_variables_empty_default() {
    unsafe { remove_test_var("PAR_EMPTY_DEFAULT") };
    let result = substitute_variables("val: ${PAR_EMPTY_DEFAULT:-}");
    assert_eq!(result, "val: ");
}

// ============================================================================
// Tab Bar Position Configuration Tests
// ============================================================================

#[test]
fn test_tab_bar_position_default() {
    let config = Config::default();
    assert_eq!(config.tab_bar_position, TabBarPosition::Top);
    assert_eq!(config.tab_bar_width, 160.0);
}

#[test]
fn test_tab_bar_position_serialization() {
    // Round-trip serialization for all variants
    for &position in TabBarPosition::all() {
        let mut config = Config::default();
        config.tab_bar_position = position;

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            deserialized.tab_bar_position, position,
            "Round-trip failed for {:?}",
            position
        );
    }
}

#[test]
fn test_tab_bar_position_yaml_variants() {
    let yaml = r#"tab_bar_position: top"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_position, TabBarPosition::Top);

    let yaml = r#"tab_bar_position: bottom"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_position, TabBarPosition::Bottom);

    let yaml = r#"tab_bar_position: left"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_position, TabBarPosition::Left);
}

#[test]
fn test_tab_bar_position_partial_yaml() {
    // Missing tab_bar_position should default to Top
    let yaml = r#"
cols: 100
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_position, TabBarPosition::Top);
    assert_eq!(config.tab_bar_width, 160.0);
}

#[test]
fn test_tab_bar_width_yaml_deserialization() {
    let yaml = r#"
tab_bar_position: left
tab_bar_width: 250.0
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_bar_position, TabBarPosition::Left);
    assert!((config.tab_bar_width - 250.0).abs() < f32::EPSILON);
}

#[test]
fn test_tab_bar_width_yaml_serialization() {
    let mut config = Config::default();
    config.tab_bar_position = TabBarPosition::Left;
    config.tab_bar_width = 200.0;

    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("tab_bar_position: left"));
    assert!(yaml.contains("tab_bar_width: 200.0"));
}
