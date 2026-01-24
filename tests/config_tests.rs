use par_term::config::Config;

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
    // Default unfocused FPS is 10
    assert_eq!(config.unfocused_fps, 10);
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
