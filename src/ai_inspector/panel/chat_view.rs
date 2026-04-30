//! Chat and agent UI rendering for the AI Inspector panel.
//!
//! Contains the agent bar, action bar, and the chat input widget.
//! Chat message list and rich-text renderer live in `message_render.rs`.

use egui::{Color32, Key, RichText};

use crate::ui_constants::{AI_PANEL_CHAT_INPUT_BASE_HEIGHT, AI_PANEL_CHAT_INPUT_LINE_HEIGHT};
use par_term_acp::{AgentConfig, AgentStatus};

use super::types::{AGENT_CONNECTED, AGENT_DISCONNECTED, EXIT_FAILURE};
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

        if matches!(
            self.agent_status,
            AgentStatus::Connected | AgentStatus::Connecting
        ) && let Some(project_label) = self.agent_project_label()
        {
            let mut hover_lines = vec![format!(
                "Project root: {}",
                self.connected_agent_project_root
                    .as_deref()
                    .unwrap_or_default()
            )];
            if let Some(cwd) = &self.connected_agent_cwd {
                hover_lines.push(format!("Session cwd: {cwd}"));
            }
            ui.label(
                RichText::new(project_label)
                    .small()
                    .color(Color32::from_gray(150)),
            )
            .on_hover_text(hover_lines.join("\n"));
        }

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

    pub(super) fn action_for_assistant_prompt(
        prompt: &par_term_config::AssistantPrompt,
    ) -> InspectorAction {
        if prompt.auto_submit {
            InspectorAction::SendPrompt(prompt.prompt.clone())
        } else {
            InspectorAction::LoadPrompt(prompt.prompt.clone())
        }
    }

    /// Render the chat text input.
    ///
    /// Multiline: Enter sends, Shift+Enter inserts a newline.
    pub(super) fn render_chat_input(&mut self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;

        // Determine input height based on line count (min 1 row, max 6 rows)
        let line_count = self.chat.input.lines().count().clamp(1, 6);
        let input_height = AI_PANEL_CHAT_INPUT_BASE_HEIGHT
            + (line_count as f32 - 1.0) * AI_PANEL_CHAT_INPUT_LINE_HEIGHT;
        let input_width = ui.available_width().max(60.0);

        // Check for Enter (without Shift) before rendering the TextEdit,
        // since egui may consume the key event.
        let enter_pressed = ui.input(|i| {
            i.key_pressed(Key::Enter)
                && !i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.command
        });

        let chat_input_id = egui::Id::new("assistant_chat_input");
        let cursor_index_before_edit =
            text_edit_cursor_index(ui.ctx(), chat_input_id, &self.chat.input);
        let response = ui.add_sized(
            [input_width, input_height],
            egui::TextEdit::multiline(&mut self.chat.input)
                .id(chat_input_id)
                .hint_text("Message... (Shift+Enter for newline)")
                .desired_width(input_width)
                .desired_rows(line_count),
        );

        // Store the chat input Id for focus detection in Escape key handling
        self.chat_input_id = Some(chat_input_id);

        let is_focused = response.has_focus();
        if is_focused {
            let cursor_index = cursor_index_before_edit;
            let (up_pressed, down_pressed, modifiers) = ui.input(|i| {
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.modifiers,
                )
            });
            let allow_history_navigation = modifiers_allow_input_history(modifiers);
            let navigated_history = if allow_history_navigation
                && up_pressed
                && input_cursor_is_on_first_line(&self.chat.input, cursor_index)
                && self.chat.navigate_input_history_older()
            {
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
                true
            } else if allow_history_navigation
                && down_pressed
                && input_cursor_is_on_last_line(&self.chat.input, cursor_index)
                && self.chat.navigate_input_history_newer()
            {
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
                true
            } else {
                false
            };

            if navigated_history {
                set_text_edit_cursor_to_end(ui.ctx(), response.id, &self.chat.input);
                response.request_focus();
            }
        }

        let should_send = is_focused && enter_pressed;
        if should_send && !self.chat.input.trim().is_empty() {
            let text = self.chat.input.trim().to_string();
            self.chat.input.clear();
            action = InspectorAction::SendPrompt(text);
        }

        // Remove the trailing newline that Enter adds before we send.
        if should_send {
            while self.chat.input.ends_with('\n') {
                self.chat.input.pop();
            }
            response.request_focus();
        }

        action
    }

    /// Render prompt, send, and clear controls for the chat input.
    pub(super) fn render_chat_controls(&mut self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;
        let menu_width = ui.available_width().clamp(220.0, 360.0);

        ui.menu_button(RichText::new("Prompts").small(), |ui| {
            ui.set_min_width(menu_width);

            if let Some(error) = &self.assistant_prompts_error {
                ui.label(
                    RichText::new(format!("Load error: {error}"))
                        .small()
                        .color(EXIT_FAILURE),
                );
                ui.separator();
            }

            if self.assistant_prompts.is_empty() {
                ui.label(
                    RichText::new("No prompts saved")
                        .small()
                        .color(Color32::from_gray(100))
                        .italics(),
                );
            } else {
                for prompt in &self.assistant_prompts {
                    let label = if prompt.auto_submit {
                        format!("{}  (send)", prompt.title)
                    } else {
                        prompt.title.clone()
                    };
                    if ui.button(label).clicked() {
                        action = Self::action_for_assistant_prompt(prompt);
                        ui.close();
                    }
                }
            }
        });

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

        if send_clicked && !self.chat.input.trim().is_empty() {
            let text = self.chat.input.trim().to_string();
            self.chat.input.clear();
            action = InspectorAction::SendPrompt(text);
        }

        action
    }
}

