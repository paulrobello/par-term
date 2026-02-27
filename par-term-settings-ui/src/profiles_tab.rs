//! Profiles settings tab.
//!
//! Contains:
//! - Inline profile management (create, edit, delete, reorder)
//! - Display options for the profile drawer
//! - Dynamic profile sources management

use super::SettingsUI;
use super::section::{collapsing_section, collapsing_section_with_state, section_matches};
use crate::profile_modal_ui::ProfileModalAction;
use par_term_config::ConflictResolution;
use par_term_config::DynamicProfileSource;
use std::collections::HashSet;

/// Show the profiles tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Profile management section (inline)
    if section_matches(
        &query,
        "Profile Management",
        &[
            "profile",
            "manage",
            "create",
            "edit",
            "delete",
            "duplicate",
            "default",
        ],
    ) {
        show_management_section(ui, settings, collapsed);
    }

    // Display options section
    if section_matches(
        &query,
        "Display Options",
        &[
            "drawer",
            "button",
            "toggle",
            "show",
            "hide",
            "profile indicator",
        ],
    ) {
        show_display_options_section(ui, settings, changes_this_frame, collapsed);
    }

    // Dynamic profile sources section
    if section_matches(
        &query,
        "Dynamic Profile Sources",
        &[
            "dynamic", "remote", "url", "fetch", "refresh", "team", "shared", "download", "sync",
        ],
    ) {
        show_dynamic_sources_section(ui, settings, changes_this_frame, collapsed);
    }
}

// ============================================================================
// Profile Management Section (inline)
// ============================================================================

fn show_management_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Profile Management",
        "profiles_management",
        true,
        collapsed,
        |ui, collapsed| {
            // Render the profile list/edit UI inline
            let action = settings.profile_modal_ui.show_inline(ui, collapsed);

            // Handle returned actions
            match action {
                ProfileModalAction::Save => {
                    settings.profile_save_requested = true;
                }
                ProfileModalAction::OpenProfile(id) => {
                    settings.profile_open_requested = Some(id);
                }
                ProfileModalAction::Cancel | ProfileModalAction::None => {}
            }
        },
    );
}

// ============================================================================
// Display Options Section
// ============================================================================

fn show_display_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Display Options",
        "profiles_display",
        true,
        collapsed,
        |ui| {
            if ui
            .checkbox(
                &mut settings.config.show_profile_drawer_button,
                "Show profile drawer toggle button",
            )
            .on_hover_text("Show/hide the profile drawer toggle button on the right edge of the terminal window. The drawer can still be accessed via keyboard shortcuts when hidden.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            ui.add_space(8.0);

            ui.label(
            egui::RichText::new("The profile drawer provides quick access to your profiles without opening the full settings window.")
                .small()
                .color(egui::Color32::GRAY),
        );
        },
    );
}

// ============================================================================
// Dynamic Profile Sources Section
// ============================================================================

