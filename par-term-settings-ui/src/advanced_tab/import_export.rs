//! Import/Export Preferences section for the advanced settings tab.
//!
//! Covers: export to file, import from file (replace/merge), import from URL.
//! Also provides the public `merge_config` function used externally.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, collapsing_section};
use par_term_config::Config;
use std::collections::HashSet;

// ============================================================================
// Import/Export Section
// ============================================================================

pub(super) fn show_import_export_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Import/Export Preferences",
        "advanced_import_export",
        true,
        collapsed,
        |ui| {
            ui.label("Export your current configuration or import settings from a file or URL.");
            ui.add_space(8.0);

            // --- Export ---
            ui.label(egui::RichText::new("Export").strong());
            ui.add_space(4.0);

            if ui
                .button("Export Preferences to File")
                .on_hover_text("Save the current configuration to a YAML file")
                .clicked()
            {
                export_preferences(settings);
            }

            ui.add_space(12.0);

            // --- Import from File ---
            ui.label(egui::RichText::new("Import from File").strong());
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui
                    .button("Import & Replace")
                    .on_hover_text("Replace the entire configuration with settings from a file")
                    .clicked()
                {
                    import_preferences_from_file(settings, changes_this_frame, ImportMode::Replace);
                }

                if ui
                    .button("Import & Merge")
                    .on_hover_text(
                        "Merge settings from a file into the current configuration \
                         (only overrides non-default values)",
                    )
                    .clicked()
                {
                    import_preferences_from_file(settings, changes_this_frame, ImportMode::Merge);
                }
            });

            ui.add_space(12.0);

            // --- Import from URL ---
            ui.label(egui::RichText::new("Import from URL").strong());
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.add(
                    egui::TextEdit::singleline(&mut settings.temp_import_url)
                        .desired_width(INPUT_WIDTH)
                        .hint_text("https://example.com/config.yaml"),
                );
            });

            ui.horizontal(|ui| {
                let url_valid = !settings.temp_import_url.trim().is_empty()
                    && (settings.temp_import_url.starts_with("http://")
                        || settings.temp_import_url.starts_with("https://"));

                if ui
                    .add_enabled(url_valid, egui::Button::new("Fetch & Replace"))
                    .on_hover_text("Download and replace the current configuration")
                    .clicked()
                {
                    import_preferences_from_url(settings, changes_this_frame, ImportMode::Replace);
                }

                if ui
                    .add_enabled(url_valid, egui::Button::new("Fetch & Merge"))
                    .on_hover_text("Download and merge into the current configuration")
                    .clicked()
                {
                    import_preferences_from_url(settings, changes_this_frame, ImportMode::Merge);
                }
            });

            // Show status/error messages
            if let Some(ref msg) = settings.import_export_status {
                ui.add_space(4.0);
                let color = if settings.import_export_is_error {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    egui::Color32::from_rgb(100, 200, 100)
                };
                ui.label(egui::RichText::new(msg.as_str()).color(color));
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Merge mode preserves your existing settings and only applies \
                     values that differ from defaults in the imported file.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        },
    );
}

// ============================================================================
// Import/Export Helpers
// ============================================================================

/// Whether to replace or merge when importing preferences.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    /// Replace the entire configuration.
    Replace,
    /// Merge non-default values from the imported config.
    Merge,
}

/// Export the current configuration to a YAML file.
fn export_preferences(settings: &mut SettingsUI) {
    let path = rfd::FileDialog::new()
        .set_title("Export Preferences")
        .add_filter("YAML", &["yaml", "yml"])
        .set_file_name("par-term-config.yaml")
        .save_file();

    if let Some(path) = path {
        match serde_yml::to_string(&settings.config) {
            Ok(yaml) => {
                if let Err(e) = std::fs::write(&path, yaml) {
                    settings.import_export_status = Some(format!("Failed to write file: {}", e));
                    settings.import_export_is_error = true;
                    log::error!("Failed to export preferences: {}", e);
                } else {
                    settings.import_export_status = Some(format!("Exported to {}", path.display()));
                    settings.import_export_is_error = false;
                    log::info!("Exported preferences to {}", path.display());
                }
            }
            Err(e) => {
                settings.import_export_status = Some(format!("Failed to serialize config: {}", e));
                settings.import_export_is_error = true;
                log::error!("Failed to serialize preferences: {}", e);
            }
        }
    }
}

