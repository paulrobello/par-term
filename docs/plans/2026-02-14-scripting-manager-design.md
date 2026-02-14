# Frontend Scripting Manager Design

**Date**: 2026-02-14
**Issue**: #150
**Status**: Approved

## Summary

Add a frontend scripting manager that allows users to load, manage, and run Python scripts that interact with terminal state via the core library's `TerminalObserver` API.

## Architecture

### Subprocess + JSON Protocol

Scripts run as **child processes** communicating via JSON over stdin/stdout, with **per-tab scope** (like coprocesses).

**Rationale over alternatives:**
- **vs. Embedded PyO3**: No compile-time Python dependency, no GIL contention, clean process isolation. Script crashes don't affect the terminal.
- **vs. Unix sockets**: Simpler lifecycle - no socket management or port allocation. Process stdin/stdout is sufficient.
- **vs. Shell scripts**: Python gives structured data handling and rich scripting capability.

### Data Flow

```
Terminal Events (core observer)
    -> ScriptManager (Rust)
    -> JSON encode -> script stdin

Script stdout -> JSON decode -> ScriptManager
    -> Execute actions (write to PTY, notify, set badge, etc.)

Script stderr -> captured for error display in Settings UI
```

## JSON Protocol

### Events (terminal -> script)

```json
{"type": "event", "kind": "CwdChanged", "data": {"cwd": "/home/user/project"}}
{"type": "event", "kind": "CommandComplete", "data": {"command": "make test", "exit_code": 0}}
{"type": "event", "kind": "BellRang", "data": {}}
```

### Commands (script -> terminal)

Scripts have **full terminal control**:

```json
{"type": "write_text", "text": "echo hello\n"}
{"type": "notify", "title": "Build done", "body": "make test passed"}
{"type": "set_badge", "text": "passing"}
{"type": "set_variable", "name": "foo", "value": "bar"}
{"type": "run_command", "command": "ls -la"}
{"type": "change_config", "key": "font_size", "value": 14.0}
{"type": "log", "level": "info", "message": "Script initialized"}
```

### Markdown UI Panels

Scripts can register markdown-rendered panels:

```json
{"type": "set_panel", "title": "Build Status", "content": "## Build\n- Status: passing\n- Duration: 2.3s"}
```

## Config Structure

```rust
pub struct ScriptConfig {
    pub name: String,
    pub enabled: bool,
    pub script_path: String,
    pub args: Vec<String>,
    pub auto_start: bool,
    pub restart_policy: RestartPolicy,
    pub restart_delay_ms: u64,
    pub subscriptions: Vec<String>,    // Event kinds (empty = all)
    pub env_vars: HashMap<String, String>,
}
```

Added to `Config.scripts: Vec<ScriptConfig>`.

## Components

### `src/scripting/mod.rs` - ScriptManager

- Spawns Python subprocess per script config
- Implements `TerminalObserver` to receive events from core
- Encodes events to JSON, writes to script stdin
- Reads JSON commands from script stdout, dispatches actions
- Lifecycle: start/stop/restart, status tracking, error capture

### `src/scripting/protocol.rs` - JSON Protocol Types

- `ScriptEvent` / `ScriptCommand` enums with serde serialization
- Input validation and error handling

### `src/scripting/actions.rs` - Command Execution

Maps JSON commands to terminal actions:
- Write text to PTY
- Show notifications
- Set badge text / user variables
- Change config settings
- Manipulate tabs/panes

### `src/settings_ui/scripts_tab.rs` - Settings UI

CRUD for script configs following automation_tab pattern:
- Script list with status (running/stopped/error)
- Start/Stop/Restart buttons
- Event subscription checkboxes
- Output/error log viewer (expandable, 200-line buffer)
- Markdown panel viewer for script-registered panels

### `src/config/scripting.rs` - Config Serialization

`ScriptConfig` struct with serde, default values, validation.

## Settings UI Layout

```
Scripts Tab:
+-- [+ Add Script] button
+-- Script List (collapsible per script):
|   +-- Name, Path, Status (running/stopped/error)
|   +-- Start/Stop/Restart buttons
|   +-- Event Subscriptions (checkboxes)
|   +-- Output Log (expandable, 200 lines)
|   +-- Error Log (expandable)
|   +-- Markdown Panel (if script registered one)
+-- Script form (when editing/adding)
```

## Per-Tab Lifecycle

- Scripts spawned when tab opens (if `auto_start: true`)
- Each tab gets its own script process instances
- Script processes killed when tab closes
- State synced via `sync_script_running_state()` in window_manager (same pattern as coprocesses)

## Scope Boundaries (v1)

**In scope:**
- Python script subprocess management
- JSON protocol for events and commands
- Full terminal control commands
- Markdown-rendered UI panels
- Per-tab lifecycle
- Settings UI for CRUD + monitoring

**Deferred:**
- Custom egui widget API from scripts
- Multi-language support (Lua, JS)
- Global (cross-tab) scripts
- Script marketplace / sharing
- Script debugging tools
