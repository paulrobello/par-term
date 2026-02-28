//! ACP agent message processing for WindowState.
//!
//! Contains:
//! - `process_agent_messages_tick`: drain agent message queue, update AI inspector,
//!   auto-context feeding, snapshot refresh, and bounded skill-failure recovery.
//!
//! Config update application is in `agent_config.rs`.
//! Screenshot capture is in `agent_screenshot.rs`.

use crate::ai_inspector::chat::{
    ChatMessage, extract_inline_config_update, extract_inline_tool_function_name,
};
use crate::app::window_state::WindowState;
use par_term_acp::{AgentMessage, AgentStatus, ContentBlock};

const AUTO_CONTEXT_MIN_INTERVAL_MS: u64 = 1200;
const AUTO_CONTEXT_MAX_COMMAND_LEN: usize = 400;

// ---------------------------------------------------------------------------
// Auto-context helpers (private to this module)
// ---------------------------------------------------------------------------

fn is_sensitive_key(key: &str) -> bool {
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

fn redact_auto_context_command(command: &str) -> (String, bool) {
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

fn is_terminal_screenshot_permission_tool(tool_call: &serde_json::Value) -> bool {
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

// ---------------------------------------------------------------------------
// WindowState impl
// ---------------------------------------------------------------------------

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
                        && !self.config.ai_inspector_agent_screenshot_access
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
                self.config_changed_by_agent = true;
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
                    self.config_changed_by_agent = true;
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

        // Bounded recovery: if the prompt failed due to a local backend tool
        // mismatch (failed Skill/Write or inline tool markup), or if a shader
        // activation request completed without a config_update call, nudge the
        // agent to continue the same task with proper ACP tool use.
        if saw_prompt_complete_this_tick
            && (self.agent_state.agent_skill_failure_detected || shader_activation_incomplete)
            && self.agent_state.agent_skill_recovery_attempts < 3
            && let Some(agent) = &self.agent_state.agent
        {
            let had_recoverable_failure = self.agent_state.agent_skill_failure_detected;
            self.agent_state.agent_skill_recovery_attempts = self
                .agent_state
                .agent_skill_recovery_attempts
                .saturating_add(1);
            self.agent_state.agent_skill_failure_detected = false;
            self.overlay_ui.ai_inspector.chat.streaming = true;
            if shader_activation_incomplete && !had_recoverable_failure {
                self.overlay_ui.ai_inspector.chat.add_system_message(
                    format!(
                        "Agent completed a shader task response without activating the shader via \
                         config_update. Auto-retrying (attempt {}/3) to finish the activation step.",
                        self.agent_state.agent_skill_recovery_attempts
                    ),
                );
            } else {
                self.overlay_ui.ai_inspector.chat.add_system_message(
                    format!(
                        "Recoverable local-backend tool failure detected (failed Skill/Write or \
                         inline tool markup). Auto-retrying (attempt {}/3) with stricter ACP tool guidance.",
                        self.agent_state.agent_skill_recovery_attempts
                    ),
                );
            }

            let mut content: Vec<ContentBlock> = vec![ContentBlock::Text {
                text: format!(
                    "{}[End system instructions]",
                    crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE
                ),
            }];

            if let Some(ref user_text) = last_user_text
                && crate::ai_inspector::shader_context::should_inject_shader_context(
                    user_text,
                    &self.config,
                )
            {
                content.push(ContentBlock::Text {
                    text: crate::ai_inspector::shader_context::build_shader_context(&self.config),
                });
            }

            let extra_recovery_strictness = if self.agent_state.agent_skill_recovery_attempts >= 2 {
                " Do not explore unrelated files or dependencies. For shader tasks, go directly \
                 to the shader file write and config_update activation steps."
            } else {
                ""
            };
            content.push(ContentBlock::Text {
                text: format!(
                    "[Host recovery note]\nContinue the previous user task and stay on the \
                       same domain/problem (do not switch to unrelated examples/files). Do NOT \
                       use `Skill`, `Task`, or `TodoWrite`. Do NOT emit XML-style tool markup \
                       (`<function=...>`). Use normal ACP file/system/MCP tools directly. If \
                       a `Read` fails because the target is a directory, do not retry `Read` on \
                       that directory; use a listing/search tool or write the known target file \
                       path directly. \
                       Complete the full requested workflow before declaring success (for shader \
                       tasks: write the requested shader content, then call config_update to \
                       activate it). \
                       using `Write`, use exact parameters like `file_path` and `content` (not \
                       `filepath`). For par-term settings changes use \
                       `mcp__par-term-config__config_update` / `config_update`. If a tool \
                       fails, correct the call and retry the same task with the available \
                       tools. If you have already created the requested shader file, do not \
                       stop there: call config_update now to activate it before declaring \
                       success. Do not ask the user to restate the request unless you truly \
                       need missing information.{}",
                    extra_recovery_strictness
                ),
            });

            let agent = agent.clone();
            let tx = self.agent_state.agent_tx.clone();
            let handle = self.runtime.spawn(async move {
                let agent = agent.lock().await;
                if let Some(ref tx) = tx {
                    let _ = tx.send(AgentMessage::PromptStarted);
                }
                let _ = agent.send_prompt(content).await;
                if let Some(tx) = tx {
                    let _ = tx.send(AgentMessage::PromptComplete);
                }
            });
            self.agent_state.pending_send_handles.push_back(handle);
            self.focus_state.needs_redraw = true;
        }

        // Auto-execute new CommandSuggestion messages when terminal access is enabled.
        if self.config.ai_inspector_agent_terminal_access {
            let new_messages = &self.overlay_ui.ai_inspector.chat.messages[msg_count_before..];
            let commands_to_run: Vec<String> = new_messages
                .iter()
                .filter_map(|msg| {
                    if let ChatMessage::CommandSuggestion(cmd) = msg {
                        Some(format!("{cmd}\n"))
                    } else {
                        None
                    }
                })
                .collect();

            if !commands_to_run.is_empty()
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_write()
            {
                for cmd in &commands_to_run {
                    let _ = term.write(cmd.as_bytes());
                }
                crate::debug_info!(
                    "AI_INSPECTOR",
                    "Auto-executed {} command(s) in terminal",
                    commands_to_run.len()
                );
            }
        }

        // Detect new command completions and auto-refresh the snapshot.
        // This is separate from agent auto-context so the panel always shows
        // up-to-date command history regardless of agent connection state.
        if self.overlay_ui.ai_inspector.open
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
        {
            let history = term.core_command_history();
            let current_count = history.len();

            if current_count != self.overlay_ui.ai_inspector.last_command_count {
                // Command count changed — refresh the snapshot
                let had_commands = self.overlay_ui.ai_inspector.last_command_count > 0;
                self.overlay_ui.ai_inspector.last_command_count = current_count;
                self.overlay_ui.ai_inspector.needs_refresh = true;

                // Auto-context feeding: send latest command info to agent
                if had_commands
                    && current_count > 0
                    && self.config.ai_inspector_auto_context
                    && self.overlay_ui.ai_inspector.agent_status == AgentStatus::Connected
                    && let Some((cmd, exit_code, duration_ms)) = history.last()
                {
                    let now = std::time::Instant::now();
                    let throttled =
                        self.agent_state
                            .last_auto_context_sent_at
                            .is_some_and(|last_sent| {
                                now.duration_since(last_sent)
                                    < std::time::Duration::from_millis(AUTO_CONTEXT_MIN_INTERVAL_MS)
                            });

                    if !throttled {
                        let exit_code_str = exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        let duration = duration_ms.unwrap_or(0);

                        let cwd = term.shell_integration_cwd().unwrap_or_default();
                        let (sanitized_cmd, was_redacted) = redact_auto_context_command(cmd);

                        let context = format!(
                            "[Auto-context event]\nCommand completed:\n$ {}\nExit code: {}\nDuration: {}ms\nCWD: {}\nSensitive arguments redacted: {}",
                            sanitized_cmd, exit_code_str, duration, cwd, was_redacted
                        );

                        if let Some(agent) = &self.agent_state.agent {
                            self.agent_state.last_auto_context_sent_at = Some(now);
                            self.overlay_ui.ai_inspector.chat.add_system_message(if was_redacted {
                                "Auto-context sent command metadata to the agent (sensitive values redacted).".to_string()
                            } else {
                                "Auto-context sent command metadata to the agent.".to_string()
                            });
                            self.focus_state.needs_redraw = true;
                            let agent = agent.clone();
                            let content = vec![ContentBlock::Text { text: context }];
                            self.runtime.spawn(async move {
                                let agent = agent.lock().await;
                                let _ = agent.send_prompt(content).await;
                            });
                        }
                    }
                }
            }
        }

        // Refresh AI Inspector snapshot if needed
        if self.overlay_ui.ai_inspector.open
            && self.overlay_ui.ai_inspector.needs_refresh
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
        {
            let snapshot = crate::ai_inspector::snapshot::SnapshotData::gather(
                &term,
                &self.overlay_ui.ai_inspector.scope,
                self.config.ai_inspector_context_max_lines,
            );
            self.overlay_ui.ai_inspector.snapshot = Some(snapshot);
            self.overlay_ui.ai_inspector.needs_refresh = false;
        }
    }
}
