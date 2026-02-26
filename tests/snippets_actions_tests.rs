//! Integration tests for snippets and actions system.
//!
//! These tests verify the end-to-end functionality of:
//! - Snippet creation, storage, and execution
//! - Variable substitution
//! - Custom action execution
//! - Keybinding generation
//! - Config persistence

use par_term::badge::SessionVariables;
use par_term::config::{Config, CustomActionConfig, SnippetConfig, SnippetLibrary};
use par_term::keybindings::parser::parse_key_sequence;
use par_term::snippets::VariableSubstitutor;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a temporary config directory
fn setup_temp_config() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("par-term");
    fs::create_dir_all(&config_dir).unwrap();
    (temp_dir, config_dir)
}

#[test]
fn test_snippet_creation_and_storage() {
    let snippet = SnippetConfig::new(
        "test_snippet".to_string(),
        "Test Snippet".to_string(),
        "echo 'Hello, World!'".to_string(),
    );

    assert_eq!(snippet.id, "test_snippet");
    assert_eq!(snippet.title, "Test Snippet");
    assert_eq!(snippet.content, "echo 'Hello, World!'");
    assert!(snippet.enabled);
    assert!(snippet.keybinding.is_none());
    assert!(snippet.variables.is_empty());
}

#[test]
fn test_snippet_with_keybinding() {
    let snippet = SnippetConfig::new(
        "test_snippet".to_string(),
        "Test Snippet".to_string(),
        "echo 'test'".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());

    assert_eq!(snippet.keybinding, Some("Ctrl+Shift+T".to_string()));
}

#[test]
fn test_snippet_with_folder() {
    let snippet = SnippetConfig::new(
        "test_snippet".to_string(),
        "Test Snippet".to_string(),
        "echo 'test'".to_string(),
    )
    .with_folder("Git".to_string());

    assert_eq!(snippet.folder, Some("Git".to_string()));
}

#[test]
fn test_snippet_with_custom_variable() {
    let snippet = SnippetConfig::new(
        "test_snippet".to_string(),
        "Test Snippet".to_string(),
        "echo 'test'".to_string(),
    )
    .with_variable("name".to_string(), "value".to_string());

    assert_eq!(snippet.variables.get("name"), Some(&"value".to_string()));
}

#[test]
fn test_variable_substitution_builtin() {
    let substitutor = VariableSubstitutor::new();
    let custom_vars = HashMap::new();

    let result = substitutor
        .substitute("Hello \\(user), today is \\(date)", &custom_vars)
        .unwrap();

    assert!(result.contains("Hello "));
    assert!(result.contains(", today is "));
    assert!(!result.contains("\\("));
}

#[test]
fn test_variable_substitution_custom() {
    let substitutor = VariableSubstitutor::new();
    let mut custom_vars = HashMap::new();
    custom_vars.insert("name".to_string(), "Alice".to_string());

    let result = substitutor
        .substitute("Hello \\(name)!", &custom_vars)
        .unwrap();

    assert_eq!(result, "Hello Alice!");
}

#[test]
fn test_variable_substitution_mixed() {
    let substitutor = VariableSubstitutor::new();
    let mut custom_vars = HashMap::new();
    custom_vars.insert("greeting".to_string(), "Welcome".to_string());

    let result = substitutor
        .substitute("\\(greeting) \\(user)!", &custom_vars)
        .unwrap();

    assert!(result.starts_with("Welcome "));
    assert!(result.ends_with("!"));
    assert!(!result.contains("\\("));
}

#[test]
fn test_variable_substitution_undefined() {
    let substitutor = VariableSubstitutor::new();
    let custom_vars = HashMap::new();

    let result = substitutor.substitute("Value: \\(undefined)", &custom_vars);

    assert!(result.is_err());
    match result.unwrap_err() {
        par_term::snippets::SubstitutionError::UndefinedVariable(name) => {
            assert_eq!(name, "undefined");
        }
        _ => panic!("Expected UndefinedVariable error"),
    }
}

#[test]
fn test_variable_substitution_empty() {
    let substitutor = VariableSubstitutor::new();
    let custom_vars = HashMap::new();

    let result = substitutor
        .substitute("No variables here", &custom_vars)
        .unwrap();

    assert_eq!(result, "No variables here");
}

