//! tmux Integration section for the advanced settings tab.
//!
//! Covers: tmux enable/disable, path, default session, auto-attach, clipboard sync,
//! status bar (left/right format, refresh interval), prefix key.

use crate::SettingsUI;
use crate::search::SearchTag;
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
            // par-mux is not tmux: this key works with tmux integration off,
            // so it sits outside the tmux-dependent block below.
            show_mux_auto_attach(ui, settings, changes_this_frame);
            ui.add_space(12.0);

            ui.label("Configure tmux control mode integration");
            ui.add_space(8.0);

            if ui
                .checkbox(
                    &mut settings.config.tmux.tmux_enabled,
                    "Enable tmux integration",
                )
                .search_tag(&["tmux_enabled"])
                .on_hover_text("Use tmux control mode for session management and split panes")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            crate::dependent::dependent(
                ui,
                settings.config.tmux.tmux_enabled,
                "Enable tmux integration",
                |ui| {
                    // tmux Path
                    ui.label(egui::RichText::new("Executable").strong());
                    ui.horizontal(|ui| {
                        ui.label("tmux path:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut settings.config.tmux.tmux_path)
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
                            .tmux
                            .tmux_default_session
                            .clone()
                            .unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut session_name)
                                    .desired_width(INPUT_WIDTH),
                            )
                            .on_hover_text(
                                "Session used when tmux starts without a name; attached if it \
                         exists, created otherwise (leave empty to let tmux pick a name)",
                            )
                            .changed()
                        {
                            settings.config.tmux.tmux_default_session = if session_name.is_empty() {
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
                            &mut settings.config.tmux.tmux_auto_attach,
                            "Auto-attach on startup",
                        )
                        .search_tag(&["tmux_auto_attach"])
                        .on_hover_text(
                            "Automatically attach to a tmux session when par-term starts",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    crate::dependent::dependent(
                        ui,
                        settings.config.tmux.tmux_auto_attach,
                        "Auto-attach on startup",
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Session to attach:");
                                let mut attach_session = settings
                                    .config
                                    .tmux
                                    .tmux_auto_attach_session
                                    .clone()
                                    .unwrap_or_default();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut attach_session)
                                            .desired_width(INPUT_WIDTH),
                                    )
                                    .on_hover_text(
                                        "Session name to auto-attach (leave empty for most recent)",
                                    )
                                    .changed()
                                {
                                    settings.config.tmux.tmux_auto_attach_session =
                                        if attach_session.is_empty() {
                                            None
                                        } else {
                                            Some(attach_session)
                                        };
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            });
                        },
                    );

                    ui.add_space(8.0);

                    // Clipboard Sync
                    ui.label(egui::RichText::new("Clipboard").strong());
                    if ui
                        .checkbox(
                            &mut settings.config.tmux.tmux_clipboard_sync,
                            "Sync clipboard with tmux",
                        )
                        .search_tag(&["tmux_clipboard_sync"])
                        .on_hover_text(
                            "When copying, also update tmux's paste buffer via set-buffer",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    ui.add_space(8.0);

                    // Gateway Tab
                    ui.label(egui::RichText::new("Gateway Tab").strong());
                    if ui
                .checkbox(
                    &mut settings.config.tmux.tmux_hide_gateway_tab,
                    "Hide control-mode tab",
                ).search_tag(&["tmux_hide_gateway_tab"])
                .on_hover_text(
                    "Hide the tmux -CC gateway tab from the tab bar while tmux windows are active. \
                     The tab is restored when the session ends.",
                )
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
                            &mut settings.config.tmux.tmux_show_status_bar,
                            "Show tmux status bar",
                        )
                        .search_tag(&["tmux_show_status_bar"])
                        .on_hover_text("Display tmux status bar at bottom when connected")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    crate::dependent::dependent(
                        ui,
                        settings.config.tmux.tmux_show_status_bar,
                        "Show tmux status bar",
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Refresh interval:");
                                let mut refresh_secs =
                                    settings.config.tmux.tmux_status_bar_refresh_ms as f32 / 1000.0;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut refresh_secs, 0.5..=10.0)
                                            .suffix(" s"),
                                    )
                                    .on_hover_text("How often to update the status bar content")
                                    .changed()
                                {
                                    settings.config.tmux.tmux_status_bar_refresh_ms =
                                        (refresh_secs * 1000.0) as u64;
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
                            egui::TextEdit::singleline(&mut settings.config.tmux.tmux_status_bar_left)
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
                            egui::TextEdit::singleline(&mut settings.config.tmux.tmux_status_bar_right)
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
                        },
                    );

                    ui.add_space(8.0);

                    // Prefix Key
                    ui.label(egui::RichText::new("Prefix Key").strong());
                    ui.horizontal(|ui| {
                        ui.label("Prefix key:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(
                                    &mut settings.config.tmux.tmux_prefix_key,
                                )
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
        },
    );
}

/// The par-mux auto-attach field (UX.md M15): the session the first window
/// attaches to on launch, created when missing. Empty turns it off.
fn show_mux_auto_attach(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.label(egui::RichText::new("par-mux Auto-Attach").strong());
    ui.horizontal(|ui| {
        ui.label("Session name:");
        let mut name = settings
            .config
            .tmux
            .mux_auto_attach
            .clone()
            .unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(INPUT_WIDTH))
            .on_hover_text(
                "The first window attaches to this par-mux session on launch, creating it \
                 when it does not exist. Leave empty to start with local tabs.",
            )
            .changed()
        {
            let name = name.trim().to_string();
            settings.config.tmux.mux_auto_attach = (!name.is_empty()).then_some(name);
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}