fn text_edit_cursor_index(ctx: &egui::Context, id: egui::Id, text: &str) -> usize {
    egui::TextEdit::load_state(ctx, id)
        .and_then(|state| state.cursor.char_range())
        .map(|range| range.primary.index)
        .unwrap_or_else(|| text.chars().count())
}

fn set_text_edit_cursor_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let end = egui::text::CCursor::new(text.chars().count());
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(end)));
    state.store(ctx, id);
}

fn modifiers_allow_input_history(modifiers: egui::Modifiers) -> bool {
    modifiers == egui::Modifiers::NONE
}

fn input_cursor_is_on_first_line(text: &str, cursor_index: usize) -> bool {
    !text.chars().take(cursor_index).any(|ch| ch == '\n')
}

fn input_cursor_is_on_last_line(text: &str, cursor_index: usize) -> bool {
    !text.chars().skip(cursor_index).any(|ch| ch == '\n')
}

#[cfg(test)]
mod tests {
    use super::{
        input_cursor_is_on_first_line, input_cursor_is_on_last_line, modifiers_allow_input_history,
    };

    #[test]
    fn input_history_cursor_allows_top_and_bottom_boundaries() {
        assert!(input_cursor_is_on_first_line("single line", 5));
        assert!(input_cursor_is_on_last_line("single line", 5));
        assert!(input_cursor_is_on_first_line("first\nsecond", 0));
        assert!(input_cursor_is_on_last_line(
            "first\nsecond",
            "first\nsecond".chars().count(),
        ));
    }

    #[test]
    fn input_history_cursor_blocks_middle_lines() {
        let text = "first\nsecond\nthird";
        let second_line_start = "first\n".chars().count();
        let second_line_middle = "first\nsec".chars().count();

        assert!(!input_cursor_is_on_first_line(text, second_line_start));
        assert!(!input_cursor_is_on_first_line(text, second_line_middle));
        assert!(!input_cursor_is_on_last_line(text, second_line_start));
        assert!(!input_cursor_is_on_last_line(text, second_line_middle));
    }

    #[test]
    fn input_history_navigation_requires_unmodified_arrow_keys() {
        assert!(modifiers_allow_input_history(egui::Modifiers::NONE));
        assert!(!modifiers_allow_input_history(egui::Modifiers {
            shift: true,
            ..Default::default()
        }));
        assert!(!modifiers_allow_input_history(egui::Modifiers {
            alt: true,
            ..Default::default()
        }));
        assert!(!modifiers_allow_input_history(egui::Modifiers {
            command: true,
            ..Default::default()
        }));
        assert!(!modifiers_allow_input_history(egui::Modifiers {
            ctrl: true,
            ..Default::default()
        }));
    }
}
