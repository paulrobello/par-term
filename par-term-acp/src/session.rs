//! ACP session lifecycle helpers.
//!
//! This module contains helpers for building the `session/new` request
//! parameters, including MCP server injection and Claude-wrapper metadata.
//! The actual async handshake lives in [`Agent::connect`] in `agent.rs`;
//! these helpers extract the stateless parts so `connect` stays readable.

use std::path::Path;

use super::agents::AgentConfig;

/// Build the MCP server descriptor for the embedded `par-term-config` server.
///
/// The MCP server exposes `config_update` and `terminal_screenshot` tools so
/// the agent can modify settings and capture screenshots without editing
/// `config.yaml` directly.
///
/// # Arguments
/// * `config_dir` - Path to the par-term configuration directory.
/// * `agent_config` - The agent configuration (for optional screenshot fallback env var).
/// * `mcp_server_bin` - Path to the par-term binary that acts as MCP server.
pub fn build_mcp_server_descriptor(
    config_dir: &Path,
    agent_config: &AgentConfig,
    mcp_server_bin: &Path,
) -> serde_json::Value {
    let config_update_path = config_dir.join(".config-update.json");
    let screenshot_request_path = config_dir.join(".screenshot-request.json");
    let screenshot_response_path = config_dir.join(".screenshot-response.json");

    let mut mcp_env = vec![
        serde_json::json!({
            "name": "PAR_TERM_CONFIG_UPDATE_PATH",
            "value": config_update_path.to_string_lossy(),
        }),
        serde_json::json!({
            "name": "PAR_TERM_SCREENSHOT_REQUEST_PATH",
            "value": screenshot_request_path.to_string_lossy(),
        }),
        serde_json::json!({
            "name": "PAR_TERM_SCREENSHOT_RESPONSE_PATH",
            "value": screenshot_response_path.to_string_lossy(),
        }),
    ];

    if let Some(fallback_path) = agent_config
        .env
        .get("PAR_TERM_SCREENSHOT_FALLBACK_PATH")
        .filter(|v| !v.trim().is_empty())
    {
        mcp_env.push(serde_json::json!({
            "name": "PAR_TERM_SCREENSHOT_FALLBACK_PATH",
            "value": fallback_path.trim(),
        }));
    }

    serde_json::json!({
        "name": "par-term-config",
        "command": mcp_server_bin.to_string_lossy(),
        "args": ["mcp-server"],
        "env": mcp_env,
    })
}

/// Build optional Claude-wrapper session metadata.
///
/// Claude ACP wrappers support extra session metadata that prevents local/project
/// Claude settings from unexpectedly overriding the intended model/backend for
/// custom Ollama sessions. This function returns `None` for non-Claude wrappers.
///
/// # Arguments
/// * `agent_config` - The agent configuration.
/// * `run_command_template` - The resolved run command string (used to detect
///   claude wrapper binaries).
pub fn build_claude_session_meta(
    agent_config: &AgentConfig,
    run_command_template: &str,
) -> Option<serde_json::Value> {
    let is_claude_wrapper = agent_config.identity.contains("claude")
        || run_command_template.contains("claude-agent-acp")
        || run_command_template.contains("claude-code-acp");

    if !is_claude_wrapper {
        return None;
    }

    let mut runtime_note = "Runtime note: You are running through par-term ACP. Do not call Skill, Task, or TodoWrite tools unless they are explicitly available and working in this host. Do not switch into plan mode for direct executable requests (file edits, shader creation, config changes), and do not call EnterPlanMode/Todo unless explicitly required and available. There is no generic `Skill file-write` helper here; use normal file read/write/edit tools directly. If a Read call fails because the target is a directory, do not retry Read on that directory; use a listing/search tool or write the known target file path directly. When using Write, use exact parameter names like `file_path` and `content` (not `filepath`). If a tool call fails, correct the parameters and retry the same task instead of switching to an unrelated example/file. For multi-step requests, complete the full workflow before declaring success (e.g. shader file write + config_update activation). For visual shader/debug issues, you can request a screenshot using the `terminal_screenshot` MCP tool (user permission may be required). If planning/task tools are unavailable, continue with an inline plain-text checklist instead of failing. Do not emit XML-style function tags like <function=...> in normal chat output.".to_string();

    if let Some(model) = agent_config
        .env
        .get("ANTHROPIC_MODEL")
        .filter(|v| !v.trim().is_empty())
    {
        runtime_note.push_str(&format!(" Configured model hint: `{}`.", model.trim()));
    }

    if let Some(base_url) = agent_config
        .env
        .get("ANTHROPIC_BASE_URL")
        .filter(|v| !v.trim().is_empty())
    {
        runtime_note.push_str(&format!(" Configured backend hint: `{}`.", base_url.trim()));
    }

    Some(serde_json::json!({
        "claudeCode": {
            "options": {
                "settingSources": ["user"]
            }
        },
        "systemPrompt": {
            "append": runtime_note
        }
    }))
}
