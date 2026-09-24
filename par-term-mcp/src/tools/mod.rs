//! Tool registration, descriptors, and dispatch for the MCP server.
//!
//! This module owns the tool registry: it builds the `tools/list` response and
//! dispatches `tools/call` requests to the appropriate per-tool handler.

pub mod agent_commands;
pub mod config_update;
pub mod diagnostics;
pub mod screenshot;

use serde_json::Value;

// Re-export per-tool handlers for use in lib.rs dispatch
pub use agent_commands::{handle_command_create, handle_command_delete, handle_command_list};
pub use config_update::handle_config_update;
pub use diagnostics::handle_shader_diagnostics;
pub use screenshot::handle_terminal_screenshot;

// ---------------------------------------------------------------------------
// Tool descriptors
// ---------------------------------------------------------------------------

/// Build the input schema for the `config_update` tool.
fn config_update_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "updates": {
                "type": "object",
                "description": "Map of config key -> JSON value to apply"
            }
        },
        "required": ["updates"]
    })
}

/// Build the tool descriptor for `config_update`.
fn config_update_tool() -> Value {
    serde_json::json!({
        "name": "config_update",
        "description": "Update par-term configuration settings. Write a JSON object of config key-value pairs to apply immediately. Supported keys include: custom_shader (string|null), custom_shader_enabled (bool), custom_shader_animation (bool), custom_shader_animation_speed (float), custom_shader_brightness (float), custom_shader_text_opacity (float), custom_shader_full_content (bool), cursor_shader (string|null), cursor_shader_enabled (bool), cursor_shader_animation (bool), cursor_shader_animation_speed (float), cursor_shader_glow_radius (float), cursor_shader_glow_intensity (float), cursor_shader_trail_duration (float), cursor_shader_hides_cursor (bool), window_opacity (float), font_size (float). Do NOT edit config.yaml directly.",
        "inputSchema": config_update_input_schema()
    })
}

/// Build the input schema for the `terminal_screenshot` tool.
fn terminal_screenshot_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {}
    })
}

/// Build the tool descriptor for `terminal_screenshot`.
fn terminal_screenshot_tool() -> Value {
    serde_json::json!({
        "name": "terminal_screenshot",
        "description": "Capture a screenshot of the currently visible terminal output (including active shader/cursor visual effects) from the running par-term app. Returns an image for visual debugging. Requires user permission.",
        "inputSchema": terminal_screenshot_input_schema()
    })
}

/// Build the input schema for the `shader_diagnostics` tool.
fn shader_diagnostics_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {}
    })
}

/// Build the tool descriptor for `shader_diagnostics`.
fn shader_diagnostics_tool() -> Value {
    serde_json::json!({
        "name": "shader_diagnostics",
        "description": "Return live shader diagnostics from the running par-term app: active background/cursor shader names, enabled state, last compile/reload errors, shaders directory, and debug WGSL/wrapped GLSL paths. Use after creating, editing, or activating shaders, especially if rendering fails or a shader appears unchanged.",
        "inputSchema": shader_diagnostics_input_schema()
    })
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Input schema for the `command_create` tool.
fn command_create_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "created_by": {
                "type": "string",
                "enum": ["agent"],
                "description": "Provenance tag; command_create writes agent commands only"
            },
            "source_agent": {
                "type": "string",
                "description": "The authoring agent's name (e.g. claude-code)"
            },
            "created_at": {
                "type": "string",
                "description": "RFC 3339 timestamp (optional, informational)"
            },
            "action": {
                "type": "object",
                "description": "One CustomActionConfig variant, serde-tagged by 'type': \
                                shell_command (script kind) or new_tab | insert_text | \
                                split_pane | key_sequence | sequence (macro kinds). Each \
                                embeds required 'id' ([a-z0-9-]+, == filename) and 'title'."
            }
        },
        "required": ["created_by", "source_agent", "action"]
    })
}

/// Build the tool descriptor for `command_create`.
fn command_create_tool() -> Value {
    serde_json::json!({
        "name": "command_create",
        "description": "Create (or update) an agent-authored par-term command: a palette + \
                        CLI command usable immediately with no restart. Script commands \
                        (action.type=shell_command) run a shell command and require the \
                        user's one-time confirmation of the exact body; macros replay \
                        built-in actions (new tab, insert text, split pane, key sequence, \
                        sequences). Commands appear as 'agent-cmd:<id>' palette rows and \
                        run as 'par-term <id>' on the CLI.",
        "inputSchema": command_create_input_schema()
    })
}

/// Input schema for the `command_list` tool.
fn command_list_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {}
    })
}

/// Build the tool descriptor for `command_list`.
fn command_list_tool() -> Value {
    serde_json::json!({
        "name": "command_list",
        "description": "List every par-term command in the commands directory with id, \
                        title, kind (script|macro), created_by, and source_agent.",
        "inputSchema": command_list_input_schema()
    })
}

/// Input schema for the `command_delete` tool.
fn command_delete_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "string",
                "description": "The command id (filename stem) to delete"
            }
        },
        "required": ["id"]
    })
}

/// Build the tool descriptor for `command_delete`.
fn command_delete_tool() -> Value {
    serde_json::json!({
        "name": "command_delete",
        "description": "Delete an agent-authored par-term command by id. User-authored \
                        commands are refused; the user deletes those by hand.",
        "inputSchema": command_delete_input_schema()
    })
}

/// Handle the `tools/list` request.
pub fn handle_tools_list() -> Value {
    serde_json::json!({
        "tools": [
            config_update_tool(),
            terminal_screenshot_tool(),
            shader_diagnostics_tool(),
            command_create_tool(),
            command_list_tool(),
            command_delete_tool(),
        ]
    })
}

/// Handle the `tools/call` request.
pub fn handle_tools_call(params: Option<Value>) -> Value {
    let params = match params {
        Some(p) => p,
        None => {
            return tool_error("Missing params for tools/call");
        }
    };

    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

    match name {
        "config_update" => handle_config_update(&params),
        "terminal_screenshot" => handle_terminal_screenshot(&params),
        "shader_diagnostics" => handle_shader_diagnostics(&params),
        "command_create" => handle_command_create(&params),
        "command_list" => handle_command_list(&params),
        "command_delete" => handle_command_delete(&params),
        _ => tool_error(&format!("Unknown tool: {name}")),
    }
}

// ---------------------------------------------------------------------------
// Error helper (shared by tool handlers in submodules)
// ---------------------------------------------------------------------------

/// Build a tool error result.
pub fn tool_error(message: &str) -> Value {
    serde_json::json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": message
        }]
    })
}