#[test]
fn test_variable_substitution_duplicate() {
    let substitutor = VariableSubstitutor::new();
    let mut custom_vars = HashMap::new();
    custom_vars.insert("name".to_string(), "Alice".to_string());

    let result = substitutor
        .substitute("\\(name) and \\(name)", &custom_vars)
        .unwrap();

    assert_eq!(result, "Alice and Alice");
}

#[test]
fn test_has_variables() {
    let substitutor = VariableSubstitutor::new();

    assert!(substitutor.has_variables("Hello \\(user)"));
    assert!(!substitutor.has_variables("Hello world"));
}

#[test]
fn test_extract_variables() {
    let substitutor = VariableSubstitutor::new();

    let vars = substitutor.extract_variables("\\(user) and \\(date) and \\(time)");

    assert_eq!(vars, vec!["user", "date", "time"]);
}

#[test]
fn test_custom_action_shell_command() {
    let action = CustomActionConfig::ShellCommand {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        notify_on_success: false,
        keybinding: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert_eq!(action.title(), "Test Action");
    assert!(action.is_shell_command());
    assert!(!action.is_insert_text());
    assert!(!action.is_key_sequence());
}

#[test]
fn test_custom_action_insert_text() {
    let action = CustomActionConfig::InsertText {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        text: "echo 'test'".to_string(),
        variables: HashMap::new(),
        keybinding: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert!(action.is_insert_text());
    assert!(!action.is_shell_command());
    assert!(!action.is_key_sequence());
}

#[test]
fn test_custom_action_key_sequence() {
    let action = CustomActionConfig::KeySequence {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        keys: "Ctrl+C".to_string(),
        keybinding: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert!(action.is_key_sequence());
    assert!(!action.is_shell_command());
    assert!(!action.is_insert_text());
}

#[test]
fn test_config_persistence_snippets() {
    let (_temp_dir, config_dir) = setup_temp_config();

    // Create config with snippets
    let mut config = Config::default();
    config.snippets.push(SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    ));

    // Save config
    let config_path = config_dir.join("config.yaml");
    let yaml = serde_yml::to_string(&config).unwrap();
    fs::write(&config_path, yaml).unwrap();

    // Load config
    let loaded_yaml = fs::read_to_string(&config_path).unwrap();
    let loaded_config: Config = serde_yml::from_str(&loaded_yaml).unwrap();

    assert_eq!(loaded_config.snippets.len(), 1);
    assert_eq!(loaded_config.snippets[0].id, "test");
    assert_eq!(loaded_config.snippets[0].title, "Test");
}

#[test]
fn test_config_persistence_actions() {
    let (_temp_dir, config_dir) = setup_temp_config();

    // Create config with actions
    let mut config = Config::default();
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        keybinding: None,
        keybinding_enabled: true,
        description: None,
    });

    // Save config
    let config_path = config_dir.join("config.yaml");
    let yaml = serde_yml::to_string(&config).unwrap();
    fs::write(&config_path, yaml).unwrap();

    // Load config
    let loaded_yaml = fs::read_to_string(&config_path).unwrap();
    let loaded_config: Config = serde_yml::from_str(&loaded_yaml).unwrap();

    assert_eq!(loaded_config.actions.len(), 1);
    assert_eq!(loaded_config.actions[0].id(), "test");
    assert_eq!(loaded_config.actions[0].title(), "Test");
}

#[test]
fn test_generate_snippet_keybindings() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add snippet with keybinding
    config.snippets.push(
        SnippetConfig::new(
            "test".to_string(),
            "Test".to_string(),
            "content".to_string(),
        )
        .with_keybinding("Ctrl+Shift+T".to_string()),
    );

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Check that keybinding was generated
    assert_eq!(config.keybindings.len(), initial_count + 1);
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+T");
    assert_eq!(config.keybindings.last().unwrap().action, "snippet:test");
}

#[test]
fn test_generate_snippet_keybindings_no_duplicates() {
    let mut config = Config::default();

    // Add snippet with keybinding
    config.snippets.push(
        SnippetConfig::new(
            "test".to_string(),
            "Test".to_string(),
            "content".to_string(),
        )
        .with_keybinding("Ctrl+Shift+T".to_string()),
    );

    // Generate keybindings twice
    config.generate_snippet_action_keybindings();
    let count_after_first = config.keybindings.len();

    config.generate_snippet_action_keybindings();
    let count_after_second = config.keybindings.len();

    // Should not add duplicates
    assert_eq!(count_after_first, count_after_second);
}

