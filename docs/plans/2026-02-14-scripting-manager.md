# Scripting Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a frontend scripting manager that runs Python scripts as subprocesses, forwarding terminal events as JSON and executing script commands back to the terminal.

**Architecture:** Scripts are child processes communicating via JSON over stdin/stdout, scoped per-tab (like coprocesses). A `ScriptEventForwarder` implements `TerminalObserver` to capture events from the core library, queues them via `crossbeam` channel, and a writer thread feeds them to script stdin. Script stdout is read for JSON commands which are processed in the main event loop.

**Tech Stack:** Rust (serde_json for protocol, crossbeam for channels, std::process for subprocesses), egui (Settings UI), par-term-emu-core-rust (TerminalObserver API)

**Design Doc:** `docs/plans/2026-02-14-scripting-manager-design.md`

---

## Task 1: ScriptConfig + Config Integration

**Files:**
- Create: `src/config/scripting.rs`
- Modify: `src/config/mod.rs` (add `pub mod scripting;` and `scripts` field to Config)
- Modify: `src/config/automation.rs` (reuse `RestartPolicy` - already public)
- Test: `tests/config_tests.rs` (add script config tests)

**Step 1: Write failing test for ScriptConfig serialization**

In `tests/config_tests.rs`, add:

```rust
#[test]
fn test_script_config_default_roundtrip() {
    let config = par_term::config::scripting::ScriptConfig {
        name: "test-script".to_string(),
        enabled: true,
        script_path: "/path/to/script.py".to_string(),
        args: vec!["--verbose".to_string()],
        auto_start: false,
        restart_policy: par_term::config::automation::RestartPolicy::Never,
        restart_delay_ms: 0,
        subscriptions: vec!["BellRang".to_string(), "CwdChanged".to_string()],
        env_vars: std::collections::HashMap::new(),
    };
    let yaml = serde_yaml::to_string(&config).unwrap();
    let deserialized: par_term::config::scripting::ScriptConfig =
        serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config, deserialized);
}

#[test]
fn test_script_config_defaults() {
    let yaml = "name: minimal\nscript_path: test.py\n";
    let config: par_term::config::scripting::ScriptConfig =
        serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.name, "minimal");
    assert!(config.enabled); // default true
    assert!(!config.auto_start); // default false
    assert!(config.subscriptions.is_empty()); // default empty = all events
    assert!(config.env_vars.is_empty());
}

#[test]
fn test_config_scripts_field_default_empty() {
    let yaml = "cols: 80\nrows: 24\n";
    let config: par_term::config::Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.scripts.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_script_config -- --nocapture`
Expected: FAIL — module `scripting` not found

**Step 3: Create `src/config/scripting.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::automation::RestartPolicy;

fn default_enabled() -> bool {
    true
}

/// Configuration for a terminal automation script
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptConfig {
    /// Display name for this script
    pub name: String,

    /// Whether this script is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Path to the script file (absolute or relative to config dir)
    pub script_path: String,

    /// Additional command-line arguments for the script
    #[serde(default)]
    pub args: Vec<String>,

    /// Start automatically when a new tab opens
    #[serde(default)]
    pub auto_start: bool,

    /// Restart policy when the script exits
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Delay in milliseconds before restarting
    #[serde(default)]
    pub restart_delay_ms: u64,

    /// Terminal event kinds to forward (empty = all events)
    #[serde(default)]
    pub subscriptions: Vec<String>,

    /// Extra environment variables passed to the script process
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}
```

**Step 4: Add module declaration and Config field**

In `src/config/mod.rs`:
- Add `pub mod scripting;` near the other module declarations
- Add field to `Config` struct (after `coprocesses` field around line 1711):
  ```rust
  /// Script definitions for terminal automation via observer API
  #[serde(default)]
  pub scripts: Vec<scripting::ScriptConfig>,
  ```
- Add to `Default` impl (after `coprocesses: Vec::new(),`):
  ```rust
  scripts: Vec::new(),
  ```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_script_config test_config_scripts_field -- --nocapture`
Expected: PASS (3 tests)

**Step 6: Commit**

```bash
git add src/config/scripting.rs tests/config_tests.rs src/config/mod.rs
git commit -m "feat(config): add ScriptConfig for terminal automation scripts"
```

---

## Task 2: JSON Protocol Types

**Files:**
- Create: `src/scripting/protocol.rs`
- Create: `src/scripting/mod.rs` (module declaration only for now)
- Modify: `src/lib.rs` or `src/main.rs` — add `pub mod scripting;`
- Test: `tests/script_protocol_tests.rs`

**Step 1: Write failing tests for protocol serialization**

Create `tests/script_protocol_tests.rs`:

```rust
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};

#[test]
fn test_event_serialize_bell() {
    let event = ScriptEvent {
        kind: "BellRang".to_string(),
        data: ScriptEventData::Empty {},
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"kind\":\"BellRang\""));
}

#[test]
fn test_event_serialize_cwd_changed() {
    let event = ScriptEvent {
        kind: "CwdChanged".to_string(),
        data: ScriptEventData::CwdChanged {
            cwd: "/home/user".to_string(),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("/home/user"));
}

#[test]
fn test_event_serialize_command_complete() {
    let event = ScriptEvent {
        kind: "CommandComplete".to_string(),
        data: ScriptEventData::CommandComplete {
            command: "make test".to_string(),
            exit_code: Some(0),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["data"]["command"], "make test");
    assert_eq!(parsed["data"]["exit_code"], 0);
}

#[test]
fn test_command_deserialize_write_text() {
    let json = r#"{"type": "write_text", "text": "echo hello\n"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::WriteText { text } if text == "echo hello\n"));
}

#[test]
fn test_command_deserialize_notify() {
    let json = r#"{"type": "notify", "title": "Done", "body": "Build passed"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::Notify { title, body }
        if title == "Done" && body == "Build passed"));
}

#[test]
fn test_command_deserialize_set_badge() {
    let json = r#"{"type": "set_badge", "text": "OK"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::SetBadge { text } if text == "OK"));
}

#[test]
fn test_command_deserialize_set_panel() {
    let json = r#"{"type": "set_panel", "title": "Status", "content": "## OK"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::SetPanel { title, content }
        if title == "Status" && content == "## OK"));
}

#[test]
fn test_command_deserialize_log() {
    let json = r#"{"type": "log", "level": "info", "message": "Started"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::Log { level, message }
        if level == "info" && message == "Started"));
}

#[test]
fn test_command_deserialize_set_variable() {
    let json = r#"{"type": "set_variable", "name": "foo", "value": "bar"}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::SetVariable { name, value }
        if name == "foo" && value == "bar"));
}

#[test]
fn test_command_deserialize_change_config() {
    let json = r#"{"type": "change_config", "key": "font_size", "value": 14.0}"#;
    let cmd: ScriptCommand = serde_json::from_str(json).unwrap();
    assert!(matches!(cmd, ScriptCommand::ChangeConfig { key, value }
        if key == "font_size"));
}

#[test]
fn test_command_deserialize_unknown_type_errors() {
    let json = r#"{"type": "explode_computer"}"#;
    let result = serde_json::from_str::<ScriptCommand>(json);
    assert!(result.is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test script_protocol_tests`
Expected: FAIL — module not found

**Step 3: Create `src/scripting/mod.rs`**

```rust
pub mod protocol;
```

**Step 4: Create `src/scripting/protocol.rs`**

```rust
use serde::{Deserialize, Serialize};

/// Event sent from terminal to script (JSON over stdin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEvent {
    /// Event kind name (matches TerminalEventKind variant names)
    pub kind: String,
    /// Event-specific data
    #[serde(flatten)]
    pub data: ScriptEventData,
}

/// Event data variants — each maps to a TerminalEvent from the core library
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "data_type")]
pub enum ScriptEventData {
    /// No additional data (BellRang, etc.)
    #[serde(rename = "empty")]
    Empty {},

    /// CwdChanged event
    #[serde(rename = "cwd_changed")]
    CwdChanged { cwd: String },

    /// Command completed with optional exit code
    #[serde(rename = "command_complete")]
    CommandComplete {
        command: String,
        exit_code: Option<i32>,
    },

    /// Title changed
    #[serde(rename = "title_changed")]
    TitleChanged { title: String },

    /// Terminal resized
    #[serde(rename = "size_changed")]
    SizeChanged { cols: usize, rows: usize },

    /// User variable changed
    #[serde(rename = "variable_changed")]
    VariableChanged {
        name: String,
        value: String,
        old_value: Option<String>,
    },

    /// Environment variable changed
    #[serde(rename = "environment_changed")]
    EnvironmentChanged {
        key: String,
        value: String,
        old_value: Option<String>,
    },

