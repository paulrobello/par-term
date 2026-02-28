//! tmux Integration section for the advanced settings tab.
//!
//! Covers: tmux enable/disable, path, default session, auto-attach, clipboard sync,
//! status bar (left/right format, refresh interval), prefix key.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, collapsing_section};
use std::collections::HashSet;

pub(super) fn show_tmux_section(
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
