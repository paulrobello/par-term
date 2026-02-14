//! Integration tests for the ScriptEventForwarder observer bridge.
//!
//! These tests verify that the forwarder correctly captures, converts,
//! and filters terminal events into scripting protocol events.

use std::collections::HashSet;
use std::sync::Arc;

use par_term_emu_core_rust::observer::TerminalObserver;
use par_term_emu_core_rust::terminal::{BellEvent, TerminalEvent};

use par_term::scripting::observer::ScriptEventForwarder;
use par_term::scripting::protocol::ScriptEventData;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bell_event() -> TerminalEvent {
    TerminalEvent::BellRang(BellEvent::VisualBell)
}

fn title_event(title: &str) -> TerminalEvent {
    TerminalEvent::TitleChanged(title.to_string())
}

fn size_event(cols: usize, rows: usize) -> TerminalEvent {
    TerminalEvent::SizeChanged(cols, rows)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_event_forwarder_captures_events() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    // Call on_event directly (simulates the core library dispatching).
    forwarder.on_event(&bell_event());

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1, "Expected exactly 1 event after bell");
    assert_eq!(events[0].kind, "bell_rang");
    assert_eq!(events[0].data, ScriptEventData::Empty {});
}

#[test]
fn test_event_forwarder_filters_by_subscription() {
    let filter = HashSet::from(["bell_rang".to_string()]);
    let forwarder = Arc::new(ScriptEventForwarder::new(Some(filter)));

    // Send a bell (should pass) and a title change (should be filtered out).
    forwarder.on_event(&bell_event());
    forwarder.on_event(&title_event("Hello"));

    let events = forwarder.drain_events();
    assert_eq!(
        events.len(),
        1,
        "Only the bell event should pass the filter"
    );
    assert_eq!(events[0].kind, "bell_rang");
}

#[test]
fn test_event_forwarder_no_filter_captures_all() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    forwarder.on_event(&bell_event());
    forwarder.on_event(&title_event("World"));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 2, "Both events should be captured");
    assert_eq!(events[0].kind, "bell_rang");
    assert_eq!(events[1].kind, "title_changed");

    // Verify title data payload.
    assert_eq!(
        events[1].data,
        ScriptEventData::TitleChanged {
            title: "World".to_string(),
        }
    );
}

#[test]
fn test_event_forwarder_size_changed() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    forwarder.on_event(&size_event(120, 40));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "size_changed");
    assert_eq!(
        events[0].data,
        ScriptEventData::SizeChanged {
            cols: 120,
            rows: 40,
        }
    );
}

#[test]
fn test_drain_clears_buffer() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    forwarder.on_event(&bell_event());

    let first = forwarder.drain_events();
    assert_eq!(first.len(), 1);

    let second = forwarder.drain_events();
    assert!(second.is_empty(), "Buffer should be empty after drain");
}

#[test]
fn test_environment_changed_event() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    let event = TerminalEvent::EnvironmentChanged {
        key: "cwd".to_string(),
        value: "/home/user".to_string(),
        old_value: Some("/tmp".to_string()),
    };
    forwarder.on_event(&event);

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "environment_changed");
    assert_eq!(
        events[0].data,
        ScriptEventData::EnvironmentChanged {
            key: "cwd".to_string(),
            value: "/home/user".to_string(),
            old_value: Some("/tmp".to_string()),
        }
    );
}

#[test]
fn test_user_var_changed_event() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    let event = TerminalEvent::UserVarChanged {
        name: "MY_VAR".to_string(),
        value: "new_val".to_string(),
        old_value: None,
    };
    forwarder.on_event(&event);

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "user_var_changed");
    assert_eq!(
        events[0].data,
        ScriptEventData::VariableChanged {
            name: "MY_VAR".to_string(),
            value: "new_val".to_string(),
            old_value: None,
        }
    );
}

#[test]
fn test_filter_with_multiple_subscriptions() {
    let filter = HashSet::from(["bell_rang".to_string(), "size_changed".to_string()]);
    let forwarder = Arc::new(ScriptEventForwarder::new(Some(filter)));

    forwarder.on_event(&bell_event());
    forwarder.on_event(&title_event("skip"));
    forwarder.on_event(&size_event(80, 24));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 2, "bell_rang and size_changed pass filter");
    assert_eq!(events[0].kind, "bell_rang");
    assert_eq!(events[1].kind, "size_changed");
}

#[test]
fn test_badge_changed_event() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    let event = TerminalEvent::BadgeChanged(Some("Build OK".to_string()));
    forwarder.on_event(&event);

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "badge_changed");
    assert_eq!(
        events[0].data,
        ScriptEventData::BadgeChanged {
            text: Some("Build OK".to_string()),
        }
    );
}

#[test]
fn test_shell_integration_event() {
    let forwarder = Arc::new(ScriptEventForwarder::new(None));

    let event = TerminalEvent::ShellIntegrationEvent {
        event_type: "command_finished".to_string(),
        command: Some("ls -la".to_string()),
        exit_code: Some(0),
        timestamp: Some(1700000000000),
        cursor_line: None,
    };
    forwarder.on_event(&event);

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "command_complete");
    assert_eq!(
        events[0].data,
        ScriptEventData::CommandComplete {
            command: "ls -la".to_string(),
            exit_code: Some(0),
        }
    );
}