/// Import preferences from a local file.
fn import_preferences_from_file(
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    mode: ImportMode,
) {
    let path = rfd::FileDialog::new()
        .set_title("Import Preferences")
        .add_filter("YAML", &["yaml", "yml"])
        .pick_file();

    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                apply_imported_config(settings, changes_this_frame, &content, mode);
            }
            Err(e) => {
                settings.import_export_status = Some(format!("Failed to read file: {}", e));
                settings.import_export_is_error = true;
                log::error!("Failed to read preferences file: {}", e);
            }
        }
    }
}

/// Import preferences from a URL.
fn import_preferences_from_url(
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    mode: ImportMode,
) {
    let url = settings.temp_import_url.trim().to_string();
    if url.is_empty() {
        return;
    }

    let agent = crate::http_agent();
    match agent.get(&url).call() {
        Ok(response) => match response.into_body().read_to_string() {
            Ok(body) => {
                apply_imported_config(settings, changes_this_frame, &body, mode);
            }
            Err(e) => {
                settings.import_export_status = Some(format!("Failed to read response: {}", e));
                settings.import_export_is_error = true;
                log::error!("Failed to read URL response body: {}", e);
            }
        },
        Err(e) => {
            settings.import_export_status = Some(format!("Failed to fetch URL: {}", e));
            settings.import_export_is_error = true;
            log::error!("Failed to fetch preferences from URL: {}", e);
        }
    }
}

/// Parse YAML content as a Config and apply it to the settings.
fn apply_imported_config(
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    content: &str,
    mode: ImportMode,
) {
    match serde_yml::from_str::<Config>(content) {
        Ok(imported) => {
            match mode {
                ImportMode::Replace => {
                    settings.config = imported;
                }
                ImportMode::Merge => {
                    merge_config(&mut settings.config, &imported);
                }
            }
            settings.sync_all_temps_from_config();
            settings.has_changes = true;
            *changes_this_frame = true;
            settings.import_export_status = Some(match mode {
                ImportMode::Replace => "Configuration replaced successfully.".to_string(),
                ImportMode::Merge => "Configuration merged successfully.".to_string(),
            });
            settings.import_export_is_error = false;
            log::info!(
                "Imported preferences (mode={:?})",
                match mode {
                    ImportMode::Replace => "replace",
                    ImportMode::Merge => "merge",
                }
            );
        }
        Err(e) => {
            settings.import_export_status = Some(format!("Invalid config file: {}", e));
            settings.import_export_is_error = true;
            log::error!("Failed to parse imported config: {}", e);
        }
    }
}

/// Merge an imported Config into the current config.
///
/// For each field, if the imported value differs from the default, it overwrites
/// the current value. This lets users share partial configs that only override
/// specific settings.
pub fn merge_config(current: &mut Config, imported: &Config) {
    let defaults = Config::default();

    // Serialize all three to serde_yml::Value for field-by-field comparison
    let default_val: serde_yml::Value =
        serde_yml::from_str(&serde_yml::to_string(&defaults).unwrap_or_default())
            .unwrap_or(serde_yml::Value::Null);
    let imported_val: serde_yml::Value =
        serde_yml::from_str(&serde_yml::to_string(imported).unwrap_or_default())
            .unwrap_or(serde_yml::Value::Null);
    let mut current_val: serde_yml::Value =
        serde_yml::from_str(&serde_yml::to_string(&*current).unwrap_or_default())
            .unwrap_or(serde_yml::Value::Null);

    if let (
        serde_yml::Value::Mapping(ref default_map),
        serde_yml::Value::Mapping(ref imported_map),
        serde_yml::Value::Mapping(current_map),
    ) = (default_val, imported_val, &mut current_val)
    {
        for (key, imported_field) in imported_map {
            let default_field = default_map.get(key);
            // Only override if the imported value differs from the default
            if default_field != Some(imported_field) {
                current_map.insert(key.clone(), imported_field.clone());
            }
        }
    }

    // Deserialize the merged value back into Config
    if let Ok(merged) = serde_yml::from_value::<Config>(current_val) {
        *current = merged;
    }
}