    /// Badge text changed
    #[serde(rename = "badge_changed")]
    BadgeChanged { text: Option<String> },

    /// Trigger matched
    #[serde(rename = "trigger_matched")]
    TriggerMatched {
        pattern: String,
        matched_text: String,
        line: usize,
    },

    /// Zone lifecycle event
    #[serde(rename = "zone_event")]
    ZoneEvent {
        zone_id: u64,
        zone_type: String,
        event: String, // "opened", "closed", "scrolled_out"
    },

    /// Generic key-value data for events not yet specifically modeled
    #[serde(rename = "generic")]
    Generic {
        #[serde(flatten)]
        fields: std::collections::HashMap<String, serde_json::Value>,
    },
}

/// Command sent from script to terminal (JSON over stdout)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ScriptCommand {
    /// Write text to the terminal PTY
    #[serde(rename = "write_text")]
    WriteText { text: String },

    /// Show a desktop notification
    #[serde(rename = "notify")]
    Notify { title: String, body: String },

    /// Set the tab badge text
    #[serde(rename = "set_badge")]
    SetBadge { text: String },

    /// Set a user variable (can be used in badge templates)
    #[serde(rename = "set_variable")]
    SetVariable { name: String, value: String },

    /// Run a shell command (executes in a subshell, output not captured)
    #[serde(rename = "run_command")]
    RunCommand { command: String },

    /// Change a config setting
    #[serde(rename = "change_config")]
    ChangeConfig {
        key: String,
        value: serde_json::Value,
    },

    /// Log a message (displayed in script output panel)
    #[serde(rename = "log")]
    Log { level: String, message: String },

    /// Set or update a markdown panel
    #[serde(rename = "set_panel")]
    SetPanel { title: String, content: String },

    /// Clear the script's markdown panel
    #[serde(rename = "clear_panel")]
    ClearPanel {},
}
```

**Step 5: Add `pub mod scripting;` to main module**

Find the file that declares public modules (likely `src/main.rs` or `src/lib.rs`) and add `pub mod scripting;`. Check existing module pattern — if there's a `src/lib.rs`, add it there. Otherwise add to wherever other `pub mod` declarations live.

**Step 6: Run tests to verify they pass**

Run: `cargo test --test script_protocol_tests`
Expected: PASS (11 tests)

**Step 7: Commit**

```bash
git add src/scripting/ tests/script_protocol_tests.rs src/lib.rs
git commit -m "feat(scripting): add JSON protocol types for script communication"
```

---

## Task 3: ScriptProcess — Single Script Subprocess Management

**Files:**
- Create: `src/scripting/process.rs`
- Modify: `src/scripting/mod.rs` (add `pub mod process;`)
- Test: `tests/script_process_tests.rs`

**Step 1: Write failing test for ScriptProcess lifecycle**

Create `tests/script_process_tests.rs`:

```rust
use par_term::scripting::process::ScriptProcess;
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};

#[test]
fn test_script_process_spawn_and_stop() {
    // Use a simple Python script that reads stdin and exits
    let mut proc = ScriptProcess::spawn(
        "python3",
        &["-c", "import sys; sys.stdin.readline(); print('{\"type\": \"log\", \"level\": \"info\", \"message\": \"hello\"}')"],
        &Default::default(),
    )
    .unwrap();

    assert!(proc.is_running());

    // Send an event to unblock the readline
    let event = ScriptEvent {
        kind: "BellRang".to_string(),
        data: ScriptEventData::Empty {},
    };
    proc.send_event(&event).unwrap();

    // Give it a moment to process
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Read any commands
    let commands = proc.read_commands();
    assert!(!commands.is_empty());
    assert!(matches!(&commands[0], ScriptCommand::Log { level, message }
        if level == "info" && message == "hello"));

    proc.stop();
    assert!(!proc.is_running());
}

#[test]
fn test_script_process_captures_stderr() {
    let mut proc = ScriptProcess::spawn(
        "python3",
        &["-c", "import sys; print('error msg', file=sys.stderr); sys.exit(1)"],
        &Default::default(),
    )
    .unwrap();

    // Wait for process to exit
    std::thread::sleep(std::time::Duration::from_millis(500));

    let errors = proc.read_errors();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("error msg")));
    assert!(!proc.is_running());
}

#[test]
fn test_script_process_invalid_command_fails() {
    let result = ScriptProcess::spawn(
        "nonexistent_binary_xyz_12345",
        &[],
        &Default::default(),
    );
    assert!(result.is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test script_process_tests`
Expected: FAIL — module not found

**Step 3: Implement `src/scripting/process.rs`**

```rust
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use super::protocol::{ScriptCommand, ScriptEvent};

/// Manages a single script subprocess
pub struct ScriptProcess {
    child: Option<Child>,
    stdin_writer: Option<std::process::ChildStdin>,
    /// Commands read from script stdout (thread-safe buffer)
    command_buffer: Arc<Mutex<Vec<ScriptCommand>>>,
    /// Errors read from script stderr (thread-safe buffer)
    error_buffer: Arc<Mutex<Vec<String>>>,
    /// Reader thread handles
    _stdout_thread: Option<std::thread::JoinHandle<()>>,
    _stderr_thread: Option<std::thread::JoinHandle<()>>,
}

impl ScriptProcess {
    /// Spawn a script subprocess
    pub fn spawn(
        command: &str,
        args: &[&str],
        env_vars: &HashMap<String, String>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn script: {e}"))?;

        let stdin_writer = child.stdin.take();

        // Spawn stdout reader thread
        let command_buffer: Arc<Mutex<Vec<ScriptCommand>>> = Arc::new(Mutex::new(Vec::new()));
        let cmd_buf = Arc::clone(&command_buffer);
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture stdout")?;
        let stdout_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) if line.trim().is_empty() => continue,
                    Ok(line) => match serde_json::from_str::<ScriptCommand>(&line) {
                        Ok(cmd) => {
                            if let Ok(mut buf) = cmd_buf.lock() {
                                buf.push(cmd);
                            }
                        }
                        Err(e) => {
                            log::warn!("Script sent invalid JSON: {e}: {line}");
                        }
                    },
                    Err(_) => break,
                }
            }
        });

        // Spawn stderr reader thread
        let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let err_buf = Arc::clone(&error_buffer);
        let stderr = child
            .stderr
            .take()
            .ok_or("Failed to capture stderr")?;
        let stderr_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        if let Ok(mut buf) = err_buf.lock() {
                            buf.push(line);
                        }
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            child: Some(child),
            stdin_writer,
            command_buffer,
            error_buffer,
            _stdout_thread: Some(stdout_thread),
            _stderr_thread: Some(stderr_thread),
        })
    }

    /// Check if the script process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// Send a terminal event to the script (JSON over stdin)
    pub fn send_event(&mut self, event: &ScriptEvent) -> Result<(), String> {
        if let Some(stdin) = &mut self.stdin_writer {
            let json =
                serde_json::to_string(event).map_err(|e| format!("Failed to serialize event: {e}"))?;
            writeln!(stdin, "{json}").map_err(|e| format!("Failed to write to script stdin: {e}"))?;
            stdin
                .flush()
                .map_err(|e| format!("Failed to flush script stdin: {e}"))?;
            Ok(())
        } else {
            Err("Script stdin not available".to_string())
        }
    }

    /// Drain pending commands from the script
    pub fn read_commands(&self) -> Vec<ScriptCommand> {
        if let Ok(mut buf) = self.command_buffer.lock() {
            std::mem::take(&mut *buf)
        } else {
            Vec::new()
        }
    }

    /// Drain pending error lines from the script
    pub fn read_errors(&self) -> Vec<String> {
        if let Ok(mut buf) = self.error_buffer.lock() {
            std::mem::take(&mut *buf)
        } else {
            Vec::new()
        }
    }

    /// Stop the script process
    pub fn stop(&mut self) {
        // Drop stdin to signal EOF to the script
        self.stdin_writer.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ScriptProcess {
    fn drop(&mut self) {
        self.stop();
    }
}
```

**Step 4: Add module to `src/scripting/mod.rs`**

```rust
pub mod process;
pub mod protocol;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --test script_process_tests -- --nocapture`
Expected: PASS (3 tests)

**Step 6: Commit**

```bash
git add src/scripting/process.rs src/scripting/mod.rs tests/script_process_tests.rs
git commit -m "feat(scripting): add ScriptProcess for subprocess lifecycle management"
```

---

## Task 4: Observer Bridge — Event Forwarding from Core

**Files:**
- Create: `src/scripting/observer.rs`
- Modify: `src/scripting/mod.rs` (add `pub mod observer;`)
- Modify: `src/terminal/mod.rs` (add `add_observer`/`remove_observer` wrappers on TerminalManager)
- Test: `tests/script_observer_tests.rs`

**Step 1: Write failing test for observer event capture**

Create `tests/script_observer_tests.rs`:

```rust
use par_term::scripting::observer::ScriptEventForwarder;
use std::collections::HashSet;

#[test]
fn test_event_forwarder_captures_events() {
    let forwarder = ScriptEventForwarder::new(None);
    // Simulate an event being dispatched (call the trait method directly)
    use par_term_emu_core_rust::observer::TerminalObserver;
    use par_term_emu_core_rust::terminal::TerminalEvent;

    forwarder.on_event(&TerminalEvent::BellRang(Default::default()));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "BellRang");
}

