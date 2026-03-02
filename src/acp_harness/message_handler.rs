//! Agent message dispatch for the ACP harness binary.
//!
//! Provides [`handle_agent_message`] — the central handler called for every
//! [`AgentMessage`] received from the ACP event loop — along with helper
//! predicates and the [`apply_updates_to_config`] utility.

use std::collections::HashMap;

use par_term_acp::harness::HarnessEventFlags;
use par_term_acp::{Agent, AgentMessage};
use par_term_config::Config;

use crate::acp_harness::harness_output::{
    format_status, print_new_chat_messages, print_updates, truncate_json,
};
use crate::ai_inspector::chat::{ChatMessage, ChatState};

macro_rules! println {
    () => {
        par_term_acp::harness::println_tee(format_args!(""))
    };
    ($($arg:tt)*) => {
        par_term_acp::harness::println_tee(format_args!($($arg)*))
    };
}

/// Returns `true` when `title` identifies a `config_update` tool call.
pub fn is_config_update_tool(title: &str) -> bool {
    title.to_ascii_lowercase().contains("config_update")
}

/// Returns `true` when a Skill or Write tool call has failed.
pub fn is_failed_tool(title: &str, status: &str) -> bool {
    let title_l = title.to_ascii_lowercase();
    let status_l = status.to_ascii_lowercase();
    status_l.contains("fail") && (title_l.contains("skill") || title_l.contains("write"))
}

/// Apply a map of key/value updates to `config` and persist it to disk.
pub fn apply_updates_to_config(
    config: &mut Config,
    updates: &HashMap<String, serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut root = serde_json::to_value(&*config)?;
    let obj = root
        .as_object_mut()
        .ok_or("Serialized config is not a JSON object")?;
    for (k, v) in updates {
        obj.insert(k.clone(), v.clone());
    }
    let mut new_config: Config = serde_json::from_value(root)?;
    new_config.generate_snippet_action_keybindings();
    new_config.save()?;
    *config = new_config;
    println!(
        "[config_update] applied to {}",
        Config::config_path().display()
    );
    Ok(())
}

/// Dispatch a single [`AgentMessage`] from the ACP event loop.
///
/// Updates `chat`, `event_flags`, handles permission responses, and applies
/// config updates when `apply_config_updates` is `true`.
pub async fn handle_agent_message(
    agent: &Agent,
    config: &mut Config,
    chat: &mut ChatState,
    event_flags: &mut HarnessEventFlags,
    auto_approve: bool,
    apply_config_updates: bool,
    msg: AgentMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match msg {
        AgentMessage::StatusChanged(status) => {
            println!("[status] {}", format_status(&status));
        }
        AgentMessage::SessionUpdate(update) => {
            // Track failures before `ChatState` potentially coalesces messages.
            match &update {
                par_term_acp::SessionUpdate::ToolCall(info) => {
                    println!(
                        "[tool] {} kind={} status={}",
                        info.title, info.kind, info.status
                    );
                    if is_config_update_tool(info.title.as_str()) {
                        event_flags.saw_config_update = true;
                        if let Some(content) = &info.content {
                            println!("[tool:config_update] content={}", truncate_json(content));
                        }
                    }
                    if is_failed_tool(info.title.as_str(), info.status.as_str()) {
                        event_flags.saw_failed_tool_since_prompt = true;
                        event_flags.saw_any_failed_tool = true;
                    }
                }
                par_term_acp::SessionUpdate::ToolCallUpdate(info) => {
                    if let Some(status) = &info.status {
                        println!(
                            "[tool-update] id={} status={} title={}",
                            info.tool_call_id,
                            status,
                            info.title.clone().unwrap_or_default()
                        );
                        let title = info.title.as_deref().unwrap_or("");
                        let status_l = status.to_ascii_lowercase();
                        if is_config_update_tool(title)
                            && !(status_l.contains("fail") || status_l.contains("error"))
                        {
                            event_flags.saw_config_update = true;
                        }
                        let failed_without_title = title.is_empty()
                            && (status_l.contains("fail") || status_l.contains("error"));
                        if failed_without_title || is_failed_tool(title, status) {
                            event_flags.saw_failed_tool_since_prompt = true;
                            event_flags.saw_any_failed_tool = true;
                        }
                    }
                }
                par_term_acp::SessionUpdate::Plan(info) => {
                    println!("[plan] {} step(s)", info.entries.len());
                    for entry in &info.entries {
                        println!("  - [{}] {}", entry.status, entry.content);
                    }
                }
                par_term_acp::SessionUpdate::CurrentModeUpdate { mode_id } => {
                    println!("[mode] {}", mode_id);
                }
                par_term_acp::SessionUpdate::Unknown(v) => {
                    println!("[update:unknown] {}", truncate_json(v));
                }
                _ => {}
            }

            let before = chat.messages.len();
            chat.handle_update(update);
            for msg in &chat.messages[before..] {
                if let ChatMessage::ToolCall { title, status, .. } = msg {
                    if is_config_update_tool(title)
                        && !status.to_ascii_lowercase().contains("fail")
                        && !status.to_ascii_lowercase().contains("error")
                    {
                        event_flags.saw_config_update = true;
                    }
                    if is_failed_tool(title, status) {
                        event_flags.saw_failed_tool_since_prompt = true;
                        event_flags.saw_any_failed_tool = true;
                    }
                }
            }
            print_new_chat_messages(chat, Some(before));
        }
        AgentMessage::PermissionRequest {
            request_id,
            tool_call,
            options,
        } => {
            let title = tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Permission requested");
            println!("[perm] id={request_id} title={title}");
            for (i, opt) in options.iter().enumerate() {
                println!(
                    "  [{}] {} (id={} kind={})",
                    i,
                    opt.name,
                    opt.option_id,
                    opt.kind.as_deref().unwrap_or("")
                );
            }
            let choice = par_term_acp::harness::choose_permission_option(&options, auto_approve);
            match choice {
                Some((option_id, label)) => {
                    println!("[perm] auto-select {}", label);
                    if let Err(e) = agent.respond_permission(request_id, option_id, false).await {
                        println!("[perm] respond failed: {e}");
                    }
                }
                None => {
                    println!("[perm] cancelling (auto_approve=false)");
                    if let Err(e) = agent.respond_permission(request_id, "", true).await {
                        println!("[perm] cancel failed: {e}");
                    }
                }
            }
        }
        AgentMessage::ConfigUpdate { updates, reply } => {
            event_flags.saw_config_update = true;
            println!("[config_update] received {} key(s)", updates.len());
            print_updates(&updates);

            let result = if apply_config_updates {
                apply_updates_to_config(config, &updates)
            } else {
                Ok(())
            };
            let _ = reply.send(result.map_err(|e| e.to_string()));
        }
        AgentMessage::ClientReady(_) => {
            println!("[client] ready");
        }
        AgentMessage::AutoApproved(description) => {
            println!("[auto-approved] {}", description);
        }
        AgentMessage::PromptStarted => {
            println!("[prompt] started");
        }
        AgentMessage::PromptComplete => {
            chat.flush_agent_message();
            print_new_chat_messages(chat, None);
            println!("[prompt] complete");
        }
    }
    Ok(())
}
