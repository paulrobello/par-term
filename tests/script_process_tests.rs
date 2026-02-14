use par_term::scripting::process::ScriptProcess;
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};
use std::collections::HashMap;

#[test]
fn test_script_process_spawn_and_stop() {
    // Spawn a Python script that reads one line from stdin,
    // parses the JSON, and outputs a Log command with the event kind.
    let python_script = r#"
import sys, json
line = sys.stdin.readline()
event = json.loads(line)
cmd = {"type": "Log", "level": "info", "message": "got " + event["kind"]}
print(json.dumps(cmd), flush=True)
"#;

    let mut proc = ScriptProcess::spawn("python3", &["-c", python_script], &HashMap::new())
        .expect("Failed to spawn python3 script process");

    assert!(proc.is_running(), "Process should be running after spawn");

    // Send a bell_rang event
    let event = ScriptEvent {
        kind: "bell_rang".to_string(),
        data: ScriptEventData::Empty {},
    };
    proc.send_event(&event)
        .expect("Failed to send event to script process");

    // Wait for the script to process and output
    std::thread::sleep(std::time::Duration::from_millis(500));

    let commands = proc.read_commands();
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

    proc.stop();
    assert!(
        !proc.is_running(),
        "Process should not be running after stop"
    );
}

#[test]
fn test_script_process_captures_stderr() {
    // Spawn a Python script that writes to stderr and exits.
    let python_script = r#"
import sys
print("error line 1", file=sys.stderr, flush=True)
print("error line 2", file=sys.stderr, flush=True)
"#;

    let proc = ScriptProcess::spawn("python3", &["-c", python_script], &HashMap::new())
        .expect("Failed to spawn python3 script process");

    // Wait for the script to run and exit
    std::thread::sleep(std::time::Duration::from_millis(500));

    let errors = proc.read_errors();
    assert!(
        errors.len() >= 2,
        "Should have captured at least 2 stderr lines, got: {:?}",
        errors
    );
    assert_eq!(errors[0], "error line 1");
    assert_eq!(errors[1], "error line 2");
}

#[test]
fn test_script_process_invalid_command_fails() {
    let result = ScriptProcess::spawn(
        "nonexistent_binary_that_does_not_exist_12345",
        &[],
        &HashMap::new(),
    );
    assert!(
        result.is_err(),
        "Spawning a nonexistent binary should return an error"
    );
}
