//! Advanced settings tab.
//!
//! Consolidates: tmux_tab, logging_tab, screenshot_tab, update_tab
//!
//! Contains:
//! - Import/export preferences
//! - tmux integration settings
//! - Session logging settings
//! - Screenshot settings
//! - Update settings
//! - File transfer settings
//! - Debug logging settings
//! - Security settings (env var allowlist)

use super::SettingsUI;
use super::section::{INPUT_WIDTH, collapsing_section};
use crate::format_timestamp;
use par_term_config::{
    Config, DownloadSaveLocation, LogLevel, SessionLogFormat, UpdateCheckFrequency,
};
use std::collections::HashSet;

/// Show the advanced tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Import/Export Preferences section
    if section_matches(
        &query,
        "Import/Export Preferences",
        &[
            "import",
            "export",
            "preferences",
            "backup",
            "restore",
            "config",
            "yaml",
            "url import",
            "merge",
        ],
    ) {
        show_import_export_section(ui, settings, changes_this_frame, collapsed);
    }

    // tmux Integration section
    if section_matches(
        &query,
        "tmux Integration",
        &[
            "tmux",
            "control mode",
            "session",
            "attach",
            "prefix key",
            "status bar",
            "clipboard sync",
            "auto-attach",
        ],
    ) {
        show_tmux_section(ui, settings, changes_this_frame, collapsed);
    }

    // Session Logging section
    if section_matches(
        &query,
        "Session Logging",
        &[
            "logging",
            "recording",
            "asciicast",
            "asciinema",
            "plain text",
            "html",
            "auto-log",
            "log directory",
        ],
    ) {
        show_logging_section(ui, settings, changes_this_frame, collapsed);
    }

    // Screenshots section (collapsed by default)
    if section_matches(
        &query,
        "Screenshots",
        &["screenshot", "format", "png", "jpeg", "svg", "capture"],
    ) {
        show_screenshot_section(ui, settings, changes_this_frame, collapsed);
    }

    // Updates section
    if section_matches(
        &query,
        "Updates",
        &[
            "update",
            "version",
            "check",
            "release",
            "frequency",
            "homebrew",
            "cargo",
            "self-update",
            "daily",
            "weekly",
            "monthly",
        ],
    ) {
        show_updates_section(ui, settings, changes_this_frame, collapsed);
    }

    // File Transfers section
    if section_matches(
        &query,
        "File Transfers",
        &[
            "download",
            "upload",
            "transfer",
            "file transfer",
            "save location",
            "save directory",
        ],
    ) {
        show_file_transfers_section(ui, settings, changes_this_frame, collapsed);
    }

    // Debug Logging section
    if section_matches(
        &query,
        "Debug Logging",
        &[
            "debug",
            "log",
            "log level",
            "log file",
            "trace",
            "verbose",
            "diagnostics",
        ],
    ) {
        show_debug_logging_section(ui, settings, changes_this_frame, collapsed);
    }

    // Security section
    if section_matches(
        &query,
        "Security",
        &[
            "security",
            "environment",
            "env var",
            "allowlist",
            "allow all env",
            "variable substitution",
        ],
    ) {
        show_security_section(ui, settings, changes_this_frame, collapsed);
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
// Import/Export Preferences Section
// ============================================================================

fn show_import_export_section(
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

// ============================================================================
// tmux Integration Section
// ============================================================================

fn show_tmux_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "tmux Integration",
        "advanced_tmux",
        true,
        collapsed,
        |ui| {
            ui.label("Configure tmux control mode integration");
            ui.add_space(8.0);

            if ui
                .checkbox(&mut settings.config.tmux_enabled, "Enable tmux integration")
                .on_hover_text("Use tmux control mode for session management and split panes")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.config.tmux_enabled {
                ui.label(egui::RichText::new("tmux integration is disabled").italics());
                return;
            }

            ui.add_space(8.0);

            // tmux Path
            ui.label(egui::RichText::new("Executable").strong());
            ui.horizontal(|ui| {
                ui.label("tmux path:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.tmux_path)
                            .desired_width(INPUT_WIDTH),
                    )
                    .on_hover_text("Path to tmux executable (default: 'tmux' uses PATH)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);

            // Session Settings
            ui.label(egui::RichText::new("Sessions").strong());
            ui.horizontal(|ui| {
                ui.label("Default session name:");
                let mut session_name = settings
                    .config
                    .tmux_default_session
                    .clone()
                    .unwrap_or_default();
                if ui
                    .add(egui::TextEdit::singleline(&mut session_name).desired_width(INPUT_WIDTH))
                    .on_hover_text("Name for new tmux sessions (leave empty for tmux default)")
                    .changed()
                {
                    settings.config.tmux_default_session = if session_name.is_empty() {
                        None
                    } else {
                        Some(session_name)
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);

            // Auto-attach
            ui.label(egui::RichText::new("Auto-Attach").strong());
            if ui
                .checkbox(
                    &mut settings.config.tmux_auto_attach,
                    "Auto-attach on startup",
                )
                .on_hover_text("Automatically attach to a tmux session when par-term starts")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.tmux_auto_attach {
                ui.horizontal(|ui| {
                    ui.label("Session to attach:");
                    let mut attach_session = settings
                        .config
                        .tmux_auto_attach_session
                        .clone()
                        .unwrap_or_default();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut attach_session)
                                .desired_width(INPUT_WIDTH),
                        )
                        .on_hover_text("Session name to auto-attach (leave empty for most recent)")
                        .changed()
                    {
                        settings.config.tmux_auto_attach_session = if attach_session.is_empty() {
                            None
                        } else {
                            Some(attach_session)
                        };
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(8.0);

            // Clipboard Sync
            ui.label(egui::RichText::new("Clipboard").strong());
            if ui
                .checkbox(
                    &mut settings.config.tmux_clipboard_sync,
                    "Sync clipboard with tmux",
                )
                .on_hover_text("When copying, also update tmux's paste buffer via set-buffer")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            // Status Bar
            ui.label(egui::RichText::new("Status Bar").strong());
            if ui
                .checkbox(
                    &mut settings.config.tmux_show_status_bar,
                    "Show tmux status bar",
                )
                .on_hover_text("Display tmux status bar at bottom when connected")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Status bar settings (only show if status bar is enabled)
            if settings.config.tmux_show_status_bar {
                ui.horizontal(|ui| {
                    ui.label("Refresh interval:");
                    let mut refresh_secs =
                        settings.config.tmux_status_bar_refresh_ms as f32 / 1000.0;
                    if ui
                        .add(egui::Slider::new(&mut refresh_secs, 0.5..=10.0).suffix("s"))
                        .on_hover_text("How often to update the status bar content")
                        .changed()
                    {
                        settings.config.tmux_status_bar_refresh_ms = (refresh_secs * 1000.0) as u64;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.add_space(4.0);

                // Left format string
                ui.horizontal(|ui| {
                ui.label("Left format:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.tmux_status_bar_left)
                            .desired_width(INPUT_WIDTH),
                    )
                    .on_hover_text(
                        "Format string for left side. Variables: {session}, {windows}, {pane}, {time:FORMAT}, {hostname}, {user}",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

                // Right format string
                ui.horizontal(|ui| {
                ui.label("Right format:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.tmux_status_bar_right)
                            .desired_width(INPUT_WIDTH),
                    )
                    .on_hover_text(
                        "Format string for right side. Variables: {session}, {windows}, {pane}, {time:FORMAT}, {hostname}, {user}",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

                // Help text for format variables
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(
                        "Variables: {session}, {windows}, {pane}, {time:%H:%M}, {hostname}, {user}",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
            }

            ui.add_space(8.0);

            // Prefix Key
            ui.label(egui::RichText::new("Prefix Key").strong());
            ui.horizontal(|ui| {
                ui.label("Prefix key:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.tmux_prefix_key)
                            .desired_width(INPUT_WIDTH),
                    )
                    .on_hover_text("Key combination for tmux commands (e.g., C-b, C-Space)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Session Logging Section
// ============================================================================

fn show_logging_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Session Logging",
        "advanced_logging",
        true,
        collapsed,
        |ui| {
            ui.label(
                "Automatically record terminal sessions for later review, debugging, or sharing.",
            );
            ui.add_space(8.0);

            let mut auto_log = settings.config.auto_log_sessions;
            if ui
                .checkbox(&mut auto_log, "Enable automatic session logging")
                .on_hover_text("When enabled, all terminal output is logged to files")
                .changed()
            {
                settings.config.auto_log_sessions = auto_log;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log format:");

                let current_format = settings.config.session_log_format;
                let format_name = current_format.display_name();

                egui::ComboBox::from_id_salt("advanced_session_log_format")
                    .width(180.0)
                    .selected_text(format_name)
                    .show_ui(ui, |ui| {
                        for format in SessionLogFormat::all() {
                            if ui
                                .selectable_label(current_format == *format, format.display_name())
                                .clicked()
                                && current_format != *format
                            {
                                settings.config.session_log_format = *format;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(match settings.config.session_log_format {
                    SessionLogFormat::Plain => {
                        "Plain text without escape sequences - smallest files"
                    }
                    SessionLogFormat::Html => "HTML with colors preserved - viewable in browser",
                    SessionLogFormat::Asciicast => "asciinema format - can be replayed or shared",
                })
                .weak(),
            );

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log directory:");
                let mut dir = settings.config.session_log_directory.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut dir)
                        .desired_width(300.0)
                        .hint_text("~/.local/share/par-term/logs/"),
                );
                if response.changed() {
                    settings.config.session_log_directory = dir;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            let resolved_path = settings.config.logs_dir();
            ui.label(
                egui::RichText::new(format!("Resolved: {}", resolved_path.display()))
                    .weak()
                    .small(),
            );

            ui.add_space(8.0);

            let mut archive = settings.config.archive_on_close;
            if ui
                .checkbox(&mut archive, "Archive session on tab close")
                .on_hover_text("Ensures session is fully written when tab closes")
                .changed()
            {
                settings.config.archive_on_close = archive;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            let mut redact = settings.config.session_log_redact_passwords;
            if ui
                .checkbox(&mut redact, "Redact passwords in session logs")
                .on_hover_text(
                    "Detects password prompts (sudo, ssh, etc.) and replaces \
                     keyboard input with a redaction marker. Prevents passwords \
                     from being written to session log files on disk.",
                )
                .changed()
            {
                settings.config.session_log_redact_passwords = redact;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.config.session_log_redact_passwords {
                ui.label(
                    egui::RichText::new(
                        "\u{26a0} Warning: Session logs may contain passwords and credentials",
                    )
                    .color(egui::Color32::from_rgb(255, 193, 7))
                    .small(),
                );
            }
        },
    );
}

// ============================================================================
// Screenshots Section
// ============================================================================

fn show_screenshot_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Screenshots",
        "advanced_screenshots",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Format:");

                let options = ["png", "jpeg", "svg", "html"];
                let mut selected = settings.config.screenshot_format.clone();

                egui::ComboBox::from_id_salt("advanced_screenshot_format")
                    .width(140.0)
                    .selected_text(selected.as_str())
                    .show_ui(ui, |ui| {
                        for opt in options {
                            ui.selectable_value(&mut selected, opt.to_string(), opt);
                        }
                    });

                if selected != settings.config.screenshot_format {
                    settings.config.screenshot_format = selected;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
            ui.label("Supported: png, jpeg, svg, html");
        },
    );
}

// ============================================================================
// Updates Section
// ============================================================================

fn show_updates_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Updates", "advanced_updates", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.label(settings.app_version);
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Check for updates:");

            let current = settings.config.update_check_frequency;
            egui::ComboBox::from_id_salt("advanced_update_check_frequency")
                .selected_text(current.display_name())
                .show_ui(ui, |ui| {
                    for freq in [
                        UpdateCheckFrequency::Never,
                        UpdateCheckFrequency::Hourly,
                        UpdateCheckFrequency::Daily,
                        UpdateCheckFrequency::Weekly,
                        UpdateCheckFrequency::Monthly,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.update_check_frequency,
                                freq,
                                freq.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        if let Some(ref last_check) = settings.config.last_update_check {
            ui.horizontal(|ui| {
                ui.label("Last checked:");
                ui.label(format_timestamp(last_check));
            });
        }

        if let Some(skipped) = settings.config.skipped_version.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Skipping notifications for v{}", skipped))
                        .small()
                        .color(egui::Color32::GRAY),
                );
                if ui.small_button("Clear").clicked() {
                    settings.config.skipped_version = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(8.0);

        // Check Now button
        ui.horizontal(|ui| {
            if ui
                .button("Check Now")
                .on_hover_text("Check for updates immediately")
                .clicked()
            {
                settings.check_now_requested = true;
            }
        });

        // Show update check result
        if let Some(ref result) = settings.last_update_result {
            ui.add_space(4.0);
            match result {
                crate::UpdateCheckResult::UpToDate => {
                    ui.label(
                        egui::RichText::new("You are running the latest version.")
                            .color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                }
                crate::UpdateCheckResult::UpdateAvailable(info) => {
                    let version_str = info.version.strip_prefix('v').unwrap_or(&info.version);
                    ui.label(
                        egui::RichText::new(format!("Version {} is available!", version_str))
                            .color(egui::Color32::YELLOW)
                            .strong(),
                    );

                    // Show release URL as clickable link
                    ui.hyperlink_to("View release on GitHub", &info.release_url);

                    ui.add_space(4.0);

                    // Detect installation type to decide what button to show
                    let installation = settings.installation_type;
                    match installation {
                        crate::InstallationType::Homebrew => {
                            ui.label(
                                egui::RichText::new(
                                    "Update via Homebrew: brew upgrade --cask par-term",
                                )
                                .color(egui::Color32::GRAY),
                            );
                        }
                        crate::InstallationType::CargoInstall => {
                            ui.label(
                                egui::RichText::new("Update via cargo: cargo install par-term")
                                    .color(egui::Color32::GRAY),
                            );
                        }
                        _ => {
                            // Show Install Update button
                            let installing = settings.update_installing;
                            let button_text = if installing {
                                "Installing..."
                            } else {
                                "Install Update"
                            };

                            let button =
                                egui::Button::new(egui::RichText::new(button_text).strong());

                            if ui
                                .add_enabled(!installing, button)
                                .on_hover_text(format!("Download and install v{}", version_str))
                                .clicked()
                            {
                                settings.update_install_requested = true;
                            }
                        }
                    }
                }
                crate::UpdateCheckResult::Error(e) => {
                    ui.label(
                        egui::RichText::new(format!("Check failed: {}", e))
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                }
                _ => {}
            }
        }

        // Show update status/result
        if let Some(ref status) = settings.update_status {
            ui.add_space(4.0);
            let color = if settings.update_result.as_ref().is_some_and(|r| r.is_err()) {
                egui::Color32::from_rgb(255, 100, 100)
            } else if settings.update_result.as_ref().is_some_and(|r| r.is_ok()) {
                egui::Color32::from_rgb(100, 200, 100)
            } else {
                egui::Color32::YELLOW
            };
            ui.label(egui::RichText::new(status.as_str()).color(color));
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "par-term checks for updates periodically based on the frequency above.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

// ============================================================================
// File Transfers Section
// ============================================================================

fn show_file_transfers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "File Transfers",
        "advanced_file_transfers",
        true,
        collapsed,
        |ui| {
            ui.label("Configure where downloaded files are saved.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Download save location:");

                // Determine which variant is currently selected (ignoring Custom's inner string)
                let is_custom = matches!(
                    settings.config.download_save_location,
                    DownloadSaveLocation::Custom(_)
                );
                let selected_text = settings.config.download_save_location.display_name();

                egui::ComboBox::from_id_salt("advanced_download_save_location")
                    .width(200.0)
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        // Non-custom variants
                        for variant in DownloadSaveLocation::variants() {
                            if ui
                                .selectable_label(
                                    !is_custom
                                        && settings.config.download_save_location == *variant,
                                    variant.display_name(),
                                )
                                .clicked()
                                && settings.config.download_save_location != *variant
                            {
                                settings.config.download_save_location = variant.clone();
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                        // Custom variant
                        if ui.selectable_label(is_custom, "Custom directory").clicked()
                            && !is_custom
                        {
                            settings.config.download_save_location =
                                DownloadSaveLocation::Custom(String::new());
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });

            // Show custom path picker when Custom is selected
            if let DownloadSaveLocation::Custom(ref path) = settings.config.download_save_location {
                let mut custom_path = path.clone();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Custom path:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut custom_path)
                                .desired_width(INPUT_WIDTH)
                                .hint_text("/path/to/downloads"),
                        )
                        .changed()
                    {
                        settings.config.download_save_location =
                            DownloadSaveLocation::Custom(custom_path.clone());
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if ui.button("Browse...").clicked() {
                        let mut dialog =
                            rfd::FileDialog::new().set_title("Select Download Directory");
                        if !custom_path.is_empty() {
                            dialog = dialog.set_directory(&custom_path);
                        }
                        if let Some(folder) = dialog.pick_folder() {
                            settings.config.download_save_location =
                                DownloadSaveLocation::Custom(folder.display().to_string());
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
            }
        },
    );
}

// ============================================================================
// Debug Logging Section
// ============================================================================

fn show_debug_logging_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Debug Logging",
        "advanced_debug_logging",
        true,
        collapsed,
        |ui| {
            ui.label("Configure diagnostic logging to file for troubleshooting.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log level:");

                let current = settings.config.log_level;
                egui::ComboBox::from_id_salt("advanced_log_level")
                    .width(120.0)
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for level in LogLevel::all() {
                            if ui
                                .selectable_label(current == *level, level.display_name())
                                .clicked()
                                && current != *level
                            {
                                settings.config.log_level = *level;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);
            let log_path = crate::log_path();
            ui.horizontal(|ui| {
                ui.label("Log file:");
                ui.label(
                    egui::RichText::new(log_path.display().to_string())
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(4.0);
            if ui.button("Open Log File").clicked() {
                settings.open_log_requested = true;
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Set to Off to suppress log file creation. \
                     RUST_LOG env var and --log-level CLI flag override this setting.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        },
    );
}

// ============================================================================
// Security Section
// ============================================================================

fn show_security_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Security", "advanced_security", true, collapsed, |ui| {
        ui.label("Environment variable substitution in config files.");
        ui.add_space(8.0);

        let mut allow_all = settings.config.allow_all_env_vars;
        if ui
            .checkbox(
                &mut allow_all,
                "Allow all environment variables in config substitution",
            )
            .changed()
        {
            settings.config.allow_all_env_vars = allow_all;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "When disabled (default), only safe environment variables (HOME, USER, \
                     SHELL, XDG_*, PAR_TERM_*, LC_*, etc.) are substituted in config files. \
                     Enable this to allow any environment variable — use with caution if \
                     loading configs from untrusted sources.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}
