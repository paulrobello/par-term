//! Handler for the `config_update` MCP tool.
//!
//! Writes a JSON object of config key-value pairs to the IPC config-update
//! file for the main par-term app to pick up.

use crate::ipc::{config_update_path, open_restricted_write};
use serde_json::Value;
use std::io::Write;

/// Config keys that the MCP `config_update` tool is allowed to write (SEC-005).
///
/// This is an explicit allowlist. The original implementation wrote caller
/// keys verbatim, which let any MCP client flip security-sensitive settings
/// such as `bypassPermissions`, `permissions`, `automation`, or `triggers`.
/// Only cosmetic / rendering keys are exposed here. The list mirrors the
/// `config_update` tool descriptor in `tools/mod.rs`.
const ALLOWED_CONFIG_KEYS: &[&str] = &[
    // Background shader
    "custom_shader",
    "custom_shader_enabled",
    "custom_shader_animation",
    "custom_shader_animation_speed",
    "custom_shader_brightness",
    "custom_shader_text_opacity",
    "custom_shader_full_content",
    // Cursor shader
    "cursor_shader",
    "cursor_shader_enabled",
    "cursor_shader_animation",
    "cursor_shader_animation_speed",
    "cursor_shader_glow_radius",
    "cursor_shader_glow_intensity",
    "cursor_shader_trail_duration",
    "cursor_shader_hides_cursor",
    // Window / font
    "window_opacity",
    "font_size",
];

/// Execute the `config_update` tool.
pub fn handle_config_update(params: &Value) -> Value {
    let arguments = match params.get("arguments") {
        Some(args) => args,
        None => {
            return super::tool_error("Missing 'arguments' in tools/call params");
        }
    };

    let updates = match arguments.get("updates") {
        Some(u) if u.is_object() => u,
        Some(_) => {
            return super::tool_error("'updates' must be a JSON object");
        }
        None => {
            return super::tool_error("Missing 'updates' in tool arguments");
        }
    };

    let path = config_update_path();
    write_config_updates(updates, &path)
}

/// Write config updates to the specified path atomically.
///
/// Creates parent directories if needed, writes to a temp file, then renames.
pub fn write_config_updates(updates: &Value, path: &std::path::Path) -> Value {
    // SEC-005: enforce an explicit key allowlist BEFORE any disk I/O. The
    // original implementation wrote caller-supplied keys verbatim, so any
    // local MCP client could flip security-sensitive settings like
    // `bypassPermissions` or `permissions`. Unknown keys (which includes all
    // security-sensitive ones) are rejected with the list of allowed keys.
    let updates_obj = match updates.as_object() {
        Some(o) => o,
        None => return super::tool_error("'updates' must be a JSON object"),
    };
    let rejected: Vec<&str> = updates_obj
        .keys()
        .filter(|k| !ALLOWED_CONFIG_KEYS.contains(&k.as_str()))
        .map(String::as_str)
        .collect();
    if !rejected.is_empty() {
        return super::tool_error(&format!(
            "Rejected unknown config key(s): [{}]. Allowed keys: [{}]",
            rejected.join(", "),
            ALLOWED_CONFIG_KEYS.join(", ")
        ));
    }

    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return super::tool_error(&format!(
            "Failed to create config directory {}: {e}",
            parent.display()
        ));
    }

    // Atomic write: write to temp file, then rename
    let temp_path = path.with_extension("json.tmp");

    let json_bytes = match serde_json::to_vec_pretty(updates) {
        Ok(bytes) => bytes,
        Err(e) => {
            return super::tool_error(&format!("Failed to serialize updates: {e}"));
        }
    };

    // Write temp file with restricted permissions from creation (0o600 on Unix)
    match open_restricted_write(&temp_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(&json_bytes) {
                return super::tool_error(&format!(
                    "Failed to write temp file {}: {e}",
                    temp_path.display()
                ));
            }
        }
        Err(e) => {
            return super::tool_error(&format!(
                "Failed to create temp file {}: {e}",
                temp_path.display()
            ));
        }
    }

    if let Err(e) = std::fs::rename(&temp_path, path) {
        // Clean up temp file on rename failure
        let _ = std::fs::remove_file(&temp_path);
        return super::tool_error(&format!(
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