#[test]
fn test_event_forwarder_filters_by_subscription() {
    let mut subs = HashSet::new();
    subs.insert("BellRang".to_string());

    let forwarder = ScriptEventForwarder::new(Some(subs));
    use par_term_emu_core_rust::observer::TerminalObserver;
    use par_term_emu_core_rust::terminal::TerminalEvent;

    // This should be captured (matches subscription)
    forwarder.on_event(&TerminalEvent::BellRang(Default::default()));

    // This should be filtered out (not in subscriptions)
    forwarder.on_event(&TerminalEvent::TitleChanged("test".to_string()));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "BellRang");
}

#[test]
fn test_event_forwarder_no_filter_captures_all() {
    let forwarder = ScriptEventForwarder::new(None);
    use par_term_emu_core_rust::observer::TerminalObserver;
    use par_term_emu_core_rust::terminal::TerminalEvent;

    forwarder.on_event(&TerminalEvent::BellRang(Default::default()));
    forwarder.on_event(&TerminalEvent::TitleChanged("test".to_string()));

    let events = forwarder.drain_events();
    assert_eq!(events.len(), 2);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test script_observer_tests`
Expected: FAIL — module not found

**Step 3: Implement `src/scripting/observer.rs`**

This implements `TerminalObserver` from the core library, converting `TerminalEvent` values into our `ScriptEvent` JSON-friendly structs and queuing them.

```rust
use std::collections::HashSet;
use std::sync::Mutex;

use par_term_emu_core_rust::observer::TerminalObserver;
use par_term_emu_core_rust::terminal::TerminalEvent;

use super::protocol::{ScriptEvent, ScriptEventData};

/// Bridges core library's TerminalObserver to ScriptEvent queue.
/// Registered with PtySession to receive events, converts and buffers them.
pub struct ScriptEventForwarder {
    /// Subscription filter: if Some, only forward events whose kind name is in the set.
    /// If None, forward all events.
    subscription_filter: Option<HashSet<String>>,
    /// Thread-safe event buffer
    event_buffer: Mutex<Vec<ScriptEvent>>,
}

impl ScriptEventForwarder {
    pub fn new(subscriptions: Option<HashSet<String>>) -> Self {
        Self {
            subscription_filter: subscriptions,
            event_buffer: Mutex::new(Vec::new()),
        }
    }

    /// Drain all buffered events
    pub fn drain_events(&self) -> Vec<ScriptEvent> {
        if let Ok(mut buf) = self.event_buffer.lock() {
            std::mem::take(&mut *buf)
        } else {
            Vec::new()
        }
    }

    /// Convert a TerminalEvent to a ScriptEvent
    fn convert_event(event: &TerminalEvent) -> ScriptEvent {
        match event {
            TerminalEvent::BellRang(_) => ScriptEvent {
                kind: "BellRang".to_string(),
                data: ScriptEventData::Empty {},
            },
            TerminalEvent::TitleChanged(title) => ScriptEvent {
                kind: "TitleChanged".to_string(),
                data: ScriptEventData::TitleChanged {
                    title: title.clone(),
                },
            },
            TerminalEvent::SizeChanged(cols, rows) => ScriptEvent {
                kind: "SizeChanged".to_string(),
                data: ScriptEventData::SizeChanged {
                    cols: *cols,
                    rows: *rows,
                },
            },
            TerminalEvent::CwdChanged(cwd_change) => ScriptEvent {
                kind: "CwdChanged".to_string(),
                data: ScriptEventData::CwdChanged {
                    cwd: cwd_change.new_cwd.clone(),
                },
            },
            TerminalEvent::UserVarChanged {
                name,
                value,
                old_value,
            } => ScriptEvent {
                kind: "UserVarChanged".to_string(),
                data: ScriptEventData::VariableChanged {
                    name: name.clone(),
                    value: value.clone(),
                    old_value: old_value.clone(),
                },
            },
            TerminalEvent::EnvironmentChanged {
                key,
                value,
                old_value,
            } => ScriptEvent {
                kind: "EnvironmentChanged".to_string(),
                data: ScriptEventData::EnvironmentChanged {
                    key: key.clone(),
                    value: value.clone(),
                    old_value: old_value.clone(),
                },
            },
            TerminalEvent::BadgeChanged(text) => ScriptEvent {
                kind: "BadgeChanged".to_string(),
                data: ScriptEventData::BadgeChanged {
                    text: text.clone(),
                },
            },
            TerminalEvent::ShellIntegrationEvent {
                command, exit_code, ..
            } => ScriptEvent {
                kind: "CommandComplete".to_string(),
                data: ScriptEventData::CommandComplete {
                    command: command.clone().unwrap_or_default(),
                    exit_code: *exit_code,
                },
            },
            TerminalEvent::ZoneOpened {
                zone_id,
                zone_type,
                ..
            } => ScriptEvent {
                kind: "ZoneOpened".to_string(),
                data: ScriptEventData::ZoneEvent {
                    zone_id: *zone_id,
                    zone_type: format!("{zone_type:?}"),
                    event: "opened".to_string(),
                },
            },
            TerminalEvent::ZoneClosed {
                zone_id,
                zone_type,
                ..
            } => ScriptEvent {
                kind: "ZoneClosed".to_string(),
                data: ScriptEventData::ZoneEvent {
                    zone_id: *zone_id,
                    zone_type: format!("{zone_type:?}"),
                    event: "closed".to_string(),
                },
            },
            TerminalEvent::ZoneScrolledOut {
                zone_id,
                zone_type,
                ..
            } => ScriptEvent {
                kind: "ZoneScrolledOut".to_string(),
                data: ScriptEventData::ZoneEvent {
                    zone_id: *zone_id,
                    zone_type: format!("{zone_type:?}"),
                    event: "scrolled_out".to_string(),
                },
            },
            // For any event not explicitly mapped, use generic
            other => ScriptEvent {
                kind: format!("{other:?}").split('(').next().unwrap_or("Unknown").to_string(),
                data: ScriptEventData::Generic {
                    fields: std::collections::HashMap::new(),
                },
            },
        }
    }

    /// Get the kind name string for filtering
    fn event_kind_name(event: &TerminalEvent) -> String {
        match event {
            TerminalEvent::BellRang(_) => "BellRang".to_string(),
            TerminalEvent::TitleChanged(_) => "TitleChanged".to_string(),
            TerminalEvent::SizeChanged(_, _) => "SizeChanged".to_string(),
            TerminalEvent::CwdChanged(_) => "CwdChanged".to_string(),
            TerminalEvent::UserVarChanged { .. } => "UserVarChanged".to_string(),
            TerminalEvent::EnvironmentChanged { .. } => "EnvironmentChanged".to_string(),
            TerminalEvent::BadgeChanged(_) => "BadgeChanged".to_string(),
            TerminalEvent::ShellIntegrationEvent { .. } => "CommandComplete".to_string(),
            TerminalEvent::ZoneOpened { .. } => "ZoneOpened".to_string(),
            TerminalEvent::ZoneClosed { .. } => "ZoneClosed".to_string(),
            TerminalEvent::ZoneScrolledOut { .. } => "ZoneScrolledOut".to_string(),
            other => format!("{other:?}").split('(').next().unwrap_or("Unknown").to_string(),
        }
    }
}

impl TerminalObserver for ScriptEventForwarder {
    fn on_event(&self, event: &TerminalEvent) {
        // Check subscription filter
        if let Some(filter) = &self.subscription_filter {
            let kind = Self::event_kind_name(event);
            if !filter.contains(&kind) {
                return;
            }
        }

        let script_event = Self::convert_event(event);
        if let Ok(mut buf) = self.event_buffer.lock() {
            buf.push(script_event);
        }
    }
}
```

**Step 4: Add `add_observer`/`remove_observer` wrapper methods to `TerminalManager`**

In `src/terminal/mod.rs`, add these methods to the `impl TerminalManager` block:

```rust
/// Register a terminal observer for push-based event delivery.
/// Returns an observer ID that can be used to remove it later.
pub fn add_observer(
    &self,
    observer: std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
) -> par_term_emu_core_rust::observer::ObserverId {
    let mut session = self.pty_session.lock();
    session.terminal_mut().add_observer(observer)
}

/// Remove a previously registered observer.
/// Returns true if the observer was found and removed.
pub fn remove_observer(
    &self,
    id: par_term_emu_core_rust::observer::ObserverId,
) -> bool {
    let mut session = self.pty_session.lock();
    session.terminal_mut().remove_observer(id)
}
```

Note: Check that `PtySession` has a `terminal_mut()` method. If not, the accessor may be named differently (e.g., `terminal()` returning `&mut Terminal`). Adjust based on what the core library exposes.

**Step 5: Update `src/scripting/mod.rs`**

```rust
pub mod observer;
pub mod process;
pub mod protocol;
```

**Step 6: Run tests to verify they pass**

Run: `cargo test --test script_observer_tests -- --nocapture`
Expected: PASS (3 tests)

Note: These tests may need adjustment based on exact `TerminalEvent` constructor signatures in the core library. Check `TerminalEvent::BellRang` — it may require a `BellEvent` struct rather than `Default::default()`.

**Step 7: Commit**

```bash
git add src/scripting/observer.rs src/terminal/mod.rs tests/script_observer_tests.rs
git commit -m "feat(scripting): add observer bridge for event forwarding to scripts"
```

---

## Task 5: ScriptManager — Per-Tab Multi-Script Orchestrator

**Files:**
- Create: `src/scripting/manager.rs`
- Modify: `src/scripting/mod.rs` (add `pub mod manager;`)
- Test: `tests/script_manager_tests.rs`

**Step 1: Write failing test for ScriptManager**

Create `tests/script_manager_tests.rs`:

```rust
use par_term::config::scripting::ScriptConfig;
use par_term::config::automation::RestartPolicy;
use par_term::scripting::manager::ScriptManager;

fn test_config() -> ScriptConfig {
    ScriptConfig {
        name: "test".to_string(),
        enabled: true,
        script_path: "python3".to_string(),
        args: vec![
            "-c".to_string(),
            "import sys, json; line = sys.stdin.readline(); print(json.dumps({\"type\": \"log\", \"level\": \"info\", \"message\": \"started\"})); sys.stdout.flush()".to_string(),
        ],
        auto_start: false,
        restart_policy: RestartPolicy::Never,
        restart_delay_ms: 0,
        subscriptions: vec![],
        env_vars: Default::default(),
    }
}

#[test]
fn test_manager_start_stop_script() {
    let mut manager = ScriptManager::new();
    let config = test_config();

    let id = manager.start_script(&config).unwrap();
    assert!(manager.is_running(id));

    manager.stop_script(id);
    assert!(!manager.is_running(id));
}

#[test]
fn test_manager_stop_all() {
    let mut manager = ScriptManager::new();
    let config = test_config();

    let id1 = manager.start_script(&config).unwrap();
    let id2 = manager.start_script(&config).unwrap();

    assert!(manager.is_running(id1));
    assert!(manager.is_running(id2));

    manager.stop_all();

    assert!(!manager.is_running(id1));
    assert!(!manager.is_running(id2));
}

#[test]
fn test_manager_read_commands_and_errors() {
    let mut manager = ScriptManager::new();
    let config = test_config();

    let id = manager.start_script(&config).unwrap();

    // Send an event to trigger script output
    use par_term::scripting::protocol::{ScriptEvent, ScriptEventData};
    let event = ScriptEvent {
        kind: "BellRang".to_string(),
        data: ScriptEventData::Empty {},
    };
    let _ = manager.send_event(id, &event);
    std::thread::sleep(std::time::Duration::from_millis(300));

    let commands = manager.read_commands(id);
    // Script should have sent a log command
    assert!(!commands.is_empty());

    let errors = manager.read_errors(id);
    // No errors expected from this script
    // (errors vec may or may not be empty depending on Python startup)

    manager.stop_script(id);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test script_manager_tests`
Expected: FAIL — module not found

**Step 3: Implement `src/scripting/manager.rs`**

```rust
use std::collections::HashMap;

use super::process::ScriptProcess;
use super::protocol::{ScriptCommand, ScriptEvent};
use crate::config::scripting::ScriptConfig;

/// Unique identifier for a running script instance
pub type ScriptId = u64;

/// Manages multiple script subprocesses for a single tab
pub struct ScriptManager {
    next_id: ScriptId,
    processes: HashMap<ScriptId, ScriptProcess>,
    /// Markdown panel content set by scripts: script_id -> (title, content)
    panels: HashMap<ScriptId, (String, String)>,
}

impl ScriptManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            processes: HashMap::new(),
            panels: HashMap::new(),
        }
    }

    /// Start a script from config. Returns a ScriptId on success.
    pub fn start_script(&mut self, config: &ScriptConfig) -> Result<ScriptId, String> {
        let env_vars = config.env_vars.clone();

        // Build args: the script_path is the first arg to python3
        // If script_path looks like it already is a command (not .py), use it directly
        let (command, args): (String, Vec<String>) = if config.script_path.ends_with(".py") {
            (
                "python3".to_string(),
                std::iter::once(config.script_path.clone())
                    .chain(config.args.iter().cloned())
                    .collect(),
            )
        } else {
            // Treat script_path as the command itself (for testing or non-Python scripts)
            (config.script_path.clone(), config.args.clone())
        };

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let process = ScriptProcess::spawn(&command, &args_refs, &env_vars)?;

        let id = self.next_id;
        self.next_id += 1;
        self.processes.insert(id, process);
        Ok(id)
    }

    /// Check if a script is running
    pub fn is_running(&mut self, id: ScriptId) -> bool {
        self.processes
            .get_mut(&id)
            .is_some_and(|p| p.is_running())
    }

    /// Send a terminal event to a specific script
    pub fn send_event(&mut self, id: ScriptId, event: &ScriptEvent) -> Result<(), String> {
        if let Some(process) = self.processes.get_mut(&id) {
            process.send_event(event)
        } else {
            Err(format!("Script {id} not found"))
        }
    }

    /// Send a terminal event to all running scripts
    pub fn broadcast_event(&mut self, event: &ScriptEvent) {
        let ids: Vec<ScriptId> = self.processes.keys().copied().collect();
        for id in ids {
            if let Some(process) = self.processes.get_mut(&id) {
                if process.is_running() {
                    let _ = process.send_event(event);
                }
            }
        }
    }

    /// Read pending commands from a script
    pub fn read_commands(&self, id: ScriptId) -> Vec<ScriptCommand> {
        self.processes
            .get(&id)
            .map(|p| p.read_commands())
            .unwrap_or_default()
    }

    /// Read pending errors from a script
    pub fn read_errors(&self, id: ScriptId) -> Vec<String> {
        self.processes
            .get(&id)
            .map(|p| p.read_errors())
            .unwrap_or_default()
    }

    /// Stop a specific script
    pub fn stop_script(&mut self, id: ScriptId) {
        if let Some(mut process) = self.processes.remove(&id) {
            process.stop();
        }
        self.panels.remove(&id);
    }

    /// Stop all running scripts
    pub fn stop_all(&mut self) {
        let ids: Vec<ScriptId> = self.processes.keys().copied().collect();
        for id in ids {
            self.stop_script(id);
        }
    }

    /// Get panel content for a script
    pub fn get_panel(&self, id: ScriptId) -> Option<&(String, String)> {
        self.panels.get(&id)
    }

    /// Set panel content for a script (called when processing SetPanel command)
    pub fn set_panel(&mut self, id: ScriptId, title: String, content: String) {
        self.panels.insert(id, (title, content));
    }

    /// Clear panel for a script
    pub fn clear_panel(&mut self, id: ScriptId) {
        self.panels.remove(&id);
    }

    /// List all script IDs
    pub fn script_ids(&self) -> Vec<ScriptId> {
        self.processes.keys().copied().collect()
    }
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScriptManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}
```

**Step 4: Update `src/scripting/mod.rs`**

```rust
pub mod manager;
pub mod observer;
pub mod process;
pub mod protocol;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --test script_manager_tests -- --nocapture`
Expected: PASS (3 tests)

**Step 6: Commit**

```bash
git add src/scripting/manager.rs tests/script_manager_tests.rs
git commit -m "feat(scripting): add ScriptManager for per-tab multi-script orchestration"
```

---

## Task 6: Tab Integration — Script Lifecycle on Tab

**Files:**
- Modify: `src/tab/mod.rs` (add `script_manager` field, auto-start logic, cleanup on drop)
- Modify: `src/scripting/manager.rs` (no changes expected — already self-contained)

**Step 1: Add ScriptManager to Tab struct**

In `src/tab/mod.rs`, add to the `Tab` struct fields (near `coprocess_ids`):

```rust
/// Script manager for this tab (manages script subprocesses)
pub script_manager: crate::scripting::manager::ScriptManager,
/// Maps config index to ScriptId for running scripts
pub script_ids: Vec<Option<crate::scripting::manager::ScriptId>>,
/// Observer IDs registered with the terminal for script event forwarding
pub script_observer_ids: Vec<Option<par_term_emu_core_rust::observer::ObserverId>>,
/// Event forwarders (shared with observer registration)
pub script_forwarders: Vec<Option<std::sync::Arc<crate::scripting::observer::ScriptEventForwarder>>>,
```

**Step 2: Initialize in Tab constructor**

In the Tab creation logic (where `coprocess_ids` is initialized), add:

```rust
let mut script_manager = crate::scripting::manager::ScriptManager::new();
let mut script_ids = Vec::with_capacity(config.scripts.len());
let mut script_observer_ids = Vec::with_capacity(config.scripts.len());
let mut script_forwarders = Vec::with_capacity(config.scripts.len());

