//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Reads line-delimited JSON-RPC 2.0 from stdin and writes responses to stdout.
//! Exposes tools for par-term ACP integrations:
//! - `config_update`: writes configuration changes to a file for the main app
//!   to pick up
//! - `terminal_screenshot`: requests a live terminal screenshot from the app
//!   via a file-based IPC handshake (with an optional fallback image path for
//!   non-GUI test harnesses)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name reported during initialization.
const SERVER_NAME: &str = "par-term";

/// Server version reported during initialization.
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Environment variable for overriding the config update file path.
pub const CONFIG_UPDATE_PATH_ENV: &str = "PAR_TERM_CONFIG_UPDATE_PATH";
/// Environment variable for screenshot request IPC file path.
pub const SCREENSHOT_REQUEST_PATH_ENV: &str = "PAR_TERM_SCREENSHOT_REQUEST_PATH";
/// Environment variable for screenshot response IPC file path.
pub const SCREENSHOT_RESPONSE_PATH_ENV: &str = "PAR_TERM_SCREENSHOT_RESPONSE_PATH";
/// Optional environment variable for a static fallback screenshot file path.
/// Used by the ACP harness to test the screenshot tool flow without a GUI.
pub const SCREENSHOT_FALLBACK_PATH_ENV: &str = "PAR_TERM_SCREENSHOT_FALLBACK_PATH";

/// Default config update filename (relative to config dir).
pub const CONFIG_UPDATE_FILENAME: &str = ".config-update.json";
/// Default screenshot request filename (relative to config dir).
pub const SCREENSHOT_REQUEST_FILENAME: &str = ".screenshot-request.json";
/// Default screenshot response filename (relative to config dir).
pub const SCREENSHOT_RESPONSE_FILENAME: &str = ".screenshot-response.json";

/// Screenshot request written by the MCP server for the GUI app to fulfill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalScreenshotRequest {
    pub request_id: String,
}

/// Screenshot response written by the GUI app for the MCP server to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalScreenshotResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub data_base64: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

// ---------------------------------------------------------------------------
// JSON-RPC wire types (minimal, server-side only)
// ---------------------------------------------------------------------------

/// An incoming JSON-RPC 2.0 message from the client.
#[derive(Debug, Deserialize)]
struct IncomingMessage {
    #[allow(dead_code)] // Deserialized from JSON-RPC protocol; required by spec
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
}

/// An outgoing JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
    id: Value,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Tool definitions
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
// Config update file path resolution
// ---------------------------------------------------------------------------

/// Resolve the path where config updates should be written.
///
/// Checks `PAR_TERM_CONFIG_UPDATE_PATH` env var first, then falls back to
/// `~/.config/par-term/.config-update.json`.
fn config_update_path() -> PathBuf {
    resolve_ipc_path(CONFIG_UPDATE_PATH_ENV, CONFIG_UPDATE_FILENAME)
}

/// Resolve the path where screenshot requests should be written.
pub fn screenshot_request_path() -> PathBuf {
    resolve_ipc_path(SCREENSHOT_REQUEST_PATH_ENV, SCREENSHOT_REQUEST_FILENAME)
}

/// Resolve the path where screenshot responses should be written.
pub fn screenshot_response_path() -> PathBuf {
    resolve_ipc_path(SCREENSHOT_RESPONSE_PATH_ENV, SCREENSHOT_RESPONSE_FILENAME)
}

/// Resolve a path from env var or default filename under `~/.config/par-term`.
fn resolve_ipc_path(env_var: &str, default_filename: &str) -> PathBuf {
    if let Ok(path) = std::env::var(env_var) {
        return PathBuf::from(path);
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| {
            // Last resort: ~/.config
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("par-term");

    config_dir.join(default_filename)
}

// ---------------------------------------------------------------------------
// Request handlers
// ---------------------------------------------------------------------------

/// Handle the `initialize` request.
fn handle_initialize() -> Value {
    serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": SERVER_VERSION
        }
    })
}

/// Handle the `tools/list` request.
fn handle_tools_list() -> Value {
    serde_json::json!({
        "tools": [config_update_tool(), terminal_screenshot_tool()]
    })
}

