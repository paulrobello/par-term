use par_term::config::{Config, UnfocusedCursorStyle};

#[test]
fn test_config_defaults() {
    let config = Config::default();
    assert_eq!(config.cols, 80);
    assert_eq!(config.rows, 24);
    assert_eq!(config.font_size, 13.0);
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
}

#[test]
fn test_config_new() {
    let config = Config::new();
    assert_eq!(config.cols, 80);
    assert_eq!(config.rows, 24);
}

#[test]
fn test_config_with_dimensions() {
    let config = Config::new().with_dimensions(100, 30);
    assert_eq!(config.cols, 100);
    assert_eq!(config.rows, 30);
}

#[test]
fn test_config_with_font_size() {
    let config = Config::new().with_font_size(16.0);
    assert_eq!(config.font_size, 16.0);
}

#[test]
fn test_config_with_font_family() {
    let config = Config::new().with_font_family("Consolas");
    assert_eq!(config.font_family, "Consolas");
}

#[test]
fn test_config_with_scrollback() {
    let config = Config::new().with_scrollback(5000);
    assert_eq!(config.scrollback_lines, 5000);
}

#[test]
fn test_config_with_title() {
    let config = Config::new().with_title("My Terminal");
    assert_eq!(config.window_title, "My Terminal");
}

#[test]
fn test_config_yaml_serialization() {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("cols: 80"));
    assert!(yaml.contains("rows: 24"));
    assert!(yaml.contains("font_size: 13.0"));
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
    let config = Config::new()
        .with_dimensions(120, 40)
        .with_font_size(18.0)
        .with_font_family("Fira Code")
        .with_scrollback(20000)
        .with_title("Custom Terminal");

    assert_eq!(config.cols, 120);
    assert_eq!(config.rows, 40);
    assert_eq!(config.font_size, 18.0);
    assert_eq!(config.font_family, "Fira Code");
    assert_eq!(config.scrollback_lines, 20000);
    assert_eq!(config.window_title, "Custom Terminal");
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