for script_config in config.scripts.iter() {
    if script_config.enabled && script_config.auto_start {
        // Create subscription filter
        let filter = if script_config.subscriptions.is_empty() {
            None
        } else {
            Some(script_config.subscriptions.iter().cloned().collect())
        };

        // Register observer for event forwarding
        let forwarder = std::sync::Arc::new(
            crate::scripting::observer::ScriptEventForwarder::new(filter),
        );
        let observer_id = pty_session.terminal_mut().add_observer(
            std::sync::Arc::clone(&forwarder) as std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
        );

        // Start the script process
        match script_manager.start_script(script_config) {
            Ok(id) => {
                log::info!("Auto-started script '{}' (id={})", script_config.name, id);
                script_ids.push(Some(id));
                script_observer_ids.push(Some(observer_id));
                script_forwarders.push(Some(forwarder));
            }
            Err(e) => {
                log::warn!("Failed to auto-start script '{}': {}", script_config.name, e);
                // Remove the observer we just registered since script failed
                pty_session.terminal_mut().remove_observer(observer_id);
                script_ids.push(None);
                script_observer_ids.push(None);
                script_forwarders.push(None);
            }
        }
    } else {
        script_ids.push(None);
        script_observer_ids.push(None);
        script_forwarders.push(None);
    }
}
```

**Step 3: Clean up scripts in Tab drop or close logic**

Find where Tab cleanup happens (e.g., `Drop` impl or explicit close method) and add:

```rust
self.script_manager.stop_all();
```

**Step 4: Build to verify compilation**

Run: `cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add src/tab/mod.rs
git commit -m "feat(tab): integrate ScriptManager with tab lifecycle and auto-start"
```

---

## Task 7: Settings UI — Script State Fields

**Files:**
- Modify: `src/settings_ui/mod.rs` (add script UI state fields + initialization)

**Step 1: Add script state fields to SettingsUI struct**

In `src/settings_ui/mod.rs`, add after the coprocess fields (around line 258):

```rust
// === Script management state ===
pub(crate) editing_script_index: Option<usize>,
pub(crate) temp_script_name: String,
pub(crate) temp_script_path: String,
pub(crate) temp_script_args: String,
pub(crate) temp_script_auto_start: bool,
pub(crate) temp_script_enabled: bool,
pub(crate) temp_script_restart_policy: crate::config::automation::RestartPolicy,
pub(crate) temp_script_restart_delay_ms: u64,
pub(crate) temp_script_subscriptions: String,
pub(crate) adding_new_script: bool,
pub(crate) pending_script_actions: Vec<(usize, bool)>,
pub script_running: Vec<bool>,
pub script_errors: Vec<String>,
pub script_output: Vec<Vec<String>>,
pub(crate) script_output_expanded: Vec<bool>,
pub script_panels: Vec<Option<(String, String)>>,
```

**Step 2: Initialize in SettingsUI constructor**

Add to the initialization block (after coprocess inits):

```rust
editing_script_index: None,
temp_script_name: String::new(),
temp_script_path: String::new(),
temp_script_args: String::new(),
temp_script_auto_start: false,
temp_script_enabled: true,
temp_script_restart_policy: crate::config::automation::RestartPolicy::Never,
temp_script_restart_delay_ms: 0,
temp_script_subscriptions: String::new(),
adding_new_script: false,
pending_script_actions: Vec::new(),
script_running: Vec::new(),
script_errors: Vec::new(),
script_output: Vec::new(),
script_output_expanded: Vec::new(),
script_panels: Vec::new(),
```

**Step 3: Build to verify compilation**

Run: `cargo build`
Expected: Compiles (there will be dead_code warnings — that's fine)

**Step 4: Commit**

```bash
git add src/settings_ui/mod.rs
git commit -m "feat(settings): add script management UI state fields"
```

---

## Task 8: Settings UI — Sidebar Tab Addition

**Files:**
- Modify: `src/settings_ui/sidebar.rs` (add `Scripts` variant to enum, display name, icon, keywords)

**Step 1: Add Scripts to SettingsTab enum**

In `src/settings_ui/sidebar.rs`:

1. Add `Scripts,` variant after `Automation,` in the `SettingsTab` enum
2. Add `Self::Scripts => "Scripts",` to `display_name()` match
3. Add `Self::Scripts => "📜",` to `icon()` match (or use a suitable icon from the existing icon set)
4. Add search keywords in `tab_search_keywords()`:
   ```rust
   SettingsTab::Scripts => &[
       "script", "scripting", "python", "automation", "observer",
       "event", "terminal event", "subprocess",
   ],
   ```
5. Add `Self::Scripts` to the `ALL` array or wherever tabs are listed for rendering

**Step 2: Build to verify compilation**

Run: `cargo build`
Expected: Compiles (no errors, may have warnings about unmatched patterns)

**Step 3: Commit**

```bash
git add src/settings_ui/sidebar.rs
git commit -m "feat(settings): add Scripts tab to settings sidebar"
```

---

## Task 9: Settings UI — Scripts Tab Implementation

**Files:**
- Create: `src/settings_ui/scripts_tab.rs`
- Modify: `src/settings_ui/mod.rs` (add `pub(crate) mod scripts_tab;` and render dispatch)

Follow the exact pattern from `automation_tab.rs` for coprocesses.

**Step 1: Create `src/settings_ui/scripts_tab.rs`**

This is a large file. The structure follows `automation_tab.rs`:

```rust
use std::collections::HashSet;
use egui;