#[test]
fn test_generate_snippet_keybindings_disabled_snippet() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add disabled snippet with keybinding
    let mut snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());
    snippet.enabled = false;
    config.snippets.push(snippet);

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Should not generate keybinding for disabled snippet
    assert_eq!(config.keybindings.len(), initial_count);
}

#[test]
fn test_generate_snippet_keybindings_empty_keybinding() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add snippet without keybinding
    config.snippets.push(SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    ));

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Should not generate keybinding
    assert_eq!(config.keybindings.len(), initial_count);
}

#[test]
fn test_snippet_serialization() {
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test Snippet".to_string(),
        "echo 'Hello'".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string())
    .with_folder("Git".to_string());

    // Serialize
    let yaml = serde_yml::to_string(&snippet).unwrap();

    // Deserialize
    let deserialized: SnippetConfig = serde_yml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.id, snippet.id);
    assert_eq!(deserialized.title, snippet.title);
    assert_eq!(deserialized.content, snippet.content);
    assert_eq!(deserialized.keybinding, snippet.keybinding);
    assert_eq!(deserialized.folder, snippet.folder);
}

#[test]
fn test_action_serialization() {
    let action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test Action".to_string(),
        command: "npm".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: true,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: true,
        description: Some("Run tests".to_string()),
    };

    // Serialize
    let yaml = serde_yml::to_string(&action).unwrap();

    // Deserialize
    let deserialized: CustomActionConfig = serde_yml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.id(), action.id());
    assert_eq!(deserialized.title(), action.title());
}

#[test]
fn test_variable_substitution_all_builtins() {
    let substitutor = VariableSubstitutor::new();
    let custom_vars = HashMap::new();

    // Test that all built-in variables resolve without errors
    let builtins = vec![
        "date",
        "time",
        "datetime",
        "hostname",
        "user",
        "path",
        "git_branch",
        "git_commit",
        "uuid",
        "random",
    ];

    for var in builtins {
        let result = substitutor.substitute(&format!("\\({})", var), &custom_vars);
        assert!(result.is_ok(), "Variable {} should resolve", var);
        let resolved = result.unwrap();
        assert!(
            !resolved.contains("\\("),
            "Variable {} should be substituted",
            var
        );
    }
}

#[test]
fn test_snippet_with_multiple_variables() {
    let substitutor = VariableSubstitutor::new();
    let mut custom_vars = HashMap::new();
    custom_vars.insert("project".to_string(), "par-term".to_string());

    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "echo '\\(user) working on \\(project) at \\(path)'".to_string(),
    )
    .with_variable("project".to_string(), "par-term".to_string());

    let result = substitutor
        .substitute(&snippet.content, &snippet.variables)
        .unwrap();

    // Should contain the substituted values
    assert!(result.contains("working on"));
    assert!(result.contains("at"));
    assert!(!result.contains("\\("));
}

#[test]
fn test_action_types_serialization_roundtrip() {
    let actions = vec![
        CustomActionConfig::ShellCommand {
            id: "shell".to_string(),
            title: "Shell".to_string(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            notify_on_success: false,
            keybinding: Some("Ctrl+Shift+S".to_string()),
            keybinding_enabled: true,
            description: None,
        },
        CustomActionConfig::InsertText {
            id: "insert".to_string(),
            title: "Insert".to_string(),
            text: "hello".to_string(),
            variables: HashMap::new(),
            keybinding: None,
            keybinding_enabled: true,
            description: None,
        },
        CustomActionConfig::KeySequence {
            id: "keys".to_string(),
            title: "Keys".to_string(),
            keys: "Ctrl+C".to_string(),
            keybinding: Some("Ctrl+Shift+K".to_string()),
            keybinding_enabled: false,
            description: None,
        },
    ];

    for action in actions {
        // Serialize
        let yaml = serde_yml::to_string(&action).unwrap();

        // Deserialize
        let deserialized: CustomActionConfig = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.id(), action.id());
        assert_eq!(deserialized.title(), action.title());
        assert_eq!(deserialized.is_shell_command(), action.is_shell_command());
        assert_eq!(deserialized.is_insert_text(), action.is_insert_text());
        assert_eq!(deserialized.is_key_sequence(), action.is_key_sequence());
    }
}