/// Handle the `tools/call` request.
fn handle_tools_call(params: Option<Value>) -> Value {
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

/// Execute the `config_update` tool.
fn handle_config_update(params: &Value) -> Value {
    let arguments = match params.get("arguments") {
        Some(args) => args,
        None => {
            return tool_error("Missing 'arguments' in tools/call params");
        }
    };

    let updates = match arguments.get("updates") {
        Some(u) if u.is_object() => u,
        Some(_) => {
            return tool_error("'updates' must be a JSON object");
        }
        None => {
            return tool_error("Missing 'updates' in tool arguments");
        }
    };

    let path = config_update_path();
    write_config_updates(updates, &path)
}

/// Execute the `terminal_screenshot` tool.
fn handle_terminal_screenshot(params: &Value) -> Value {
    // MCP tools/call always includes "arguments", but this tool takes none.
    if let Some(arguments) = params.get("arguments")
        && !arguments.is_object()
    {
        return tool_error("'arguments' must be an object");
    }

    if let Ok(fallback) = std::env::var(SCREENSHOT_FALLBACK_PATH_ENV)
        && !fallback.trim().is_empty()
    {
        let path = PathBuf::from(fallback.trim());
        return image_tool_result_from_file(&path);
    }

    let request_path = screenshot_request_path();
    let response_path = screenshot_response_path();

    let request_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let request = TerminalScreenshotRequest {
        request_id: request_id.clone(),
    };

    if let Err(e) = write_json_atomic(&request, &request_path) {
        return tool_error(&format!(
            "Failed to write screenshot request {}: {e}",
            request_path.display()
        ));
    }

    let timeout = Duration::from_secs(15);
    let poll_interval = Duration::from_millis(100);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match try_read_screenshot_response(&response_path) {
            Ok(Some(response)) if response.request_id == request_id => {
                // Clear response file after consuming
                let _ = std::fs::write(&response_path, "");
                if !response.ok {
                    return tool_error(
                        response
                            .error
                            .as_deref()
                            .unwrap_or("Screenshot capture failed"),
                    );
                }
                let mime_type = response
                    .mime_type
                    .unwrap_or_else(|| "image/png".to_string());
                let data_base64 = match response.data_base64 {
                    Some(data) if !data.is_empty() => data,
                    _ => return tool_error("Screenshot response missing image data"),
                };
                let width = response.width.unwrap_or(0);
                let height = response.height.unwrap_or(0);
                return serde_json::json!({
                    "content": [
                        {
                            "type": "image",
                            "mimeType": mime_type,
                            "data": data_base64,
                        },
                        {
                            "type": "text",
                            "text": format!("Captured terminal screenshot ({}x{}).", width, height),
                        }
                    ]
                });
            }
            Ok(Some(_other_response)) => {
                // Stale response for a different request ID; keep waiting.
            }
            Ok(None) => {}
            Err(e) => {
                return tool_error(&format!(
                    "Failed to read screenshot response {}: {e}",
                    response_path.display()
                ));
            }
        }
        std::thread::sleep(poll_interval);
    }

    tool_error("Timed out waiting for par-term app screenshot response")
}

/// Write config updates to the specified path atomically.
///
/// Creates parent directories if needed, writes to a temp file, then renames.
fn write_config_updates(updates: &Value, path: &std::path::Path) -> Value {
    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return tool_error(&format!(
            "Failed to create config directory {}: {e}",
            parent.display()
        ));
    }

    // Atomic write: write to temp file, then rename
    let temp_path = path.with_extension("json.tmp");

    let json_bytes = match serde_json::to_vec_pretty(updates) {
        Ok(bytes) => bytes,
        Err(e) => {
            return tool_error(&format!("Failed to serialize updates: {e}"));
        }
    };

    if let Err(e) = std::fs::write(&temp_path, &json_bytes) {
        return tool_error(&format!(
            "Failed to write temp file {}: {e}",
            temp_path.display()
        ));
    }

    if let Err(e) = std::fs::rename(&temp_path, path) {
        // Clean up temp file on rename failure
        let _ = std::fs::remove_file(&temp_path);
        return tool_error(&format!(
            "Failed to rename temp file to {}: {e}",
            path.display()
        ));
    }

    let keys: Vec<&str> = updates
        .as_object()
        .map(|obj| obj.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    eprintln!(
        "[mcp-server] config_update: wrote {} key(s) to {}",
        keys.len(),
        path.display()
    );

    serde_json::json!({
        "content": [{
            "type": "text",
            "text": format!(
                "Successfully applied config update ({} key(s): {})",
                keys.len(),
                keys.join(", ")
            )
        }]
    })
}

/// Atomically write a JSON payload to a path.
fn write_json_atomic<T: Serialize>(payload: &T, path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(format!(
            "Failed to create parent directory {}: {e}",
            parent.display()
        ));
    }

    let temp_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(payload).map_err(|e| e.to_string())?;
    std::fs::write(&temp_path, &bytes).map_err(|e| {
        format!(
            "Failed to write temp file {}: {e}",
            temp_path.to_string_lossy()
        )
    })?;
    std::fs::rename(&temp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Failed to rename temp file to {}: {e}",
            path.to_string_lossy()
        )
    })?;
    Ok(())
}

