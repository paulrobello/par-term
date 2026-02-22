use par_term::config::automation::PrettifyScope;
use par_term::config::{
    Config, CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig,
};

#[test]
fn test_default_config_has_empty_triggers_and_coprocesses() {
    let config = Config::default();
    assert!(config.triggers.is_empty());
    assert!(config.coprocesses.is_empty());
}

#[test]
fn test_trigger_config_yaml_roundtrip() {
    let trigger = TriggerConfig {
        name: "error-detect".to_string(),
        pattern: r"ERROR:\s+(.+)".to_string(),
        enabled: true,
        actions: vec![
            TriggerActionConfig::Highlight {
                fg: Some([255, 0, 0]),
                bg: None,
                duration_ms: 5000,
            },
            TriggerActionConfig::Notify {
                title: "Error!".to_string(),
                message: "Found an error".to_string(),
            },
        ],
    };

    let yaml = serde_yaml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
}

#[test]
fn test_trigger_config_disabled() {
    let yaml = r#"
name: test
pattern: "foo"
enabled: false
actions: []
"#;
    let trigger: TriggerConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(!trigger.enabled);
}

#[test]
fn test_trigger_config_defaults() {
    // enabled defaults to true, actions defaults to empty
    let yaml = r#"
name: test
pattern: "foo"
"#;
    let trigger: TriggerConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(trigger.enabled);
    assert!(trigger.actions.is_empty());
}

#[test]
fn test_all_trigger_action_variants_serialize_deserialize() {
    let actions = vec![
        TriggerActionConfig::Highlight {
            fg: Some([255, 0, 0]),
            bg: None,
            duration_ms: 5000,
        },
        TriggerActionConfig::Notify {
            title: "t".into(),
            message: "m".into(),
        },
        TriggerActionConfig::MarkLine {
            label: Some("mark".into()),
            color: None,
        },
        TriggerActionConfig::SetVariable {
            name: "n".into(),
            value: "v".into(),
        },
        TriggerActionConfig::RunCommand {
            command: "echo".into(),
            args: vec!["hi".into()],
        },
        TriggerActionConfig::PlaySound {
            sound_id: "bell".into(),
            volume: 80,
        },
        TriggerActionConfig::SendText {
            text: "hello".into(),
            delay_ms: 100,
        },
        TriggerActionConfig::Prettify {
            format: "json".into(),
            scope: PrettifyScope::CommandOutput,
            block_end: None,
            sub_format: None,
            command_filter: None,
        },
    ];

    for action in &actions {
        let yaml = serde_yaml::to_string(action).unwrap();
        let deserialized: TriggerActionConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(action, &deserialized);
    }
}

#[test]
fn test_trigger_action_highlight_defaults() {
    let yaml = r#"
type: highlight
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        action,
        TriggerActionConfig::Highlight {
            fg: None,
            bg: None,
            duration_ms: 5000,
        }
    );
}

#[test]
fn test_trigger_action_play_sound_defaults() {
    let yaml = r#"
type: play_sound
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        action,
        TriggerActionConfig::PlaySound {
            sound_id: String::new(),
            volume: 50,
        }
    );
}

#[test]
fn test_trigger_action_to_core_action_highlight() {
    use par_term_emu_core_rust::terminal::TriggerAction;

    let config_action = TriggerActionConfig::Highlight {
        fg: Some([255, 0, 0]),
        bg: Some([0, 255, 0]),
        duration_ms: 3000,
    };
    let core_action = config_action.to_core_action();
    assert_eq!(
        core_action,
        TriggerAction::Highlight {
            fg: Some((255, 0, 0)),
            bg: Some((0, 255, 0)),
            duration_ms: 3000,
        }
    );
}

#[test]
fn test_trigger_action_to_core_action_all_variants() {
    use par_term_emu_core_rust::terminal::TriggerAction;

    let pairs: Vec<(TriggerActionConfig, TriggerAction)> = vec![
        (
            TriggerActionConfig::Notify {
                title: "t".into(),
                message: "m".into(),
            },
            TriggerAction::Notify {
                title: "t".into(),
                message: "m".into(),
            },
        ),
        (
            TriggerActionConfig::MarkLine {
                label: Some("L".into()),
                color: None,
            },
            TriggerAction::MarkLine {
                label: Some("L".into()),
                color: None,
            },
        ),
        (
            TriggerActionConfig::SetVariable {
                name: "n".into(),
                value: "v".into(),
            },
            TriggerAction::SetVariable {
                name: "n".into(),
                value: "v".into(),
            },
        ),
        (
            TriggerActionConfig::RunCommand {
                command: "echo".into(),
                args: vec!["hi".into()],
            },
            TriggerAction::RunCommand {
                command: "echo".into(),
                args: vec!["hi".into()],
            },
        ),
        (
            TriggerActionConfig::PlaySound {
                sound_id: "bell".into(),
                volume: 80,
            },
            TriggerAction::PlaySound {
                sound_id: "bell".into(),
                volume: 80,
            },
        ),
        (
            TriggerActionConfig::SendText {
                text: "hello".into(),
                delay_ms: 100,
            },
            TriggerAction::SendText {
                text: "hello".into(),
                delay_ms: 100,
            },
        ),
    ];

    for (config_action, expected_core) in pairs {
        let core = config_action.to_core_action();
        assert_eq!(core, expected_core);
    }
}

#[test]
fn test_coprocess_def_config_yaml_roundtrip() {
    let coproc = CoprocessDefConfig {
        name: "logger".to_string(),
        command: "/usr/bin/tee".to_string(),
        args: vec!["/tmp/log.txt".to_string()],
        auto_start: true,
        copy_terminal_output: true,
        restart_policy: RestartPolicy::Never,
        restart_delay_ms: 0,
    };

    let yaml = serde_yaml::to_string(&coproc).unwrap();
    let deserialized: CoprocessDefConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(coproc, deserialized);
}

