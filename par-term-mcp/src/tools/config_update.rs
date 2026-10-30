//! Handler for the `config_update` MCP tool.
//!
//! Writes a JSON object of config key-value pairs to the IPC config-update
//! file for the main par-term app to pick up.

use crate::ipc::{config_update_path, write_json_atomic};
use serde_json::Value;

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

/// Allowlisted keys whose value is a shader file path (SEC-003).
const SHADER_PATH_KEYS: &[&str] = &["custom_shader", "cursor_shader"];

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
/// Rejects keys outside the allowlist and shader paths outside the shaders
/// directory before any disk I/O, then delegates to
/// [`write_json_atomic`](crate::ipc::write_json_atomic). Every failure is
/// returned as an MCP tool-error result, never a panic.
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

    // SEC-003: a shader path value is GPU code the renderer compiles every
    // frame, so it must name an existing file inside the shaders directory —
    // never e.g. a GLSL file the agent wrote to the auto-approved temp dir.
    // The app re-checks on apply (`agent_config.rs`); this gives the agent a
    // clear error instead of a reported success the app then refuses.
    for key in SHADER_PATH_KEYS {
        if let Some(name) = updates_obj.get(*key).and_then(Value::as_str)
            && let Err(e) = par_term_config::Config::validate_agent_shader_path(name)
        {
            return super::tool_error(&format!("Rejected {key}: {e}"));
        }
    }

    // Atomic, owner-only (0o600 on Unix) write: temp file, then rename.
    if let Err(e) = write_json_atomic(updates, path) {
        return super::tool_error(&format!("Failed to write config update: {e}"));
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
