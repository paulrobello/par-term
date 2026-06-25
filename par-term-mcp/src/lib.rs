//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Reads line-delimited JSON-RPC 2.0 from stdin and writes responses to stdout.
//! Exposes tools for par-term ACP integrations:
//! - `config_update`: writes configuration changes to a file for the main app
//!   to pick up
//! - `terminal_screenshot`: requests a live terminal screenshot from the app
//!   via a file-based IPC handshake (with an optional fallback image path for
//!   non-GUI test harnesses)
//! - `shader_diagnostics`: requests live shader state and last compile/reload
//!   errors from the running app via file-based IPC
//!
//! # Module layout
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 wire types, response helpers, and stdout framing
//! - [`ipc`] — IPC path resolution, atomic writes, and restricted-permission helpers
//! - [`tools`] — tool registration, descriptors, and dispatch
//! - [`tools::config_update`] — `config_update` tool handler
//! - [`tools::screenshot`] — `terminal_screenshot` tool handler
//! - [`tools::diagnostics`] — `shader_diagnostics` tool handler
//!
//! # SEC-006 / SEC-008: Trust Boundary — stdin/stdout IPC Channel
//!
//! This MCP server communicates over stdin and stdout using JSON-RPC 2.0.
//! Any process that can write to the MCP server's stdin can, by default,
//! invoke any tool (including `config_update`, which writes to the user's
//! configuration file on disk).
//!
//! **SEC-006 mitigation:** the server now requires a launch-time session
//! token in the `initialize` handshake. The token is either:
//! 1. Read from the [`MCP_AUTH_TOKEN_ENV`] env var (set by the spawner), or
//! 2. Generated at startup as a CSPRNG-backed `Uuid` and logged to stderr
//!    so the parent process (which owns stderr) can capture it.
//!
//! The client must echo the token back in `initialize` params at
//! `_meta.<AUTH_TOKEN_FIELD>`. Until a valid handshake completes, every
//! `tools/call` and `tools/list` request is rejected with a `-32001` error.
//!
//! **The stdin/stdout channel is still a trust boundary.** Only trusted MCP
//! client processes (i.e., ACP agents that par-term itself has spawned, with
//! the auth token plumbed through) should be connected to this server. Agent
//! TOML files (which define which agents are launched) are themselves a trust
//! boundary — only install agents from sources you trust.
//!
//! The file-based IPC paths used for screenshot and diagnostics requests use
//! restrictive permissions (0o600) to prevent unauthorized reads or writes.

pub mod ipc;
pub mod jsonrpc;
pub mod tools;

use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::sync::OnceLock;

use jsonrpc::{
    IncomingMessage, Response, RpcError, method_not_found, parse_error, send_response,
    success_response,
};
use tools::{handle_tools_call, handle_tools_list};
use uuid::Uuid;

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
/// Environment variable for shader diagnostics request IPC file path.
pub const SHADER_DIAGNOSTICS_REQUEST_PATH_ENV: &str = "PAR_TERM_SHADER_DIAGNOSTICS_REQUEST_PATH";
/// Environment variable for shader diagnostics response IPC file path.
pub const SHADER_DIAGNOSTICS_RESPONSE_PATH_ENV: &str = "PAR_TERM_SHADER_DIAGNOSTICS_RESPONSE_PATH";
/// Optional environment variable for a static fallback screenshot file path.
/// Used by the ACP harness to test the screenshot tool flow without a GUI.
pub const SCREENSHOT_FALLBACK_PATH_ENV: &str = "PAR_TERM_SCREENSHOT_FALLBACK_PATH";

/// Environment variable carrying the per-process MCP session auth token (SEC-006).
///
/// When par-term's ACP subsystem spawns this server it sets this env var to a
/// random per-process value (CSPRNG-backed) and passes the same value to the
/// spawned agent, which must echo it back in the `initialize` handshake. If the
/// env var is unset, the server generates a token itself and prints it to
/// stderr so the parent process (which owns stderr) can capture it.
pub const MCP_AUTH_TOKEN_ENV: &str = "PAR_TERM_MCP_AUTH_TOKEN";

/// Field name in the `initialize` params (`_meta.<field>`) that carries the
/// session auth token (SEC-006).
const AUTH_TOKEN_FIELD: &str = "parTermAuthToken";

/// Resolve the session auth token (SEC-006).
///
/// Reads [`MCP_AUTH_TOKEN_ENV`] first; if unset, generates a fresh CSPRNG-backed
/// `Uuid` and logs it to stderr so the parent process can capture and forward
/// it. Trims whitespace from the env-var value.
fn resolve_auth_token() -> String {
    if let Ok(t) = std::env::var(MCP_AUTH_TOKEN_ENV)
        && !t.trim().is_empty()
    {
        return t.trim().to_string();
    }
    let generated = Uuid::new_v4().to_string();
    eprintln!(
        "[mcp-server] no {MCP_AUTH_TOKEN_ENV} env var set; generated session auth token: {generated}"
    );
    generated
}

