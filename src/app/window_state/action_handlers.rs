//! Post-render action dispatch for WindowState.
//!
//! All handlers are called from `update_post_render_state()` (in render_pipeline.rs)
//! after the renderer borrow is released.

use crate::ai_inspector::chat::ChatMessage;
use crate::ai_inspector::panel::InspectorAction;
use crate::app::window_state::WindowState;
use crate::clipboard_history_ui::ClipboardHistoryAction;
use crate::config::ShaderInstallPrompt;
use crate::integrations_ui::IntegrationsResponse;
use crate::tab_bar_ui::TabBarAction;
use par_term_acp::{AgentMessage, AgentStatus};

impl WindowState {
    /// Handle tab bar actions collected during egui rendering (called after renderer borrow released).
    pub(crate) fn handle_tab_bar_action_after_render(
        &mut self,
        action: crate::tab_bar_ui::TabBarAction,
    ) {
        // Handle tab bar actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
            TabBarAction::SwitchTo(id) => {
                self.tab_manager.switch_to(id);
                // Clear renderer cells and invalidate cache to ensure clean switch
                self.clear_and_invalidate();
            }
            TabBarAction::Close(id) => {
                // Switch to the tab first so close_current_tab() operates on it.
                // This routes through the full close path: running-jobs confirmation,
                // session undo capture, and preserve-shell logic.
                self.tab_manager.switch_to(id);
                let was_last = self.close_current_tab();
                if was_last {
                    self.is_shutting_down = true;
                }
                self.request_redraw();
            }
            TabBarAction::NewTab => {
                self.new_tab();
                self.request_redraw();
            }
            TabBarAction::SetColor(id, color) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.set_custom_color(color);
                    log::info!(
                        "Set custom color for tab {}: RGB({}, {}, {})",
                        id,
                        color[0],
                        color[1],
                        color[2]
                    );
                }
                self.request_redraw();
            }
            TabBarAction::ClearColor(id) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.clear_custom_color();
                    log::info!("Cleared custom color for tab {}", id);
                }
                self.request_redraw();
            }
            TabBarAction::Reorder(id, target_index) => {
                if self.tab_manager.move_tab_to_index(id, target_index) {
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
            }
            TabBarAction::NewTabWithProfile(profile_id) => {
                self.open_profile(profile_id);
                self.request_redraw();
            }
            TabBarAction::RenameTab(id, name) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    if name.is_empty() {
                        // Blank name: revert to auto title mode
                        tab.user_named = false;
                        tab.has_default_title = true;
                        // Trigger immediate title update
                        tab.update_title(self.config.tab_title_mode);
                    } else {
                        tab.title = name;
                        tab.user_named = true;
                        tab.has_default_title = false;
                    }
                }
                self.request_redraw();
            }
            TabBarAction::Duplicate(id) => {
                self.duplicate_tab_by_id(id);
                self.request_redraw();
            }
            TabBarAction::ToggleAssistantPanel => {
                let just_opened = self.overlay_ui.ai_inspector.toggle();
                self.sync_ai_inspector_width();
                if just_opened {
                    self.try_auto_connect_agent();
                }
                self.request_redraw();
            }
            TabBarAction::SetTabIcon(tab_id, icon) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.custom_icon = icon;
                }
                self.request_redraw();
            }
            TabBarAction::None => {}
        }
    }

    /// Handle clipboard history actions collected during egui rendering.
    pub(crate) fn handle_clipboard_history_action_after_render(
        &mut self,
        action: crate::clipboard_history_ui::ClipboardHistoryAction,
    ) {
        // Handle clipboard actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
            ClipboardHistoryAction::Paste(content) => {
                self.paste_text(&content);
            }
            ClipboardHistoryAction::ClearAll => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                {
                    term.clear_all_clipboard_history();
                    log::info!("Cleared all clipboard history");
                }
                self.overlay_ui
                    .clipboard_history_ui
                    .update_entries(Vec::new());
            }
            ClipboardHistoryAction::ClearSlot(slot) => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                {
                    term.clear_clipboard_history(slot);
                    log::info!("Cleared clipboard history for slot {:?}", slot);
                }
            }
            ClipboardHistoryAction::None => {}
        }
    }

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
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                {
                    let _ = term.write(cmd.as_bytes());
                }
            }
            InspectorAction::RunCommandAndNotify(cmd) => {
                // Write command + Enter to terminal
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                {
                    let _ = term.write(format!("{cmd}\n").as_bytes());
                }
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
                self.config.ai_inspector_agent_terminal_access = enabled;
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
                self.config.ai_inspector_auto_approve = is_yolo;
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

    /// Handle responses from the integrations welcome dialog
    pub(crate) fn handle_integrations_response(&mut self, response: &IntegrationsResponse) {
        // Nothing to do if dialog wasn't interacted with
        if !response.install_shaders
            && !response.install_shell_integration
            && !response.skipped
            && !response.never_ask
            && !response.closed
            && response.shader_conflict_action.is_none()
        {
            return;
        }

        let current_version = env!("CARGO_PKG_VERSION").to_string();

        // Determine install intent and overwrite behavior
        let mut install_shaders = false;
        let mut install_shell_integration = false;
        let mut force_overwrite_modified_shaders = false;
        let mut triggered_install = false;

        // If we're waiting on a shader overwrite decision, handle that first
        if let Some(action) = response.shader_conflict_action {
            triggered_install = true;
            install_shaders = self.overlay_ui.integrations_ui.pending_install_shaders;
            install_shell_integration = self
                .overlay_ui
                .integrations_ui
                .pending_install_shell_integration;

            match action {
                crate::integrations_ui::ShaderConflictAction::Overwrite => {
                    force_overwrite_modified_shaders = true;
                }
                crate::integrations_ui::ShaderConflictAction::SkipModified => {
                    force_overwrite_modified_shaders = false;
                }
                crate::integrations_ui::ShaderConflictAction::Cancel => {
                    // Reset pending state and exit without installing
                    self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
                    self.overlay_ui.integrations_ui.shader_conflicts.clear();
                    self.overlay_ui.integrations_ui.pending_install_shaders = false;
                    self.overlay_ui
                        .integrations_ui
                        .pending_install_shell_integration = false;
                    self.overlay_ui.integrations_ui.error_message = None;
                    self.overlay_ui.integrations_ui.success_message = None;
                    self.focus_state.needs_redraw = true;
                    return;
                }
            }

            // Clear the conflict prompt regardless of choice
            self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
            self.overlay_ui.integrations_ui.shader_conflicts.clear();
            self.overlay_ui.integrations_ui.error_message = None;
            self.overlay_ui.integrations_ui.success_message = None;
            self.overlay_ui.integrations_ui.installing = false;
        } else if response.install_shaders || response.install_shell_integration {
            triggered_install = true;
            install_shaders = response.install_shaders;
            install_shell_integration = response.install_shell_integration;

            if install_shaders {
                match crate::shader_installer::detect_modified_bundled_shaders() {
                    Ok(conflicts) if !conflicts.is_empty() => {
                        log::info!(
                            "Detected {} modified bundled shaders; prompting for overwrite",
                            conflicts.len()
                        );
                        self.overlay_ui.integrations_ui.awaiting_shader_overwrite = true;
                        self.overlay_ui.integrations_ui.shader_conflicts = conflicts;
                        self.overlay_ui.integrations_ui.pending_install_shaders = install_shaders;
                        self.overlay_ui
                            .integrations_ui
                            .pending_install_shell_integration = install_shell_integration;
                        self.overlay_ui.integrations_ui.installing = false;
                        self.overlay_ui.integrations_ui.error_message = None;
                        self.overlay_ui.integrations_ui.success_message = None;
                        self.focus_state.needs_redraw = true;
                        return; // Wait for user decision
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!(
                            "Unable to check existing shaders for modifications: {}. Proceeding without overwrite prompt.",
                            e
                        );
                    }
                }
            }
        }

        // Handle "Install Selected" - user wants to install one or both integrations
        if triggered_install {
            log::info!(
                "User requested installations: shaders={}, shell_integration={}, overwrite_modified={}",
                install_shaders,
                install_shell_integration,
                force_overwrite_modified_shaders
            );

            let mut success_parts = Vec::new();
            let mut error_parts = Vec::new();

            // Install shaders if requested
            if install_shaders {
                self.overlay_ui
                    .integrations_ui
                    .set_installing("Installing shaders...");
                self.focus_state.needs_redraw = true;
                self.request_redraw();

                match crate::shader_installer::install_shaders_with_manifest(
                    force_overwrite_modified_shaders,
                ) {
                    Ok(result) => {
                        log::info!(
                            "Installed {} shader files ({} skipped, {} removed)",
                            result.installed,
                            result.skipped,
                            result.removed
                        );
                        let detail = if result.skipped > 0 {
                            format!("{} shaders ({} skipped)", result.installed, result.skipped)
                        } else {
                            format!("{} shaders", result.installed)
                        };
                        success_parts.push(detail);
                        self.config.integration_versions.shaders_installed_version =
                            Some(current_version.clone());
                        self.config.integration_versions.shaders_prompted_version =
                            Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shaders: {}", e);
                        error_parts.push(format!("Shaders: {}", e));
                    }
                }
            }

            // Install shell integration if requested
            if install_shell_integration {
                self.overlay_ui
                    .integrations_ui
                    .set_installing("Installing shell integration...");
                self.focus_state.needs_redraw = true;
                self.request_redraw();

                match crate::shell_integration_installer::install(None) {
                    Ok(result) => {
                        log::info!(
                            "Installed shell integration for {}",
                            result.shell.display_name()
                        );
                        success_parts.push(format!(
                            "shell integration ({})",
                            result.shell.display_name()
                        ));
                        self.config
                            .integration_versions
                            .shell_integration_installed_version = Some(current_version.clone());
                        self.config
                            .integration_versions
                            .shell_integration_prompted_version = Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shell integration: {}", e);
                        error_parts.push(format!("Shell: {}", e));
                    }
                }
            }

            // Show result
            if error_parts.is_empty() {
                self.overlay_ui
                    .integrations_ui
                    .set_success(&format!("Installed: {}", success_parts.join(", ")));
            } else if success_parts.is_empty() {
                self.overlay_ui
                    .integrations_ui
                    .set_error(&format!("Installation failed: {}", error_parts.join("; ")));
            } else {
                // Partial success
                self.overlay_ui.integrations_ui.set_success(&format!(
                    "Installed: {}. Errors: {}",
                    success_parts.join(", "),
                    error_parts.join("; ")
                ));
            }

            // Save config
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after integration install: {}", e);
            }

            // Clear pending flags
            self.overlay_ui.integrations_ui.pending_install_shaders = false;
            self.overlay_ui
                .integrations_ui
                .pending_install_shell_integration = false;

            self.focus_state.needs_redraw = true;
        }

        // Handle "Skip" - just close the dialog for this session
        if response.skipped {
            log::info!("User skipped integrations dialog for this session");
            self.overlay_ui.integrations_ui.hide();
            // Update prompted versions so we don't ask again this version
            self.config.integration_versions.shaders_prompted_version =
                Some(current_version.clone());
            self.config
                .integration_versions
                .shell_integration_prompted_version = Some(current_version.clone());
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after skipping integrations: {}", e);
            }
        }

        // Handle "Never Ask" - disable prompting permanently
        if response.never_ask {
            log::info!("User declined integrations (never ask again)");
            self.overlay_ui.integrations_ui.hide();
            // Set install prompts to Never
            self.config.shader_install_prompt = ShaderInstallPrompt::Never;
            self.config.shell_integration_state = crate::config::InstallPromptState::Never;
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after declining integrations: {}", e);
            }
        }

        // Handle dialog closed (OK button after success)
        if response.closed {
            self.overlay_ui.integrations_ui.hide();
        }
    }
}
