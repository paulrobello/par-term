//! Advanced settings tab.
//!
//! Consolidates: tmux_tab, logging_tab, screenshot_tab, update_tab
//!
//! Contains:
//! - tmux integration settings
//! - Session logging settings
//! - Screenshot settings
//! - Update settings

use super::section::{collapsing_section, INPUT_WIDTH};
use super::SettingsUI;
use crate::config::{SessionLogFormat, UpdateCheckFrequency};
use crate::update_checker::format_timestamp;

/// Show the advanced tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // tmux Integration section
    if section_matches(
        &query,
        "tmux Integration",
        &["tmux", "control mode", "session", "attach"],
    ) {
        show_tmux_section(ui, settings, changes_this_frame);
    }

    // Session Logging section
    if section_matches(
        &query,
        "Session Logging",
        &["logging", "recording", "asciicast", "asciinema"],
    ) {
        show_logging_section(ui, settings, changes_this_frame);
    }

    // Screenshots section (collapsed by default)
    if section_matches(&query, "Screenshots", &["screenshot", "format", "png", "jpeg"]) {
        show_screenshot_section(ui, settings, changes_this_frame);
    }

    // Updates section
    if section_matches(&query, "Updates", &["update", "version", "check", "release"]) {
        show_updates_section(ui, settings, changes_this_frame);
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
// tmux Integration Section
// ============================================================================

fn show_tmux_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "tmux Integration", "advanced_tmux", true, |ui| {
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
                .add(egui::TextEdit::singleline(&mut settings.config.tmux_path).desired_width(INPUT_WIDTH))
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
                    .add(egui::TextEdit::singleline(&mut attach_session).desired_width(INPUT_WIDTH))
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

        ui.add_space(8.0);

        // Prefix Key
        ui.label(egui::RichText::new("Prefix Key").strong());
        ui.horizontal(|ui| {
            ui.label("Prefix key:");
            if ui
                .add(egui::TextEdit::singleline(&mut settings.config.tmux_prefix_key).desired_width(INPUT_WIDTH))
                .on_hover_text("Key combination for tmux commands (e.g., C-b, C-Space)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Session Logging Section
// ============================================================================

fn show_logging_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Session Logging", "advanced_logging", true, |ui| {
        ui.label("Automatically record terminal sessions for later review, debugging, or sharing.");
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
                SessionLogFormat::Plain => "Plain text without escape sequences - smallest files",
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
    });
}

// ============================================================================
// Screenshots Section
// ============================================================================

fn show_screenshot_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Screenshots", "advanced_screenshots", false, |ui| {
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
    });
}

// ============================================================================
// Updates Section
// ============================================================================

fn show_updates_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Updates", "advanced_updates", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.label(env!("CARGO_PKG_VERSION"));
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

        ui.add_space(4.0);
        #[cfg(target_os = "macos")]
        let help_text = "par-term only checks for updates and notifies you. \
            If installed via Homebrew: brew upgrade --cask par-term. \
            Otherwise, download from GitHub releases.";

        #[cfg(not(target_os = "macos"))]
        let help_text = "par-term only checks for updates and notifies you. \
            Download from GitHub releases or your package manager.";

        ui.label(
            egui::RichText::new(help_text)
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}
