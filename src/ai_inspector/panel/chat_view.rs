//! Chat and agent UI rendering for the AI Inspector panel.
//!
//! Contains the agent bar, action bar, chat message list, rich-text renderer,
//! and the chat input widget.

use egui::{Color32, Frame, Key, Label, RichText, Stroke};

use crate::ai_inspector::chat::{ChatMessage, ChatState, TextSegment, parse_text_segments};
use crate::ui_constants::{
    AI_PANEL_CHAT_BUTTON_WIDTH, AI_PANEL_CHAT_INPUT_BASE_HEIGHT, AI_PANEL_CHAT_INPUT_LINE_HEIGHT,
};
use par_term_acp::{AgentConfig, AgentStatus};

use super::types::{
    AGENT_CONNECTED, AGENT_DISCONNECTED, AGENT_MSG_BG, CMD_SUGGEST_BG, CODE_BLOCK_BG,
    CODE_LANG_COLOR, EXIT_FAILURE, SYSTEM_MSG_COLOR, USER_MSG_BG,
};
use super::{AIInspectorPanel, InspectorAction};

impl AIInspectorPanel {
    /// Render the action bar at the bottom of the panel.
    pub(super) fn render_action_bar(&self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;

        ui.horizontal(|ui| {
            // Copy JSON button
            if ui
                .button(RichText::new(" Copy JSON").small())
                .on_hover_text("Copy snapshot as JSON to clipboard")
                .clicked()
                && let Some(ref snapshot) = self.snapshot
                && let Ok(json) = snapshot.to_json()
            {
                action = InspectorAction::CopyJson(json);
            }

            // Save to file button
            if ui
                .button(RichText::new(" Save").small())
                .on_hover_text("Save snapshot JSON to file")
                .clicked()
                && let Some(ref snapshot) = self.snapshot
                && let Ok(json) = snapshot.to_json()
            {
                action = InspectorAction::SaveToFile(json);
            }
        });

        action
    }

    /// Render the agent connection status bar with connect/disconnect controls.
    pub(super) fn render_agent_bar(
        &mut self,
        ui: &mut egui::Ui,
        available_agents: &[AgentConfig],
    ) -> InspectorAction {
        let mut action = InspectorAction::None;

        ui.horizontal(|ui| {
            // Status indicator
            let connected_label = self
                .connected_agent_name
                .as_deref()
                .or(self.connected_agent_identity.as_deref())
                .unwrap_or("agent");
            let (status_icon, status_color, status_text) = match &self.agent_status {
                AgentStatus::Connected => (
                    "*",
                    AGENT_CONNECTED,
                    format!("Connected: {connected_label}"),
                ),
                AgentStatus::Connecting => (
                    "o",
                    Color32::from_rgb(255, 193, 7),
                    format!("Connecting: {connected_label}..."),
                ),
                AgentStatus::Disconnected => ("o", AGENT_DISCONNECTED, "Disconnected".to_string()),
                AgentStatus::Error(msg) => ("*", EXIT_FAILURE, format!("Error: {msg}")),
            };
            ui.label(RichText::new(status_icon).color(status_color).small());
            let mut status_response =
                ui.label(RichText::new(&status_text).color(status_color).small());
            if matches!(
                self.agent_status,
                AgentStatus::Connected | AgentStatus::Connecting
            ) && let Some(identity) = &self.connected_agent_identity
            {
                status_response = status_response.on_hover_text(format!("Identity: {identity}"));
            }
            if let AgentStatus::Error(msg) = &self.agent_status {
                status_response.on_hover_text(msg);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match self.agent_status {
                    AgentStatus::Connected => {
                        if ui
                            .button(RichText::new("Reset approvals").small())
                            .on_hover_text(
                                "Reconnect the agent session and revoke all \"Always allow\" permission selections (local chat context is restored on the next prompt)",
                            )
                            .clicked()
                        {
                            action = InspectorAction::RevokeAlwaysAllowSelections;
                        }
                        if ui
                            .button(RichText::new("Disconnect").small())
                            .on_hover_text("Disconnect from agent")
                            .clicked()
                        {
                            action = InspectorAction::DisconnectAgent;
                        }
                    }
                    AgentStatus::Disconnected | AgentStatus::Error(_) => {
                        if !available_agents.is_empty() {
                            // Clamp selected index to valid range
                            let idx = self.selected_agent_index.min(available_agents.len() - 1);
                            let agent = &available_agents[idx];
                            if ui
                                .button(RichText::new("Connect").small())
                                .on_hover_text(format!("Connect to {}", agent.name))
                                .clicked()
                            {
                                action = InspectorAction::ConnectAgent(agent.identity.clone());
                            }

                            // Agent selector dropdown (if multiple)
                            if available_agents.len() > 1 {
                                let selected_name = &available_agents[idx].short_name;
                                egui::ComboBox::from_id_salt("agent_selector")
                                    .selected_text(selected_name)
                                    .width(80.0)
                                    .show_ui(ui, |ui| {
                                        for (i, agent) in available_agents.iter().enumerate() {
                                            if ui.selectable_label(i == idx, &agent.name).clicked()
                                            {
                                                self.selected_agent_index = i;
                                            }
                                        }
                                    });
                            }
                        } else {
                            ui.label(
                                RichText::new("No agents found")
                                    .color(Color32::from_gray(80))
                                    .small()
                                    .italics(),
                            );
                        }
                    }
                    AgentStatus::Connecting => {
                        ui.spinner();
                    }
                }
            });
        });