use super::helpers::{collapsing_section, section_matches};
use super::SettingsUI;
use crate::config::automation::RestartPolicy;
use crate::config::scripting::ScriptConfig;

pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    if section_matches(
        &query,
        "Scripts",
        &[
            "script", "python", "automation", "observer", "event",
            "subprocess", "terminal event",
        ],
    ) {
        show_scripts_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_scripts_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Scripts",
        "scripts_section",
        true,
        collapsed,
        |ui| {
            ui.label("Python scripts that react to terminal events and control the terminal via JSON protocol.");
            ui.add_space(4.0);

            let script_count = settings.config.scripts.len();

            // Collect mutations to apply after iteration
            let mut remove_index: Option<usize> = None;
            let mut toggle_index: Option<usize> = None;

            for i in 0..script_count {
                let is_editing = settings.editing_script_index == Some(i);
                let script = &settings.config.scripts[i];
                let name = script.name.clone();
                let enabled = script.enabled;
                let path = script.script_path.clone();

                let is_running = settings
                    .script_running
                    .get(i)
                    .copied()
                    .unwrap_or(false);
                let has_error = settings
                    .script_errors
                    .get(i)
                    .is_some_and(|e| !e.is_empty());

                ui.horizontal(|ui| {
                    // Status indicator
                    if is_running {
                        ui.colored_label(egui::Color32::from_rgb(76, 175, 80), "●");
                    } else if has_error {
                        ui.colored_label(egui::Color32::from_rgb(244, 67, 54), "●");
                    } else {
                        ui.colored_label(egui::Color32::GRAY, "○");
                    }

                    // Enabled checkbox
                    let mut en = enabled;
                    if ui.checkbox(&mut en, "").changed() {
                        toggle_index = Some(i);
                    }

                    // Name and path
                    ui.label(egui::RichText::new(&name).strong());
                    ui.label(
                        egui::RichText::new(&path)
                            .small()
                            .color(egui::Color32::GRAY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Delete button
                        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                            remove_index = Some(i);
                        }

                        // Edit button
                        if ui.small_button("✏").on_hover_text("Edit").clicked() {
                            settings.editing_script_index = Some(i);
                            let s = &settings.config.scripts[i];
                            settings.temp_script_name = s.name.clone();
                            settings.temp_script_path = s.script_path.clone();
                            settings.temp_script_args = s.args.join(" ");
                            settings.temp_script_auto_start = s.auto_start;
                            settings.temp_script_enabled = s.enabled;
                            settings.temp_script_restart_policy = s.restart_policy;
                            settings.temp_script_restart_delay_ms = s.restart_delay_ms;
                            settings.temp_script_subscriptions =
                                s.subscriptions.join(", ");
                        }

                        // Start/Stop buttons
                        if is_running {
                            if ui.small_button("⏹ Stop").clicked() {
                                settings.pending_script_actions.push((i, false));
                            }
                        } else if ui.small_button("▶ Start").clicked() {
                            settings.pending_script_actions.push((i, true));
                        }
                    });
                });

                // Error display
                if has_error {
                    let error = &settings.script_errors[i];
                    ui.colored_label(
                        egui::Color32::from_rgb(244, 67, 54),
                        format!("  Error: {error}"),
                    );
                }

                // Output viewer (collapsible)
                let has_output = settings
                    .script_output
                    .get(i)
                    .is_some_and(|o| !o.is_empty());
                if has_output {
                    let expanded = settings
                        .script_output_expanded
                        .get(i)
                        .copied()
                        .unwrap_or(false);
                    let header = if expanded {
                        "▼ Output"
                    } else {
                        "▶ Output"
                    };
                    if ui.small_button(header).clicked() {
                        while settings.script_output_expanded.len() <= i {
                            settings.script_output_expanded.push(false);
                        }
                        settings.script_output_expanded[i] = !expanded;
                    }

                    if expanded {
                        if let Some(output) = settings.script_output.get(i) {
                            egui::ScrollArea::vertical()
                                .max_height(150.0)
                                .id_salt(format!("script_output_{i}"))
                                .show(ui, |ui| {
                                    for line in output {
                                        ui.monospace(line);
                                    }
                                });
                        }
                    }
                }

                // Markdown panel viewer
                if let Some(Some((title, content))) = settings.script_panels.get(i) {
                    ui.separator();
                    ui.label(egui::RichText::new(title).strong().underline());
                    // Simple markdown rendering (egui doesn't have native markdown,
                    // so render as monospace text for now)
                    ui.monospace(content);
                }

                // Edit form (inline, replaces row when editing)
                if is_editing {
                    show_script_edit_form(ui, settings, changes_this_frame, Some(i));
                }

                ui.separator();
            }

            // Apply mutations
            if let Some(idx) = remove_index {
                settings.config.scripts.remove(idx);
                settings.editing_script_index = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if let Some(idx) = toggle_index {
                settings.config.scripts[idx].enabled =
                    !settings.config.scripts[idx].enabled;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Add new script button
            if !settings.adding_new_script {
                if ui.button("+ Add Script").clicked() {
                    settings.adding_new_script = true;
                    settings.temp_script_name = String::new();
                    settings.temp_script_path = String::new();
                    settings.temp_script_args = String::new();
                    settings.temp_script_auto_start = false;
                    settings.temp_script_enabled = true;
                    settings.temp_script_restart_policy = RestartPolicy::Never;
                    settings.temp_script_restart_delay_ms = 0;
                    settings.temp_script_subscriptions = String::new();
                }
            } else {
                show_script_edit_form(ui, settings, changes_this_frame, None);
            }
        },
    );
}