#[test]
fn test_coprocess_def_config_defaults() {
    let yaml = r#"
name: test
command: /bin/cat
"#;
    let coproc: CoprocessDefConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(coproc.name, "test");
    assert_eq!(coproc.command, "/bin/cat");
    assert!(coproc.args.is_empty());
    assert!(!coproc.auto_start);
    assert!(coproc.copy_terminal_output); // defaults to true
    assert_eq!(coproc.restart_policy, RestartPolicy::Never); // defaults to Never
    assert_eq!(coproc.restart_delay_ms, 0); // defaults to 0
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_config_with_triggers_and_coprocesses_yaml_roundtrip() {
    let mut config = Config::default();
    config.triggers = vec![TriggerConfig {
        name: "error".to_string(),
        pattern: "ERROR".to_string(),
        enabled: true,
        actions: vec![TriggerActionConfig::Highlight {
            fg: Some([255, 0, 0]),
            bg: None,
            duration_ms: 5000,
        }],
    }];
    config.coprocesses = vec![CoprocessDefConfig {
        name: "logger".to_string(),
        command: "/usr/bin/tee".to_string(),
        args: vec!["/tmp/log.txt".to_string()],
        auto_start: false,
        copy_terminal_output: true,
        restart_policy: RestartPolicy::Never,
        restart_delay_ms: 0,
    }];

    let yaml = serde_yaml::to_string(&config).unwrap();
    let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.triggers, deserialized.triggers);
    assert_eq!(config.coprocesses, deserialized.coprocesses);
}

#[test]
fn test_prettify_action_yaml_roundtrip() {
    let action = TriggerActionConfig::Prettify {
        format: "markdown".into(),
        scope: PrettifyScope::Block,
        block_end: Some(r"^```$".into()),
        sub_format: Some("plantuml".into()),
        command_filter: Some(r"^myapi\s+".into()),
    };

    let yaml = serde_yaml::to_string(&action).unwrap();
    let deserialized: TriggerActionConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(action, deserialized);
}

#[test]
fn test_prettify_action_defaults() {
    let yaml = r#"
type: prettify
format: json
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        action,
        TriggerActionConfig::Prettify {
            format: "json".into(),
            scope: PrettifyScope::CommandOutput, // default
            block_end: None,
            sub_format: None,
            command_filter: None,
        }
    );
}

#[test]
fn test_prettify_scope_deserialization() {
    // Line scope
    let yaml = r#"
type: prettify
format: json
scope: line
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    match action {
        TriggerActionConfig::Prettify { scope, .. } => assert_eq!(scope, PrettifyScope::Line),
        _ => panic!("expected Prettify"),
    }

    // Block scope
    let yaml = r#"
type: prettify
format: json
scope: block
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    match action {
        TriggerActionConfig::Prettify { scope, .. } => assert_eq!(scope, PrettifyScope::Block),
        _ => panic!("expected Prettify"),
    }

    // CommandOutput scope
    let yaml = r#"
type: prettify
format: json
scope: command_output
"#;
    let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
    match action {
        TriggerActionConfig::Prettify { scope, .. } => {
            assert_eq!(scope, PrettifyScope::CommandOutput);
        }
        _ => panic!("expected Prettify"),
    }
}

#[test]
fn test_prettify_to_core_action_relays_through_mark_line() {
    use par_term::config::automation::PRETTIFY_RELAY_PREFIX;
    use par_term_emu_core_rust::terminal::TriggerAction;

    let action = TriggerActionConfig::Prettify {
        format: "json".into(),
        scope: PrettifyScope::CommandOutput,
        block_end: None,
        sub_format: None,
        command_filter: Some(r"^myapi\s+".into()),
    };

    let core = action.to_core_action();

    // Should relay through MarkLine with __prettify__ label prefix.
    match core {
        TriggerAction::MarkLine { label, color } => {
            assert!(color.is_none());
            let lbl = label.expect("label should be set");
            assert!(
                lbl.starts_with(PRETTIFY_RELAY_PREFIX),
                "label should start with prettify prefix"
            );
            let json = lbl.strip_prefix(PRETTIFY_RELAY_PREFIX).unwrap();
            let payload: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(payload["format"], "json");
            assert_eq!(payload["scope"], "command_output");
            assert_eq!(payload["command_filter"], r"^myapi\s+");
        }
        other => panic!("expected MarkLine relay, got {:?}", other),
    }
}

#[test]
fn test_prettify_none_format_serializes() {
    let action = TriggerActionConfig::Prettify {
        format: "none".into(),
        scope: PrettifyScope::CommandOutput,
        block_end: None,
        sub_format: None,
        command_filter: Some(r"^bat\s+".into()),
    };

    let yaml = serde_yaml::to_string(&action).unwrap();
    let deserialized: TriggerActionConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(action, deserialized);

    match deserialized {
        TriggerActionConfig::Prettify { format, .. } => assert_eq!(format, "none"),
        _ => panic!("expected Prettify"),
    }
}

#[test]
fn test_trigger_with_prettify_action_roundtrip() {
    let trigger = TriggerConfig {
        name: "Prettify myapi output".to_string(),
        pattern: r#"^\{"api_version":"#.to_string(),
        enabled: true,
        actions: vec![TriggerActionConfig::Prettify {
            format: "json".into(),
            scope: PrettifyScope::CommandOutput,
            block_end: None,
            sub_format: None,
            command_filter: None,
        }],
    };

    let yaml = serde_yaml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
}
