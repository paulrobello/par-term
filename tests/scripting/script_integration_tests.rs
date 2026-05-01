use par_term::config::automation::RestartPolicy;
use par_term::config::scripting::ScriptConfig;
use par_term::scripting::manager::ScriptManager;
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};

use std::collections::HashMap;

fn make_integration_config(args: Vec<String>) -> ScriptConfig {
    ScriptConfig {
        name: "integration-test".to_string(),
        enabled: true,
        script_path: "python3".to_string(),
        args,
        auto_start: false,
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

/// End-to-end integration test that verifies the full script lifecycle through ScriptManager.
///
/// A Python script is spawned that:
/// 1. Reads JSON events from stdin (one per line)
/// 2. For "bell_rang" events: sends a Log command back
/// 3. For "cwd_changed" events: sends a SetPanel command back
/// 4. Exits when stdin closes (EOF)
///
/// The test verifies:
/// - Script starts successfully
/// - Events are serialized and sent correctly
/// - Commands are deserialized and received correctly
/// - Script stops cleanly
#[test]
fn test_full_script_lifecycle() {
    let python_script = r#"
import json, sys

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    event = json.loads(line)
    kind = event.get("kind", "")
    if kind == "bell_rang":
        cmd = {"type": "Log", "level": "info", "message": "bell received"}
        print(json.dumps(cmd), flush=True)
    elif kind == "cwd_changed":
        cwd = event.get("data", {}).get("cwd", "unknown")
        heading = chr(35) + chr(35) + " "
        cmd = {"type": "SetPanel", "title": "CWD", "content": heading + cwd}
        print(json.dumps(cmd), flush=True)
"#;

    let config = make_integration_config(vec!["-c".to_string(), python_script.to_string()]);

    let mut manager = ScriptManager::new();
    let id = manager
        .start_script(&config)
        .expect("Failed to start integration test script");
    assert!(
        manager.is_running(id),
        "Script should be running after start"
    );

    // Send bell event
    let bell = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    manager
        .send_event(id, &bell)
        .expect("Failed to send bell event");

    // Send cwd_changed event
    let cwd = ScriptEvent {
        kind: "cwd_changed".to_string(),
        data: ScriptEventData::CwdChanged {
            cwd: "/tmp/test".to_string(),
        },
    };
    manager
        .send_event(id, &cwd)
        .expect("Failed to send cwd event");

    // Wait for the Python script to process both events and emit responses
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Read all commands from the script
    let commands = manager.read_commands(id);
    assert!(
        commands.len() >= 2,
        "Expected at least 2 commands, got {}",
        commands.len()
    );

    // Verify we got a Log command for the bell event
    assert!(
        commands.iter().any(
            |c| matches!(c, ScriptCommand::Log { level, message } if level == "info" && message == "bell received")
        ),
        "Expected a Log command with message 'bell received', got: {:?}",
        commands
    );

    // Verify we got a SetPanel command for the cwd event
    assert!(
        commands.iter().any(
            |c| matches!(c, ScriptCommand::SetPanel { title, content } if title == "CWD" && content == "## /tmp/test")
        ),
        "Expected a SetPanel command with title 'CWD', got: {:?}",
        commands
    );

    // Stop and verify cleanup
    manager.stop_script(id);
    assert!(
        !manager.is_running(id),
        "Script should not be running after stop"
    );
}

/// Test that a script which exits on its own is detected as not running.
#[test]
fn test_script_natural_exit() {
    let python_script = r#"
import json, sys

# Read one event, respond, then exit
line = sys.stdin.readline()
event = json.loads(line)
cmd = {"type": "Log", "level": "info", "message": "processed " + event["kind"]}
print(json.dumps(cmd), flush=True)
# Script exits naturally here
"#;

    let config = make_integration_config(vec!["-c".to_string(), python_script.to_string()]);

    let mut manager = ScriptManager::new();
    let id = manager
        .start_script(&config)
        .expect("Failed to start script");

    let event = ScriptEvent {
        kind: "test_event".to_string(),
        data: ScriptEventData::Empty {},
    };
    manager
        .send_event(id, &event)
        .expect("Failed to send event");

    // Wait for the script to process and exit
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Script should have produced a command before exiting
    let commands = manager.read_commands(id);
    assert!(
        !commands.is_empty(),
        "Expected at least 1 command, got {}",
        commands.len()
    );
    assert!(
        commands.iter().any(
            |c| matches!(c, ScriptCommand::Log { message, .. } if message == "processed test_event")
        ),
        "Expected Log command with 'processed test_event', got: {:?}",
        commands
    );

    // Script exited naturally, so it should no longer be running
    assert!(
        !manager.is_running(id),
        "Script should not be running after natural exit"
    );

    // Cleanup
    manager.stop_script(id);
}

/// Test that stderr output from scripts is captured correctly.
#[test]
fn test_script_stderr_capture() {
    let python_script = r#"
import json, sys

line = sys.stdin.readline()
event = json.loads(line)
print("warning: something happened", file=sys.stderr, flush=True)
cmd = {"type": "Log", "level": "warn", "message": "done"}
print(json.dumps(cmd), flush=True)
import time; time.sleep(5)
"#;

    let config = make_integration_config(vec!["-c".to_string(), python_script.to_string()]);

    let mut manager = ScriptManager::new();
    let id = manager
        .start_script(&config)
        .expect("Failed to start script");

    let event = ScriptEvent {
        kind: "test".to_string(),
        data: ScriptEventData::Empty {},
    };
    manager
        .send_event(id, &event)
        .expect("Failed to send event");

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check that stderr was captured
    let errors = manager.read_errors(id);
    assert!(
        !errors.is_empty(),
        "Should have captured stderr output from the script"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("warning: something happened")),
        "Expected stderr to contain warning message, got: {:?}",
        errors
    );

    // And stdout command was also captured
    let commands = manager.read_commands(id);
    assert!(
        !commands.is_empty(),
        "Should have received command from stdout"
    );

    manager.stop_all();
}

/// Test broadcasting events to multiple scripts simultaneously.
#[test]
fn test_broadcast_to_multiple_scripts() {
    let python_script = r#"
import json, sys

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    event = json.loads(line)
    cmd = {"type": "Log", "level": "info", "message": "ack " + event["kind"]}
    print(json.dumps(cmd), flush=True)
"#;

    let config = make_integration_config(vec!["-c".to_string(), python_script.to_string()]);

    let mut manager = ScriptManager::new();
    let id1 = manager
        .start_script(&config)
        .expect("Failed to start script 1");
    let id2 = manager
        .start_script(&config)
        .expect("Failed to start script 2");

    // Broadcast a single event to both scripts
    let event = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    manager.broadcast_event(&event);

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Both scripts should have responded
    let cmds1 = manager.read_commands(id1);
    let cmds2 = manager.read_commands(id2);

    assert!(
        !cmds1.is_empty(),
        "Script 1 should have responded to broadcast"
    );
    assert!(
        !cmds2.is_empty(),
        "Script 2 should have responded to broadcast"
    );

    assert!(
        cmds1
            .iter()
            .any(|c| matches!(c, ScriptCommand::Log { message, .. } if message == "ack bell_rang")),
        "Script 1 should have acked bell_rang, got: {:?}",
        cmds1
    );
    assert!(
        cmds2
            .iter()
            .any(|c| matches!(c, ScriptCommand::Log { message, .. } if message == "ack bell_rang")),
        "Script 2 should have acked bell_rang, got: {:?}",
        cmds2
    );

    manager.stop_all();
}
