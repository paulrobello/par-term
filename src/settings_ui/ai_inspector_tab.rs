//! AI Inspector settings tab.
//!
//! Contains:
//! - Panel settings (enabled, width, scope, view mode)
//! - Agent settings (default agent, auto-launch, auto-context)
//! - Permission settings (auto-approve / yolo mode)

use super::SettingsUI;
use super::section::collapsing_section;
use std::collections::HashSet;

/// Show the AI Inspector tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Panel section
    if section_matches(
        &query,
        "Panel",
        &[
            "enabled", "width", "scope", "view", "live", "update", "zones", "cards", "timeline",
            "tree",
        ],
    ) {
        show_panel_section(ui, settings, changes_this_frame, collapsed);
    }

    // Agent section
    if section_matches(
        &query,
        "Agent",
        &[
            "agent",
            "launch",
            "auto-launch",
            "context",
            "auto-context",
            "max lines",
        ],
    ) {
        show_agent_section(ui, settings, changes_this_frame, collapsed);
    }

    // Permissions section
    if section_matches(
        &query,
        "Permissions",
        &["approve", "auto-approve", "yolo", "permission"],
    ) {
        show_permissions_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Panel Section
// ============================================================================

fn show_panel_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Panel", "ai_inspector_panel", true, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.ai_inspector_enabled,
                "Enable Assistant Panel",
            )
            .on_hover_text("Show Assistant panel toggle keybinding (Cmd+I / Ctrl+Shift+I)")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_open_on_startup,
                "Open on startup",
            )
            .on_hover_text("Automatically open the Assistant panel when a new window opens")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default width:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.ai_inspector_width, 200.0..=600.0)
                        .suffix("px"),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default scope:");
            egui::ComboBox::from_id_salt("ai_scope")
                .selected_text(&settings.config.ai_inspector_default_scope)
                .show_ui(ui, |ui| {
                    for scope in &[
                        "visible",
                        "recent_5",
                        "recent_10",
                        "recent_25",
                        "recent_50",
                        "full",
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_default_scope,
                                scope.to_string(),
                                *scope,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default view:");
            egui::ComboBox::from_id_salt("ai_view")
                .selected_text(&settings.config.ai_inspector_view_mode)
                .show_ui(ui, |ui| {
                    for mode in &["cards", "timeline", "tree", "list_detail"] {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_view_mode,
                                mode.to_string(),
                                *mode,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        if ui
            .checkbox(&mut settings.config.ai_inspector_live_update, "Live update")
            .on_hover_text("Automatically refresh panel content when terminal changes")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_show_zones,
                "Show zone content",
            )
            .on_hover_text(
                "Display command zones in the panel (disable for compact agent-only mode)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Agent Section
// ============================================================================

fn show_agent_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Agent", "ai_inspector_agent", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Default agent:");
            // Find display name for the currently selected agent
            let selected_display = settings
                .available_agent_ids
                .iter()
                .find(|(id, _)| *id == settings.config.ai_inspector_agent)
                .map(|(_, name)| name.as_str())
                .unwrap_or(&settings.config.ai_inspector_agent);
            egui::ComboBox::from_id_salt("ai_agent")
                .selected_text(selected_display)
                .show_ui(ui, |ui| {
                    for (agent_id, agent_name) in &settings.available_agent_ids {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_agent,
                                agent_id.clone(),
                                agent_name,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_auto_launch,
                "Auto-launch agent",
            )
            .on_hover_text("Automatically connect to the configured agent when the panel opens")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_auto_context,
                "Auto-send context",
            )
            .on_hover_text("Automatically send command results to the agent when commands complete")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Max context lines:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.ai_inspector_context_max_lines,
                    50..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Permissions Section
// ============================================================================

fn show_permissions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Permissions",
        "ai_inspector_permissions",
        true,
        collapsed,
        |ui| {
            let yolo_response = ui
                .checkbox(&mut settings.config.ai_inspector_auto_approve, "Yolo Mode")
                .on_hover_text("Auto-approve all agent permission requests. Use with caution!");
            if yolo_response.changed() {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.config.ai_inspector_auto_approve {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 193, 7),
                    "All agent permission requests will be auto-approved",
                );
            }

            ui.add_space(8.0);

            let terminal_access_response = ui
                .checkbox(
                    &mut settings.config.ai_inspector_agent_terminal_access,
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
            if settings.config.ai_inspector_agent_terminal_access {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 152, 0),
                    "Agent can write commands to the terminal",
                );
            }
        },
    );
}
