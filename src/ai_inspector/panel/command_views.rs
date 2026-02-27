//! Terminal-capture command list rendering for the AI Inspector panel.
//!
//! Contains the controls row, environment strip, and the four view-mode
//! renderers: Cards, Timeline, Tree, and ListDetail.

use egui::{Color32, Frame, Label, RichText};

use crate::ai_inspector::panel_helpers::{format_duration, truncate_chars, truncate_output};
use crate::ai_inspector::snapshot::{CommandEntry, SnapshotData};
use crate::ui_constants::{AI_PANEL_CARD_ROUNDING, AI_PANEL_INNER_MARGIN};

use super::AIInspectorPanel;
use super::types::{CARD_BG, CARD_BORDER, EXIT_FAILURE, EXIT_SUCCESS, SCOPE_OPTIONS, ViewMode};

impl AIInspectorPanel {
    /// Render the controls row (scope, view mode, live/paused, refresh).
    pub(super) fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Scope dropdown
            let current_scope_label = SCOPE_OPTIONS
                .iter()
                .find(|opt| opt.scope == self.scope)
                .map_or("Visible", |opt| opt.label);

            egui::ComboBox::from_id_salt("ai_inspector_scope")
                .selected_text(current_scope_label)
                .width(90.0)
                .show_ui(ui, |ui| {
                    for opt in SCOPE_OPTIONS {
                        if ui
                            .selectable_label(self.scope == opt.scope, opt.label)
                            .clicked()
                        {
                            self.scope = opt.scope.clone();
                            self.needs_refresh = true;
                        }
                    }
                });

            // View mode dropdown
            egui::ComboBox::from_id_salt("ai_inspector_view_mode")
                .selected_text(self.view_mode.label())
                .width(80.0)
                .show_ui(ui, |ui| {
                    for mode in ViewMode::all() {
                        if ui
                            .selectable_label(self.view_mode == *mode, mode.label())
                            .clicked()
                        {
                            self.view_mode = *mode;
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            // Live/Paused toggle
            let live_label = if self.live_update {
                RichText::new("* Live").color(EXIT_SUCCESS).small()
            } else {
                RichText::new("o Paused")
                    .color(Color32::from_gray(140))
                    .small()
            };
            if ui
                .button(live_label)
                .on_hover_text(if self.live_update {
                    "Click to pause auto-refresh"
                } else {
                    "Click to enable auto-refresh"
                })
                .clicked()
            {
                self.live_update = !self.live_update;
            }

            // Refresh button
            if ui
                .button(RichText::new("~ Refresh").small())
                .on_hover_text("Refresh snapshot now")
                .clicked()
            {
                self.needs_refresh = true;
            }
        });
    }

    /// Render the environment info strip.
    pub(super) fn render_environment(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        let env = &snapshot.environment;
        let dim_color = Color32::from_gray(120);
        let val_color = Color32::from_gray(190);

        ui.horizontal_wrapped(|ui| {
            // user@host
            if let Some(ref user) = env.username {
                ui.label(RichText::new(user).color(val_color).small());
                if env.hostname.is_some() {
                    ui.label(RichText::new("@").color(dim_color).small());
                }
            }
            if let Some(ref host) = env.hostname {
                ui.label(RichText::new(host).color(val_color).small());
            }

            // Separator
            if env.username.is_some() || env.hostname.is_some() {
                ui.label(RichText::new("|").color(dim_color).small());
            }

            // CWD
            if let Some(ref cwd) = env.cwd {
                ui.label(RichText::new(cwd).color(val_color).small());
            }
        });

        ui.horizontal(|ui| {
            // Shell
            if let Some(ref shell) = env.shell {
                ui.label(RichText::new("Shell:").color(dim_color).small());
                ui.label(RichText::new(shell).color(val_color).small());
                ui.label(RichText::new("|").color(dim_color).small());
            }

            // Command count
            let cmd_count = snapshot.commands.len();
            ui.label(RichText::new("Commands:").color(dim_color).small());
            ui.label(
                RichText::new(cmd_count.to_string())
                    .color(val_color)
                    .small(),
            );
        });
    }

    /// Render cards view: each command in a framed card.
    pub(super) fn render_cards(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        for (i, cmd) in commands.iter().enumerate() {
            let card_frame = Frame::new()
                .fill(CARD_BG)
                .stroke(CARD_BORDER)
                .corner_radius(AI_PANEL_CARD_ROUNDING)
                .inner_margin(AI_PANEL_INNER_MARGIN);

            card_frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Command text (wrap to prevent panel overflow)
                ui.add(
                    Label::new(
                        RichText::new(&cmd.command)
                            .color(Color32::from_gray(230))
                            .monospace(),
                    )
                    .wrap(),
                );

                ui.add_space(4.0);

                // Exit code badge + duration
                ui.horizontal(|ui| {
                    if let Some(code) = cmd.exit_code {
                        let (color, text) = if code == 0 {
                            (EXIT_SUCCESS, format!("OK {code}"))
                        } else {
                            (EXIT_FAILURE, format!("FAIL {code}"))
                        };
                        ui.label(RichText::new(text).color(color).small().strong());
                    }

                    // Duration
                    let duration_str = format_duration(cmd.duration_ms);
                    ui.label(
                        RichText::new(duration_str)
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                });

                // CWD if present
                if let Some(ref cwd) = cmd.cwd {
                    ui.label(
                        RichText::new(cwd)
                            .color(Color32::from_gray(90))
                            .small()
                            .italics(),
                    );
                }

                // Collapsible output
                if let Some(ref output) = cmd.output
                    && !output.is_empty()
                {
                    egui::CollapsingHeader::new(
                        RichText::new("Output")
                            .color(Color32::from_gray(140))
                            .small(),
                    )
                    .id_salt(format!("card_output_{i}"))
                    .show(ui, |ui| {
                        let truncated = truncate_output(output, 20);
                        ui.add(
                            Label::new(
                                RichText::new(truncated)
                                    .color(Color32::from_gray(160))
                                    .monospace()
                                    .small(),
                            )
                            .wrap(),
                        );
                    });
                }
            });

            ui.add_space(4.0);
        }
    }

