//! Tool registration, descriptors, and dispatch for the MCP server.
//!
//! This module owns the tool registry: it builds the `tools/list` response and
//! dispatches `tools/call` requests to the appropriate per-tool handler.

pub mod config_update;
pub mod screenshot;

use serde_json::Value;

// Re-export per-tool handlers for use in lib.rs dispatch
pub use config_update::handle_config_update;
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

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Handle the `tools/list` request.
pub fn handle_tools_list() -> Value {
    serde_json::json!({
        "tools": [config_update_tool(), terminal_screenshot_tool()]
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
