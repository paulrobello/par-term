//! Arrangements settings tab.
//!
//! Contains:
//! - List of saved arrangements with restore/rename/delete/reorder controls
//! - Save current layout button
//! - Auto-restore on startup setting

use super::SettingsUI;
use super::section::collapsing_section;
use crate::arrangements::ArrangementManager;
use crate::settings_window::SettingsWindowAction;
use std::collections::HashSet;

/// Show the arrangements tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    if section_matches(
        &query,
        "Save Current Layout",
        &["save", "capture", "current", "window", "layout", "snapshot"],
    ) {
        show_save_section(ui, settings, collapsed);
    }

    if section_matches(
        &query,
        "Saved Arrangements",
        &[
            "arrangement",
            "restore",
            "rename",
            "delete",
            "layout",
            "workspace",
        ],
    ) {
        show_arrangements_list(ui, settings, collapsed);
    }

    if section_matches(
        &query,
        "Auto-Restore",
        &["auto", "startup", "restore", "default", "launch", "open"],
    ) {
        show_auto_restore_section(ui, settings, changes_this_frame, collapsed);
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
// Save Current Layout Section
// ============================================================================

fn show_save_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Save Current Layout",
        "arrangements_save",
        true,
        collapsed,
        |ui| {
            ui.label("Save the current window arrangement (positions, sizes, and tab working directories) for later restoration.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut settings.arrangement_save_name);

                let name_valid = !settings.arrangement_save_name.trim().is_empty();
                if ui
                    .add_enabled(name_valid, egui::Button::new("Save"))
                    .clicked()
                {
                    let name = settings.arrangement_save_name.trim().to_string();
                    if settings.arrangement_manager.find_by_name(&name).is_some() {
                        settings.arrangement_confirm_overwrite = Some(name);
                    } else {
                        settings
                            .pending_arrangement_actions
                            .push(SettingsWindowAction::SaveArrangement(name));
                        settings.arrangement_save_name.clear();
                    }
                }
            });

            show_confirm_overwrite_dialog(ui, settings);
        },
    );
}

// ============================================================================
// Saved Arrangements List
// ============================================================================

fn show_arrangements_list(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Saved Arrangements",
        "arrangements_list",
        true,
        collapsed,
        |ui| {
            let manager = settings.arrangement_manager.clone();
            show_arrangements_with_manager(ui, settings, &manager);

            ui.add_space(4.0);

            // Show confirmation dialogs
            show_confirm_restore_dialog(ui, settings);
            show_confirm_delete_dialog(ui, settings);
            show_rename_dialog(ui, settings);
        },
    );
}

/// Render the arrangements list with data from the ArrangementManager.
///
/// Called by the settings window with the actual manager data.
pub fn show_arrangements_with_manager(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    manager: &ArrangementManager,
) {
    if manager.is_empty() {
        ui.label(
            egui::RichText::new("No saved arrangements yet.")
                .italics()
                .color(egui::Color32::from_rgb(150, 150, 150)),
        );
        return;
    }

    let arrangements = manager.arrangements_ordered();
    for (i, arr) in arrangements.iter().enumerate() {
        let id = arr.id;
        let total_tabs: usize = arr.windows.iter().map(|w| w.tabs.len()).sum();

        ui.horizontal(|ui| {
            // Name and summary
            ui.label(
                egui::RichText::new(&arr.name)
                    .strong()
                    .color(egui::Color32::from_rgb(220, 220, 220)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "({} window{}, {} tab{})",
                    arr.windows.len(),
                    if arr.windows.len() == 1 { "" } else { "s" },
                    total_tabs,
                    if total_tabs == 1 { "" } else { "s" },
                ))
                .color(egui::Color32::from_rgb(140, 140, 140)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Reorder buttons
                if i < arrangements.len() - 1 && ui.small_button("▼").clicked() {
                    settings.pending_arrangement_actions.push(
                        SettingsWindowAction::RenameArrangement(id, "__move_down__".to_string()),
                    );
                }
                if i > 0 && ui.small_button("▲").clicked() {
                    settings.pending_arrangement_actions.push(
                        SettingsWindowAction::RenameArrangement(id, "__move_up__".to_string()),
                    );
                }

                if ui.small_button("Delete").clicked() {
                    settings.arrangement_confirm_delete = Some(id);
                }
                if ui.small_button("Rename").clicked() {
                    settings.arrangement_rename_id = Some(id);
                    settings.arrangement_rename_text = arr.name.clone();
                }
                if ui.small_button("Restore").clicked() {
                    settings.arrangement_confirm_restore = Some(id);
                }
            });
        });

        // Show created date if available
        if !arr.created_at.is_empty() {
            ui.label(
                egui::RichText::new(format!("  Created: {}", format_date(&arr.created_at)))
                    .small()
                    .color(egui::Color32::from_rgb(100, 100, 100)),
            );
        }

        if i < arrangements.len() - 1 {
            ui.separator();
        }
    }
}

