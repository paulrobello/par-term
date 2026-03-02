//! Permissions section: auto-approve (yolo mode), terminal access, screenshot access.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use std::collections::HashSet;

pub(super) fn show_permissions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Permissions",
        &[
            "approve",
            "auto-approve",
            "yolo",
            "permission",
            "screenshot",
            "image",
        ],
    ) {
        collapsing_section(
            ui,
            "Permissions",
            "ai_inspector_permissions",
            true,
            collapsed,
            |ui| {
                let yolo_response = ui
                    .checkbox(
                        &mut settings.config.ai_inspector.ai_inspector_auto_approve,
                        "Yolo Mode",
                    )
                    .on_hover_text("Auto-approve all agent permission requests. Use with caution!");
                if yolo_response.changed() {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if settings.config.ai_inspector.ai_inspector_auto_approve {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 193, 7),
                        "Most agent permission requests will be auto-approved \
                         (screenshots still prompt)",
                    );
                }

                ui.add_space(8.0);

                let terminal_access_response = ui
                    .checkbox(
                        &mut settings
                            .config
                            .ai_inspector
                            .ai_inspector_agent_terminal_access,
                        "Allow Terminal Access",
                    )
                    .on_hover_text(
                        "Allow the AI agent to write input directly to the terminal. \
                         When enabled, command suggestions from the agent will be \
                         auto-executed in the active terminal.",
                    );
                if terminal_access_response.changed() {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if settings
                    .config
                    .ai_inspector
                    .ai_inspector_agent_terminal_access
                {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 152, 0),
                        "Agent can write commands to the terminal",
                    );
                }

                ui.add_space(8.0);

                let screenshot_access_response = ui
                    .checkbox(
                        &mut settings
                            .config
                            .ai_inspector
                            .ai_inspector_agent_screenshot_access,
                        "Allow Agent Screenshots",
                    )
                    .on_hover_text(
                        "Allow the agent to request terminal screenshots via the \
                         `terminal_screenshot` MCP tool for visual debugging. \
                         Screenshot captures remain permission-gated per request and \
                         are not auto-approved by Yolo Mode.",
                    );
                if screenshot_access_response.changed() {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if settings
                    .config
                    .ai_inspector
                    .ai_inspector_agent_screenshot_access
                {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 193, 7),
                        "Screenshot requests still require per-request approval in chat",
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 152, 0),
                        "Agent screenshot requests will be denied",
                    );
                }
            },
        );
    }
}