fn show_script_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.indent("script_edit_form", |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_script_name);
        });
        ui.horizontal(|ui| {
            ui.label("Script path:");
            ui.text_edit_singleline(&mut settings.temp_script_path);
        });
        ui.horizontal(|ui| {
            ui.label("Arguments:");
            ui.text_edit_singleline(&mut settings.temp_script_args);
        });
        ui.horizontal(|ui| {
            ui.label("Subscriptions (comma-separated, empty=all):");
            ui.text_edit_singleline(&mut settings.temp_script_subscriptions);
        });
        ui.checkbox(&mut settings.temp_script_enabled, "Enabled");
        ui.checkbox(&mut settings.temp_script_auto_start, "Auto-start with new tabs");

        ui.horizontal(|ui| {
            ui.label("Restart policy:");
            egui::ComboBox::from_id_salt("script_restart_policy")
                .selected_text(settings.temp_script_restart_policy.display_name())
                .show_ui(ui, |ui| {
                    for policy in RestartPolicy::all() {
                        ui.selectable_value(
                            &mut settings.temp_script_restart_policy,
                            *policy,
                            policy.display_name(),
                        );
                    }
                });
        });

        if settings.temp_script_restart_policy != RestartPolicy::Never {
            ui.horizontal(|ui| {
                ui.label("Restart delay (ms):");
                ui.add(
                    egui::DragValue::new(&mut settings.temp_script_restart_delay_ms)
                        .range(0..=30000),
                );
            });
        }

        ui.horizontal(|ui| {
            let save_label = if edit_index.is_some() {
                "Save"
            } else {
                "Add"
            };
            if ui.button(save_label).clicked() {
                let subscriptions: Vec<String> = settings
                    .temp_script_subscriptions
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let args: Vec<String> = settings
                    .temp_script_args
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                let new_config = ScriptConfig {
                    name: settings.temp_script_name.clone(),
                    enabled: settings.temp_script_enabled,
                    script_path: settings.temp_script_path.clone(),
                    args,
                    auto_start: settings.temp_script_auto_start,
                    restart_policy: settings.temp_script_restart_policy,
                    restart_delay_ms: settings.temp_script_restart_delay_ms,
                    subscriptions,
                    env_vars: Default::default(),
                };

                if let Some(idx) = edit_index {
                    settings.config.scripts[idx] = new_config;
                    settings.editing_script_index = None;
                } else {
                    settings.config.scripts.push(new_config);
                    settings.adding_new_script = false;
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if ui.button("Cancel").clicked() {
                settings.editing_script_index = None;
                settings.adding_new_script = false;
            }
        });
    });
}
```

**Step 2: Add module and render dispatch**

In `src/settings_ui/mod.rs`:
- Add `pub(crate) mod scripts_tab;`
- In the tab rendering dispatch (where each `SettingsTab` variant calls its `show` function), add:
  ```rust
  SettingsTab::Scripts => scripts_tab::show(ui, settings, changes_this_frame, collapsed),
  ```

**Step 3: Build to verify compilation**

Run: `cargo build`
Expected: Compiles

**Step 4: Commit**

```bash
git add src/settings_ui/scripts_tab.rs src/settings_ui/mod.rs
git commit -m "feat(settings): add Scripts tab UI with CRUD, status, and output viewer"
```

---

## Task 10: Window Manager — Script State Sync + Start/Stop

**Files:**
- Modify: `src/app/window_manager.rs` (add `sync_script_running_state()`, `start_script()`, `stop_script()`)

**Step 1: Add `sync_script_running_state()` method**

Follow the exact pattern of `sync_coprocess_running_state()`. Add after that method:

```rust
const SCRIPT_OUTPUT_MAX_LINES: usize = 200;

