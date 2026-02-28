//! Tests for status bar configuration serialization/deserialization.

use par_term::config::Config;
use par_term::status_bar::config::{
    StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets,
};

#[test]
fn test_default_config_has_status_bar_fields() {
    let config = Config::default();
    assert!(!config.status_bar_enabled);
    assert_eq!(config.status_bar_height, 22.0);
    assert_eq!(config.status_bar_bg_color, [30, 30, 30]);
    assert!(!config.status_bar_widgets.is_empty());
}

#[test]
fn test_default_widgets_complete() {
    let widgets = default_widgets();
    assert_eq!(widgets.len(), 10);
    let ids: Vec<&WidgetId> = widgets.iter().map(|w| &w.id).collect();
    assert!(ids.contains(&&WidgetId::Clock));
    assert!(ids.contains(&&WidgetId::UsernameHostname));
    assert!(ids.contains(&&WidgetId::CurrentDirectory));
    assert!(ids.contains(&&WidgetId::GitBranch));
    assert!(ids.contains(&&WidgetId::CpuUsage));
    assert!(ids.contains(&&WidgetId::MemoryUsage));
    assert!(ids.contains(&&WidgetId::NetworkStatus));
    assert!(ids.contains(&&WidgetId::BellIndicator));
    assert!(ids.contains(&&WidgetId::CurrentCommand));
    assert!(ids.contains(&&WidgetId::UpdateAvailable));
}

#[test]
fn test_widget_config_serialization_roundtrip() {
    let widget = StatusBarWidgetConfig {
        id: WidgetId::GitBranch,
        enabled: true,
        section: StatusBarSection::Left,
        order: 2,
        format: None,
    };
    let yaml = serde_yaml_ng::to_string(&widget).expect("serialize");
    let deserialized: StatusBarWidgetConfig = serde_yaml_ng::from_str(&yaml).expect("deserialize");
    assert_eq!(deserialized.id, widget.id);
    assert_eq!(deserialized.enabled, widget.enabled);
    assert_eq!(deserialized.section, widget.section);
    assert_eq!(deserialized.order, widget.order);
}

#[test]
fn test_custom_widget_config_serialization() {
    let widget = StatusBarWidgetConfig {
        id: WidgetId::Custom("my_widget".to_string()),
        enabled: true,
        section: StatusBarSection::Center,
        order: 0,
        format: Some("\\(session.username) on \\(session.hostname)".to_string()),
    };
    let yaml = serde_yaml_ng::to_string(&widget).expect("serialize");
    let deserialized: StatusBarWidgetConfig = serde_yaml_ng::from_str(&yaml).expect("deserialize");
    assert_eq!(deserialized.id, WidgetId::Custom("my_widget".to_string()));
    assert_eq!(
        deserialized.format,
        Some("\\(session.username) on \\(session.hostname)".to_string())
    );
}

#[test]
fn test_config_yaml_with_status_bar() {
    let yaml = r#"
status_bar_enabled: true
status_bar_position: top
status_bar_height: 28.0
status_bar_fg_color: [255, 255, 255]
status_bar_widgets:
  - id: clock
    enabled: true
    section: right
    order: 0
  - id: git_branch
    enabled: true
    section: left
    order: 0
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).expect("deserialize");
    assert!(config.status_bar_enabled);
    assert_eq!(config.status_bar_height, 28.0);
    assert_eq!(config.status_bar_widgets.len(), 2);
}
