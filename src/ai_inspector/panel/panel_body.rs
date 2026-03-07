//! Inner panel body rendering for [`AIInspectorPanel`].
//!
//! Contains `render_panel_body`, which is called from `show()` inside the
//! egui `Area` closure.  Extracted here to keep `mod.rs` under 500 lines.

use egui::{Color32, Frame, RichText, Stroke};
use par_term_acp::{AgentConfig, AgentStatus};

use crate::ui_constants::{
    AI_PANEL_CMD_SCROLL_MAX_HEIGHT, AI_PANEL_CMD_SCROLL_MIN_HEIGHT, AI_PANEL_INNER_INSET,
    AI_PANEL_INNER_MARGIN,
};

use super::AIInspectorPanel;
use super::types::{InspectorAction, PANEL_BG};

impl AIInspectorPanel {
    /// Render the full panel interior (frame + all sections) for a single egui frame.
    ///
    /// Returns the [`InspectorAction`] produced by user interactions inside the panel.
    pub(super) fn render_panel_body(
        &mut self,
        ui: &mut egui::Ui,
        available_agents: &[AgentConfig],
    ) -> (InspectorAction, bool) {
        let mut close_requested = false;
        let mut action = InspectorAction::None;

        let inner_width = self.width - AI_PANEL_INNER_INSET;
        let panel_frame = Frame::new()
            .fill(PANEL_BG)
            .stroke(Stroke::new(1.0, Color32::from_gray(50)))
            .inner_margin(AI_PANEL_INNER_MARGIN);

        let viewport = ui.ctx().input(|i| i.viewport_rect());

        panel_frame.show(ui, |ui| {
            let panel_inner_height =
                (viewport.height() - crate::ui_constants::AI_PANEL_HEIGHT_INSET).max(0.0);
            ui.set_min_width(inner_width);
            ui.set_max_width(inner_width);
            ui.set_min_height(panel_inner_height);
            ui.set_max_height(panel_inner_height);

            // === Title bar ===
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("Assistant")
                        .strong()
                        .color(Color32::from_gray(220)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("X").size(14.0))
                        .on_hover_text("Close (Escape)")
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // === Agent connection bar (above terminal capture) ===
            let agent_action = self.render_agent_bar(ui, available_agents);
            if !matches!(agent_action, InspectorAction::None) {
                action = agent_action;
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // === Collapsible terminal capture section ===
            egui::CollapsingHeader::new(
                RichText::new("Terminal Capture")
                    .color(Color32::from_gray(180))
                    .strong(),
            )
            .id_salt("terminal_capture_section")
            .default_open(false)
            .show(ui, |ui| {
                // --- Controls row ---
                self.render_controls(ui);

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // --- Environment strip ---
                if let Some(ref snapshot) = self.snapshot {
                    self.render_environment(ui, snapshot);
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }

                // --- Commands content ---
                let cmd_height = (ui.available_height() * 0.5).clamp(
                    AI_PANEL_CMD_SCROLL_MIN_HEIGHT,
                    AI_PANEL_CMD_SCROLL_MAX_HEIGHT,
                );
                egui::ScrollArea::vertical()
                    .id_salt("capture_commands_scroll")
                    .max_height(cmd_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(ref snapshot) = self.snapshot {
                            if snapshot.commands.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.label(
                                        RichText::new("No commands captured yet")
                                            .color(Color32::from_gray(100))
                                            .italics(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Run some commands in the terminal\nto see them here.",
                                        )
                                        .color(Color32::from_gray(80))
                                        .small(),
                                    );
                                });
                            } else {
                                match self.view_mode {
                                    super::types::ViewMode::Cards => {
                                        Self::render_cards(ui, &snapshot.commands);
                                    }
                                    super::types::ViewMode::Timeline => {
                                        Self::render_timeline(ui, &snapshot.commands);
                                    }
                                    super::types::ViewMode::Tree => {
                                        Self::render_tree(ui, &snapshot.commands);
                                    }
                                    super::types::ViewMode::ListDetail => {
                                        Self::render_list_detail(ui, &snapshot.commands);
                                    }
                                }
                            }
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new("No snapshot available")
                                        .color(Color32::from_gray(100))
                                        .italics(),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Click Refresh to capture terminal state.")
                                        .color(Color32::from_gray(80))
                                        .small(),
                                );
                            });
                        }
                    });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Reserve space for pinned bottom elements
            let input_lines = if self.agent_status == AgentStatus::Connected {
                self.chat.input.lines().count().clamp(1, 6) as f32
            } else {
                0.0
            };
            let bottom_reserve = if self.agent_status == AgentStatus::Connected {
                90.0 + (input_lines - 1.0).max(0.0) * 14.0
            } else {
                36.0
            };
            let available_height = (ui.available_height() - bottom_reserve).max(50.0);

            // === Scrollable content: chat messages ===
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .max_height(available_height)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if !self.chat.messages.is_empty() || self.chat.streaming {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Chat")
                                .color(Color32::from_gray(160))
                                .small()
                                .strong(),
                        );
                        ui.add_space(4.0);

                        let chat_action =
                            Self::render_chat_messages(ui, &self.chat, self.chat_font_size, self.agent_terminal_access);
                        if !matches!(chat_action, InspectorAction::None) {
                            action = chat_action;
                        }
                    }
                });

            // === Pinned bottom: Chat input + checkbox + action bar ===
            if self.agent_status == AgentStatus::Connected {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                let input_action = self.render_chat_input(ui);
                if !matches!(input_action, InspectorAction::None) {
                    action = input_action;
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut self.agent_terminal_access,
                            RichText::new("Terminal access")
                                .small()
                                .color(Color32::from_gray(160)),
                        )
                        .on_hover_text(
                            "When enabled, shell commands the agent writes in fenced code \
                             blocks (```bash / ```sh) are automatically sent to your active \
                             terminal and executed. The agent is notified of the exit code \
                             after each command completes.",
                        )
                        .changed()
                    {
                        action = InspectorAction::SetTerminalAccess(self.agent_terminal_access);
                    }
                    let yolo_color = if self.auto_approve {
                        Color32::from_rgb(255, 193, 7)
                    } else {
                        Color32::from_gray(160)
                    };
                    if ui
                        .checkbox(
                            &mut self.auto_approve,
                            RichText::new("YOLO").small().color(yolo_color),
                        )
                        .on_hover_text("Auto-approve all agent permission requests")
                        .changed()
                    {
                        let mode = if self.auto_approve {
                            "bypassPermissions"
                        } else {
                            "default"
                        };
                        action = InspectorAction::SetAgentMode(mode.to_string());
                    }
                });
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
            let bar_action = self.render_action_bar(ui);
            if !matches!(bar_action, InspectorAction::None) {
                action = bar_action;
            }
        });

        (action, close_requested)
    }
}
