//! Output formatting and transcript helpers for the ACP harness binary.
//!
//! Provides utilities for printing chat messages, agent status, updates,
//! and formatted JSON to the harness console and transcript file.

use par_term_acp::{AgentConfig, AgentStatus};

use crate::ai_inspector::chat::ChatMessage;
use crate::ai_inspector::chat::ChatState;

/// Print a list of available ACP agents, marking the currently selected one.
pub fn print_agents(agents: &[AgentConfig], selected: &str) {
    par_term_acp::harness::println_tee(format_args!("Available ACP agents:"));
    for agent in agents {
        let marker = if agent.identity == selected { "*" } else { " " };
        let cmd = agent.run_command_for_platform().unwrap_or("<none>");
        par_term_acp::harness::println_tee(format_args!(
            "{} {} ({}) [{}] cmd={} installed={}",
            marker, agent.name, agent.identity, agent.short_name, cmd, agent.connector_installed
        ));
    }
}

/// Print all chat messages starting from `from` index (or all if `None`).
pub fn print_new_chat_messages(chat: &ChatState, from: Option<usize>) {
    let start = from.unwrap_or(0).min(chat.messages.len());
    for msg in &chat.messages[start..] {
        match msg {
            ChatMessage::User { text, pending } => {
                par_term_acp::harness::println_tee(format_args!(
                    "[chat:user{}] {}",
                    if *pending { ":queued" } else { "" },
                    text.replace('\n', " ")
                ));
            }
            ChatMessage::Agent(text) => {
                par_term_acp::harness::println_tee(format_args!("[chat:agent]\n{}\n", text));
            }
            ChatMessage::Thinking(text) => {
                par_term_acp::harness::println_tee(format_args!(
                    "[chat:thinking] {}",
                    text.replace('\n', " ")
                ));
            }
            ChatMessage::ToolCall {
                title,
                kind,
                status,
                ..
            } => {
                par_term_acp::harness::println_tee(format_args!(
                    "[chat:tool] {} kind={} status={}",
                    title, kind, status
                ));
            }
            ChatMessage::CommandSuggestion(cmd) => {
                par_term_acp::harness::println_tee(format_args!("[chat:cmd] {}", cmd));
            }
            ChatMessage::Permission {
                request_id,
                description,
                resolved,
                ..
            } => {
                par_term_acp::harness::println_tee(format_args!(
                    "[chat:permission] id={} resolved={} {}",
                    request_id, resolved, description
                ));
            }
            ChatMessage::AutoApproved(desc) => {
                par_term_acp::harness::println_tee(format_args!("[chat:auto-approved] {}", desc));
            }
            ChatMessage::System(text) => {
                par_term_acp::harness::println_tee(format_args!("[chat:system] {}", text));
            }
        }
    }
}

/// Format an `AgentStatus` value for display.
pub fn format_status(status: &AgentStatus) -> String {
    match status {
        AgentStatus::Disconnected => "Disconnected".to_string(),
        AgentStatus::Connecting => "Connecting".to_string(),
        AgentStatus::Connected => "Connected".to_string(),
        AgentStatus::Error(e) => format!("Error: {e}"),
    }
}

/// Print a map of config update key/value pairs, sorted by key.
pub fn print_updates(updates: &std::collections::HashMap<String, serde_json::Value>) {
    let mut keys: Vec<_> = updates.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = updates.get(key) {
            par_term_acp::harness::println_tee(format_args!("  - {} = {}", key, value));
        }
    }
}

/// Truncate a JSON value to at most 500 characters for console display.
pub fn truncate_json(v: &serde_json::Value) -> String {
    let s = v.to_string();
    if s.len() > 500 {
        format!("{}...", &s[..500])
    } else {
        s
    }
}