pub fn sync_script_running_state(&mut self) {
    let focused = self.get_focused_window_id();
    let (running_state, error_state, new_output, new_commands, panels): (
        Vec<bool>,
        Vec<String>,
        Vec<Vec<String>>,
        Vec<Vec<crate::scripting::protocol::ScriptCommand>>,
        Vec<Option<(String, String)>>,
    ) = if let Some(window_id) = focused
        && let Some(ws) = self.windows.get_mut(&window_id)
        && let Some(tab) = ws.tab_manager.active_tab_mut()
    {
        let script_count = ws.config.scripts.len();
        let mut running = Vec::with_capacity(script_count);
        let mut errors = Vec::with_capacity(script_count);
        let mut output = Vec::with_capacity(script_count);
        let mut commands = Vec::with_capacity(script_count);
        let mut panels_vec = Vec::with_capacity(script_count);

        for i in 0..script_count {
            let has_id = tab.script_ids.get(i).and_then(|opt| *opt);

            if let Some(id) = has_id {
                let is_running = tab.script_manager.is_running(id);
                running.push(is_running);

                // Forward events from observer to script
                if is_running {
                    if let Some(Some(forwarder)) = tab.script_forwarders.get(i) {
                        let events = forwarder.drain_events();
                        for event in &events {
                            let _ = tab.script_manager.send_event(id, event);
                        }
                    }
                }

                // Read commands and errors
                let cmds = tab.script_manager.read_commands(id);
                let errs = tab.script_manager.read_errors(id);
                let panel = tab.script_manager.get_panel(id).cloned();

                // Process commands that affect terminal state
                for cmd in &cmds {
                    self.process_script_command(cmd, tab);
                }

                errors.push(if is_running {
                    String::new()
                } else {
                    errs.join("\n")
                });
                output.push(
                    cmds.iter()
                        .filter_map(|c| {
                            if let crate::scripting::protocol::ScriptCommand::Log {
                                level,
                                message,
                            } = c
                            {
                                Some(format!("[{level}] {message}"))
                            } else {
                                None
                            }
                        })
                        .collect(),
                );
                commands.push(cmds);
                panels_vec.push(panel);
            } else {
                running.push(false);
                errors.push(String::new());
                output.push(Vec::new());
                commands.push(Vec::new());
                panels_vec.push(None);
            }
        }

        (running, errors, output, commands, panels_vec)
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };

    if let Some(sw) = &mut self.settings_window {
        let running_changed = sw.settings_ui.script_running != running_state;
        let errors_changed = sw.settings_ui.script_errors != error_state;
        let has_new_output = new_output.iter().any(|lines| !lines.is_empty());

        let count = running_state.len();
        sw.settings_ui.script_output.resize_with(count, Vec::new);
        sw.settings_ui
            .script_output_expanded
            .resize(count, false);
        sw.settings_ui.script_panels.resize_with(count, || None);

        for (i, lines) in new_output.into_iter().enumerate() {
            if !lines.is_empty() {
                let buf = &mut sw.settings_ui.script_output[i];
                buf.extend(lines);
                let overflow = buf.len().saturating_sub(Self::SCRIPT_OUTPUT_MAX_LINES);
                if overflow > 0 {
                    buf.drain(..overflow);
                }
            }
        }

        // Update panels
        for (i, panel) in panels.into_iter().enumerate() {
            if i < sw.settings_ui.script_panels.len() {
                sw.settings_ui.script_panels[i] = panel;
            }
        }

        if running_changed || errors_changed || has_new_output {
            sw.settings_ui.script_running = running_state;
            sw.settings_ui.script_errors = error_state;
            sw.request_redraw();
        }
    }
}
```

Note: The `process_script_command()` helper handles executing commands. It will need a reference to the tab's terminal. The exact implementation depends on how `self` is structured with borrow checker — may need to split the borrow.

**Step 2: Add `start_script()` and `stop_script()` methods**

Follow the `start_coprocess`/`stop_coprocess` pattern:

```rust
pub fn start_script(&mut self, config_index: usize) {
    log::debug!("start_script called with index {}", config_index);
    let focused = self.get_focused_window_id();
    if let Some(window_id) = focused
        && let Some(ws) = self.windows.get_mut(&window_id)
        && let Some(tab) = ws.tab_manager.active_tab_mut()
    {
        if config_index >= ws.config.scripts.len() {
            log::warn!("Script config index {} out of range", config_index);
            return;
        }
        let script_config = &ws.config.scripts[config_index];

        // Create subscription filter
        let filter = if script_config.subscriptions.is_empty() {
            None
        } else {
            Some(script_config.subscriptions.iter().cloned().collect())
        };

        // Register observer
        let forwarder = std::sync::Arc::new(
            crate::scripting::observer::ScriptEventForwarder::new(filter),
        );
        let term = tab.terminal.blocking_lock();
        let observer_id = term.add_observer(
            std::sync::Arc::clone(&forwarder)
                as std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
        );
        drop(term);

        // Start script process
        match tab.script_manager.start_script(script_config) {
            Ok(id) => {
                log::info!(
                    "Started script '{}' (id={})",
                    script_config.name,
                    id
                );
                while tab.script_ids.len() <= config_index {
                    tab.script_ids.push(None);
                }
                tab.script_ids[config_index] = Some(id);

                while tab.script_observer_ids.len() <= config_index {
                    tab.script_observer_ids.push(None);
                }
                tab.script_observer_ids[config_index] = Some(observer_id);

                while tab.script_forwarders.len() <= config_index {
                    tab.script_forwarders.push(None);
                }
                tab.script_forwarders[config_index] = Some(forwarder);
            }
            Err(e) => {
                log::error!(
                    "Failed to start script '{}': {}",
                    script_config.name,
                    e
                );
                // Remove the observer since script failed
                let term = tab.terminal.blocking_lock();
                term.remove_observer(observer_id);
                drop(term);

                if let Some(sw) = &mut self.settings_window {
                    let errors = &mut sw.settings_ui.script_errors;
                    while errors.len() <= config_index {
                        errors.push(String::new());
                    }
                    errors[config_index] = format!("Failed to start: {e}");
                    sw.request_redraw();
                }
                return;
            }
        }
        self.sync_script_running_state();
    }
}

pub fn stop_script(&mut self, config_index: usize) {
    log::debug!("stop_script called with index {}", config_index);
    let focused = self.get_focused_window_id();
    if let Some(window_id) = focused
        && let Some(ws) = self.windows.get_mut(&window_id)
        && let Some(tab) = ws.tab_manager.active_tab_mut()
    {
        // Stop the script process
        if let Some(Some(id)) = tab.script_ids.get(config_index).copied() {
            tab.script_manager.stop_script(id);
            tab.script_ids[config_index] = None;
        }

        // Remove the observer
        if let Some(Some(observer_id)) = tab.script_observer_ids.get(config_index).copied()
        {
            let term = tab.terminal.blocking_lock();
            term.remove_observer(observer_id);
            drop(term);
            tab.script_observer_ids[config_index] = None;
        }

        tab.script_forwarders
            .get_mut(config_index)
            .map(|f| f.take());

        self.sync_script_running_state();
    }
}
```

**Step 3: Add `process_script_command()` helper**

```rust
fn process_script_command(
    &self,
    cmd: &crate::scripting::protocol::ScriptCommand,
    tab: &mut crate::tab::Tab,
) {
    use crate::scripting::protocol::ScriptCommand;
    match cmd {
        ScriptCommand::WriteText { text } => {
            if let Ok(term) = tab.terminal.try_lock() {
                let _ = term.write_str(text);
            }
        }
        ScriptCommand::Notify { title, body } => {
            let _ = notify_rust::Notification::new()
                .summary(title)
                .body(body)
                .show();
        }
        ScriptCommand::SetBadge { text } => {
            // Update badge via terminal user variable
            if let Ok(term) = tab.terminal.try_lock() {
                let _ = term.write_str(&format!("\x1b]1337;SetBadgeFormat={}\x07",
                    base64::engine::general_purpose::STANDARD.encode(text)));
            }
        }
        ScriptCommand::Log { .. } => {
            // Handled in sync_script_running_state (added to output buffer)
        }
        ScriptCommand::SetPanel { .. } | ScriptCommand::ClearPanel {} => {
            // Handled in sync_script_running_state (panels updated there)
        }
        ScriptCommand::RunCommand { command } => {
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .spawn();
        }
        ScriptCommand::SetVariable { name, value } => {
            if let Ok(term) = tab.terminal.try_lock() {
                let _ = term.write_str(&format!(
                    "\x1b]1337;SetUserVar={}={}\x07",
                    name,
                    base64::engine::general_purpose::STANDARD.encode(value)
                ));
            }
        }
        ScriptCommand::ChangeConfig { .. } => {
            // Config changes are complex — defer to a future iteration
            log::warn!("Script ChangeConfig command not yet implemented");
        }
    }
}
```

**Step 4: Wire sync into the main event loop**

Find where `sync_coprocess_running_state()` is called (in handler.rs or the event loop) and add a call to `sync_script_running_state()` right after it.

**Step 5: Build to verify compilation**

Run: `cargo build`
Expected: Compiles (may need to adjust borrow patterns)

**Step 6: Commit**

```bash
git add src/app/window_manager.rs
git commit -m "feat(window-manager): add script state sync, start, and stop methods"
```

---

## Task 11: Settings Window + Handler — Action Routing

**Files:**
- Modify: `src/settings_window.rs` (add `StartScript`/`StopScript` action variants, queue processing)
- Modify: `src/app/handler.rs` (dispatch `StartScript`/`StopScript` to window_manager)

**Step 1: Add action variants to `SettingsWindowAction`**

In `src/settings_window.rs`, find the `SettingsWindowAction` enum and add:

```rust
StartScript(usize),
StopScript(usize),
```

**Step 2: Add queue processing**

In the settings window update method (where `pending_coprocess_actions` is processed), add similar logic:

```rust
if let Some((index, start)) = self.settings_ui.pending_script_actions.pop() {
    log::info!(
        "Settings window: popped script action index={} start={}",
        index,
        start
    );
    self.window.request_redraw();
    return if start {
        SettingsWindowAction::StartScript(index)
    } else {
        SettingsWindowAction::StopScript(index)
    };
}
```

**Step 3: Add handler dispatch**

In `src/app/handler.rs`, find where `SettingsWindowAction::StartCoprocess` is matched and add:

```rust
SettingsWindowAction::StartScript(index) => {
    log::debug!("Handler: received StartScript({})", index);
    self.start_script(index);
}
SettingsWindowAction::StopScript(index) => {
    log::debug!("Handler: received StopScript({})", index);
    self.stop_script(index);
}
```

**Step 4: Build to verify compilation**

Run: `cargo build`
Expected: Compiles

**Step 5: Commit**

```bash
git add src/settings_window.rs src/app/handler.rs
git commit -m "feat(handler): route script start/stop actions from settings to window manager"
```

---

## Task 12: Example Python Script + Documentation

**Files:**
- Create: `scripts/examples/hello_observer.py` (example script demonstrating the JSON protocol)
- Modify: `docs/AUTOMATION.md` (add Scripts section documenting the feature)

**Step 1: Create example Python script**

Create `scripts/examples/hello_observer.py`:

```python
#!/usr/bin/env python3
"""Example par-term observer script.

Reads terminal events as JSON from stdin, responds with commands on stdout.
Demonstrates the par-term scripting protocol.

Usage in config.yaml:
  scripts:
    - name: "Hello Observer"
      script_path: "scripts/examples/hello_observer.py"
      auto_start: true
      subscriptions: ["BellRang", "CwdChanged", "CommandComplete"]
"""

