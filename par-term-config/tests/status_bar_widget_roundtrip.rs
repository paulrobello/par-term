//! Reproduction test for the Custom status-bar widget `config.yaml` round-trip bug.
//!
//! `WidgetId` is an externally-tagged enum whose `Custom(String)` newtype variant
//! must round-trip through `Config` → `#[serde(flatten)] status_bar:
//! StatusBarConfig` → `status_bar_widgets: Vec<StatusBarWidgetConfig>`.

use par_term_config::Config;
use par_term_config::status_bar::{StatusBarSection, StatusBarWidgetConfig, WidgetId};

#[test]
fn custom_widget_roundtrips_through_config_yaml() {
    let mut cfg = Config::default();
    cfg.status_bar.status_bar_widgets = vec![
        StatusBarWidgetConfig {
            id: WidgetId::GitBranch,
            enabled: true,
            section: StatusBarSection::Left,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::Custom("my-widget".to_string()),
            enabled: true,
            section: StatusBarSection::Right,
            order: 7,
            format: Some("\\(custom.var)".to_string()),
        },
        // Name containing a colon+space must be quoted by serde_yaml and still
        // round-trip (verifies the `custom:<name>` encoding is unambiguous).
        StatusBarWidgetConfig {
            id: WidgetId::Custom("weird: name with spaces".to_string()),
            enabled: false,
            section: StatusBarSection::Center,
            order: 9,
            format: None,
        },
    ];

    let yaml = serde_yaml_ng::to_string(&cfg).expect("serialize Config");
    let widget_yaml: String = yaml
        .lines()
        .skip_while(|l| !l.starts_with("status_bar_widgets"))
        .take_while(|l| {
            l.starts_with("status_bar_widgets") || l.starts_with("  ") || l.trim().is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n");
    eprintln!("--- serialized status_bar_widgets ---\n{widget_yaml}");

    let back: Config = serde_yaml_ng::from_str(&yaml).expect("deserialize Config");
    assert_eq!(
        back.status_bar.status_bar_widgets, cfg.status_bar.status_bar_widgets,
        "Custom widget did not round-trip through config.yaml"
    );
}
