//! Profile badge appearance section for the edit view.

use super::ProfileModalUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

impl ProfileModalUI {
    /// Render the badge appearance collapsing section (profile-level overrides).
    pub(super) fn render_badge_section(
        &mut self,
        ui: &mut egui::Ui,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(
            ui,
            "Badge Appearance",
            "profile_badge_appearance",
            false,
            collapsed,
            |ui| {
                egui::Grid::new("profile_form_badge_appearance")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        // Badge color
                        ui.label("Color:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_color.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_color = Some([255, 0, 0]); // Default red
                                } else {
                                    self.temp_badge_color = None;
                                }
                            }
                            if let Some(ref mut color) = self.temp_badge_color {
                                let mut egui_color =
                                    egui::Color32::from_rgb(color[0], color[1], color[2]);
                                if egui::color_picker::color_edit_button_srgba(
                                    ui,
                                    &mut egui_color,
                                    egui::color_picker::Alpha::Opaque,
                                )
                                .changed()
                                {
                                    *color = [egui_color.r(), egui_color.g(), egui_color.b()];
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge alpha/opacity
                        ui.label("Opacity:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_color_alpha.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_color_alpha = Some(0.5);
                                } else {
                                    self.temp_badge_color_alpha = None;
                                }
                            }
                            if let Some(ref mut alpha) = self.temp_badge_color_alpha {
                                ui.add(egui::Slider::new(alpha, 0.0..=1.0).step_by(0.05));
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge font
                        ui.label("Font:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_badge_font);
                            ui.label(
                                egui::RichText::new("(blank = global)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Badge font bold
                        ui.label("Bold:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_font_bold.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_font_bold = Some(true);
                                } else {
                                    self.temp_badge_font_bold = None;
                                }
                            }
                            if let Some(ref mut bold) = self.temp_badge_font_bold {
                                ui.checkbox(bold, "Bold text");
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge top margin
                        ui.label("Top Margin:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_top_margin.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_top_margin = Some(0.0);
                                } else {
                                    self.temp_badge_top_margin = None;
                                }
                            }
                            if let Some(ref mut margin) = self.temp_badge_top_margin {
                                ui.add(
                                    egui::DragValue::new(margin)
                                        .range(0.0..=100.0)
                                        .suffix(" px"),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge right margin
                        ui.label("Right Margin:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_right_margin.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_right_margin = Some(16.0);
                                } else {
                                    self.temp_badge_right_margin = None;
                                }
                            }
                            if let Some(ref mut margin) = self.temp_badge_right_margin {
                                ui.add(
                                    egui::DragValue::new(margin)
                                        .range(0.0..=100.0)
                                        .suffix(" px"),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge max width
                        ui.label("Max Width:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_max_width.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_max_width = Some(0.5);
                                } else {
                                    self.temp_badge_max_width = None;
                                }
                            }
                            if let Some(ref mut width) = self.temp_badge_max_width {
                                ui.add(
                                    egui::Slider::new(width, 0.1..=1.0)
                                        .step_by(0.05)
                                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        // Badge max height
                        ui.label("Max Height:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_badge_max_height.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_badge_max_height = Some(0.2);
                                } else {
                                    self.temp_badge_max_height = None;
                                }
                            }
                            if let Some(ref mut height) = self.temp_badge_max_height {
                                ui.add(
                                    egui::Slider::new(height, 0.05..=0.5)
                                        .step_by(0.05)
                                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("(use global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Check boxes to override global badge settings for this profile.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
            },
        );
    }

    /// Render per-profile shader override settings.
    pub(super) fn render_shader_section(
        &mut self,
        ui: &mut egui::Ui,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(
            ui,
            "Shader Overrides",
            "profile_shader_overrides",
            false,
            collapsed,
            |ui| {
                ui.label(
                    egui::RichText::new(
                        "Optional background shader settings applied when this profile opens.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(6.0);

                egui::Grid::new("profile_shader_form")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Shader:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_shader);
                            ui.label(egui::RichText::new("(blank = global)").small().weak());
                        });
                        ui.end_row();

                        optional_slider_row(
                            ui,
                            "Brightness:",
                            &mut self.temp_shader_brightness,
                            0.05..=1.0,
                            0.35,
                        );
                        optional_slider_row(
                            ui,
                            "Text opacity:",
                            &mut self.temp_shader_text_opacity,
                            0.0..=1.0,
                            1.0,
                        );
                        optional_slider_row(
                            ui,
                            "Animation speed:",
                            &mut self.temp_shader_animation_speed,
                            0.0..=5.0,
                            1.0,
                        );

                        for index in 0..4 {
                            ui.label(format!("iChannel{}:", index));
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.temp_shader_channels[index]);
                                ui.label(egui::RichText::new("(blank = inherit)").small().weak());
                            });
                            ui.end_row();
                        }
                    });
            },
        );
    }

    /// Render the tmux auto-connect collapsing section.
    pub(super) fn render_tmux_section(
        &mut self,
        ui: &mut egui::Ui,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(
            ui,
            "Tmux Auto-Connect",
            "profile_tmux",
            false,
            collapsed,
            |ui| {
                ui.label(
                    egui::RichText::new(
                        "Automatically connect to a tmux session when this profile opens.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(6.0);

                egui::Grid::new("profile_tmux_form")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Session Name:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tmux_session_name);
                            ui.label(
                                egui::RichText::new("(empty = disabled)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label("Connection Mode:");
                        ui.horizontal(|ui| {
                            ui.radio_value(
                                &mut self.temp_tmux_connection_mode,
                                par_term_config::TmuxConnectionMode::ControlMode,
                                "Control Mode",
                            )
                            .on_hover_text(
                                "Full par-term integration via tmux -CC. Enables pane sync, layout, and input routing.",
                            );
                            ui.add_space(8.0);
                            ui.radio_value(
                                &mut self.temp_tmux_connection_mode,
                                par_term_config::TmuxConnectionMode::Normal,
                                "Normal",
                            )
                            .on_hover_text(
                                "Plain tmux running in the terminal. No par-term integration.",
                            );
                        });
                        ui.end_row();
                    });
            },
        );
    }

    /// Render the SSH connection collapsing section.
    pub(super) fn render_ssh_section(
        &mut self,
        ui: &mut egui::Ui,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(
            ui,
            "SSH Connection",
            "profile_ssh_connection",
            false,
            collapsed,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Host:");
                    ui.text_edit_singleline(&mut self.temp_ssh_host);
                });
                ui.horizontal(|ui| {
                    ui.label("User:");
                    ui.text_edit_singleline(&mut self.temp_ssh_user);
                });
                ui.horizontal(|ui| {
                    ui.label("Port:");
                    ui.add(egui::TextEdit::singleline(&mut self.temp_ssh_port).desired_width(60.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Identity File:");
                    ui.text_edit_singleline(&mut self.temp_ssh_identity_file);
                });
                ui.horizontal(|ui| {
                    ui.label("Extra Args:");
                    ui.text_edit_singleline(&mut self.temp_ssh_extra_args);
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "When SSH Host is set, opening this profile connects via SSH instead of launching a shell.",
                    )
                    .weak()
                    .size(11.0),
                );
            },
        );
    }
}

fn optional_slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f32>,
    range: std::ops::RangeInclusive<f32>,
    default_value: f32,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let mut use_custom = value.is_some();
        if ui.checkbox(&mut use_custom, "").changed() {
            *value = if use_custom {
                Some(default_value)
            } else {
                None
            };
        }
        if let Some(current) = value {
            ui.add(egui::Slider::new(current, range));
        } else {
            ui.label(egui::RichText::new("(use global)").small().weak());
        }
    });
    ui.end_row();
}