/// Constant-time string comparison to avoid timing side-channels on the token
/// check (SEC-006). The threat model is local-process access control; this is
/// defense-in-depth, not the primary gate.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Build the JSON-RPC `-32001` "auth required" error response used by the
/// SEC-006 dispatch gate.
fn auth_required_error(id: serde_json::Value, message: impl Into<String>) -> Response {
    Response {
        jsonrpc: "2.0",
        result: None,
        error: Some(RpcError {
            code: -32001,
            message: message.into(),
            data: None,
        }),
        id,
    }
}

/// Dispatch a JSON-RPC request with the SEC-006 authentication gate applied.
///
/// `initialize` must echo the session token in `params._meta.<AUTH_TOKEN_FIELD>`;
/// on success the `authenticated` flag is flipped and the server info is
/// returned. `tools/list` and `tools/call` are rejected with `-32001` until a
/// valid handshake has completed.
fn dispatch_authenticated(
    method: &str,
    id: serde_json::Value,
    params: Option<serde_json::Value>,
    expected_token: &str,
    authenticated: &mut bool,
) -> Response {
    match method {
        "initialize" => {
            let provided = params
                .as_ref()
                .and_then(|p| p.get("_meta"))
                .and_then(|m| m.get(AUTH_TOKEN_FIELD))
                .and_then(|v| v.as_str());
            if provided.is_some_and(|p| constant_time_eq(p, expected_token)) {
                *authenticated = true;
                success_response(id, handle_initialize())
            } else {
                auth_required_error(
                    id,
                    format!(
                        "Authentication failed: provide the correct session token in \
                         initialize params._meta.{AUTH_TOKEN_FIELD}"
                    ),
                )
            }
        }
        "tools/list" | "tools/call" if !*authenticated => auth_required_error(
            id,
            "Not authenticated: complete the initialize handshake (with the \
             session token) before invoking tools.",
        ),
        "tools/list" => success_response(id, handle_tools_list()),
        "tools/call" => success_response(id, handle_tools_call(params)),
        _ => method_not_found(id, method),
    }
}

/// Default config update filename (relative to config dir).
pub const CONFIG_UPDATE_FILENAME: &str = ".config-update.json";
/// Default screenshot request filename (relative to config dir).
pub const SCREENSHOT_REQUEST_FILENAME: &str = ".screenshot-request.json";
/// Default screenshot response filename (relative to config dir).
pub const SCREENSHOT_RESPONSE_FILENAME: &str = ".screenshot-response.json";
/// Default shader diagnostics request filename (relative to config dir).
pub const SHADER_DIAGNOSTICS_REQUEST_FILENAME: &str = ".shader-diagnostics-request.json";
/// Default shader diagnostics response filename (relative to config dir).
pub const SHADER_DIAGNOSTICS_RESPONSE_FILENAME: &str = ".shader-diagnostics-response.json";

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

/// Shader diagnostics request written by the MCP server for the GUI app to fulfill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDiagnosticsRequest {
    pub request_id: String,
}

/// Per-shader diagnostics included in [`ShaderDiagnosticsResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDiagnosticsEntry {
    pub shader: Option<String>,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub wgsl_path: Option<String>,
}

/// Live shader diagnostics returned by the GUI app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDiagnostics {
    pub background: ShaderDiagnosticsEntry,
    pub cursor: ShaderDiagnosticsEntry,
    pub shaders_dir: String,
    pub wrapped_glsl_path: String,
}

/// Shader diagnostics response written by the GUI app for the MCP server to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDiagnosticsResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub diagnostics: Option<ShaderDiagnostics>,
}

// Re-export IPC path helpers so callers don't need to name the submodule.
pub use ipc::{
    screenshot_request_path, screenshot_response_path, shader_diagnostics_request_path,
    shader_diagnostics_response_path,
};

