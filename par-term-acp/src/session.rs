//! ACP session lifecycle helpers.
//!
//! This module contains helpers for building the `session/new` request
//! parameters, including MCP server injection and Claude-wrapper metadata.
//! The actual async handshake lives in [`Agent::connect`] in `agent.rs`;
//! these helpers extract the stateless parts so `connect` stays readable.

use std::path::Path;

use super::agents::AgentConfig;

fn shell_quote_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn is_codex_agent(agent_config: &AgentConfig, run_command: &str) -> bool {
    agent_config.identity == "openai.com" || run_command.contains("codex-acp")
}

fn is_gemini_agent(agent_config: &AgentConfig, run_command: &str) -> bool {
    agent_config.identity == "geminicli.com"
        || run_command
            .split_whitespace()
            .next()
            .is_some_and(|binary| binary == "gemini" || binary.ends_with("/gemini"))
}

/// Add known-agent extra-root flags to an ACP subprocess run command.
pub fn adapt_run_command_for_extra_roots(
    agent_config: &AgentConfig,
    run_command: &str,
    extra_roots: &[String],
) -> String {
    if extra_roots.is_empty() {
        return run_command.to_string();
    }

    if is_codex_agent(agent_config, run_command) {
        let roots_array = extra_roots
            .iter()
            .map(|root| serde_json::to_string(root).expect("string serialization cannot fail"))
            .collect::<Vec<_>>()
            .join(",");
        let sandbox_mode = shell_quote_arg("sandbox_mode=\"workspace-write\"");
        let writable_roots = shell_quote_arg(&format!(
            "sandbox_workspace_write.writable_roots=[{roots_array}]"
        ));
        return format!("{run_command} -c {sandbox_mode} -c {writable_roots}");
    }

    if is_gemini_agent(agent_config, run_command) {
        let include_dirs = shell_quote_arg(&extra_roots.join(","));
        return format!("{run_command} --include-directories {include_dirs}");
    }

    run_command.to_string()
}

/// Build optional session metadata, including generic extra roots.
pub fn build_session_meta(
    agent_config: &AgentConfig,
    run_command_template: &str,
    extra_roots: &[String],
) -> Option<serde_json::Value> {
    let mut meta = build_claude_session_meta(agent_config, run_command_template)
        .unwrap_or_else(|| serde_json::json!({}));

    if !extra_roots.is_empty()
        && let Some(object) = meta.as_object_mut()
    {
        object.insert(
            "additionalRoots".to_string(),
            serde_json::json!(extra_roots),
        );
    }

    if meta.as_object().is_some_and(|object| !object.is_empty()) {
        Some(meta)
    } else {
        None
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn agent(identity: &str) -> AgentConfig {
        AgentConfig {
            identity: identity.to_string(),
            name: identity.to_string(),
            short_name: identity.to_string(),
            protocol: "acp".to_string(),
            r#type: "coding".to_string(),
            active: None,
            run_command: HashMap::from([("*".to_string(), "agent-acp".to_string())]),
            env: HashMap::new(),
            install_command: None,
            actions: HashMap::new(),
            connector_installed: false,
        }
    }

    #[test]
    fn session_meta_includes_additional_roots_for_generic_agents() {
        let meta = build_session_meta(
            &agent("generic.example"),
            "agent-acp",
            &["/workspace/shared".to_string(), "/tmp/shaders".to_string()],
        )
        .expect("meta");

        assert_eq!(
            meta.get("additionalRoots"),
            Some(&serde_json::json!(["/workspace/shared", "/tmp/shaders"]))
        );
    }

    #[test]
    fn session_meta_merges_claude_options_and_additional_roots() {
        let meta = build_session_meta(
            &agent("claude.com"),
            "claude-agent-acp",
            &["/workspace/shared".to_string()],
        )
        .expect("meta");

        assert_eq!(
            meta.pointer("/claudeCode/options/settingSources"),
            Some(&serde_json::json!(["user"]))
        );
        assert_eq!(
            meta.get("additionalRoots"),
            Some(&serde_json::json!(["/workspace/shared"]))
        );
    }

    #[test]
    fn codex_run_command_gets_writable_roots_config() {
        let command = adapt_run_command_for_extra_roots(
            &agent("openai.com"),
            "npx @zed-industries/codex-acp",
            &["/workspace/shared".to_string(), "/tmp/shaders".to_string()],
        );

        assert!(command.contains("-c 'sandbox_mode=\"workspace-write\"'"));
        assert!(command.contains(
            "-c 'sandbox_workspace_write.writable_roots=[\"/workspace/shared\",\"/tmp/shaders\"]'"
        ));
    }

    #[test]
    fn gemini_run_command_gets_include_directories() {
        let command = adapt_run_command_for_extra_roots(
            &agent("geminicli.com"),
            "gemini --experimental-acp",
            &["/workspace/shared".to_string(), "/tmp/shaders".to_string()],
        );

        assert_eq!(
            command,
            "gemini --experimental-acp --include-directories '/workspace/shared,/tmp/shaders'"
        );
    }
}
