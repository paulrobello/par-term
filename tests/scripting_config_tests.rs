use par_term::config::scripting::ScriptConfig;
use par_term::config::{Config, RestartPolicy};
use std::collections::HashMap;

#[test]
fn test_default_config_has_empty_scripts() {
    let config = Config::default();
    assert!(config.scripts.is_empty());
}

#[test]
fn test_script_config_yaml_roundtrip() {
    let mut env_vars = HashMap::new();
    env_vars.insert("FOO".to_string(), "bar".to_string());
    env_vars.insert("BAZ".to_string(), "qux".to_string());

    let script = ScriptConfig {
        name: "my-observer".to_string(),
        enabled: true,
        script_path: "/usr/local/bin/my-script.py".to_string(),
        args: vec!["--verbose".to_string(), "--mode=watch".to_string()],
        auto_start: true,
        restart_policy: RestartPolicy::OnFailure,
        restart_delay_ms: 5000,
        subscriptions: vec!["output".to_string(), "title_change".to_string()],
        env_vars,
    };

    let yaml = serde_yaml::to_string(&script).unwrap();
    let deserialized: ScriptConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(script, deserialized);
}

#[test]
fn test_script_config_defaults_minimal_yaml() {
    // Only name and script_path are required; everything else should use defaults
    let yaml = r#"
name: test-script
script_path: /bin/my-script
"#;
    let script: ScriptConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(script.name, "test-script");
    assert_eq!(script.script_path, "/bin/my-script");
    assert!(script.enabled); // defaults to true
    assert!(script.args.is_empty());
    assert!(!script.auto_start); // defaults to false
    assert_eq!(script.restart_policy, RestartPolicy::Never); // defaults to Never
    assert_eq!(script.restart_delay_ms, 0); // defaults to 0
    assert!(script.subscriptions.is_empty());
    assert!(script.env_vars.is_empty());
}

#[test]
fn test_script_config_enabled_defaults_to_true() {
    let yaml = r#"
name: test
script_path: /bin/test
"#;
    let script: ScriptConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(script.enabled);
}

#[test]
fn test_script_config_can_be_disabled() {
    let yaml = r#"
name: test
script_path: /bin/test
enabled: false
"#;
    let script: ScriptConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(!script.enabled);
}

#[test]
fn test_script_config_with_subscriptions() {
    let yaml = r#"
name: filtered-observer
script_path: /bin/observer
subscriptions:
  - output
  - bell
  - title_change
"#;
    let script: ScriptConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(
        script.subscriptions,
        vec![
            "output".to_string(),
            "bell".to_string(),
            "title_change".to_string()
        ]
    );
}

#[test]
fn test_script_config_with_env_vars() {
    let yaml = r#"
name: env-test
script_path: /bin/env-script
env_vars:
  API_KEY: secret123
  DEBUG: "true"
"#;
    let script: ScriptConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(script.env_vars.get("API_KEY").unwrap(), "secret123");
    assert_eq!(script.env_vars.get("DEBUG").unwrap(), "true");
    assert_eq!(script.env_vars.len(), 2);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_config_with_scripts_yaml_roundtrip() {
    let mut config = Config::default();
    config.scripts = vec![ScriptConfig {
        name: "logger".to_string(),
        enabled: true,
        script_path: "/usr/local/bin/logger.py".to_string(),
        args: vec!["--output".to_string(), "/tmp/log.txt".to_string()],
        auto_start: true,
        restart_policy: RestartPolicy::Always,
        restart_delay_ms: 1000,
        subscriptions: vec!["output".to_string()],
        env_vars: HashMap::new(),
    }];

    let yaml = serde_yaml::to_string(&config).unwrap();
    let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.scripts, deserialized.scripts);
}

#[test]
fn test_script_config_all_restart_policies() {
    for (yaml_val, expected) in [
        ("never", RestartPolicy::Never),
        ("always", RestartPolicy::Always),
        ("on_failure", RestartPolicy::OnFailure),
    ] {
        let yaml = format!(
            r#"
name: test
script_path: /bin/test
restart_policy: {}
"#,
            yaml_val
        );
        let script: ScriptConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            script.restart_policy, expected,
            "Failed for restart_policy: {}",
            yaml_val
        );
    }
}
