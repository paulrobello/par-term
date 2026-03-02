//! Chat message and rich-text rendering for [`AIInspectorPanel`].
//!
//! Contains `render_chat_messages` and `render_rich_text`, extracted from
//! `chat_view.rs` to keep that file under 500 lines.

use egui::{Color32, Frame, Label, RichText, Stroke};

use crate::ai_inspector::chat::{ChatMessage, ChatState, TextSegment, parse_text_segments};

use super::types::{
    AGENT_CONNECTED, AGENT_MSG_BG, CMD_SUGGEST_BG, CODE_BLOCK_BG, CODE_LANG_COLOR, EXIT_FAILURE,
    SYSTEM_MSG_COLOR, USER_MSG_BG,
};
use super::{AIInspectorPanel, InspectorAction};

impl AIInspectorPanel {
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
}
