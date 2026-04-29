//! ACP agent message processing for WindowState.
//!
//! Contains:
//! - `process_agent_messages_tick`: drain agent message queue, update AI inspector,
//!   auto-context feeding, snapshot refresh, and bounded skill-failure recovery.
//!
//! Private stateless helpers (sensitive-key detection, command redaction, tool-name
//! extraction) live in the sibling `agent_message_helpers` module.
//!
//! Per-tick helper methods (recovery retry, auto-context, snapshot refresh) live in
//! the sibling `agent_tick_helpers` module.
//!
//! Config update application is in `agent_config.rs`.
//! Screenshot capture is in `agent_screenshot.rs`.

use crate::ai_inspector::chat::{
    ChatMessage, extract_inline_config_update, extract_inline_tool_function_name,
};
use crate::app::window_state::WindowState;
use crate::app::window_state::agent_message_helpers::is_terminal_screenshot_permission_tool;
use par_term_acp::{AgentMessage, AgentStatus};

impl WindowState {
    /// Process incoming ACP agent messages for this render tick and refresh
    /// the AI Inspector snapshot when needed.
    ///
    /// Called once per frame from `submit_gpu_frame()`. Handles the full agent message
    /// dispatch loop, deferred config updates, inline tool-markup fallback,
    /// bounded skill-failure recovery, auto-context feeding, and snapshot refresh.
    pub(crate) fn process_agent_messages_tick(&mut self) {
        let mut saw_prompt_complete_this_tick = false;

        // Process agent messages
        let msg_count_before = self.overlay_ui.ai_inspector.chat.messages.len();
        // Config update requests are deferred until message processing completes.
        type ConfigUpdateEntry = (
            std::collections::HashMap<String, serde_json::Value>,
            tokio::sync::oneshot::Sender<Result<(), String>>,
        );
        let mut pending_config_updates: Vec<ConfigUpdateEntry> = Vec::new();
        let messages = self.agent_state.drain_messages();
        for msg in messages {
            match msg {
                AgentMessage::StatusChanged(status) => {
                    // Flush any pending agent text on status change.
                    self.overlay_ui.ai_inspector.chat.flush_agent_message();
                    if matches!(status, AgentStatus::Disconnected | AgentStatus::Error(_)) {
                        self.overlay_ui.ai_inspector.connected_agent_project_root = None;
                        self.overlay_ui.ai_inspector.connected_agent_cwd = None;
                    }
                    self.overlay_ui.ai_inspector.agent_status = status;
                    self.focus_state.needs_redraw = true;
                }
                AgentMessage::SessionUpdate(update) => {
                    match &update {
                        par_term_acp::SessionUpdate::ToolCall(info) => {
                            let title_l = info.title.to_ascii_lowercase();
                            if title_l.contains("skill")
                                || title_l.contains("todo")
                                || title_l.contains("enterplanmode")
                            {
                                self.agent_state.agent_skill_failure_detected = true;
                            }
                        }
                        par_term_acp::SessionUpdate::ToolCallUpdate(info) => {
                            if let Some(status) = &info.status {
                                let status_l = status.to_ascii_lowercase();
                                if status_l.contains("fail") || status_l.contains("error") {
                                    self.agent_state.agent_skill_failure_detected = true;
                                }
                            }
                        }
                        par_term_acp::SessionUpdate::CurrentModeUpdate { mode_id } => {
                            if mode_id.eq_ignore_ascii_case("plan") {
                                self.agent_state.agent_skill_failure_detected = true;
                                self.overlay_ui.ai_inspector.chat.add_system_message(
                                        "Agent switched to plan mode during an executable task. Requesting default mode and retry guidance."
                                            .to_string(),
                                    );
                                if let Some(agent) = &self.agent_state.agent {
                                    let agent = agent.clone();
                                    self.runtime.spawn(async move {
                                            let agent = agent.lock().await;
                                            if let Err(e) = agent.set_mode("default").await {
                                                log::error!(
                                                    "ACP: failed to auto-reset mode from plan to default: {e}"
                                                );
                                            }
                                        });
                                }
                            }
                        }
                        _ => {}
                    }
                    self.overlay_ui.ai_inspector.chat.handle_update(update);
                    self.focus_state.needs_redraw = true;
                }
                AgentMessage::PermissionRequest {
                    request_id,
                    tool_call,
                    options,
                } => {
                    log::info!(
                        "ACP: permission request id={request_id} options={}",
                        options.len()
                    );
                    let description = tool_call
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Permission requested")
                        .to_string();
                    if is_terminal_screenshot_permission_tool(&tool_call)
                        && !self
                            .config
                            .ai_inspector
                            .ai_inspector_agent_screenshot_access
                    {
                        let deny_option_id = options
                            .iter()
                            .find(|o| {
                                matches!(
                                    o.kind.as_deref(),
                                    Some("deny")
                                        | Some("reject")
                                        | Some("cancel")
                                        | Some("disallow")
                                ) || o.name.to_lowercase().contains("deny")
                                    || o.name.to_lowercase().contains("reject")
                                    || o.name.to_lowercase().contains("cancel")
                            })
                            .or_else(|| options.first())
                            .map(|o| o.option_id.clone());

                        if let Some(client) = &self.agent_state.agent_client {
                            let client = client.clone();
                            self.runtime.spawn(async move {
                                use par_term_acp::{PermissionOutcome, RequestPermissionResponse};
                                let outcome = RequestPermissionResponse {
                                    outcome: PermissionOutcome {
                                        outcome: "selected".to_string(),
                                        option_id: deny_option_id,
                                    },
                                };
                                let response_json =
                                    serde_json::to_value(&outcome).unwrap_or_default();
                                if let Err(e) =
                                    client.respond(request_id, Some(response_json), None).await
                                {
                                    log::error!(
                                        "ACP: failed to auto-deny screenshot permission: {e}"
                                    );
                                }
                            });
                        } else {
                            log::error!(
                                "ACP: cannot auto-deny screenshot permission id={request_id} \
                                     — agent_client is None!"
                            );
                        }

                        self.overlay_ui.ai_inspector.chat.add_system_message(format!(
                                "Blocked screenshot request (`{description}`) because \"Allow Agent Screenshots\" is disabled in Settings > Assistant > Permissions."
                            ));
                        self.focus_state.needs_redraw = true;
                        continue;
                    }

                    self.overlay_ui
                        .ai_inspector
                        .chat
                        .messages
                        .push(ChatMessage::Permission {
                            request_id,
                            description,
                            options: options
                                .iter()
                                .map(|o| (o.option_id.clone(), o.name.clone()))
                                .collect(),
                            resolved: false,
                        });
                    self.focus_state.needs_redraw = true;
                }
                AgentMessage::PromptStarted => {
                    self.agent_state.agent_skill_failure_detected = false;
                    self.overlay_ui.ai_inspector.chat.mark_oldest_pending_sent();
                    // Remove the corresponding handle (first in queue).
                    if !self.agent_state.pending_send_handles.is_empty() {
                        self.agent_state.pending_send_handles.pop_front();
                    }
                    self.focus_state.needs_redraw = true;
                }
                AgentMessage::PromptComplete => {
                    saw_prompt_complete_this_tick = true;
                    self.overlay_ui.ai_inspector.chat.flush_agent_message();
                    self.focus_state.needs_redraw = true;
                }
                AgentMessage::ConfigUpdate { updates, reply } => {
                    pending_config_updates.push((updates, reply));
                }
                AgentMessage::ClientReady(client) => {
                    log::info!("ACP: agent_client ready");
                    self.agent_state.agent_client = Some(client);
                }
                AgentMessage::AutoApproved(description) => {
                    self.overlay_ui
                        .ai_inspector
                        .chat
                        .add_auto_approved(description);
                    self.focus_state.needs_redraw = true;
                }
            }
        }
        // Process deferred config updates now that message processing completes.
        for (updates, reply) in pending_config_updates {
            let result = self.apply_agent_config_updates(&updates);
            if result.is_ok() {
                self.render_loop.config_changed_by_agent = true;
            }
            let _ = reply.send(result);
            self.focus_state.needs_redraw = true;
        }

        // Track recoverable local backend tool failures during the current
        // prompt (for example failed `Skill`/`Write` calls).
        if !self.agent_state.agent_skill_failure_detected {
            let mut seen_user_boundary = false;
            for msg in self.overlay_ui.ai_inspector.chat.messages.iter().rev() {
                if matches!(msg, ChatMessage::User { .. }) {
                    seen_user_boundary = true;
                    break;
                }
                if let ChatMessage::ToolCall { title, status, .. } = msg {
                    let title_l = title.to_ascii_lowercase();
                    let status_l = status.to_ascii_lowercase();
                    let is_failed = status_l.contains("fail") || status_l.contains("error");
                    let is_recoverable_tool = title_l.contains("skill")
                        || title_l == "write"
                        || title_l.starts_with("write ")
                        || title_l.contains(" write ");
                    if is_failed && is_recoverable_tool {
                        self.agent_state.agent_skill_failure_detected = true;
                        break;
                    }
                }
            }
            // If there is no user message yet, ignore stale history.
            if !seen_user_boundary {
                self.agent_state.agent_skill_failure_detected = false;
            }
        }

        // Compatibility fallback: some local ACP backends emit literal
        // `<function=...>` tool markup in chat instead of structured tool calls.
        // Parse inline `config_update` payloads from newly added agent messages
        // and apply them so config changes still work.
        let inline_updates: Vec<(usize, std::collections::HashMap<String, serde_json::Value>)> =
            self.overlay_ui
                .ai_inspector
                .chat
                .messages
                .iter()
                .enumerate()
                .skip(msg_count_before)
                .filter_map(|(idx, msg)| match msg {
                    ChatMessage::Agent(text) => {
                        extract_inline_config_update(text).map(|updates| (idx, updates))
                    }
                    _ => None,
                })
                .collect();

        for (idx, updates) in inline_updates {
            match self.apply_agent_config_updates(&updates) {
                Ok(()) => {
                    self.render_loop.config_changed_by_agent = true;
                    if let Some(ChatMessage::Agent(text)) =
                        self.overlay_ui.ai_inspector.chat.messages.get_mut(idx)
                    {
                        *text = "Applied config update request.".to_string();
                    }
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Applied inline config_update fallback from agent output.".to_string(),
                    );
                }
                Err(e) => {
                    self.overlay_ui
                        .ai_inspector
                        .chat
                        .add_system_message(format!("Inline config_update fallback failed: {e}"));
                }
            }
            self.focus_state.needs_redraw = true;
        }

