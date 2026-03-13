//! Trigger edit form: name, pattern, actions list, save/cancel.

use super::action_fields::show_action_fields;
use super::{ACTION_TYPE_NAMES, default_action_for_type};
use crate::SettingsUI;
use par_term_config::automation::{TriggerActionConfig, TriggerConfig};

/// Show the inline edit form for a trigger.
///
/// `edit_index` is `Some(i)` when editing an existing trigger, `None` when adding a new one.
pub(super) fn show_trigger_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.indent("trigger_edit_form", |ui| {
        // Name field
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_trigger_name);
        });

        // Pattern field with regex validation
        ui.horizontal(|ui| {
            ui.label("Pattern:");
            let response = ui.text_edit_singleline(&mut settings.temp_trigger_pattern);
            if response.changed() {
                // Validate regex
                settings.trigger_pattern_error =
                    match regex::Regex::new(&settings.temp_trigger_pattern) {
                        Ok(_) => None,
                        Err(e) => Some(format!("Invalid regex: {e}")),
                    };
            }
        });
        if let Some(ref err) = settings.trigger_pattern_error {
            ui.colored_label(egui::Color32::RED, err);
        }

        // Security: prompt_before_run checkbox
        // Only shown when the trigger has dangerous actions (RunCommand, SendText).
        let has_dangerous = settings
            .temp_trigger_actions
            .iter()
            .any(|a| a.is_dangerous());
        if has_dangerous {
            ui.add_space(4.0);
            ui.checkbox(
                &mut settings.temp_trigger_prompt_before_run,
                "Prompt before running dangerous actions",
            )
            .on_hover_text(
                "When enabled, a confirmation dialog is shown before RunCommand, SendText, \
                 or SplitPane actions execute. Disable to allow the trigger to run automatically.",
            );
            if !settings.temp_trigger_prompt_before_run {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 160, 50),
                    "Warning: Dangerous actions can be triggered by terminal output. \
                     A command denylist and rate limiter still apply.",
                );
            }
        }

        // Actions list
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Actions:").strong());

        let mut action_delete_index: Option<usize> = None;
        for (j, action) in settings.temp_trigger_actions.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", j + 1));
                ui.label(
                    egui::RichText::new(action_type_label(action))
                        .color(egui::Color32::from_rgb(120, 180, 255)),
                );
                show_action_fields(ui, action);
                if ui
                    .small_button(
                        egui::RichText::new("x").color(egui::Color32::from_rgb(200, 80, 80)),
                    )
                    .clicked()
                {
                    action_delete_index = Some(j);
                }
            });
        }

        if let Some(j) = action_delete_index {
            settings.temp_trigger_actions.remove(j);
        }

        // Add action combo
        ui.horizontal(|ui| {
            let combo_id = if edit_index.is_some() {
                "trigger_add_action_edit"
            } else {
                "trigger_add_action_new"
            };
            let mut selected_type: usize = 0;
            egui::ComboBox::from_id_salt(combo_id)
                .selected_text("+ Add action...")
                .show_ui(ui, |ui| {
                    for (idx, name) in ACTION_TYPE_NAMES.iter().enumerate() {
                        if ui
                            .selectable_value(&mut selected_type, idx + 1, *name)
                            .clicked()
                        {
                            // selected_type is set by selectable_value
                        }
                    }
                });
            if selected_type > 0 {
                settings
                    .temp_trigger_actions
                    .push(default_action_for_type(selected_type - 1));
            }
        });

        ui.add_space(4.0);

        // Save / Cancel buttons
        show_save_cancel(ui, settings, changes_this_frame, edit_index);
    });
}

/// Return a human-readable label for a trigger action variant.
fn action_type_label(action: &TriggerActionConfig) -> &'static str {
    match action {
        TriggerActionConfig::Highlight { .. } => "Highlight",
        TriggerActionConfig::Notify { .. } => "Notify",
        TriggerActionConfig::MarkLine { .. } => "Mark Line",
        TriggerActionConfig::SetVariable { .. } => "Set Variable",
        TriggerActionConfig::RunCommand { .. } => "Run Command",
        TriggerActionConfig::PlaySound { .. } => "Play Sound",
        TriggerActionConfig::SendText { .. } => "Send Text",
        TriggerActionConfig::Prettify { .. } => "Prettify",
        TriggerActionConfig::SplitPane { .. } => "Split Pane",
    }
}

/// Show the Save / Cancel button row and persist changes if save is clicked.
fn show_save_cancel(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.horizontal(|ui| {
        let can_save = !settings.temp_trigger_name.trim().is_empty()
            && !settings.temp_trigger_pattern.trim().is_empty()
            && settings.trigger_pattern_error.is_none();

        if ui
            .add_enabled(can_save, egui::Button::new("Save"))
            .clicked()
        {
            let new_trigger = TriggerConfig {
                name: settings.temp_trigger_name.trim().to_string(),
                pattern: settings.temp_trigger_pattern.trim().to_string(),
                enabled: true,
                actions: settings.temp_trigger_actions.clone(),
                prompt_before_run: settings.temp_trigger_prompt_before_run,
                // Preserve existing i_accept_the_risk when editing; default false for new triggers.
                i_accept_the_risk: edit_index
                    .and_then(|i| settings.config.triggers.get(i))
                    .map(|t| t.i_accept_the_risk)
                    .unwrap_or(false),
            };

            if let Some(i) = edit_index {
                // Update existing
                let was_enabled = settings.config.triggers[i].enabled;
                settings.config.triggers[i] = new_trigger;
                settings.config.triggers[i].enabled = was_enabled;
            } else {
                // Add new
                settings.config.triggers.push(new_trigger);
            }

            settings.has_changes = true;
            *changes_this_frame = true;
            settings.editing_trigger_index = None;
            settings.adding_new_trigger = false;
            settings.trigger_resync_requested = true;
        }

        if ui.button("Cancel").clicked() {
            settings.editing_trigger_index = None;
            settings.adding_new_trigger = false;
            settings.trigger_pattern_error = None;
        }
    });
}
