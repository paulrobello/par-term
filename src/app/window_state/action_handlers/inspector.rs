//! AI Inspector panel action handlers.
//!
//! Contains [`WindowState::handle_inspector_action_after_render`], dispatching
//! all 17 [`InspectorAction`] variants produced during egui rendering.

use crate::ai_inspector::chat::ChatMessage;
use crate::ai_inspector::panel::InspectorAction;
use crate::app::window_state::WindowState;
use par_term_acp::{AgentMessage, AgentStatus};

impl WindowState {
    /// Handle AI Inspector panel actions collected during egui rendering.
    pub(crate) fn handle_inspector_action_after_render(
        &mut self,
        action: crate::ai_inspector::panel::InspectorAction,
    ) {
        // Handle AI Inspector actions collected during egui rendering
        match action {
            InspectorAction::Close => {
                self.overlay_ui.ai_inspector.open = false;
                self.sync_ai_inspector_width();
            }
            InspectorAction::CopyJson(json) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(json);
                }
            }
            InspectorAction::SaveToFile(json) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(format!(
                        "par-term-snapshot-{}.json",
                        chrono::Local::now().format("%Y-%m-%d-%H%M%S")
                    ))
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    let _ = std::fs::write(path, json);
                }
            }
            InspectorAction::WriteToTerminal(cmd) => {
                self.with_active_tab(|tab| {
                    if let Ok(term) = tab.terminal.try_write() {
                        let _ = term.write(cmd.as_bytes());
                    }
                });
            }
            InspectorAction::RunCommandAndNotify(cmd) => {
                // Write command + Enter to terminal
                self.with_active_tab(|tab| {
                    if let Ok(term) = tab.terminal.try_write() {
                        let _ = term.write(format!("{cmd}\n").as_bytes());
                    }
                });
                // Record command count before execution so we can detect completion
                let history_len = self
                    .tab_manager
                    .active_tab()
                    .and_then(|tab| tab.terminal.try_write().ok())
                    .map(|term| term.core_command_history().len())
                    .unwrap_or(0);
                // Spawn a task that polls for command completion and notifies the agent
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    let tx = self.agent_state.agent_tx.clone();
                    let terminal = self
                        .tab_manager
                        .active_tab()
                        .map(|tab| tab.terminal.clone());
                    let cmd_for_msg = cmd.clone();
                    self.runtime.spawn(async move {
                        // Poll for command completion (up to 30 seconds)
                        let mut exit_code: Option<i32> = None;
                        for _ in 0..300 {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            if let Some(ref terminal) = terminal
                                && let Ok(term) = terminal.try_write()
                            {
                                let history = term.core_command_history();
                                if history.len() > history_len {
                                    // New command finished
                                    if let Some(last) = history.last() {
                                        exit_code = last.1;
                                    }
                                    break;
                                }
                            }
                        }
                        // Send feedback to agent
                        let exit_str = exit_code
                            .map(|c| format!("exit code {c}"))
                            .unwrap_or_else(|| "unknown exit code".to_string());
                        let feedback = format!(
                            "[System: The user executed `{cmd_for_msg}` in their terminal ({exit_str}). \
                             The output is available through the normal terminal capture.]"
                        );
                        let content = vec![par_term_acp::ContentBlock::Text {
                            text: feedback,
                        }];
                        let agent = agent.lock().await;
                        let _ = agent.send_prompt(content).await;
                        if let Some(tx) = tx {
                            let _ = tx.send(par_term_acp::AgentMessage::PromptComplete);
                        }
                    });
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::ConnectAgent(identity) => {
                self.connect_agent(&identity);
            }
            InspectorAction::DisconnectAgent => {
                if let Some(agent) = self.agent_state.agent.take() {
                    self.runtime.spawn(async move {
                        let mut agent = agent.lock().await;
                        agent.disconnect().await;
                    });
                }
                self.agent_state.agent_rx = None;
                self.agent_state.agent_tx = None;
                self.agent_state.agent_client = None;
                self.overlay_ui.ai_inspector.connected_agent_name = None;
                self.overlay_ui.ai_inspector.connected_agent_identity = None;
                // Abort any queued send tasks.
                for handle in self.agent_state.pending_send_handles.drain(..) {
                    handle.abort();
                }
                self.overlay_ui.ai_inspector.agent_status = AgentStatus::Disconnected;
                self.agent_state.pending_agent_context_replay = None;
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::RevokeAlwaysAllowSelections => {
                if let Some(identity) = self
                    .overlay_ui
                    .ai_inspector
                    .connected_agent_identity
                    .clone()
                {
                    // Cancel any queued prompts before replacing the session.
                    for handle in self.agent_state.pending_send_handles.drain(..) {
                        handle.abort();
                    }
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Resetting agent session to revoke all \"Always allow\" permissions. Local chat context will be replayed on your next prompt (best effort)."
                            .to_string(),
                    );
                    self.connect_agent(&identity);
                } else {
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Cannot reset permissions: no connected agent identity.".to_string(),
                    );
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::SendPrompt(text) => {
                // Reset one-shot local backend recovery for each user prompt.
                self.agent_state.agent_skill_failure_detected = false;
                self.agent_state.agent_skill_recovery_attempts = 0;
                self.overlay_ui
                    .ai_inspector
                    .chat
                    .add_user_message(text.clone());
                self.overlay_ui.ai_inspector.chat.streaming = true;
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    // Build structured prompt blocks so system/context/user roles
                    // stay explicit and stable on every turn.
                    let mut content: Vec<par_term_acp::ContentBlock> =
                        vec![par_term_acp::ContentBlock::Text {
                            text: format!(
                                "{}[End system instructions]",
                                crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE
                            ),
                        }];

                    // Inject shader context when relevant (keyword match or active shaders).
                    if crate::ai_inspector::shader_context::should_inject_shader_context(
                        &text,
                        &self.config,
                    ) {
                        content.push(par_term_acp::ContentBlock::Text {
                            text: crate::ai_inspector::shader_context::build_shader_context(
                                &self.config,
                            ),
                        });
                    }

                    if let Some(replay_prompt) =
                        self.agent_state.pending_agent_context_replay.take()
                    {
                        content.push(par_term_acp::ContentBlock::Text {
                            text: replay_prompt,
                        });
                    }

                    content.push(par_term_acp::ContentBlock::Text {
                        text: format!("[User message]\n{text}"),
                    });
                    let tx = self.agent_state.agent_tx.clone();
                    let handle = self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        // Ensure each user prompt starts in executable mode even if
                        // a previous response switched the session to plan mode.
                        if let Err(e) = agent.set_mode("default").await {
                            log::warn!("ACP: failed to pre-set default mode before prompt: {e}");
                        }
                        // Signal that we've acquired the lock — the prompt
                        // is no longer cancellable.
                        if let Some(ref tx) = tx {
                            let _ = tx.send(AgentMessage::PromptStarted);
                        }
                        let _ = agent.send_prompt(content).await;
                        // Signal the UI to flush the agent text buffer so
                        // command suggestions are extracted.
                        if let Some(tx) = tx {
                            let _ = tx.send(AgentMessage::PromptComplete);
                        }
                    });
                    self.agent_state.pending_send_handles.push_back(handle);
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::SetTerminalAccess(enabled) => {
                self.config.ai_inspector.ai_inspector_agent_terminal_access = enabled;
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::RespondPermission {
                request_id,
                option_id,
                cancelled,
            } => {
                if let Some(client) = &self.agent_state.agent_client {
                    let client = client.clone();
                    let action = if cancelled { "cancelled" } else { "selected" };
                    log::info!("ACP: sending permission response id={request_id} action={action}");
                    self.runtime.spawn(async move {
                        use par_term_acp::{PermissionOutcome, RequestPermissionResponse};
                        let outcome = if cancelled {
                            PermissionOutcome {
                                outcome: "cancelled".to_string(),
                                option_id: None,
                            }
                        } else {
                            PermissionOutcome {
                                outcome: "selected".to_string(),
                                option_id: Some(option_id),
                            }
                        };
                        let result = RequestPermissionResponse { outcome };
                        if let Err(e) = client
                            .respond(
                                request_id,
                                Some(serde_json::to_value(&result).expect("window_state: RequestPermissionResponse must be serializable to JSON")),
                                None,
                            )
                            .await
                        {
                            log::error!("ACP: failed to send permission response: {e}");
                        }
                    });
                } else {
                    log::error!(
                        "ACP: cannot send permission response id={request_id} — agent_client is None!"
                    );
                }
                // Mark the permission as resolved in the chat.
                for msg in &mut self.overlay_ui.ai_inspector.chat.messages {
                    if let ChatMessage::Permission {
                        request_id: rid,
                        resolved,
                        ..
                    } = msg
                        && *rid == request_id
                    {
                        *resolved = true;
                        break;
                    }
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::SetAgentMode(mode_id) => {
                let is_yolo = mode_id == "bypassPermissions";
                self.config.ai_inspector.ai_inspector_auto_approve = is_yolo;
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        agent
                            .auto_approve
                            .store(is_yolo, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) = agent.set_mode(&mode_id).await {
                            log::error!("ACP: failed to set mode '{mode_id}': {e}");
                        }
                    });
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::CancelPrompt => {
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        if let Err(e) = agent.cancel().await {
                            log::error!("ACP: failed to cancel prompt: {e}");
                        }
                    });
                }
                self.overlay_ui.ai_inspector.chat.flush_agent_message();
                self.overlay_ui
                    .ai_inspector
                    .chat
                    .add_system_message("Cancelled.".to_string());
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::CancelQueuedPrompt => {
                if self.overlay_ui.ai_inspector.chat.cancel_last_pending() {
                    // Abort the most recent queued send task.
                    if let Some(handle) = self.agent_state.pending_send_handles.pop_back() {
                        handle.abort();
                    }
                    self.overlay_ui
                        .ai_inspector
                        .chat
                        .add_system_message("Queued message cancelled.".to_string());
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::ClearChat => {
                let reconnect_identity = self
                    .overlay_ui
                    .ai_inspector
                    .connected_agent_identity
                    .clone();
                self.overlay_ui.ai_inspector.chat.clear();
                self.agent_state.pending_agent_context_replay = None;
                self.agent_state.agent_skill_failure_detected = false;
                self.agent_state.agent_skill_recovery_attempts = 0;
                // Abort any queued send tasks so stale prompts do not continue
                // after the conversation/session reset.
                for handle in self.agent_state.pending_send_handles.drain(..) {
                    handle.abort();
                }
                if let Some(identity) = reconnect_identity
                    && (self.agent_state.agent.is_some()
                        || self.overlay_ui.ai_inspector.agent_status != AgentStatus::Disconnected)
                {
                    self.connect_agent(&identity);
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Conversation cleared. Reconnected agent to reset session state."
                            .to_string(),
                    );
                }
                self.focus_state.needs_redraw = true;
            }
            InspectorAction::None => {}
        }
    }
}
