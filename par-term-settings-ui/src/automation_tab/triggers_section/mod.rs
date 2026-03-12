//! Triggers section of the automation settings tab.
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show_triggers_section()` dispatcher, shared constants/helpers |
//! | `editor.rs` | Trigger edit form (name, pattern, actions, save/cancel) |
//! | `action_fields.rs` | Inline field rendering for each `TriggerActionConfig` variant |

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use par_term_config::automation::TriggerActionConfig;
use std::collections::HashSet;

mod action_fields;
mod editor;

/// Action type display names (aligned with `default_action_for_type` index).
const ACTION_TYPE_NAMES: &[&str] = &[
    "Highlight",
    "Notify",
    "Mark Line",
    "Set Variable",
    "Run Command",
    "Play Sound",
    "Send Text",
    "Prettify",
];

/// Create a default action for the given type index.
fn default_action_for_type(type_index: usize) -> TriggerActionConfig {
    match type_index {
        0 => TriggerActionConfig::Highlight {
            fg: None,
            bg: Some([255, 255, 0]),
            duration_ms: 5000,
        },
        1 => TriggerActionConfig::Notify {
            title: "Trigger".to_string(),
            message: "Pattern matched".to_string(),
        },
        2 => TriggerActionConfig::MarkLine {
            label: None,
            color: Some([0, 180, 255]),
        },
        3 => TriggerActionConfig::SetVariable {
            name: String::new(),
            value: String::new(),
        },
        4 => TriggerActionConfig::RunCommand {
            command: String::new(),
            args: Vec::new(),
        },
        5 => TriggerActionConfig::PlaySound {
            sound_id: String::new(),
            volume: 50,
        },
        6 => TriggerActionConfig::SendText {
            text: String::new(),
            delay_ms: 0,
        },
        7 => TriggerActionConfig::Prettify {
            format: "json".to_string(),
            scope: crate::config::automation::PrettifyScope::default(),
            block_end: None,
            sub_format: None,
            command_filter: None,
        },
        _ => TriggerActionConfig::Highlight {
            fg: None,
            bg: Some([255, 255, 0]),
            duration_ms: 5000,
        },
    }
}

pub(super) fn show_triggers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Triggers",
        &[
            "trigger",
            "regex",
            "pattern",
            "match",
            "action",
            "highlight",
            "notify",
            "badge",
            "set variable",
            "automatic",
        ],
    ) {
        show_triggers_collapsing(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_triggers_collapsing(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Triggers",
        "automation_triggers",
        true,
        collapsed,
        |ui| {
            ui.label("Define regex patterns to match terminal output and trigger actions.");
            ui.add_space(4.0);

            // SEC-002: Section-level warning banner when any trigger has
            // `prompt_before_run: false` AND contains a dangerous action.
            // Individual per-trigger warnings are shown in the edit form and
            // list row; this banner gives a prominent at-a-glance signal when
            // opening the Automation tab.
            let has_unsafe_trigger = settings
                .config
                .triggers
                .iter()
                .any(|t| !t.prompt_before_run && t.actions.iter().any(|a| a.is_dangerous()));
            if has_unsafe_trigger {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(80, 50, 10))
                    .inner_margin(egui::Margin::symmetric(8_i8, 6_i8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new("Security Warning:")
                                    .strong()
                                    .color(egui::Color32::from_rgb(255, 190, 60)),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "One or more triggers have `prompt_before_run: false` \
                                     with dangerous actions (RunCommand / SendText). \
                                     These can be fired directly by terminal output — \
                                     malicious content could exploit pattern matching to \
                                     execute commands. The command denylist provides only \
                                     limited protection. Review [unsafe] triggers below.",
                                )
                                .color(egui::Color32::from_rgb(220, 180, 100)),
                            );
                        });
                    });
                ui.add_space(6.0);
            }

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut toggle_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;

            // List existing triggers
            let trigger_count = settings.config.triggers.len();
            for i in 0..trigger_count {
                let trigger = &settings.config.triggers[i];
                let is_editing =
                    settings.editing_trigger_index == Some(i) && !settings.adding_new_trigger;

                if is_editing {
                    // Show inline edit form for this trigger
                    editor::show_trigger_edit_form(ui, settings, changes_this_frame, Some(i));
                } else {
                    // Show trigger summary row
                    show_trigger_row(
                        ui,
                        trigger,
                        i,
                        &mut toggle_index,
                        &mut start_edit_index,
                        &mut delete_index,
                    );
                }
            }

            // Apply mutations after iteration
            if let Some(i) = toggle_index {
                settings.config.triggers[i].enabled = !settings.config.triggers[i].enabled;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if let Some(i) = delete_index {
                settings.config.triggers.remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
                // If we were editing this trigger, cancel the edit
                if settings.editing_trigger_index == Some(i) {
                    settings.editing_trigger_index = None;
                }
            }
            if let Some(i) = start_edit_index {
                let trigger = &settings.config.triggers[i];
                settings.editing_trigger_index = Some(i);
                settings.adding_new_trigger = false;
                settings.temp_trigger_name = trigger.name.clone();
                settings.temp_trigger_pattern = trigger.pattern.clone();
                settings.temp_trigger_actions = trigger.actions.clone();
                settings.temp_trigger_prompt_before_run = trigger.prompt_before_run;
                settings.trigger_pattern_error = None;
            }

            ui.add_space(4.0);

            // Add new trigger button / form
            if settings.adding_new_trigger {
                ui.separator();
                ui.label(egui::RichText::new("New Trigger").strong());
                editor::show_trigger_edit_form(ui, settings, changes_this_frame, None);
            } else if settings.editing_trigger_index.is_none()
                && ui
                    .button("+ Add Trigger")
                    .on_hover_text("Add a new trigger definition")
                    .clicked()
            {
                settings.adding_new_trigger = true;
                settings.editing_trigger_index = None;
                settings.temp_trigger_name = String::new();
                settings.temp_trigger_pattern = String::new();
                settings.temp_trigger_actions = Vec::new();
                settings.temp_trigger_prompt_before_run = true;
                settings.trigger_pattern_error = None;
            }
        },
    );
}

/// Render a single trigger summary row (when not in edit mode).
fn show_trigger_row(
    ui: &mut egui::Ui,
    trigger: &par_term_config::automation::TriggerConfig,
    i: usize,
    toggle_index: &mut Option<usize>,
    start_edit_index: &mut Option<usize>,
    delete_index: &mut Option<usize>,
) {
    ui.horizontal(|ui| {
        // Enabled checkbox
        let mut enabled = trigger.enabled;
        if ui.checkbox(&mut enabled, "").changed() {
            *toggle_index = Some(i);
        }

        // Name (bold)
        ui.label(egui::RichText::new(&trigger.name).strong());

        // Pattern (monospace)
        ui.label(
            egui::RichText::new(format!("/{}/", &trigger.pattern))
                .monospace()
                .color(egui::Color32::from_rgb(150, 150, 200)),
        );

        // Action count
        let action_count = trigger.actions.len();
        ui.label(
            egui::RichText::new(format!(
                "{} action{}",
                action_count,
                if action_count == 1 { "" } else { "s" }
            ))
            .color(egui::Color32::GRAY),
        );

        // Security indicator: warn if trigger allows dangerous
        // actions from terminal output
        if !trigger.prompt_before_run && trigger.actions.iter().any(|a| a.is_dangerous()) {
            ui.label(
                egui::RichText::new("[unsafe]")
                    .small()
                    .color(egui::Color32::from_rgb(220, 160, 50)),
            )
            .on_hover_text(
                "This trigger can execute dangerous actions \
                 (RunCommand/SendText) from passive terminal output",
            );
        }

        // Edit button
        if ui.small_button("Edit").clicked() {
            *start_edit_index = Some(i);
        }

        // Delete button
        if ui
            .small_button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(200, 80, 80)))
            .clicked()
        {
            *delete_index = Some(i);
        }
    });
}
