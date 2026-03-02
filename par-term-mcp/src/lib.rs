//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Reads line-delimited JSON-RPC 2.0 from stdin and writes responses to stdout.
//! Exposes tools for par-term ACP integrations:
//! - `config_update`: writes configuration changes to a file for the main app
//!   to pick up
//! - `terminal_screenshot`: requests a live terminal screenshot from the app
//!   via a file-based IPC handshake (with an optional fallback image path for
//!   non-GUI test harnesses)
//!
//! # Module layout
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 wire types, response helpers, and stdout framing
//! - [`ipc`] — IPC path resolution, atomic writes, and restricted-permission helpers
//! - [`tools`] — tool registration, descriptors, and dispatch
//! - [`tools::config_update`] — `config_update` tool handler
//! - [`tools::screenshot`] — `terminal_screenshot` tool handler

pub mod ipc;
pub mod jsonrpc;
pub mod tools;

use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::sync::OnceLock;

use jsonrpc::{IncomingMessage, method_not_found, parse_error, send_response, success_response};
use tools::{handle_tools_call, handle_tools_list};

// ---------------------------------------------------------------------------
// Protocol constants (pub(crate) so submodules can access them)
// ---------------------------------------------------------------------------

/// MCP protocol version.
pub(crate) const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name reported during initialization.
pub(crate) const SERVER_NAME: &str = "par-term";

/// Application version set by the main crate.
/// Use `set_app_version()` to initialize this before calling `run_mcp_server()`.
static APP_VERSION: OnceLock<String> = OnceLock::new();

/// Set the application version (should be called from the main crate with
/// the root crate's `VERSION` constant before running the MCP server).
pub fn set_app_version(version: impl Into<String>) {
    let _ = APP_VERSION.set(version.into());
}

/// Get the application version, falling back to the crate version if not set.
pub(crate) fn get_app_version() -> &'static str {
    APP_VERSION
        .get()
        .map(|s| s.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Handle the `initialize` JSON-RPC request.
fn handle_initialize() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": get_app_version()
        }
    })
}

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

// Re-export IPC path helpers so callers don't need to name the submodule.
pub use ipc::{screenshot_request_path, screenshot_response_path};

/// Run the MCP server loop. Reads JSON-RPC messages from stdin until the
/// stream is closed or an I/O error occurs, then returns normally so that
/// callers can run destructors and exit cleanly.
pub fn run_mcp_server() {
    let version = get_app_version();
    eprintln!("[mcp-server] Starting par-term MCP server v{version}");

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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ipc::{config_update_path, set_ipc_file_permissions, write_json_atomic};
    use jsonrpc::{IncomingMessage, method_not_found, parse_error, success_response};
    use std::path::PathBuf;
    use tools::config_update::write_config_updates;
    use tools::screenshot::image_tool_result_from_file;

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
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&update_path).unwrap()).unwrap();
        assert_eq!(written["font_size"], 14.0);
        assert_eq!(written["custom_shader_enabled"], true);
    }

    #[test]
    fn test_success_response_format() {
        let resp = success_response(
            serde_json::Value::Number(1.into()),
            serde_json::json!({"ok": true}),
        );
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["ok"], true);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn test_method_not_found_response() {
        let resp = method_not_found(serde_json::Value::Number(5.into()), "bogus/method");
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
        //
        // SAFETY: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024 because
        // they are not thread-safe. This is acceptable in test code because:
        // (a) `CONFIG_UPDATE_PATH_ENV` is a unique, test-specific environment variable
        //     that is not read by any other concurrently-executing test in this crate,
        // (b) the variable is unset again at the end of this test body, and
        // (c) this code is only compiled in `#[cfg(test)]` and never runs in production.
        unsafe {
            std::env::set_var(CONFIG_UPDATE_PATH_ENV, "/tmp/test-par-term-update.json");
        }
        let path = config_update_path();
        assert_eq!(path, PathBuf::from("/tmp/test-par-term-update.json"));

        // Test default path (env var unset)
        // SAFETY: see set_var comment above.
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
        // SAFETY: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024 because
        // they are not thread-safe. This is acceptable here because:
        // (a) `SCREENSHOT_REQUEST_PATH_ENV` and `SCREENSHOT_RESPONSE_PATH_ENV` are
        //     unique, test-specific keys not shared with other concurrently-running tests,
        // (b) both variables are unset again later in this same test body, and
        // (c) this block is only compiled in `#[cfg(test)]` and never runs in production.
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

        // SAFETY: see set_var comment above — same reasoning applies to remove_var.
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

    #[cfg(unix)]
    #[test]
    fn test_set_ipc_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc-test.json");
        std::fs::write(&path, "{}").unwrap();

        set_ipc_file_permissions(&path).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "IPC file should have mode 0o600, got {mode:#o}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_config_updates_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let update_path = dir.path().join("config-update.json");

        let updates = serde_json::json!({"font_size": 14.0});
        let result = write_config_updates(&updates, &update_path);
        assert!(result.get("isError").is_none(), "Expected success result");

        let metadata = std::fs::metadata(&update_path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Config update IPC file should have mode 0o600, got {mode:#o}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_json_atomic_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atomic-test.json");

        let payload = serde_json::json!({"request_id": "test-123"});
        write_json_atomic(&payload, &path).unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Atomically written IPC file should have mode 0o600, got {mode:#o}"
        );
    }
}