        // Detect other inline XML-style tool markup (we only auto-apply
        // `config_update`). Treat these as recoverable local backend tool
        // failures so we can issue a one-shot retry with stricter guidance.
        for msg in self
            .overlay_ui
            .ai_inspector
            .chat
            .messages
            .iter()
            .skip(msg_count_before)
        {
            if let ChatMessage::Agent(text) = msg
                && let Some(function_name) = extract_inline_tool_function_name(text)
                && function_name != "mcp__par-term-config__config_update"
            {
                self.agent_state.agent_skill_failure_detected = true;
                self.overlay_ui.ai_inspector.chat.add_system_message(format!(
                    "Agent emitted inline tool markup (`{function_name}`) instead of a structured ACP tool call."
                ));
                self.focus_state.needs_redraw = true;
                break;
            }
        }

        let last_user_text = self
            .overlay_ui
            .ai_inspector
            .chat
            .messages
            .iter()
            .rev()
            .find_map(|msg| {
                if let ChatMessage::User { text, .. } = msg {
                    Some(text.clone())
                } else {
                    None
                }
            });

        let shader_activation_incomplete = if saw_prompt_complete_this_tick {
            if let Some(user_text) = last_user_text.as_deref() {
                if crate::ai_inspector::shader_context::is_shader_activation_request(user_text) {
                    let mut saw_user_boundary = false;
                    let mut saw_config_update_for_prompt = false;
                    for msg in self.overlay_ui.ai_inspector.chat.messages.iter().rev() {
                        match msg {
                            ChatMessage::User { .. } => {
                                saw_user_boundary = true;
                                break;
                            }
                            ChatMessage::ToolCall { title, .. } => {
                                let title_l = title.to_ascii_lowercase();
                                if title_l.contains("config_update") {
                                    saw_config_update_for_prompt = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    saw_user_boundary && !saw_config_update_for_prompt
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Delegate recovery/retry to agent_tick_helpers.
        self.attempt_skill_failure_recovery(
            saw_prompt_complete_this_tick,
            shader_activation_incomplete,
            &last_user_text,
        );

        // Delegate auto-context + command suggestion execution to agent_tick_helpers.
        self.feed_auto_context(msg_count_before);

        // Delegate snapshot refresh to agent_tick_helpers.
        self.refresh_inspector_snapshot();
    }
}
