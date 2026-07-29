//! Regression tests for script auto-start (issue #220).
//!
//! `auto_start` was documented as starting a script when a tab is created, but
//! nothing in the codebase ever acted on the flag — it only rendered an `[auto]`
//! badge in the Settings UI, so scripts could be started only by hand.
//!
//! These tests lock in the selection semantics used by the tab-creation
//! auto-start loop. The end-to-end check that a real tab spawns the process
//! lives in `src/tab/constructors.rs` (it needs crate-internal state and a live
//! PTY, so it is `#[ignore]`d).

use par_term::config::scripting::ScriptConfig;
use par_term::config::{Config, RestartPolicy};
use std::collections::HashMap;

/// Build a script config with the auto-start-relevant fields set.
fn script(name: &str, script_path: &str, enabled: bool, auto_start: bool) -> ScriptConfig {
    ScriptConfig {
        name: name.to_string(),
        enabled,
        script_path: script_path.to_string(),
        args: Vec::new(),
        auto_start,
        restart_policy: RestartPolicy::Never,
        restart_delay_ms: 0,
        subscriptions: Vec::new(),
        env_vars: HashMap::new(),
        allow_write_text: false,
        allow_run_command: false,
        allow_change_config: false,
        write_text_rate_limit: 0,
        run_command_rate_limit: 0,
    }
}

#[test]
fn test_enabled_auto_start_script_is_selected() {
    let s = script("observer", "/bin/echo", true, true);
    assert!(
        s.should_auto_start(),
        "enabled + auto_start must auto-start on tab creation"
    );
}

#[test]
fn test_auto_start_false_is_not_selected() {
    let s = script("manual", "/bin/echo", true, false);
    assert!(
        !s.should_auto_start(),
        "auto_start: false must remain manual-start only"
    );
}

#[test]
fn test_disabled_script_never_auto_starts() {
    // `enabled: false` disables the script entirely. The Settings UI start
    // button already refuses to start it, so auto-start must agree.
    let s = script("disabled", "/bin/echo", false, true);
    assert!(
        !s.should_auto_start(),
        "enabled: false must veto auto_start: true"
    );
}

#[test]
fn test_disabled_and_no_auto_start_is_not_selected() {
    let s = script("off", "/bin/echo", false, false);
    assert!(!s.should_auto_start());
}

#[test]
fn test_auto_start_defaults_to_off_for_minimal_config() {
    // Minimal YAML omits auto_start, so a script must not spawn implicitly.
    let yaml = r#"
name: minimal
script_path: /bin/echo
"#;
    let s: ScriptConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(s.enabled, "enabled defaults to true");
    assert!(!s.auto_start, "auto_start defaults to false");
    assert!(
        !s.should_auto_start(),
        "a minimal script config must not auto-start"
    );
}

#[test]
fn test_auto_start_selection_filters_mixed_config() {
    // Mirrors the tab-creation loop: only entries passing `should_auto_start`
    // are spawned, and the surviving indices must stay aligned with
    // `config.scripts` because per-tab tracking state is indexed by position.
    let mut config = Config::default();
    config.scripts = vec![
        script("auto-on", "/bin/echo", true, true),
        script("manual", "/bin/echo", true, false),
        script("disabled-auto", "/bin/echo", false, true),
        script("auto-on-2", "/bin/echo", true, true),
    ];

    let selected: Vec<usize> = config
        .scripts
        .iter()
        .enumerate()
        .filter(|(_, s)| s.should_auto_start())
        .map(|(i, _)| i)
        .collect();

    assert_eq!(
        selected,
        vec![0, 3],
        "only enabled + auto_start scripts start, keeping their config indices"
    );
}

#[test]
fn test_auto_start_parsed_from_yaml_config() {
    // The exact config shape from the issue report must select the script.
    let yaml = r#"
name: test-observer
script_path: /bin/echo
auto_start: true
subscriptions: []
"#;
    let s: ScriptConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(
        s.should_auto_start(),
        "issue #220 repro config must auto-start"
    );
}