#[test]
fn test_generate_snippet_keybindings_disabled_keybinding() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add snippet with keybinding but keybinding disabled
    let mut snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());
    snippet.keybinding_enabled = false;
    config.snippets.push(snippet);

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Should not generate keybinding when keybinding_enabled is false
    assert_eq!(config.keybindings.len(), initial_count);
}

#[test]
fn test_snippet_keybinding_enabled_field() {
    // Test default value is true
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());

    assert_eq!(snippet.keybinding, Some("Ctrl+Shift+T".to_string()));
    assert!(snippet.keybinding_enabled);

    // Test with_keybinding_disabled builder
    let snippet_disabled = SnippetConfig::new(
        "test2".to_string(),
        "Test2".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+X".to_string())
    .with_keybinding_disabled();

    assert_eq!(
        snippet_disabled.keybinding,
        Some("Ctrl+Shift+X".to_string())
    );
    assert!(!snippet_disabled.keybinding_enabled);
}

#[test]
fn test_generate_snippet_keybindings_update_existing() {
    let mut config = Config::default();

    // Add snippet with initial keybinding
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());
    config.snippets.push(snippet);

    // Generate keybindings first time
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+T");

    // Update snippet keybinding
    config.snippets[0].keybinding = Some("Ctrl+Shift+X".to_string());

    // Generate keybindings again - should update existing keybinding
    config.generate_snippet_action_keybindings();

    // Should still have the same number of keybindings (not add a duplicate)
    let snippet_keybindings: Vec<_> = config
        .keybindings
        .iter()
        .filter(|kb| kb.action == "snippet:test")
        .collect();

    assert_eq!(snippet_keybindings.len(), 1);
    assert_eq!(snippet_keybindings[0].key, "Ctrl+Shift+X");
}

#[test]
fn test_generate_snippet_keybindings_remove_when_cleared() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add snippet with keybinding
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_keybinding("Ctrl+Shift+T".to_string());
    config.snippets.push(snippet);

    // Generate keybindings
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.len(), initial_count + 1);

    // Remove keybinding from snippet
    config.snippets[0].keybinding = None;

    // Generate keybindings again - should remove the keybinding
    config.generate_snippet_action_keybindings();

    // Should be back to initial count
    assert_eq!(config.keybindings.len(), initial_count);
    // Should not have the snippet keybinding anymore
    assert!(
        !config
            .keybindings
            .iter()
            .any(|kb| kb.action == "snippet:test")
    );
}

#[test]
fn test_snippet_auto_execute_field() {
    // Test default value is false
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "echo 'hello'".to_string(),
    );

    assert!(!snippet.auto_execute);

    // Test with_auto_execute builder
    let snippet_auto = SnippetConfig::new(
        "test2".to_string(),
        "Test2".to_string(),
        "echo 'world'".to_string(),
    )
    .with_auto_execute();

    assert!(snippet_auto.auto_execute);
}

#[test]
fn test_snippet_serialization_with_auto_execute() {
    let snippet = SnippetConfig::new(
        "test".to_string(),
        "Test".to_string(),
        "content".to_string(),
    )
    .with_auto_execute();

    // Serialize
    let yaml = serde_yml::to_string(&snippet).unwrap();

    // Check that auto_execute is in the YAML
    assert!(yaml.contains("auto_execute"));

    // Deserialize
    let deserialized: SnippetConfig = serde_yml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.id, snippet.id);
    assert!(deserialized.auto_execute);
}

#[test]
fn test_session_variable_substitution() {
    // Create session variables with test data
    let mut session_vars = SessionVariables::new();
    session_vars.hostname = "testhost".to_string();
    session_vars.username = "testuser".to_string();
    session_vars.path = "/home/test/projects".to_string();
    session_vars.job = Some("vim".to_string());

    // Test substitution with session variables
    let substitutor = VariableSubstitutor::new();
    let custom_vars = std::collections::HashMap::new();

    let result = substitutor
        .substitute_with_session(
            "User: \\(session.username), Host: \\(session.hostname), Path: \\(session.path), Job: \\(session.job)",
            &custom_vars,
            Some(&session_vars),
        )
        .unwrap();

    assert_eq!(
        result,
        "User: testuser, Host: testhost, Path: /home/test/projects, Job: vim"
    );
}