    /// Render timeline view: flat list with icons and durations.
    pub(super) fn render_timeline(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        // Reserve space for icon (~15px), duration label (~60px), and spacing
        let avail = ui.available_width();
        let max_cmd_chars = ((avail - 90.0) / 7.5).max(10.0) as usize;

        for (i, cmd) in commands.iter().enumerate() {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("*").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("*").color(EXIT_FAILURE),
                    None => RichText::new("o").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text (truncated to fit, char-boundary-safe)
                let cmd_display = if cmd.command.len() > max_cmd_chars {
                    format!("{}...", truncate_chars(&cmd.command, max_cmd_chars))
                } else {
                    cmd.command.clone()
                };
                ui.label(
                    RichText::new(cmd_display)
                        .color(Color32::from_gray(210))
                        .monospace(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format_duration(cmd.duration_ms))
                            .color(Color32::from_gray(100))
                            .small(),
                    );
                });
            });

            // Separator between entries
            if i < commands.len() - 1 {
                ui.separator();
            }
        }
    }

    /// Render tree view: collapsing headers per command with detail children.
    pub(super) fn render_tree(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        // Truncate to fit within panel - account for collapsing header icon (~20px)
        // and monospace char width (~7-8px). Use available width dynamically.
        let avail = ui.available_width();
        let max_chars = ((avail - 20.0) / 7.5).max(10.0) as usize;

        for (i, cmd) in commands.iter().enumerate() {
            let header_text = if cmd.command.len() > max_chars {
                format!("{}...", truncate_chars(&cmd.command, max_chars))
            } else {
                cmd.command.clone()
            };

            egui::CollapsingHeader::new(
                RichText::new(header_text)
                    .color(Color32::from_gray(210))
                    .monospace(),
            )
            .id_salt(format!("tree_cmd_{i}"))
            .show(ui, |ui| {
                // Exit code
                if let Some(code) = cmd.exit_code {
                    let (color, label) = if code == 0 {
                        (EXIT_SUCCESS, "Success")
                    } else {
                        (EXIT_FAILURE, "Failed")
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Exit:")
                                .color(Color32::from_gray(120))
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("{code} ({label})"))
                                .color(color)
                                .small(),
                        );
                    });
                }

                // Duration
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Duration:")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                    ui.label(
                        RichText::new(format_duration(cmd.duration_ms))
                            .color(Color32::from_gray(180))
                            .small(),
                    );
                });

                // CWD
                if let Some(ref cwd) = cmd.cwd {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("CWD:").color(Color32::from_gray(120)).small());
                        ui.label(RichText::new(cwd).color(Color32::from_gray(180)).small());
                    });
                }

                // Output
                if let Some(ref output) = cmd.output
                    && !output.is_empty()
                {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Output:")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                    let truncated = truncate_output(output, 20);
                    ui.add(
                        Label::new(
                            RichText::new(truncated)
                                .color(Color32::from_gray(160))
                                .monospace()
                                .small(),
                        )
                        .wrap(),
                    );
                }
            });
        }
    }

    /// Render list detail view: simple list with icon and command text.
    pub(super) fn render_list_detail(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        // Truncate command to fit in horizontal layout
        let avail = ui.available_width();
        let max_cmd_chars = ((avail - 50.0) / 7.5).max(10.0) as usize;

        for cmd in commands {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("OK").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("FAIL").color(EXIT_FAILURE),
                    None => RichText::new("-").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text (truncated to fit, char-boundary-safe)
                let cmd_display = if cmd.command.len() > max_cmd_chars {
                    format!("{}...", truncate_chars(&cmd.command, max_cmd_chars))
                } else {
                    cmd.command.clone()
                };
                ui.label(
                    RichText::new(cmd_display)
                        .color(Color32::from_gray(210))
                        .monospace(),
                );
            });
        }
    }
}
