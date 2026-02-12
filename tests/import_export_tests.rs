//! Tests for import/export preferences functionality.

#![allow(clippy::field_reassign_with_default)]

use par_term::config::Config;
use par_term::settings_ui::advanced_tab::merge_config;

#[test]
fn test_export_config_round_trip() {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config).expect("serialize");
    let imported: Config = serde_yaml::from_str(&yaml).expect("deserialize");
    assert_eq!(config.cols, imported.cols);
    assert_eq!(config.rows, imported.rows);
    assert_eq!(config.font_size, imported.font_size);
    assert_eq!(config.font_family, imported.font_family);
    assert_eq!(config.theme, imported.theme);
}

#[test]
fn test_import_replace_overrides_all_fields() {
    let yaml = r#"
cols: 120
rows: 40
font_size: 18.0
font_family: "Fira Code"
"#;
    let imported: Config = serde_yaml::from_str(yaml).expect("parse imported");

    // Replace mode: entire config is the imported one
    assert_eq!(imported.cols, 120);
    assert_eq!(imported.rows, 40);
    assert_eq!(imported.font_size, 18.0);
    assert_eq!(imported.font_family, "Fira Code");
}

#[test]
fn test_merge_config_only_overrides_non_defaults() {
    let mut current = Config::default();
    current.cols = 100; // User's custom value
    current.rows = 30; // User's custom value

    // Imported config only changes font_size (non-default value)
    let mut imported = Config::default();
    imported.font_size = 18.0;
    // cols and rows are still defaults (80, 24) in imported

    merge_config(&mut current, &imported);

    // font_size should be overridden (imported differs from default)
    assert_eq!(current.font_size, 18.0);
    // cols and rows should be preserved (imported values match defaults)
    assert_eq!(current.cols, 100);
    assert_eq!(current.rows, 30);
}

#[test]
fn test_merge_config_preserves_current_when_imported_is_default() {
    let mut current = Config::default();
    current.font_family = "Hack".to_string();
    current.theme = "my-custom-theme".to_string();

    let imported = Config::default(); // All defaults

    merge_config(&mut current, &imported);

    // Nothing should change since all imported values match defaults
    assert_eq!(current.font_family, "Hack");
    assert_eq!(current.theme, "my-custom-theme");
}

#[test]
fn test_merge_config_overrides_multiple_non_default_fields() {
    let mut current = Config::default();
    current.cols = 100;

    let mut imported = Config::default();
    imported.font_size = 16.0;
    imported.font_family = "Fira Code".to_string();
    imported.scrollback_lines = 50000;

    merge_config(&mut current, &imported);

    // All non-default imported values should be applied
    assert_eq!(current.font_size, 16.0);
    assert_eq!(current.font_family, "Fira Code");
    assert_eq!(current.scrollback_lines, 50000);
    // Current's custom cols should be preserved
    assert_eq!(current.cols, 100);
}

#[test]
fn test_import_partial_yaml_uses_defaults_for_missing_fields() {
    let yaml = r#"
font_size: 20.0
theme: "solarized-dark"
"#;
    let imported: Config = serde_yaml::from_str(yaml).expect("parse partial yaml");

    // Explicitly set fields should match
    assert_eq!(imported.font_size, 20.0);
    assert_eq!(imported.theme, "solarized-dark");

    // Missing fields should use defaults
    assert_eq!(imported.cols, 80);
    assert_eq!(imported.rows, 24);
    assert_eq!(imported.font_family, "JetBrains Mono");
}

#[test]
fn test_import_invalid_yaml_returns_error() {
    let yaml = "this is not: valid: yaml: {{{";
    let result = serde_yaml::from_str::<Config>(yaml);
    assert!(result.is_err());
}
