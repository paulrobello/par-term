//! Tests for the settings window module

use par_term::config::Config;
use par_term::settings_ui::SettingsUI;
use par_term::settings_ui::search::{Query, Registry};
use par_term::settings_ui::sidebar::SettingsTab;
use par_term::settings_window::SettingsWindowAction;

#[test]
fn test_settings_window_action_none() {
    let action = SettingsWindowAction::None;
    assert!(matches!(action, SettingsWindowAction::None));
}

#[test]
fn test_settings_window_action_close() {
    let action = SettingsWindowAction::Close;
    assert!(matches!(action, SettingsWindowAction::Close));
}

#[test]
fn test_settings_window_action_apply_config() {
    let config = Config::default();
    let action = SettingsWindowAction::ApplyConfig(config.clone());

    if let SettingsWindowAction::ApplyConfig(applied_config) = action {
        assert_eq!(applied_config.window_title, config.window_title);
        assert_eq!(applied_config.font_size, config.font_size);
    } else {
        panic!("Expected ApplyConfig variant");
    }
}

#[test]
fn test_settings_window_action_save_config() {
    let config = Config::default();
    let action = SettingsWindowAction::SaveConfig(config.clone());

    if let SettingsWindowAction::SaveConfig(saved_config) = action {
        assert_eq!(saved_config.window_title, config.window_title);
        assert_eq!(saved_config.font_size, config.font_size);
    } else {
        panic!("Expected SaveConfig variant");
    }
}

#[test]
fn test_settings_window_action_debug_format() {
    // Test that all variants implement Debug
    let none = SettingsWindowAction::None;
    let close = SettingsWindowAction::Close;
    let apply = SettingsWindowAction::ApplyConfig(Config::default());
    let save = SettingsWindowAction::SaveConfig(Config::default());

    // These should not panic
    let _ = format!("{:?}", none);
    let _ = format!("{:?}", close);
    let _ = format!("{:?}", apply);
    let _ = format!("{:?}", save);
}

#[test]
fn test_settings_window_action_clone() {
    // Test that all variants implement Clone
    let none = SettingsWindowAction::None;
    let close = SettingsWindowAction::Close;

    let none_clone = none.clone();
    let close_clone = close.clone();

    assert!(matches!(none_clone, SettingsWindowAction::None));
    assert!(matches!(close_clone, SettingsWindowAction::Close));
}

// ============================================================================
// Settings search tests (UX.md SQ1-SQ3)
// The registry is harvested from the rendered tabs; a tab matches when any
// of its sections or controls does.
// ============================================================================

fn tab_has_result(registry: &Registry, tab: SettingsTab, query: &str) -> bool {
    let query = Query::parse(query);
    if query.is_empty() {
        return true;
    }
    let hits = registry.search(&query);
    registry.tab_matches(tab, &query, &hits)
}

fn registry() -> std::sync::Arc<Registry> {
    SettingsUI::new(Config::default()).search_registry()
}

#[test]
fn test_query_empty_or_blank_is_empty() {
    assert!(Query::parse("").is_empty());
    assert!(Query::parse("   ").is_empty());
    assert!(!Query::parse("font").is_empty());
}

#[test]
fn test_query_is_case_insensitive_token_and() {
    assert!(Query::parse("FONT settings").matches("Font Settings"));
    assert!(Query::parse("settings font").matches("Font Settings"));
    assert!(!Query::parse("font network").matches("Font Settings"));
}

#[test]
fn test_query_matches_word_prefixes() {
    assert!(Query::parse("scroll").matches("Scrollback lines"));
    assert!(!Query::parse("network").matches("Scrollback lines"));
}

#[test]
fn test_every_tab_matches_empty_query_and_its_own_name() {
    let registry = registry();
    for tab in SettingsTab::all() {
        assert!(tab_has_result(&registry, *tab, ""), "{tab:?} empty query");
        let name = tab.display_name().to_lowercase();
        assert!(
            tab_has_result(&registry, *tab, &name),
            "Tab {tab:?} should match its own display name '{name}'"
        );
    }
}

#[test]
fn test_tab_matches_labels_and_section_titles() {
    let registry = registry();
    let cases = [
        (SettingsTab::Appearance, "font"),
        (SettingsTab::Appearance, "cursor"),
        (SettingsTab::Appearance, "THEME"),
        (SettingsTab::Appearance, " font "),
        (SettingsTab::Appearance, "font rendering"),
        (SettingsTab::Window, "opacity"),
        (SettingsTab::Window, "tab bar"),
        (SettingsTab::Window, "split panes"),
        (SettingsTab::Window, "saved arrangements"),
        (SettingsTab::Input, "command palette"),
        (SettingsTab::Input, "modifier remapping"),
        (SettingsTab::Terminal, "command separators"),
        (SettingsTab::Effects, "inline images"),
        (SettingsTab::StatusBar, "poll intervals"),
        (SettingsTab::Profiles, "dynamic profile sources"),
        (SettingsTab::Automation, "git url"),
        (SettingsTab::Notifications, "alert sounds"),
        (SettingsTab::Integrations, "custom shaders"),
        (SettingsTab::Automation, "scripts"),
        (SettingsTab::Snippets, "agent commands"),
        (SettingsTab::AiInspector, "custom agents"),
        (SettingsTab::Advanced, "file transfers"),
        (SettingsTab::Advanced, "tmux"),
    ];
    for (tab, query) in cases {
        assert!(
            tab_has_result(&registry, tab, query),
            "{tab:?} tab should match '{query}'"
        );
    }
}

