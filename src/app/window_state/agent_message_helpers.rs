//! Private helpers for ACP agent message processing.
//!
//! Contains stateless utility functions used by `process_agent_messages_tick`:
//! - Sensitive-key detection for command redaction
//! - Command-line argument sanitisation (redacts secrets before sending auto-context)
//! - Tool-call name extraction for the terminal-screenshot permission check

const AUTO_CONTEXT_MAX_COMMAND_LEN: usize = 400;

/// Returns `true` if the flag/env-var name looks like it carries a sensitive value
/// (password, token, API key, auth credential, session cookie, etc.).
pub(super) fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "pass",
        "password",
        "token",
        "secret",
        "key",
        "apikey",
        "api_key",
        "auth",
        "credential",
        "session",
        "cookie",
    ];
    MARKERS.iter().any(|marker| key.contains(marker))
}

/// Sanitise a shell command before sending it as auto-context to the agent.
///
/// Tokens whose flag name or `key=value` key matches a sensitive marker are
/// replaced with `[REDACTED]`.  Returns the sanitised string and a boolean
/// indicating whether any redaction occurred.
pub(super) fn redact_auto_context_command(command: &str) -> (String, bool) {
    let mut redacted = false;
    let mut redact_next = false;
    let mut out: Vec<String> = Vec::new();

    for token in command.split_whitespace() {
        if redact_next {
            out.push("[REDACTED]".to_string());
            redacted = true;
            redact_next = false;
            continue;
        }

        let cleaned = token.trim_matches(|c| c == '"' || c == '\'');

        if let Some(flag) = cleaned.strip_prefix("--") {
            if let Some((name, _value)) = flag.split_once('=')
                && is_sensitive_key(name)
            {
                let prefix = token.split_once('=').map(|(p, _)| p).unwrap_or(token);
                out.push(format!("{prefix}=[REDACTED]"));
                redacted = true;
                continue;
            }
            if is_sensitive_key(flag) {
                out.push(token.to_string());
                redact_next = true;
                continue;
            }
        }

        if let Some((name, _value)) = cleaned.split_once('=')
            && is_sensitive_key(name)
        {
            let prefix = token.split_once('=').map(|(p, _)| p).unwrap_or(token);
            out.push(format!("{prefix}=[REDACTED]"));
            redacted = true;
            continue;
        }

        out.push(token.to_string());
    }

    let mut sanitized = out.join(" ");
    if sanitized.chars().count() > AUTO_CONTEXT_MAX_COMMAND_LEN {
        sanitized = sanitized
            .chars()
            .take(AUTO_CONTEXT_MAX_COMMAND_LEN)
            .collect();
        sanitized.push_str("...[truncated]");
    }
    (sanitized, redacted)
}

/// Returns `true` when `tool_call` refers to the `terminal_screenshot` permission tool.
///
/// Checks multiple JSON key paths because different ACP backends serialise the
/// tool name under different keys.
pub(super) fn is_terminal_screenshot_permission_tool(tool_call: &serde_json::Value) -> bool {
    let tool_name = tool_call
        .get("kind")
        .and_then(|v| v.as_str())
        .or_else(|| tool_call.get("name").and_then(|v| v.as_str()))
        .or_else(|| tool_call.get("toolName").and_then(|v| v.as_str()))
        .or_else(|| {
            tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .and_then(|t| t.split_whitespace().next())
        })
        .unwrap_or("");
    let lower = tool_name.to_ascii_lowercase();
    lower == "terminal_screenshot" || lower.contains("par-term-config__terminal_screenshot")
}