/// Run the MCP server loop. Reads JSON-RPC messages from stdin until the
/// stream is closed or an I/O error occurs, then returns normally so that
/// callers can run destructors and exit cleanly.
pub fn run_mcp_server() {
    let version = get_app_version();
    let expected_token = resolve_auth_token();
    eprintln!("[mcp-server] Starting par-term MCP server v{version}");
    eprintln!(
        "[mcp-server] SEC-006: session auth enabled — client must send '{AUTH_TOKEN_FIELD}' in initialize params._meta"
    );

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = stdin.lock();
    let mut authenticated = false;

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

        // Dispatch with the SEC-006 authentication gate applied.
        let response =
            dispatch_authenticated(method, id, msg.params, &expected_token, &mut authenticated);

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
    use tools::diagnostics::diagnostics_tool_result;
    use tools::screenshot::image_tool_result_from_file;

    #[test]
    fn test_handle_initialize() {
        let result = handle_initialize();
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn test_constant_time_eq_matches() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn test_sec006_initialize_rejects_missing_token() {
        let mut authed = false;
        let params = serde_json::json!({});
        let resp = dispatch_authenticated(
            "initialize",
            serde_json::json!(1),
            Some(params),
            "secret-token",
            &mut authed,
        );
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, -32001);
        assert!(!authed, "auth flag must not flip on failed handshake");
    }

    #[test]
    fn test_sec006_initialize_rejects_wrong_token() {
        let mut authed = false;
        let params = serde_json::json!({"_meta": {AUTH_TOKEN_FIELD: "wrong"}});
        let resp = dispatch_authenticated(
            "initialize",
            serde_json::json!(1),
            Some(params),
            "secret-token",
            &mut authed,
        );
        assert_eq!(resp.error.unwrap().code, -32001);
        assert!(!authed);
    }

    #[test]
    fn test_sec006_initialize_accepts_correct_token() {
        let mut authed = false;
        let params = serde_json::json!({"_meta": {AUTH_TOKEN_FIELD: "secret-token"}});
        let resp = dispatch_authenticated(
            "initialize",
            serde_json::json!(1),
            Some(params),
            "secret-token",
            &mut authed,
        );
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["serverInfo"]["name"], SERVER_NAME);
        assert!(authed, "auth flag must flip after valid handshake");
    }

    #[test]
    fn test_sec006_tools_call_blocked_before_handshake() {
        let mut authed = false;
        let params = serde_json::json!({"name": "tools/list"});
        let resp = dispatch_authenticated(
            "tools/list",
            serde_json::json!(2),
            Some(params),
            "secret-token",
            &mut authed,
        );
        assert_eq!(resp.error.unwrap().code, -32001);
        assert!(!authed);
    }

    #[test]
    fn test_sec006_tools_call_allowed_after_handshake() {
        let mut authed = true; // already authenticated
        let params = serde_json::json!({"name": "tools/list"});
        let resp = dispatch_authenticated(
            "tools/list",
            serde_json::json!(3),
            Some(params),
            "secret-token",
            &mut authed,
        );
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["tools"].is_array());
    }

    #[test]
    fn test_handle_tools_list() {
        let result = handle_tools_list();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        let names: Vec<_> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"config_update"));
        assert!(names.contains(&"terminal_screenshot"));
        assert!(names.contains(&"shader_diagnostics"));
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
    fn test_shader_diagnostics_paths_env_override_and_default() {
        // SAFETY: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024 because
        // they are not thread-safe. The diagnostics env vars are unique to this test
        // and are removed before the test returns.
        unsafe {
            std::env::set_var(
                SHADER_DIAGNOSTICS_REQUEST_PATH_ENV,
                "/tmp/test-par-term-shader-diag-req.json",
            );
            std::env::set_var(
                SHADER_DIAGNOSTICS_RESPONSE_PATH_ENV,
                "/tmp/test-par-term-shader-diag-resp.json",
            );
        }
        assert_eq!(
            shader_diagnostics_request_path(),
            PathBuf::from("/tmp/test-par-term-shader-diag-req.json")
        );
        assert_eq!(
            shader_diagnostics_response_path(),
            PathBuf::from("/tmp/test-par-term-shader-diag-resp.json")
        );

        // SAFETY: see set_var comment above.
        unsafe {
            std::env::remove_var(SHADER_DIAGNOSTICS_REQUEST_PATH_ENV);
            std::env::remove_var(SHADER_DIAGNOSTICS_RESPONSE_PATH_ENV);
        }
        assert!(
            shader_diagnostics_request_path()
                .to_string_lossy()
                .ends_with(SHADER_DIAGNOSTICS_REQUEST_FILENAME)
        );
        assert!(
            shader_diagnostics_response_path()
                .to_string_lossy()
                .ends_with(SHADER_DIAGNOSTICS_RESPONSE_FILENAME)
        );
    }

    #[test]
    fn test_diagnostics_tool_result_includes_shader_errors_and_paths() {
        let response = ShaderDiagnosticsResponse {
            request_id: "req-1".to_string(),
            ok: true,
            error: None,
            diagnostics: Some(ShaderDiagnostics {
                background: ShaderDiagnosticsEntry {
                    shader: Some("bad.glsl".to_string()),
                    enabled: true,
                    last_error: Some("naga validation failed".to_string()),
                    wgsl_path: Some("/tmp/par_term_bad_shader.wgsl".to_string()),
                },
                cursor: ShaderDiagnosticsEntry {
                    shader: None,
                    enabled: false,
                    last_error: None,
                    wgsl_path: None,
                },
                shaders_dir: "/Users/example/.config/par-term/shaders".to_string(),
                wrapped_glsl_path: "/tmp/par_term_debug_wrapped.glsl".to_string(),
            }),
        };

        let result = diagnostics_tool_result(response);

        assert!(result.get("isError").is_none());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("bad.glsl"));
        assert!(text.contains("naga validation failed"));
        assert!(text.contains("/tmp/par_term_bad_shader.wgsl"));
        assert!(text.contains("shader_diagnostics"));
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
