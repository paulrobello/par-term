//! Tests for the settings window module

use par_term::config::Config;
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