#[test]
fn test_session_variables_override_builtins() {
    // Create session variables
    let mut session_vars = SessionVariables::new();
    session_vars.hostname = "session-host".to_string();

    // Test that session variables take precedence over built-in
    let substitutor = VariableSubstitutor::new();
    let custom_vars = std::collections::HashMap::new();

    let result = substitutor
        .substitute_with_session(
            "\\(session.hostname) vs \\(hostname)",
            &custom_vars,
            Some(&session_vars),
        )
        .unwrap();

    // Both should work, giving different values
    assert!(result.contains("session-host"));
    assert!(result.contains(" vs "));
}

#[test]
fn test_custom_variables_override_session() {
    // Create session and custom variables
    let mut session_vars = SessionVariables::new();
    session_vars.hostname = "session-host".to_string();

    let mut custom_vars = std::collections::HashMap::new();
    custom_vars.insert("hostname".to_string(), "custom-host".to_string());

    // Test that custom variables have highest priority
    let substitutor = VariableSubstitutor::new();

    let result = substitutor
        .substitute_with_session(
            "\\(session.hostname) vs \\(hostname)",
            &custom_vars,
            Some(&session_vars),
        )
        .unwrap();

    assert_eq!(result, "session-host vs custom-host");
}

#[test]
fn test_generate_action_keybindings() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add action with keybinding
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Check that keybinding was generated
    assert_eq!(config.keybindings.len(), initial_count + 1);
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+R");
    assert_eq!(
        config.keybindings.last().unwrap().action,
        "action:run_tests"
    );
}

#[test]
fn test_generate_action_keybindings_no_duplicates() {
    let mut config = Config::default();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings twice
    config.generate_snippet_action_keybindings();
    let count_after_first = config.keybindings.len();

    config.generate_snippet_action_keybindings();
    let count_after_second = config.keybindings.len();

    // Should not add duplicates
    assert_eq!(count_after_first, count_after_second);
}

#[test]
fn test_generate_action_keybindings_disabled() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add action with keybinding but disabled
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: false,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Should not generate keybinding when keybinding_enabled is false
    assert_eq!(config.keybindings.len(), initial_count);
}

#[test]
fn test_generate_action_keybindings_update_existing() {
    let mut config = Config::default();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings first time
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+R");

    // Update action keybinding
    config.actions[0].set_keybinding(Some("Ctrl+Shift+X".to_string()));

    // Generate keybindings again - should update existing
    config.generate_snippet_action_keybindings();

    let action_keybindings: Vec<_> = config
        .keybindings
        .iter()
        .filter(|kb| kb.action == "action:run_tests")
        .collect();

    assert_eq!(action_keybindings.len(), 1);
    assert_eq!(action_keybindings[0].key, "Ctrl+Shift+X");
}

#[test]
fn test_generate_action_keybindings_remove_when_cleared() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.len(), initial_count + 1);

    // Clear keybinding from action
    config.actions[0].set_keybinding(None);

    // Generate keybindings again - should remove the stale keybinding
    config.generate_snippet_action_keybindings();

    assert_eq!(config.keybindings.len(), initial_count);
    assert!(
        !config
            .keybindings
            .iter()
            .any(|kb| kb.action == "action:run_tests")
    );
}

#[test]
fn test_action_keybinding_accessors() {
    let mut action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        keybinding: None,
        keybinding_enabled: true,
        description: None,
    };

    assert!(action.keybinding().is_none());
    assert!(action.keybinding_enabled());

    action.set_keybinding(Some("Ctrl+Shift+T".to_string()));
    assert_eq!(action.keybinding(), Some("Ctrl+Shift+T"));

    action.set_keybinding_enabled(false);
    assert!(!action.keybinding_enabled());
}

#[test]
fn test_action_keybinding_serialization_roundtrip() {
    let action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        keybinding: Some("Ctrl+Shift+T".to_string()),
        keybinding_enabled: true,
        description: None,
    };

    // Serialize
    let yaml = serde_yml::to_string(&action).unwrap();
    assert!(yaml.contains("keybinding"));

    // Deserialize
    let deserialized: CustomActionConfig = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(deserialized.keybinding(), Some("Ctrl+Shift+T"));
    assert!(deserialized.keybinding_enabled());
}

// ============================================================================
// Key Sequence Parsing Tests
// ============================================================================

#[test]
fn test_key_sequence_parsing_single_key() {
    let seqs = parse_key_sequence("Enter").unwrap();
    assert_eq!(seqs.len(), 1);
    assert_eq!(seqs[0], b"\r");
}