import json
import sys


def send_command(cmd: dict) -> None:
    """Send a JSON command to par-term."""
    print(json.dumps(cmd), flush=True)


def log(level: str, message: str) -> None:
    """Send a log message to the par-term output panel."""
    send_command({"type": "log", "level": level, "message": message})


def set_panel(title: str, content: str) -> None:
    """Set the markdown panel content."""
    send_command({"type": "set_panel", "title": title, "content": content})


def main() -> None:
    log("info", "Hello Observer script started")
    set_panel("Observer", "## Hello Observer\n- Status: Running\n- Events: 0")

    event_count = 0

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            event = json.loads(line)
        except json.JSONDecodeError as e:
            log("error", f"Invalid JSON: {e}")
            continue

        event_count += 1
        kind = event.get("kind", "unknown")
        log("info", f"Received event: {kind} (#{event_count})")

        # Update the panel with event count
        set_panel(
            "Observer",
            f"## Hello Observer\n- Status: Running\n- Events: {event_count}\n- Last: {kind}",
        )

        # React to specific events
        if kind == "BellRang":
            send_command(
                {
                    "type": "notify",
                    "title": "Bell!",
                    "body": "Terminal bell was triggered",
                }
            )
        elif kind == "CwdChanged":
            cwd = event.get("cwd", "unknown")
            log("info", f"Directory changed to: {cwd}")
        elif kind == "CommandComplete":
            cmd = event.get("data", {}).get("command", "")
            exit_code = event.get("data", {}).get("exit_code")
            if exit_code is not None and exit_code != 0:
                send_command(
                    {
                        "type": "notify",
                        "title": "Command Failed",
                        "body": f"{cmd} exited with code {exit_code}",
                    }
                )

    log("info", "Hello Observer script shutting down")


if __name__ == "__main__":
    main()
```

**Step 2: Update docs/AUTOMATION.md**

Add a "Scripts" section after the existing Coprocesses section documenting:
- What scripts are and how they differ from coprocesses
- The JSON protocol (events and commands)
- How to configure scripts in config.yaml
- Event subscription filtering
- Markdown panels
- The example script

**Step 3: Commit**

```bash
git add scripts/examples/hello_observer.py docs/AUTOMATION.md
git commit -m "docs: add example observer script and scripting documentation"
```

---

## Task 13: Integration Test — End-to-End Script Lifecycle

**Files:**
- Create: `tests/script_integration_tests.rs`

**Step 1: Write integration test**

```rust
use par_term::config::automation::RestartPolicy;
use par_term::config::scripting::ScriptConfig;
use par_term::scripting::manager::ScriptManager;
use par_term::scripting::protocol::{ScriptCommand, ScriptEvent, ScriptEventData};

#[test]
fn test_full_script_lifecycle() {
    let config = ScriptConfig {
        name: "integration-test".to_string(),
        enabled: true,
        script_path: "python3".to_string(),
        args: vec![
            "-c".to_string(),
            r#"
import json, sys
for line in sys.stdin:
    event = json.loads(line.strip())
    kind = event.get("kind", "")
    if kind == "BellRang":
        print(json.dumps({"type": "log", "level": "info", "message": "bell received"}), flush=True)
    elif kind == "CwdChanged":
        print(json.dumps({"type": "set_panel", "title": "CWD", "content": "## " + event.get("cwd", "")}), flush=True)
    elif kind == "shutdown":
        break
"#
            .to_string(),
        ],
        auto_start: false,
        restart_policy: RestartPolicy::Never,
        restart_delay_ms: 0,
        subscriptions: vec![],
        env_vars: Default::default(),
    };

    let mut manager = ScriptManager::new();
    let id = manager.start_script(&config).unwrap();
    assert!(manager.is_running(id));

    // Send bell event
    let bell = ScriptEvent {
        kind: "BellRang".to_string(),
        data: ScriptEventData::Empty {},
    };
    manager.send_event(id, &bell).unwrap();

    // Send cwd event
    let cwd = ScriptEvent {
        kind: "CwdChanged".to_string(),
        data: ScriptEventData::CwdChanged {
            cwd: "/tmp/test".to_string(),
        },
    };
    manager.send_event(id, &cwd).unwrap();

    // Wait for processing
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Read commands
    let commands = manager.read_commands(id);
    assert!(commands.len() >= 2, "Expected at least 2 commands, got {}", commands.len());

    // Verify we got a log command for the bell
    assert!(commands.iter().any(|c| matches!(c, ScriptCommand::Log { message, .. } if message == "bell received")));

    // Verify we got a set_panel command for the cwd change
    assert!(commands.iter().any(|c| matches!(c, ScriptCommand::SetPanel { title, .. } if title == "CWD")));

    manager.stop_script(id);
    assert!(!manager.is_running(id));
}
```

**Step 2: Run the test**

Run: `cargo test --test script_integration_tests -- --nocapture`
Expected: PASS

**Step 3: Commit**

```bash
git add tests/script_integration_tests.rs
git commit -m "test: add end-to-end integration test for script lifecycle"
```

---

## Task 14: Final Checks — Lint, Format, Test

**Step 1: Format**

Run: `cargo fmt`

**Step 2: Lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Fix any issues.

**Step 3: Run all tests**

Run: `cargo test`
All tests must pass.

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: fix lint and format issues from scripting manager implementation"
```

---

## Task 15: Update MATRIX.md + Create PR

**Step 1: Update MATRIX.md**

Update §18 Python API row to reflect implementation:
```
| Python API | ✅ Full scripting API | ✅ Frontend scripting manager | ✅ | ⭐⭐ | 🟡 |
```

**Step 2: Run `make checkall`**

Run: `make checkall`
Expected: All checks pass

**Step 3: Create PR**

```bash
git push -u origin feat/scripting-manager
gh pr create --title "feat: add frontend scripting manager for terminal observer API" --body "$(cat <<'EOF'
## Summary
- Adds scripting manager that runs Python scripts as subprocesses
- JSON protocol for bidirectional communication (events + commands)
- Scripts receive terminal events via TerminalObserver API
- Scripts can write to PTY, show notifications, set badges, register markdown panels
- Per-tab lifecycle with auto-start, restart policies
- Full Settings UI tab for CRUD, monitoring, and output viewing

Closes #150

## Test plan
- [ ] Unit tests for ScriptConfig serialization
- [ ] Unit tests for JSON protocol types
- [ ] Unit tests for ScriptProcess lifecycle
- [ ] Unit tests for ScriptEventForwarder observer
- [ ] Unit tests for ScriptManager orchestration
- [ ] Integration test for full script lifecycle
- [ ] Manual test: add script in Settings, start/stop, verify events
- [ ] Manual test: verify example script works with real terminal events
- [ ] `make checkall` passes
EOF
)"
```
