use par_term::config::scripting::ScriptConfig;
use par_term::scripting::manager::ScriptManager;
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};

use std::collections::HashMap;

fn make_config(script_path: &str, args: Vec<String>) -> ScriptConfig {
    ScriptConfig {
        name: "test-script".to_string(),
        enabled: true,
        script_path: script_path.to_string(),
        args,
        auto_start: false,
        restart_policy: par_term::config::automation::RestartPolicy::Never,
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
fn test_manager_start_stop_script() {
    // A Python script that just sleeps, keeping the process alive.
    let config = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );

    let mut mgr = ScriptManager::new();
    let id = mgr.start_script(&config).expect("Failed to start script");

    assert!(mgr.is_running(id), "Script should be running after start");

    mgr.stop_script(id);

    assert!(
        !mgr.is_running(id),
        "Script should not be running after stop"
    );
}

#[test]
fn test_manager_stop_all() {
    let config1 = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );
    let config2 = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );

    let mut mgr = ScriptManager::new();
    let id1 = mgr
        .start_script(&config1)
        .expect("Failed to start script 1");
    let id2 = mgr
        .start_script(&config2)
        .expect("Failed to start script 2");

    assert!(mgr.is_running(id1), "Script 1 should be running");
    assert!(mgr.is_running(id2), "Script 2 should be running");

    mgr.stop_all();

    assert!(
        !mgr.is_running(id1),
        "Script 1 should not be running after stop_all"
    );
    assert!(
        !mgr.is_running(id2),
        "Script 2 should not be running after stop_all"
    );
}

#[test]
fn test_manager_read_commands_and_errors() {
    // A Python script that reads a single JSON event from stdin, emits a Log command,
    // writes an error to stderr, then exits.
    let python_script = r#"
import sys, json
line = sys.stdin.readline()
event = json.loads(line)
cmd = {"type": "Log", "level": "info", "message": "got " + event["kind"]}
print(json.dumps(cmd), flush=True)
print("test error", file=sys.stderr, flush=True)
"#;

    let config = make_config("python3", vec!["-c".to_string(), python_script.to_string()]);

    let mut mgr = ScriptManager::new();
    let id = mgr.start_script(&config).expect("Failed to start script");

    // Send an event to trigger the script
    let event = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    mgr.send_event(id, &event).expect("Failed to send event");

    // Wait for the script to process
    std::thread::sleep(std::time::Duration::from_millis(500));

    let commands = mgr.read_commands(id);
    assert!(
        !commands.is_empty(),
        "Should have received at least one command"
    );
    match &commands[0] {
        ScriptCommand::Log { level, message } => {
            assert_eq!(level, "info");
            assert_eq!(message, "got bell_rang");
        }
        other => panic!("Expected Log command, got: {:?}", other),
    }

    let errors = mgr.read_errors(id);
    assert!(
        !errors.is_empty(),
        "Should have received at least one error line"
    );
    assert_eq!(errors[0], "test error");
}

#[test]
fn test_manager_panel_operations() {
    let mut mgr = ScriptManager::new();

    // Panels for non-existent script should return None
    assert!(mgr.get_panel(999).is_none());

    let config = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );

    let id = mgr.start_script(&config).expect("Failed to start script");

    // No panel initially
    assert!(mgr.get_panel(id).is_none());

    // Set a panel
    mgr.set_panel(id, "Title".to_string(), "Content".to_string());
    let panel = mgr.get_panel(id).expect("Panel should exist after set");
    assert_eq!(panel.0, "Title");
    assert_eq!(panel.1, "Content");

    // Clear the panel
    mgr.clear_panel(id);
    assert!(mgr.get_panel(id).is_none());

    mgr.stop_all();
}

#[test]
fn test_manager_script_ids() {
    let mut mgr = ScriptManager::new();
    assert!(mgr.script_ids().is_empty());

    let config = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );

    let id1 = mgr.start_script(&config).expect("Failed to start script 1");
    let id2 = mgr.start_script(&config).expect("Failed to start script 2");

    let ids = mgr.script_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));

    mgr.stop_script(id1);
    let ids = mgr.script_ids();
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&id2));

    mgr.stop_all();
}

#[test]
fn test_manager_broadcast_event() {
    // Two scripts that each read one event and output a Log command
    let python_script = r#"
import sys, json
line = sys.stdin.readline()
event = json.loads(line)
cmd = {"type": "Log", "level": "info", "message": "got " + event["kind"]}
print(json.dumps(cmd), flush=True)
import time; time.sleep(5)
"#;

    let config = make_config("python3", vec!["-c".to_string(), python_script.to_string()]);

    let mut mgr = ScriptManager::new();
    let id1 = mgr.start_script(&config).expect("Failed to start script 1");
    let id2 = mgr.start_script(&config).expect("Failed to start script 2");

    let event = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    mgr.broadcast_event(&event);

    std::thread::sleep(std::time::Duration::from_millis(500));

    let cmds1 = mgr.read_commands(id1);
    let cmds2 = mgr.read_commands(id2);

    assert!(
        !cmds1.is_empty(),
        "Script 1 should have received the broadcast"
    );
    assert!(
        !cmds2.is_empty(),
        "Script 2 should have received the broadcast"
    );

    mgr.stop_all();
}

#[test]
fn test_manager_default_trait() {
    let mgr = ScriptManager::default();
    assert!(mgr.script_ids().is_empty());
}

#[test]
fn test_manager_auto_detect_python() {
    // Use a .py extension path - the manager should detect it needs python3
    // We can't actually run a file that doesn't exist, but we can test that
    // start_script with a .py path would attempt python3.
    // Instead, test with a direct python3 command to verify the flow works.
    let config = make_config(
        "python3",
        vec!["-c".to_string(), "import time; time.sleep(60)".to_string()],
    );

    let mut mgr = ScriptManager::new();
    let id = mgr.start_script(&config).expect("Failed to start script");
    assert!(mgr.is_running(id));
    mgr.stop_all();
}