/// Read and parse a screenshot response file, returning `None` for empty files.
fn try_read_screenshot_response(
    path: &std::path::Path,
) -> Result<Option<TerminalScreenshotResponse>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if content.trim().is_empty() {
        return Ok(None);
    }
    let resp =
        serde_json::from_str::<TerminalScreenshotResponse>(&content).map_err(|e| e.to_string())?;
    Ok(Some(resp))
}

/// Build an MCP image tool result from an existing image file.
fn image_tool_result_from_file(path: &std::path::Path) -> Value {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return tool_error(&format!(
                "Failed to read fallback screenshot {}: {e}",
                path.display()
            ));
        }
    };
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    serde_json::json!({
        "content": [
            {
                "type": "image",
                "mimeType": "image/png",
                "data": data
            },
            {
                "type": "text",
                "text": format!("Provided fallback terminal screenshot from {}.", path.display())
            }
        ]
    })
}

/// Build a tool error result.
fn tool_error(message: &str) -> Value {
    serde_json::json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": message
        }]
    })
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

/// Build a success response.
fn success_response(id: Value, result: Value) -> Response {
    Response {
        jsonrpc: "2.0",
        result: Some(result),
        error: None,
        id,
    }
}

/// Build a method-not-found error response.
fn method_not_found(id: Value, method: &str) -> Response {
    Response {
        jsonrpc: "2.0",
        result: None,
        error: Some(RpcError {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }),
        id,
    }
}

