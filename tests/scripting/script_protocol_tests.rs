use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};
use std::collections::HashMap;

// ─── ScriptEvent serialization tests ───

#[test]
fn test_event_serialization_bell_rang() {
    let event = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["kind"], "bell_rang");
    assert_eq!(parsed["data"]["data_type"], "Empty");
}

#[test]
fn test_event_serialization_cwd_changed() {
    let event = ScriptEvent {
        kind: "cwd_changed".to_string(),
        data: ScriptEventData::CwdChanged {
            cwd: "/home/user/project".to_string(),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["kind"], "cwd_changed");
    assert_eq!(parsed["data"]["data_type"], "CwdChanged");
    assert_eq!(parsed["data"]["cwd"], "/home/user/project");
}

#[test]
fn test_event_serialization_command_complete() {
    let event = ScriptEvent {
        kind: "command_complete".to_string(),
        data: ScriptEventData::CommandComplete {
            command: "cargo build".to_string(),
            exit_code: Some(0),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["kind"], "command_complete");
    assert_eq!(parsed["data"]["data_type"], "CommandComplete");
    assert_eq!(parsed["data"]["command"], "cargo build");
    assert_eq!(parsed["data"]["exit_code"], 0);
}

#[test]
fn test_event_serialization_command_complete_no_exit_code() {
    let event = ScriptEvent {
        kind: "command_complete".to_string(),
        data: ScriptEventData::CommandComplete {
            command: "running".to_string(),
            exit_code: None,
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["exit_code"], serde_json::Value::Null);
}

#[test]
fn test_event_serialization_title_changed() {
    let event = ScriptEvent {
        kind: "title_changed".to_string(),
        data: ScriptEventData::TitleChanged {
            title: "vim main.rs".to_string(),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["data_type"], "TitleChanged");
    assert_eq!(parsed["data"]["title"], "vim main.rs");
}

#[test]
fn test_event_serialization_size_changed() {
    let event = ScriptEvent {
        kind: "size_changed".to_string(),
        data: ScriptEventData::SizeChanged {
            cols: 120,
            rows: 40,
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["cols"], 120);
    assert_eq!(parsed["data"]["rows"], 40);
}

#[test]
fn test_event_serialization_variable_changed() {
    let event = ScriptEvent {
        kind: "variable_changed".to_string(),
        data: ScriptEventData::VariableChanged {
            name: "TERM_SESSION".to_string(),
            value: "abc123".to_string(),
            old_value: Some("old123".to_string()),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["name"], "TERM_SESSION");
    assert_eq!(parsed["data"]["value"], "abc123");
    assert_eq!(parsed["data"]["old_value"], "old123");
}

#[test]
fn test_event_serialization_generic() {
    let mut fields = HashMap::new();
    fields.insert(
        "custom_key".to_string(),
        serde_json::Value::String("custom_value".to_string()),
    );
    let event = ScriptEvent {
        kind: "custom_event".to_string(),
        data: ScriptEventData::Generic { fields },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["data"]["fields"]["custom_key"], "custom_value");
}

#[test]
fn test_event_roundtrip() {
    let event = ScriptEvent {
        kind: "cwd_changed".to_string(),
        data: ScriptEventData::CwdChanged {
            cwd: "/tmp".to_string(),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: ScriptEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.kind, "cwd_changed");
    match deserialized.data {
        ScriptEventData::CwdChanged { cwd } => assert_eq!(cwd, "/tmp"),
        _ => panic!("Expected CwdChanged variant"),
    }
}

// ─── ScriptCommand deserialization tests ───

#[test]
fn test_command_deserialization_write_text() {
    let json = r#"{"type": "WriteText", "text": "hello world"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::WriteText { text } => assert_eq!(text, "hello world"),
        _ => panic!("Expected WriteText variant"),
    }
}

#[test]
fn test_command_deserialization_notify() {
    let json = r#"{"type": "Notify", "title": "Build", "body": "Build succeeded"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::Notify { title, body } => {
            assert_eq!(title, "Build");
            assert_eq!(body, "Build succeeded");
        }
        _ => panic!("Expected Notify variant"),
    }
}

#[test]
fn test_command_deserialization_set_badge() {
    let json = r#"{"type": "SetBadge", "text": "3 tasks"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::SetBadge { text } => assert_eq!(text, "3 tasks"),
        _ => panic!("Expected SetBadge variant"),
    }
}

#[test]
fn test_command_deserialization_set_panel() {
    let json = "{\"type\": \"SetPanel\", \"title\": \"Status\", \"content\": \"# All good\\n\\nNo issues.\"}";
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::SetPanel { title, content } => {
            assert_eq!(title, "Status");
            assert_eq!(content, "# All good\n\nNo issues.");
        }
        _ => panic!("Expected SetPanel variant"),
    }
}

#[test]
fn test_command_deserialization_log() {
    let json = r#"{"type": "Log", "level": "info", "message": "Script started"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::Log { level, message } => {
            assert_eq!(level, "info");
            assert_eq!(message, "Script started");
        }
        _ => panic!("Expected Log variant"),
    }
}

#[test]
fn test_command_deserialization_set_variable() {
    let json = r#"{"type": "SetVariable", "name": "MY_VAR", "value": "42"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::SetVariable { name, value } => {
            assert_eq!(name, "MY_VAR");
            assert_eq!(value, "42");
        }
        _ => panic!("Expected SetVariable variant"),
    }
}

#[test]
fn test_command_deserialization_change_config() {
    let json = r#"{"type": "ChangeConfig", "key": "font_size", "value": 14.0}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::ChangeConfig { key, value } => {
            assert_eq!(key, "font_size");
            assert_eq!(value, serde_json::json!(14.0));
        }
        _ => panic!("Expected ChangeConfig variant"),
    }
}

#[test]
fn test_command_deserialization_run_command() {
    let json = r#"{"type": "RunCommand", "command": "ls -la"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    match cmd {
        ScriptCommand::RunCommand { command } => assert_eq!(command, "ls -la"),
        _ => panic!("Expected RunCommand variant"),
    }
}

#[test]
fn test_command_deserialization_clear_panel() {
    let json = r#"{"type": "ClearPanel"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::ClearPanel {}));
}

#[test]
fn test_unknown_command_type_returns_error() {
    let json = r#"{"type": "DoSomethingUnknown", "data": "irrelevant"}"#;
    let result = serde_json::from_str::<ScriptCommand>(json);
    assert!(
        result.is_err(),
        "Unknown command type should return an error"
    );
}

#[test]
fn test_command_serialization_roundtrip() {
    let cmd = ScriptCommand::WriteText {
        text: "test output".to_string(),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: ScriptCommand = serde_json::from_str(&json).unwrap();
    match deserialized {
        ScriptCommand::WriteText { text } => assert_eq!(text, "test output"),
        _ => panic!("Expected WriteText variant"),
    }
}
