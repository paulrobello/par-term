//! Handler for the `terminal_screenshot` MCP tool.
//!
//! Requests a live terminal screenshot from the running par-term app via a
//! file-based IPC handshake, with an optional static fallback image path for
//! non-GUI test harnesses.

use crate::ipc::{
    open_restricted_write, screenshot_request_path, screenshot_response_path,
    try_read_screenshot_response, write_json_atomic,
};
use crate::{SCREENSHOT_FALLBACK_PATH_ENV, TerminalScreenshotRequest};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Execute the `terminal_screenshot` tool.
pub fn handle_terminal_screenshot(params: &Value) -> Value {
    // MCP tools/call always includes "arguments", but this tool takes none.
    if let Some(arguments) = params.get("arguments")
        && !arguments.is_object()
    {
        return super::tool_error("'arguments' must be an object");
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
        return super::tool_error(&format!(
            "Failed to write screenshot request {}: {e}",
            request_path.display()
        ));
    }

    let timeout = Duration::from_secs(15);
    let poll_interval = Duration::from_millis(100);
    let start = Instant::now();
    while start.elapsed() < timeout {
        match try_read_screenshot_response(&response_path) {
            Ok(Some(response)) if response.request_id == request_id => {
                // Clear response file after consuming; use restricted permissions
                // from creation (0o600 on Unix) to avoid a world-readable race.
                let _ = open_restricted_write(&response_path);
                if !response.ok {
                    return super::tool_error(
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
                    _ => return super::tool_error("Screenshot response missing image data"),
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
                return super::tool_error(&format!(
                    "Failed to read screenshot response {}: {e}",
                    response_path.display()
                ));
            }
        }
        std::thread::sleep(poll_interval);
    }

    super::tool_error("Timed out waiting for par-term app screenshot response")
}

/// Build an MCP image tool result from an existing image file.
pub fn image_tool_result_from_file(path: &std::path::Path) -> Value {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return super::tool_error(&format!(
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