#[test]
fn test_key_sequence_parsing_ctrl_combo() {
    let seqs = parse_key_sequence("Ctrl+C").unwrap();
    assert_eq!(seqs.len(), 1);
    assert_eq!(seqs[0], vec![0x03]); // ETX
}

#[test]
fn test_key_sequence_parsing_multi_keys() {
    let seqs = parse_key_sequence("Up Up Down Down").unwrap();
    assert_eq!(seqs.len(), 4);
    assert_eq!(seqs[0], b"\x1b[A");
    assert_eq!(seqs[1], b"\x1b[A");
    assert_eq!(seqs[2], b"\x1b[B");
    assert_eq!(seqs[3], b"\x1b[B");
}

#[test]
fn test_key_sequence_parsing_invalid() {
    assert!(parse_key_sequence("NotAKey").is_err());
    assert!(parse_key_sequence("").is_err());
}

// ============================================================================
// Snippet Library Export/Import Tests
// ============================================================================

#[test]
fn test_snippet_library_export_import() {
    let snippets = vec![
        SnippetConfig::new(
            "s1".to_string(),
            "Snippet 1".to_string(),
            "echo hello".to_string(),
        )
        .with_folder("Git".to_string()),
        SnippetConfig::new(
            "s2".to_string(),
            "Snippet 2".to_string(),
            "echo world".to_string(),
        )
        .with_keybinding("Ctrl+Shift+S".to_string()),
    ];

    let library = SnippetLibrary {
        snippets: snippets.clone(),
    };

    // Serialize
    let yaml = serde_yml::to_string(&library).unwrap();

    // Deserialize
    let deserialized: SnippetLibrary = serde_yml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.snippets.len(), 2);
    assert_eq!(deserialized.snippets[0].id, "s1");
    assert_eq!(deserialized.snippets[0].title, "Snippet 1");
    assert_eq!(deserialized.snippets[0].folder, Some("Git".to_string()));
    assert_eq!(deserialized.snippets[1].id, "s2");
    assert_eq!(
        deserialized.snippets[1].keybinding,
        Some("Ctrl+Shift+S".to_string())
    );
}

#[test]
fn test_snippet_custom_variables_roundtrip() {
    let snippet = SnippetConfig::new(
        "vars_test".to_string(),
        "Variables Test".to_string(),
        "echo \\(greeting) \\(name)".to_string(),
    )
    .with_variable("greeting".to_string(), "Hello".to_string())
    .with_variable("name".to_string(), "World".to_string());

    // Serialize
    let yaml = serde_yml::to_string(&snippet).unwrap();

    // Deserialize
    let deserialized: SnippetConfig = serde_yml::from_str(&yaml).unwrap();

    assert_eq!(deserialized.variables.len(), 2);
    assert_eq!(
        deserialized.variables.get("greeting"),
        Some(&"Hello".to_string())
    );
    assert_eq!(
        deserialized.variables.get("name"),
        Some(&"World".to_string())
    );
}

#[test]
fn test_snippet_import_duplicate_handling() {
    // Simulate import: existing snippets + imported library
    let existing = vec![SnippetConfig::new(
        "existing".to_string(),
        "Existing".to_string(),
        "content".to_string(),
    )];

    let import_library = SnippetLibrary {
        snippets: vec![
            SnippetConfig::new(
                "existing".to_string(), // Duplicate ID
                "Duplicate".to_string(),
                "other content".to_string(),
            ),
            SnippetConfig::new(
                "new_one".to_string(), // New ID
                "New".to_string(),
                "new content".to_string(),
            ),
        ],
    };

    let existing_ids: std::collections::HashSet<String> =
        existing.iter().map(|s| s.id.clone()).collect();

    let mut result = existing.clone();
    let mut imported = 0usize;
    let mut skipped = 0usize;

    for snippet in import_library.snippets {
        if existing_ids.contains(&snippet.id) {
            skipped += 1;
            continue;
        }
        result.push(snippet);
        imported += 1;
    }

    assert_eq!(imported, 1);
    assert_eq!(skipped, 1);
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].id, "new_one");
}

#[test]
fn test_snippet_library_empty() {
    let library = SnippetLibrary {
        snippets: Vec::new(),
    };

    let yaml = serde_yml::to_string(&library).unwrap();
    let deserialized: SnippetLibrary = serde_yml::from_str(&yaml).unwrap();

    assert!(deserialized.snippets.is_empty());
}