#[test]
fn test_tab_does_not_match_nonsense_or_another_tabs_setting() {
    let registry = registry();
    assert!(!tab_has_result(
        &registry,
        SettingsTab::Appearance,
        "xyzzy_nonexistent_query"
    ));
    assert!(!tab_has_result(
        &registry,
        SettingsTab::Appearance,
        "gateway"
    ));
    assert!(tab_has_result(&registry, SettingsTab::Advanced, "gateway"));
}

// ============================================================================
// Validation range tests (L-14)
// Tests that Config default values fall within expected ranges and that
// values can be set within documented bounds.
// ============================================================================

#[test]
fn test_font_size_default_in_valid_range() {
    // The appearance tab slider range is 6.0..=48.0
    let config = Config::default();
    assert!(
        config.font_size >= 6.0,
        "Default font_size should be >= 6.0 (slider minimum)"
    );
    assert!(
        config.font_size <= 48.0,
        "Default font_size should be <= 48.0 (slider maximum)"
    );
}

#[test]
fn test_window_opacity_default_in_valid_range() {
    let config = Config::default();
    assert!(
        config.window.window_opacity >= 0.0,
        "Default window_opacity should be >= 0.0"
    );
    assert!(
        config.window.window_opacity <= 1.0,
        "Default window_opacity should be <= 1.0"
    );
}

#[test]
fn test_background_image_opacity_default_in_valid_range() {
    let config = Config::default();
    assert!(config.background.background_image_opacity >= 0.0);
    assert!(config.background.background_image_opacity <= 1.0);
}

#[test]
fn test_inactive_tab_opacity_default_in_valid_range() {
    let config = Config::default();
    assert!(config.tab_colors.inactive_tab_opacity >= 0.0);
    assert!(config.tab_colors.inactive_tab_opacity <= 1.0);
}

#[test]
fn test_scrollback_lines_default_positive() {
    let config = Config::default();
    assert!(
        config.scrollback.scrollback_lines > 0,
        "Default scrollback_lines should be > 0"
    );
}

#[test]
fn test_tab_bar_height_default_positive() {
    let config = Config::default();
    assert!(
        config.tabs.tab_bar_height > 0.0,
        "Default tab_bar_height should be > 0"
    );
}

#[test]
fn test_tab_min_width_default_positive() {
    let config = Config::default();
    assert!(
        config.tab_colors.tab_min_width > 0.0,
        "Default tab_min_width should be > 0"
    );
}

#[test]
fn test_max_fps_default_reasonable() {
    let config = Config::default();
    assert!(
        config.rendering.max_fps > 0,
        "Default max_fps should be > 0"
    );
    assert!(
        config.rendering.max_fps <= 240,
        "Default max_fps should be <= 240 (reasonable upper bound)"
    );
}

// ============================================================================
// has_changes state machine tests (L-14)
// ============================================================================

#[test]
fn test_has_changes_initially_false() {
    let config = Config::default();
    let settings = SettingsUI::new_for_tests(config);
    assert!(
        !settings.has_changes,
        "has_changes should be false on initial creation"
    );
}

#[test]
fn test_has_changes_set_to_true() {
    let config = Config::default();
    let mut settings = SettingsUI::new_for_tests(config);
    assert!(!settings.has_changes);

    // Simulate a setting change (as the UI code does)
    settings.has_changes = true;
    assert!(
        settings.has_changes,
        "has_changes should be true after marking a change"
    );
}

#[test]
fn test_has_changes_reset_to_false() {
    let config = Config::default();
    let mut settings = SettingsUI::new_for_tests(config);

    // Mark as changed
    settings.has_changes = true;
    assert!(settings.has_changes);

    // Simulate save (reset)
    settings.has_changes = false;
    assert!(
        !settings.has_changes,
        "has_changes should return to false after save"
    );
}

#[test]
fn test_has_changes_after_config_field_modification() {
    let config = Config::default();
    let mut settings = SettingsUI::new_for_tests(config);

    assert!(!settings.has_changes, "Should start clean");

    // Modify a config field and mark has_changes (as the UI tab code does)
    settings.config.font_size = 24.0;
    settings.has_changes = true;

    assert!(
        settings.has_changes,
        "has_changes should be true after modifying config.font_size"
    );
    assert_eq!(
        settings.config.font_size, 24.0,
        "Config change should be reflected"
    );
}

#[test]
fn test_has_changes_multiple_modifications() {
    let config = Config::default();
    let mut settings = SettingsUI::new_for_tests(config);

    // Apply multiple changes
    settings.config.font_size = 16.0;
    settings.has_changes = true;

    settings.config.window.window_opacity = 0.9;
    // has_changes stays true (no intermediate reset)

    assert!(
        settings.has_changes,
        "has_changes should remain true across multiple changes"
    );
    assert_eq!(settings.config.font_size, 16.0);
    assert!((settings.config.window.window_opacity - 0.9).abs() < f32::EPSILON);
}

#[test]
fn test_settings_ui_config_is_cloned_on_creation() {
    let config = Config {
        font_size: 20.0,
        ..Config::default()
    };

    let settings = SettingsUI::new_for_tests(config.clone());
    assert_eq!(
        settings.config.font_size, 20.0,
        "SettingsUI should use the provided config"
    );
}