fn show_dynamic_sources_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Dynamic Profile Sources",
        "profiles_dynamic_sources",
        true,
        collapsed,
        |ui, collapsed| {
            ui.label(
                egui::RichText::new(
                    "Fetch profile definitions from remote URLs for team-shared configurations.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
            ui.add_space(4.0);

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut toggle_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;

            let source_count = settings.config.dynamic_profile_sources.len();

            if source_count == 0 && settings.dynamic_source_editing.is_none() {
                ui.label(
                    egui::RichText::new("No dynamic profile sources configured.")
                        .color(egui::Color32::GRAY),
                );
            }

            // Show each source
            for i in 0..source_count {
                let is_editing = settings.dynamic_source_editing == Some(i);

                if is_editing {
                    // Show inline edit form
                    show_dynamic_source_edit_form(
                        ui,
                        settings,
                        changes_this_frame,
                        Some(i),
                        collapsed,
                    );
                } else {
                    let source = &settings.config.dynamic_profile_sources[i];

                    ui.horizontal(|ui| {
                        // Enabled checkbox
                        let mut enabled = source.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            toggle_index = Some(i);
                        }

                        // URL (truncated)
                        let url_display = if source.url.len() > 60 {
                            format!("{}...", &source.url[..57])
                        } else {
                            source.url.clone()
                        };
                        ui.label(egui::RichText::new(&url_display).monospace().color(
                            if source.enabled {
                                egui::Color32::LIGHT_GRAY
                            } else {
                                egui::Color32::DARK_GRAY
                            },
                        ))
                        .on_hover_text(&source.url);

                        // Right-aligned buttons
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete button (rightmost)
                            if ui
                                .small_button(
                                    egui::RichText::new("Remove")
                                        .color(egui::Color32::from_rgb(200, 80, 80)),
                                )
                                .clicked()
                            {
                                delete_index = Some(i);
                            }

                            // Edit button
                            if ui.small_button("Edit").clicked() {
                                start_edit_index = Some(i);
                            }

                            // Status info
                            let conflict_label = source.conflict_resolution.display_name();
                            ui.label(
                                egui::RichText::new(conflict_label)
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });
                }
            }

            // Apply mutations after iteration
            if let Some(i) = delete_index {
                settings.config.dynamic_profile_sources.remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
                // Reset editing state if we deleted the item being edited
                if settings.dynamic_source_editing == Some(i) {
                    settings.dynamic_source_editing = None;
                    settings.dynamic_source_edit_buffer = None;
                } else if let Some(editing) = settings.dynamic_source_editing {
                    // Adjust editing index if a preceding item was deleted
                    if editing > i {
                        settings.dynamic_source_editing = Some(editing - 1);
                    }
                }
            }

            if let Some(i) = toggle_index {
                settings.config.dynamic_profile_sources[i].enabled =
                    !settings.config.dynamic_profile_sources[i].enabled;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if let Some(i) = start_edit_index {
                settings.dynamic_source_editing = Some(i);
                settings.dynamic_source_edit_buffer =
                    Some(settings.config.dynamic_profile_sources[i].clone());
                settings.dynamic_source_new_header_key = String::new();
                settings.dynamic_source_new_header_value = String::new();
            }

            ui.separator();

            // Show "add new" form if editing index is set to a new entry sentinel
            let is_adding = settings.dynamic_source_editing.is_some()
                && settings
                    .dynamic_source_editing
                    .expect("dynamic_source_editing checked is_some() above")
                    >= source_count;
            if is_adding {
                show_dynamic_source_edit_form(ui, settings, changes_this_frame, None, collapsed);
            } else if settings.dynamic_source_editing.is_none()
                && ui.button("+ Add Source").clicked()
            {
                // Use source_count as sentinel for "new entry"
                settings.dynamic_source_editing = Some(source_count);
                settings.dynamic_source_edit_buffer = Some(DynamicProfileSource::default());
                settings.dynamic_source_new_header_key = String::new();
                settings.dynamic_source_new_header_value = String::new();
            }
        },
    );
}

/// Show the edit form for a dynamic profile source.
///
/// `edit_index` is `Some(i)` when editing an existing source, `None` when adding a new one.
fn show_dynamic_source_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
    collapsed: &mut HashSet<String>,
) {
    ui.separator();

    // Save / Cancel buttons at top (always visible)
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            if let Some(buffer) = settings.dynamic_source_edit_buffer.take() {
                if let Some(i) = edit_index {
                    // Update existing source
                    settings.config.dynamic_profile_sources[i] = buffer;
                } else {
                    // Add new source
                    settings.config.dynamic_profile_sources.push(buffer);
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            settings.dynamic_source_editing = None;
        }

        if ui.button("Cancel").clicked() {
            settings.dynamic_source_editing = None;
            settings.dynamic_source_edit_buffer = None;
        }
    });

    ui.separator();

    // Edit form fields inside a scrollable area
    if let Some(ref mut source) = settings.dynamic_source_edit_buffer {
        egui::ScrollArea::vertical()
            .max_height(350.0)
            .id_salt("dynamic_source_edit_scroll")
            .show(ui, |ui| {
                egui::Grid::new("dynamic_source_edit_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        // URL
                        ui.label("URL:");
                        ui.add(
                            egui::TextEdit::singleline(&mut source.url)
                                .desired_width(350.0)
                                .hint_text("https://example.com/profiles.yaml"),
                        );
                        ui.end_row();

                        // Enabled
                        ui.label("Enabled:");
                        ui.checkbox(&mut source.enabled, "");
                        ui.end_row();

                        // Refresh interval (seconds -> displayed as minutes)
                        ui.label("Refresh interval:");
                        let mut minutes = (source.refresh_interval_secs as f32 / 60.0).round();
                        if ui
                            .add(
                                egui::Slider::new(&mut minutes, 1.0..=60.0)
                                    .suffix(" min")
                                    .integer(),
                            )
                            .changed()
                        {
                            source.refresh_interval_secs = (minutes as u64) * 60;
                        }
                        ui.end_row();

                        // Max download size (bytes -> displayed as KB)
                        ui.label("Max download size:");
                        let mut kb = (source.max_size_bytes as f32 / 1024.0).round() as u32;
                        if ui
                            .add(
                                egui::DragValue::new(&mut kb)
                                    .range(1..=10240)
                                    .suffix(" KB")
                                    .speed(10),
                            )
                            .changed()
                        {
                            source.max_size_bytes = kb as usize * 1024;
                        }
                        ui.end_row();

                        // Fetch timeout
                        ui.label("Fetch timeout:");
                        let mut timeout = source.fetch_timeout_secs as u32;
                        if ui
                            .add(
                                egui::Slider::new(&mut timeout, 5..=60)
                                    .suffix(" sec")
                                    .integer(),
                            )
                            .changed()
                        {
                            source.fetch_timeout_secs = timeout as u64;
                        }
                        ui.end_row();

                        // Conflict resolution
                        ui.label("Conflict resolution:");
                        egui::ComboBox::from_id_salt("dynamic_source_conflict")
                            .selected_text(source.conflict_resolution.display_name())
                            .show_ui(ui, |ui| {
                                for variant in ConflictResolution::variants() {
                                    ui.selectable_value(
                                        &mut source.conflict_resolution,
                                        variant.clone(),
                                        variant.display_name(),
                                    );
                                }
                            });
                        ui.end_row();
                    });

                ui.add_space(8.0);

                // Headers section
                let header_count = source.headers.len();
                let header_label = if header_count > 0 {
                    format!("HTTP Headers ({})", header_count)
                } else {
                    "HTTP Headers".to_string()
                };
                let http_default_open = header_count > 0;
                collapsing_section(ui, &header_label, "dynamic_source_headers", http_default_open, collapsed, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "Custom headers sent with each fetch request (e.g., Authorization).",
                            )
                            .small()
                            .color(egui::Color32::GRAY),
                        );
                        ui.add_space(4.0);

                        let mut delete_header_key: Option<String> = None;

                        if !source.headers.is_empty() {
                            // Sort keys for stable display order
                            let mut sorted_keys: Vec<String> =
                                source.headers.keys().cloned().collect();
                            sorted_keys.sort();

                            egui::Grid::new("dynamic_source_headers_grid")
                                .num_columns(3)
                                .spacing([8.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Key").small().strong());
                                    ui.label(egui::RichText::new("Value").small().strong());
                                    ui.label(""); // Delete column
                                    ui.end_row();

                                    for key in &sorted_keys {
                                        ui.label(egui::RichText::new(key).monospace());

                                        // Show value (mask if Authorization-like)
                                        let value = &source.headers[key];
                                        let display_value =
                                            if key.to_lowercase().contains("auth")
                                                || key.to_lowercase().contains("token")
                                            {
                                                if value.len() > 8 {
                                                    format!("{}...", &value[..8])
                                                } else {
                                                    "*".repeat(value.len())
                                                }
                                            } else {
                                                value.clone()
                                            };
                                        ui.label(
                                            egui::RichText::new(&display_value)
                                                .monospace()
                                                .color(egui::Color32::GRAY),
                                        );

                                        if ui
                                            .small_button(
                                                egui::RichText::new("X").color(
                                                    egui::Color32::from_rgb(200, 80, 80),
                                                ),
                                            )
                                            .on_hover_text("Remove header")
                                            .clicked()
                                        {
                                            delete_header_key = Some(key.clone());
                                        }
                                        ui.end_row();
                                    }
                                });
                        }

                        if let Some(key) = delete_header_key {
                            source.headers.remove(&key);
                        }

                        ui.add_space(4.0);

                        // Add header form
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(
                                    &mut settings.dynamic_source_new_header_key,
                                )
                                .desired_width(120.0)
                                .hint_text("Header name"),
                            );
                            ui.add(
                                egui::TextEdit::singleline(
                                    &mut settings.dynamic_source_new_header_value,
                                )
                                .desired_width(180.0)
                                .hint_text("Header value"),
                            );
                            if ui
                                .small_button("+ Add")
                                .on_hover_text("Add header")
                                .clicked()
                                && !settings.dynamic_source_new_header_key.is_empty()
                            {
                                source.headers.insert(
                                    settings.dynamic_source_new_header_key.clone(),
                                    settings.dynamic_source_new_header_value.clone(),
                                );
                                settings.dynamic_source_new_header_key.clear();
                                settings.dynamic_source_new_header_value.clear();
                            }
                        });
                    });
            });
    }

    ui.separator();
}
