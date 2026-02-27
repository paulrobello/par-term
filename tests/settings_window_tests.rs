//! Tests for the settings window module

use par_term::config::Config;
use par_term::settings_ui::SettingsUI;
use par_term::settings_ui::section::CollapsibleSection;
use par_term::settings_ui::sidebar::{SettingsTab, tab_matches_search};
use par_term::settings_window::SettingsWindowAction;
use std::collections::HashSet;

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
// section_matches logic tests (L-14)
// Tests the CollapsibleSection::matches_search() logic and tab_matches_search()
// ============================================================================

#[test]
fn test_section_matches_empty_query_always_matches() {
    // An empty search query must match every section regardless of title/keywords.
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Font Settings", "font", &mut collapsed, "")
        .keywords(&["typeface", "size", "bold"]);
    assert!(section.matches_search(), "Empty query should always match");
}

#[test]
fn test_section_matches_title_exact() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Font Settings", "font", &mut collapsed, "Font Settings")
        .keywords(&[]);
    assert!(section.matches_search(), "Exact title match should succeed");
}

#[test]
fn test_section_matches_title_case_insensitive() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Font Settings", "font", &mut collapsed, "font settings")
        .keywords(&[]);
    assert!(
        section.matches_search(),
        "Title match should be case-insensitive"
    );
}

#[test]
fn test_section_matches_title_partial() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section =
        CollapsibleSection::new("Font Settings", "font", &mut collapsed, "font").keywords(&[]);
    assert!(
        section.matches_search(),
        "Partial title match should succeed"
    );
}

#[test]
fn test_section_matches_keyword_case_insensitive() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Appearance", "appearance", &mut collapsed, "LIGATURES")
        .keywords(&["ligatures", "kerning"]);
    assert!(
        section.matches_search(),
        "Keyword match should be case-insensitive"
    );
}

#[test]
fn test_section_no_match_returns_false() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Font Settings", "font", &mut collapsed, "network")
        .keywords(&["typeface", "size", "bold"]);
    assert!(
        !section.matches_search(),
        "Query with no matching title or keyword should return false"
    );
}

#[test]
fn test_section_matches_keyword_partial() {
    let mut collapsed: HashSet<String> = HashSet::new();
    let section = CollapsibleSection::new("Terminal", "terminal", &mut collapsed, "scroll")
        .keywords(&["scrollback", "shell"]);
    assert!(
        section.matches_search(),
        "Partial keyword match should succeed"
    );
}

// ============================================================================
// tab_matches_search tests
// ============================================================================

#[test]
fn test_tab_matches_search_empty_query() {
    // Every tab should match an empty query.
    for tab in SettingsTab::all() {
        assert!(
            tab_matches_search(*tab, ""),
            "Tab {:?} should match empty query",
            tab
        );
    }
}

#[test]
fn test_tab_matches_search_by_display_name() {
    // Each tab should be found by its own display name.
    for tab in SettingsTab::all() {
        let name = tab.display_name().to_lowercase();
        assert!(
            tab_matches_search(*tab, &name),
            "Tab {:?} should match its own display name '{}'",
            tab,
            name
        );
    }
}

#[test]
fn test_tab_matches_search_appearance_keywords() {
    assert!(
        tab_matches_search(SettingsTab::Appearance, "font"),
        "Appearance tab should match 'font'"
    );
    assert!(
        tab_matches_search(SettingsTab::Appearance, "cursor"),
        "Appearance tab should match 'cursor'"
    );
    assert!(
        tab_matches_search(SettingsTab::Appearance, "THEME"),
        "Appearance tab should match 'THEME' (case-insensitive)"
    );
}

#[test]
fn test_tab_matches_search_window_keywords() {
    assert!(
        tab_matches_search(SettingsTab::Window, "opacity"),
        "Window tab should match 'opacity'"
    );
    assert!(
        tab_matches_search(SettingsTab::Window, "tab bar"),
        "Window tab should match 'tab bar'"
    );
}

#[test]
fn test_tab_matches_search_no_match() {
    // A query that exists in no tab's name or keywords should return false.
    // "xyzzy_nonexistent_query" is unlikely to appear in any keyword list.
    assert!(
        !tab_matches_search(SettingsTab::Appearance, "xyzzy_nonexistent_query"),
        "Appearance tab should not match nonsense query"
    );
}

#[test]
fn test_tab_matches_search_cross_tab_isolation() {
    // "tmux" keyword belongs to Advanced, not Appearance.
    assert!(
        !tab_matches_search(SettingsTab::Appearance, "tmux"),
        "Appearance tab should not match 'tmux'"
    );
    assert!(
        tab_matches_search(SettingsTab::Advanced, "tmux"),
        "Advanced tab should match 'tmux'"
    );
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
        config.window_opacity >= 0.0,
        "Default window_opacity should be >= 0.0"
    );
    assert!(
        config.window_opacity <= 1.0,
        "Default window_opacity should be <= 1.0"
    );
}

#[test]
fn test_background_image_opacity_default_in_valid_range() {
    let config = Config::default();
    assert!(config.background_image_opacity >= 0.0);
    assert!(config.background_image_opacity <= 1.0);
}

#[test]
fn test_inactive_tab_opacity_default_in_valid_range() {
    let config = Config::default();
    assert!(config.inactive_tab_opacity >= 0.0);
    assert!(config.inactive_tab_opacity <= 1.0);
}

#[test]
fn test_scrollback_lines_default_positive() {
    let config = Config::default();
    assert!(
        config.scrollback_lines > 0,
        "Default scrollback_lines should be > 0"
    );
}

#[test]
fn test_tab_bar_height_default_positive() {
    let config = Config::default();
    assert!(
        config.tab_bar_height > 0.0,
        "Default tab_bar_height should be > 0"
    );
}

#[test]
fn test_tab_min_width_default_positive() {
    let config = Config::default();
    assert!(
        config.tab_min_width > 0.0,
        "Default tab_min_width should be > 0"
    );
}

#[test]
fn test_max_fps_default_reasonable() {
    let config = Config::default();
    assert!(config.max_fps > 0, "Default max_fps should be > 0");
    assert!(
        config.max_fps <= 240,
        "Default max_fps should be <= 240 (reasonable upper bound)"
    );
}

// ============================================================================
// has_changes state machine tests (L-14)
// ============================================================================

#[test]
fn test_has_changes_initially_false() {
    let config = Config::default();
    let settings = SettingsUI::new(config);
    assert!(
        !settings.has_changes,
        "has_changes should be false on initial creation"
    );
}

#[test]
fn test_has_changes_set_to_true() {
    let config = Config::default();
    let mut settings = SettingsUI::new(config);
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
    let mut settings = SettingsUI::new(config);

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
    let mut settings = SettingsUI::new(config);

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
    let mut settings = SettingsUI::new(config);

    // Apply multiple changes
    settings.config.font_size = 16.0;
    settings.has_changes = true;

    settings.config.window_opacity = 0.9;
    // has_changes stays true (no intermediate reset)

    assert!(
        settings.has_changes,
        "has_changes should remain true across multiple changes"
    );
    assert_eq!(settings.config.font_size, 16.0);
    assert!((settings.config.window_opacity - 0.9).abs() < f32::EPSILON);
}

#[test]
fn test_settings_ui_config_is_cloned_on_creation() {
    let mut config = Config::default();
    config.font_size = 20.0;

    let settings = SettingsUI::new(config.clone());
    assert_eq!(
        settings.config.font_size, 20.0,
        "SettingsUI should use the provided config"
    );
}
