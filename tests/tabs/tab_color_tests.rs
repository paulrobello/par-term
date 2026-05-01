//! Tests for custom tab color functionality

use par_term::tab_bar_ui::TabBarAction;

#[test]
fn test_tab_bar_action_set_color() {
    let color = [255, 128, 64];
    let action = TabBarAction::SetColor(1, color);

    match action {
        TabBarAction::SetColor(id, c) => {
            assert_eq!(id, 1);
            assert_eq!(c, [255, 128, 64]);
        }
        _ => panic!("Expected SetColor action"),
    }
}

#[test]
fn test_tab_bar_action_clear_color() {
    let action = TabBarAction::ClearColor(42);

    match action {
        TabBarAction::ClearColor(id) => {
            assert_eq!(id, 42);
        }
        _ => panic!("Expected ClearColor action"),
    }
}

#[test]
fn test_tab_bar_action_equality() {
    // Test SetColor equality
    let action1 = TabBarAction::SetColor(1, [100, 150, 200]);
    let action2 = TabBarAction::SetColor(1, [100, 150, 200]);
    let action3 = TabBarAction::SetColor(1, [100, 150, 201]);
    let action4 = TabBarAction::SetColor(2, [100, 150, 200]);

    assert_eq!(action1, action2);
    assert_ne!(action1, action3); // Different color
    assert_ne!(action1, action4); // Different tab ID

    // Test ClearColor equality
    let clear1 = TabBarAction::ClearColor(1);
    let clear2 = TabBarAction::ClearColor(1);
    let clear3 = TabBarAction::ClearColor(2);

    assert_eq!(clear1, clear2);
    assert_ne!(clear1, clear3);

    // Test different action types are not equal
    assert_ne!(action1, clear1);
}

#[test]
fn test_tab_bar_action_clone() {
    let action = TabBarAction::SetColor(5, [50, 100, 150]);
    let cloned = action.clone();

    assert_eq!(action, cloned);
}

#[test]
fn test_tab_bar_action_debug() {
    let action = TabBarAction::SetColor(1, [255, 0, 0]);
    let debug_str = format!("{:?}", action);

    assert!(debug_str.contains("SetColor"));
    assert!(debug_str.contains("255"));
}
