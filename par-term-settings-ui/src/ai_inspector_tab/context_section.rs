//! Context and panel sections of the AI Inspector settings tab.
//!
//! Covers: Panel section (enabled, width, scope, view mode, live update, zones)
//! and Agent section (default agent, auto-launch, auto-context, max context lines).

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use std::collections::HashSet;

/// Build the combined agent list (built-in + active custom agents).
pub(super) fn combined_available_agents(settings: &SettingsUI) -> Vec<(String, String)> {
    let mut combined = settings.available_agent_ids.clone();
    for custom in &settings.config.ai_inspector.ai_inspector_custom_agents {
        if custom.active == Some(false) {
            continue;
        }
        let label = if custom.name.trim().is_empty() {
            custom.identity.clone()
        } else {
            format!("{} (custom)", custom.name)
        };
        if let Some(existing) = combined.iter_mut().find(|(id, _)| *id == custom.identity) {
            existing.1 = label;
        } else {
            combined.push((custom.identity.clone(), label));
        }
    }
    combined
}

pub(super) fn show_panel_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Panel",
        &[
            "enabled", "width", "scope", "view", "live", "update", "zones", "cards", "timeline",
            "tree",
        ],
    ) {
        collapsing_section(ui, "Panel", "ai_inspector_panel", true, collapsed, |ui| {
            if ui
                .checkbox(
                    &mut settings.config.ai_inspector.ai_inspector_enabled,
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
                    &mut settings.config.ai_inspector.ai_inspector_open_on_startup,
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
                        egui::Slider::new(
                            &mut settings.config.ai_inspector.ai_inspector_width,
                            200.0..=600.0,
                        )
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
                    .selected_text(&settings.config.ai_inspector.ai_inspector_default_scope)
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
                                    &mut settings.config.ai_inspector.ai_inspector_default_scope,
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
                    .selected_text(&settings.config.ai_inspector.ai_inspector_view_mode)
                    .show_ui(ui, |ui| {
                        for mode in &["cards", "timeline", "tree", "list_detail"] {
                            if ui
                                .selectable_value(
                                    &mut settings.config.ai_inspector.ai_inspector_view_mode,
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
                .checkbox(
                    &mut settings.config.ai_inspector.ai_inspector_live_update,
                    "Live update",
                )
                .on_hover_text("Automatically refresh panel content when terminal changes")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.ai_inspector.ai_inspector_show_zones,
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
}

pub(super) fn show_agent_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
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
        collapsing_section(ui, "Agent", "ai_inspector_agent", true, collapsed, |ui| {
            let all_agents = combined_available_agents(settings);

            ui.horizontal(|ui| {
                ui.label("Default agent:");
                // Find display name for the currently selected agent
                let selected_display = all_agents
                    .iter()
                    .find(|(id, _)| *id == settings.config.ai_inspector.ai_inspector_agent)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or(&settings.config.ai_inspector.ai_inspector_agent);
                egui::ComboBox::from_id_salt("ai_agent")
                    .selected_text(selected_display)
                    .show_ui(ui, |ui| {
                        for (agent_id, agent_name) in &all_agents {
                            if ui
                                .selectable_value(
                                    &mut settings.config.ai_inspector.ai_inspector_agent,
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
                    &mut settings.config.ai_inspector.ai_inspector_auto_launch,
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
                    &mut settings.config.ai_inspector.ai_inspector_auto_context,
                    "Auto-send context",
                )
                .on_hover_text(
                    "Automatically send command results to the agent when commands complete",
                )
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
                        &mut settings.config.ai_inspector.ai_inspector_context_max_lines,
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
}
