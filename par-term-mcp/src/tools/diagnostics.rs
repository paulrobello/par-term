//! Handler for the `shader_diagnostics` MCP tool.
//!
//! Requests live shader state and last compile/reload errors from the running
//! par-term app via a file-based IPC handshake.

use crate::ipc::{
    open_restricted_write, shader_diagnostics_request_path, shader_diagnostics_response_path,
    try_read_shader_diagnostics_response, write_json_atomic,
};
use crate::{ShaderDiagnosticsRequest, ShaderDiagnosticsResponse};
use serde_json::Value;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Execute the `shader_diagnostics` tool.
pub fn handle_shader_diagnostics(params: &Value) -> Value {
    // MCP tools/call always includes "arguments", but this tool takes none.
    if let Some(arguments) = params.get("arguments")
        && !arguments.is_object()
    {
        return super::tool_error("'arguments' must be an object");
    }

    let request_path = shader_diagnostics_request_path();
    let response_path = shader_diagnostics_response_path();

    let request_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let request = ShaderDiagnosticsRequest {
        request_id: request_id.clone(),
    };

    if let Err(e) = write_json_atomic(&request, &request_path) {
        return super::tool_error(&format!(
            "Failed to write shader diagnostics request {}: {e}",
            request_path.display()
        ));
    }

    let timeout = Duration::from_secs(15);
    let poll_interval = Duration::from_millis(100);
    let start = Instant::now();
    while start.elapsed() < timeout {
        match try_read_shader_diagnostics_response(&response_path) {
            Ok(Some(response)) if response.request_id == request_id => {
                let _ = open_restricted_write(&response_path);
                return diagnostics_tool_result(response);
            }
            Ok(Some(_other_response)) => {
                // Stale response for a different request ID; keep waiting.
            }
            Ok(None) => {}
            Err(e) => {
                return super::tool_error(&format!(
                    "Failed to read shader diagnostics response {}: {e}",
                    response_path.display()
                ));
            }
        }
        std::thread::sleep(poll_interval);
    }

    super::tool_error("Timed out waiting for par-term app shader diagnostics response")
}

/// Build an MCP text result from a shader diagnostics response.
pub fn diagnostics_tool_result(response: ShaderDiagnosticsResponse) -> Value {
    if !response.ok {
        return super::tool_error(
            response
                .error
                .as_deref()
                .unwrap_or("Shader diagnostics request failed"),
        );
    }

    let Some(diagnostics) = response.diagnostics else {
        return super::tool_error("Shader diagnostics response missing diagnostics data");
    };

    let text = format!(
        "shader_diagnostics\n\
Background shader:\n\
- enabled: {}\n\
- shader: {}\n\
- last_error: {}\n\
- wgsl_path: {}\n\
Cursor shader:\n\
- enabled: {}\n\
- shader: {}\n\
- last_error: {}\n\
- wgsl_path: {}\n\
Debug paths:\n\
- shaders_dir: {}\n\
- wrapped_glsl_path: {}",
        diagnostics.background.enabled,
        diagnostics.background.shader.as_deref().unwrap_or("<none>"),
        diagnostics
            .background
            .last_error
            .as_deref()
            .unwrap_or("<none>"),
        diagnostics
            .background
            .wgsl_path
            .as_deref()
            .unwrap_or("<none>"),
        diagnostics.cursor.enabled,
        diagnostics.cursor.shader.as_deref().unwrap_or("<none>"),
        diagnostics.cursor.last_error.as_deref().unwrap_or("<none>"),
        diagnostics.cursor.wgsl_path.as_deref().unwrap_or("<none>"),
        diagnostics.shaders_dir,
        diagnostics.wrapped_glsl_path,
    );

    serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ]
    })
}