        // Show install buttons only for agents whose connector binary is not in PATH
        if matches!(
            self.agent_status,
            AgentStatus::Disconnected | AgentStatus::Error(_)
        ) {
            let installable: Vec<_> = available_agents
                .iter()
                .filter(|a| a.install_command.is_some() && !a.connector_installed)
                .collect();
            if !installable.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Install ACP connectors:")
                        .color(Color32::from_gray(130))
                        .small(),
                );
                ui.horizontal_wrapped(|ui| {
                    for agent in installable {
                        let cmd = agent.install_command.as_deref().expect("agent was filtered to only include those with install_command.is_some()");
                        if ui
                            .button(RichText::new(format!("Install {}", agent.short_name)).small())
                            .on_hover_text(format!("Paste '{cmd}' into terminal"))
                            .clicked()
                        {
                            action = InspectorAction::WriteToTerminal(format!("{cmd}\n"));
                        }
                    }
                });
            }
        }

        action
    }

    /// Render chat messages from the conversation history.
    pub(super) fn render_chat_messages(ui: &mut egui::Ui, chat: &ChatState) -> InspectorAction {
        let mut action = InspectorAction::None;

        for msg in &chat.messages {
            match msg {
                ChatMessage::User { text, pending } => {
                    let frame = Frame::new()
                        .fill(USER_MSG_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("You:")
                                    .color(Color32::from_rgb(100, 160, 230))
                                    .small()
                                    .strong(),
                            );
                            if *pending {
                                ui.label(
                                    RichText::new("(queued)")
                                        .color(Color32::from_gray(100))
                                        .small()
                                        .italics(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(
                                                RichText::new("Cancel")
                                                    .small()
                                                    .color(Color32::from_rgb(255, 100, 100)),
                                            )
                                            .on_hover_text("Cancel this queued message")
                                            .clicked()
                                        {
                                            action = InspectorAction::CancelQueuedPrompt;
                                        }
                                    },
                                );
                            }
                        });
                        ui.add(
                            Label::new(RichText::new(text).color(Color32::from_gray(220)))
                                .selectable(true)
                                .wrap(),
                        );
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Agent(text) => {
                    let frame = Frame::new()
                        .fill(AGENT_MSG_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("Agent:")
                                .color(AGENT_CONNECTED)
                                .small()
                                .strong(),
                        );
                        Self::render_rich_text(ui, text);
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Thinking(text) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("").color(Color32::from_gray(80)).small());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .color(Color32::from_gray(90))
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::ToolCall { title, status, .. } => {
                    ui.horizontal(|ui| {
                        let status_icon = if status == "completed" {
                            RichText::new("OK").color(AGENT_CONNECTED).small()
                        } else if status == "error" || status == "failed" {
                            RichText::new("FAIL").color(EXIT_FAILURE).small()
                        } else if status == "in_progress" || status == "running" {
                            RichText::new(".")
                                .color(Color32::from_rgb(255, 193, 7))
                                .small()
                        } else {
                            // Empty or unknown status — show neutral pending indicator
                            RichText::new("-").color(Color32::from_gray(120)).small()
                        };
                        ui.label(status_icon);
                        ui.add(
                            Label::new(
                                RichText::new(title)
                                    .color(Color32::from_gray(150))
                                    .small()
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::CommandSuggestion(cmd) => {
                    let frame = Frame::new()
                        .fill(CMD_SUGGEST_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("Suggested command:")
                                .color(Color32::from_gray(130))
                                .small(),
                        );
                        ui.add(
                            Label::new(
                                RichText::new(format!("$ {cmd}"))
                                    .color(Color32::from_gray(220))
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("> Run").small())
                                .on_hover_text("Execute this command in the terminal")
                                .clicked()
                            {
                                // Send command + Enter to terminal and notify agent
                                action = InspectorAction::RunCommandAndNotify(cmd.clone());
                            }
                            if ui
                                .button(RichText::new("# Paste").small())
                                .on_hover_text("Paste command into terminal without executing")
                                .clicked()
                            {
                                action = InspectorAction::WriteToTerminal(cmd.clone());
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Permission {
                    request_id,
                    description,
                    options,
                    resolved,
                } => {
                    let frame = Frame::new()
                        .fill(Color32::from_rgb(50, 35, 20))
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new(if *resolved {
                                "OK Permission granted"
                            } else {
                                "! Permission requested"
                            })
                            .color(Color32::from_rgb(255, 193, 7))
                            .small()
                            .strong(),
                        );
                        ui.add(
                            Label::new(
                                RichText::new(description.as_str())
                                    .color(Color32::from_gray(180))
                                    .small(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        if !*resolved {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                for (opt_id, opt_label) in options {
                                    if ui
                                        .button(RichText::new(opt_label.as_str()).small())
                                        .clicked()
                                    {
                                        action = InspectorAction::RespondPermission {
                                            request_id: *request_id,
                                            option_id: opt_id.clone(),
                                            cancelled: false,
                                        };
                                    }
                                }
                                if ui
                                    .button(
                                        RichText::new("Deny")
                                            .small()
                                            .color(Color32::from_rgb(255, 100, 100)),
                                    )
                                    .clicked()
                                {
                                    action = InspectorAction::RespondPermission {
                                        request_id: *request_id,
                                        option_id: String::new(),
                                        cancelled: true,
                                    };
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::AutoApproved(desc) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("OK").color(Color32::from_gray(100)).small());
                        ui.add(
                            Label::new(
                                RichText::new(format!("Auto-approved: {desc}"))
                                    .color(Color32::from_gray(100))
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::System(text) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("i").color(SYSTEM_MSG_COLOR).small());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .color(SYSTEM_MSG_COLOR)
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
            }
        }

        // Show streaming text if agent is currently responding
        if chat.streaming {
            let streaming = chat.streaming_text();
            if !streaming.is_empty() {
                let frame = Frame::new()
                    .fill(AGENT_MSG_BG)
                    .corner_radius(4.0)
                    .inner_margin(6.0);
                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Agent:")
                                .color(AGENT_CONNECTED)
                                .small()
                                .strong(),
                        );
                        ui.spinner();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new("Cancel")
                                        .small()
                                        .color(Color32::from_rgb(255, 100, 100)),
                                )
                                .on_hover_text("Cancel current prompt")
                                .clicked()
                            {
                                action = InspectorAction::CancelPrompt;
                            }
                        });
                    });
                    Self::render_rich_text(ui, streaming);
                });
            } else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("Agent is thinking...")
                            .color(Color32::from_gray(120))
                            .small()
                            .italics(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("Cancel")
                                    .small()
                                    .color(Color32::from_rgb(255, 100, 100)),
                            )
                            .on_hover_text("Cancel current prompt")
                            .clicked()
                        {
                            action = InspectorAction::CancelPrompt;
                        }
                    });
                });
            }
        }

        action
    }

    /// Render agent text with code block formatting.
    ///
    /// Parses the text into plain text and fenced code block segments, rendering
    /// code blocks with a distinct background and monospace font.
    pub(super) fn render_rich_text(ui: &mut egui::Ui, text: &str) {
        let segments = parse_text_segments(text);
        for segment in &segments {
            match segment {
                TextSegment::Plain(t) => {
                    if !t.is_empty() {
                        ui.add(
                            Label::new(RichText::new(t).color(Color32::from_gray(210)))
                                .selectable(true)
                                .wrap(),
                        );
                    }
                }
                TextSegment::CodeBlock { lang, code } => {
                    let code_frame = Frame::new()
                        .fill(CODE_BLOCK_BG)
                        .corner_radius(3.0)
                        .inner_margin(6.0)
                        .stroke(Stroke::new(1.0, Color32::from_gray(40)));
                    code_frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if !lang.is_empty() {
                            ui.label(RichText::new(lang.as_str()).color(CODE_LANG_COLOR).small());
                        }
                        ui.add(
                            Label::new(
                                RichText::new(code.as_str())
                                    .color(Color32::from_gray(200))
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                }
            }
        }
    }

    /// Render the chat text input and send/clear buttons.
    ///
    /// Multiline: Enter sends, Shift+Enter inserts a newline.
    pub(super) fn render_chat_input(&mut self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;

        // Determine input height based on line count (min 1 row, max 6 rows)
        let line_count = self.chat.input.lines().count().clamp(1, 6);
        let input_height = AI_PANEL_CHAT_INPUT_BASE_HEIGHT
            + (line_count as f32 - 1.0) * AI_PANEL_CHAT_INPUT_LINE_HEIGHT;

        let button_width = AI_PANEL_CHAT_BUTTON_WIDTH;
        let input_width = ui.available_width() - button_width;

        // Check for Enter (without Shift) before rendering the TextEdit,
        // since egui may consume the key event.
        let enter_pressed = ui.input(|i| {
            i.key_pressed(Key::Enter)
                && !i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.command
        });

        ui.horizontal(|ui| {
            let response = ui.add_sized(
                [input_width, input_height],
                egui::TextEdit::multiline(&mut self.chat.input)
                    .hint_text("Message... (Shift+Enter for newline)")
                    .desired_width(input_width)
                    .desired_rows(line_count),
            );

            // Store the chat input Id for focus detection in Escape key handling
            self.chat_input_id = Some(response.id);

            let is_focused = response.has_focus();
            let should_send = is_focused && enter_pressed;

            ui.vertical(|ui| {
                let send_clicked = ui
                    .button(RichText::new(">").size(14.0))
                    .on_hover_text("Send message (Enter)")
                    .clicked();

                if ui
                    .button(RichText::new("C").size(12.0))
                    .on_hover_text("Clear conversation")
                    .clicked()
                {
                    action = InspectorAction::ClearChat;
                }

                if (should_send || send_clicked) && !self.chat.input.trim().is_empty() {
                    let text = self.chat.input.trim().to_string();
                    self.chat.input.clear();
                    action = InspectorAction::SendPrompt(text);
                }

                // Remove the trailing newline that Enter adds before we send
                if should_send {
                    // egui inserts the newline from Enter; strip it
                    while self.chat.input.ends_with('\n') {
                        self.chat.input.pop();
                    }
                }
            });

            // Re-focus input after sending
            if should_send {
                response.request_focus();
            }
        });

        action
    }
}