/// Build a parse error response.
fn parse_error() -> Response {
    Response {
        jsonrpc: "2.0",
        result: None,
        error: Some(RpcError {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }),
        id: Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Server loop
// ---------------------------------------------------------------------------

/// Send a JSON-RPC response to stdout.
fn send_response(stdout: &mut impl Write, response: &Response) {
    match serde_json::to_string(response) {
        Ok(json) => {
            // Write as a single line followed by newline
            if let Err(e) = writeln!(stdout, "{json}") {
                eprintln!("[mcp-server] Failed to write response: {e}");
            }
            if let Err(e) = stdout.flush() {
                eprintln!("[mcp-server] Failed to flush stdout: {e}");
            }
        }
        Err(e) => {
            eprintln!("[mcp-server] Failed to serialize response: {e}");
        }
    }
}

/// Run the MCP server loop. This function never returns normally — it exits
/// the process when stdin is closed or an unrecoverable error occurs.
pub fn run_mcp_server() -> ! {
    eprintln!("[mcp-server] Starting par-term MCP server v{SERVER_VERSION}");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[mcp-server] Error reading stdin: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        eprintln!("[mcp-server] <- {trimmed}");

        let msg: IncomingMessage = match serde_json::from_str(trimmed) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[mcp-server] Parse error: {e}");
                send_response(&mut stdout, &parse_error());
                continue;
            }
        };

        let method = match &msg.method {
            Some(m) => m.as_str(),
            None => {
                // No method field — not a request or notification we handle
                eprintln!("[mcp-server] Ignoring message without method");
                continue;
            }
        };

        // Check if this is a notification (no id) — notifications don't get responses
        let id = match msg.id {
            Some(id) => id,
            None => {
                eprintln!("[mcp-server] Notification: {method}");
                // No response for notifications
                continue;
            }
        };

        // Dispatch the request
        let response = match method {
            "initialize" => success_response(id, handle_initialize()),
            "tools/list" => success_response(id, handle_tools_list()),
            "tools/call" => success_response(id, handle_tools_call(msg.params)),
            _ => method_not_found(id, method),
        };

        eprintln!(
            "[mcp-server] -> {}",
            serde_json::to_string(&response).unwrap_or_else(|_| "<serialization error>".into())
        );

        send_response(&mut stdout, &response);
    }

    eprintln!("[mcp-server] stdin closed, exiting");
    std::process::exit(0);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_initialize() {
        let result = handle_initialize();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn test_handle_tools_list() {
        let result = handle_tools_list();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        let names: Vec<_> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"config_update"));
        assert!(names.contains(&"terminal_screenshot"));
        for tool in tools {
            assert!(tool["inputSchema"].is_object());
        }
    }

    #[test]
    fn test_handle_tools_call_unknown_tool() {
        let params = serde_json::json!({
            "name": "nonexistent_tool",
            "arguments": {}
        });
        let result = handle_tools_call(Some(params));
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Unknown tool")
        );
    }

    #[test]
    fn test_handle_tools_call_missing_params() {
        let result = handle_tools_call(None);
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn test_handle_config_update_missing_updates() {
        let params = serde_json::json!({
            "name": "config_update",
            "arguments": {}
        });
        let result = handle_tools_call(Some(params));
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Missing 'updates'")
        );
    }

    #[test]
    fn test_handle_config_update_invalid_updates_type() {
        let params = serde_json::json!({
            "name": "config_update",
            "arguments": {
                "updates": "not an object"
            }
        });
        let result = handle_tools_call(Some(params));
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("must be a JSON object")
        );
    }

    #[test]
    fn test_handle_config_update_success() {
        // Use a temp directory to avoid touching real config
        let dir = tempfile::tempdir().unwrap();
        let update_path = dir.path().join("test-update.json");

        let updates = serde_json::json!({
            "font_size": 14.0,
            "custom_shader_enabled": true
        });
        let result = write_config_updates(&updates, &update_path);

        // Should not be an error
        assert!(result.get("isError").is_none());
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Successfully")
        );

        // Verify the file was written
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&update_path).unwrap()).unwrap();
        assert_eq!(written["font_size"], 14.0);
        assert_eq!(written["custom_shader_enabled"], true);
    }

    #[test]
    fn test_success_response_format() {
        let resp = success_response(Value::Number(1.into()), serde_json::json!({"ok": true}));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["ok"], true);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn test_method_not_found_response() {
        let resp = method_not_found(Value::Number(5.into()), "bogus/method");
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 5);
        assert_eq!(json["error"]["code"], -32601);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("bogus/method")
        );
    }

    #[test]
    fn test_parse_error_response() {
        let resp = parse_error();
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert!(json["id"].is_null());
        assert_eq!(json["error"]["code"], -32700);
    }

    #[test]
    fn test_config_update_path_env_override_and_default() {
        // Test env var override
        // SAFETY: test env var manipulation
        unsafe {
            std::env::set_var(CONFIG_UPDATE_PATH_ENV, "/tmp/test-par-term-update.json");
        }
        let path = config_update_path();
        assert_eq!(path, PathBuf::from("/tmp/test-par-term-update.json"));

        // Test default path (env var unset)
        // SAFETY: test env var manipulation
        unsafe {
            std::env::remove_var(CONFIG_UPDATE_PATH_ENV);
        }
        let path = config_update_path();
        let path_str = path.to_str().unwrap();
        assert!(
            path_str.contains("par-term"),
            "Expected path to contain 'par-term', got: {path_str}"
        );
        assert!(
            path_str.ends_with(CONFIG_UPDATE_FILENAME),
            "Expected path to end with '{CONFIG_UPDATE_FILENAME}', got: {path_str}"
        );
    }

    #[test]
    fn test_screenshot_paths_env_override_and_default() {
        // SAFETY: test env var manipulation
        unsafe {
            std::env::set_var(
                SCREENSHOT_REQUEST_PATH_ENV,
                "/tmp/test-par-term-shot-req.json",
            );
            std::env::set_var(
                SCREENSHOT_RESPONSE_PATH_ENV,
                "/tmp/test-par-term-shot-resp.json",
            );
        }
        assert_eq!(
            screenshot_request_path(),
            PathBuf::from("/tmp/test-par-term-shot-req.json")
        );
        assert_eq!(
            screenshot_response_path(),
            PathBuf::from("/tmp/test-par-term-shot-resp.json")
        );

        // SAFETY: test env var manipulation
        unsafe {
            std::env::remove_var(SCREENSHOT_REQUEST_PATH_ENV);
            std::env::remove_var(SCREENSHOT_RESPONSE_PATH_ENV);
        }
        assert!(
            screenshot_request_path()
                .to_string_lossy()
                .ends_with(SCREENSHOT_REQUEST_FILENAME)
        );
        assert!(
            screenshot_response_path()
                .to_string_lossy()
                .ends_with(SCREENSHOT_RESPONSE_FILENAME)
        );
    }

    #[test]
    fn test_image_tool_result_from_file_missing() {
        let result = image_tool_result_from_file(std::path::Path::new(
            "/tmp/does-not-exist-terminal-screenshot.png",
        ));
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn test_incoming_message_notification() {
        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap();
        assert!(msg.id.is_none());
        assert_eq!(msg.method.as_deref(), Some("notifications/initialized"));
    }

    #[test]
    fn test_incoming_message_request() {
        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
                .unwrap();
        assert!(msg.id.is_some());
        assert_eq!(msg.method.as_deref(), Some("initialize"));
    }
}