fn format_date(iso: &str) -> String {
    // Simple formatting: just show the date portion
    if let Some(date_part) = iso.split('T').next() {
        date_part.to_string()
    } else {
        iso.to_string()
    }
}

// ============================================================================
// Confirmation Dialogs
// ============================================================================

fn show_confirm_overwrite_dialog(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    if let Some(name) = settings.arrangement_confirm_overwrite.clone() {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "⚠ An arrangement named \"{}\" already exists.",
                    name
                ))
                .strong()
                .color(egui::Color32::from_rgb(255, 193, 7)),
            );
            ui.label("Do you want to overwrite it?");
            ui.horizontal(|ui| {
                if ui.button("Overwrite").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::SaveArrangement(name));
                    settings.arrangement_confirm_overwrite = None;
                    settings.arrangement_save_name.clear();
                }
                if ui.button("Cancel").clicked() {
                    settings.arrangement_confirm_overwrite = None;
                }
            });
        });
    }
}

fn show_confirm_restore_dialog(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    if let Some(id) = settings.arrangement_confirm_restore {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(
                egui::RichText::new("⚠ Restore this arrangement?")
                    .strong()
                    .color(egui::Color32::from_rgb(255, 193, 7)),
            );
            ui.label("This will close all current windows and restore the saved layout.");
            ui.horizontal(|ui| {
                if ui.button("Restore").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::RestoreArrangement(id));
                    settings.arrangement_confirm_restore = None;
                }
                if ui.button("Cancel").clicked() {
                    settings.arrangement_confirm_restore = None;
                }
            });
        });
    }
}

fn show_confirm_delete_dialog(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    if let Some(id) = settings.arrangement_confirm_delete {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(
                egui::RichText::new("⚠ Delete this arrangement?")
                    .strong()
                    .color(egui::Color32::from_rgb(244, 67, 54)),
            );
            ui.label("This cannot be undone.");
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::DeleteArrangement(id));
                    settings.arrangement_confirm_delete = None;
                }
                if ui.button("Cancel").clicked() {
                    settings.arrangement_confirm_delete = None;
                }
            });
        });
    }
}

fn show_rename_dialog(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    if let Some(id) = settings.arrangement_rename_id {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new("Rename arrangement").strong());
            ui.horizontal(|ui| {
                ui.label("New name:");
                ui.text_edit_singleline(&mut settings.arrangement_rename_text);

                let valid = !settings.arrangement_rename_text.trim().is_empty();
                if ui.add_enabled(valid, egui::Button::new("Rename")).clicked() {
                    let new_name = settings.arrangement_rename_text.trim().to_string();
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::RenameArrangement(id, new_name));
                    settings.arrangement_rename_id = None;
                    settings.arrangement_rename_text.clear();
                }
                if ui.button("Cancel").clicked() {
                    settings.arrangement_rename_id = None;
                    settings.arrangement_rename_text.clear();
                }
            });
        });
    }
}

// ============================================================================
// Auto-Restore Section
// ============================================================================

fn show_auto_restore_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Auto-Restore on Startup",
        "arrangements_auto_restore",
        true,
        collapsed,
        |ui| {
            ui.label("Automatically restore a saved arrangement when par-term starts.");
            ui.add_space(8.0);

            let current = settings
                .config
                .auto_restore_arrangement
                .clone()
                .unwrap_or_default();

            let display = if current.is_empty() {
                "None (disabled)"
            } else {
                &current
            };

            // Build list of arrangement names for the dropdown
            let arrangements = settings.arrangement_manager.arrangements_ordered();
            let names: Vec<&str> = arrangements.iter().map(|a| a.name.as_str()).collect();

            ui.horizontal(|ui| {
                ui.label("Auto-restore:");
                egui::ComboBox::from_id_salt("auto_restore_arrangement")
                    .selected_text(display)
                    .show_ui(ui, |ui| {
                        // "None" option to disable
                        if ui
                            .selectable_label(current.is_empty(), "None (disabled)")
                            .clicked()
                        {
                            settings.config.auto_restore_arrangement = None;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }

                        // One option per saved arrangement
                        for name in &names {
                            let selected = current == *name;
                            if ui.selectable_label(selected, *name).clicked() {
                                settings.config.auto_restore_arrangement =
                                    Some((*name).to_string());
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            if names.is_empty() {
                ui.label(
                    egui::RichText::new("Save an arrangement first to enable auto-restore.")
                        .small()
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
            }
        },
    );
}
