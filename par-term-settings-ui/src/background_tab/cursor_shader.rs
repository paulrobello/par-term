use crate::SettingsUI;
use crate::section::collapsing_section_with_state;
use arboard::Clipboard;
use egui::Color32;
use std::collections::HashSet;

use super::cursor_shader_metadata::show_cursor_shader_metadata_and_settings;

pub fn show_cursor_shader(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Cursor Shader",
        "cursor_shader",
        true,
        collapsed,
        |ui, collapsed| {
            ui.label("Apply shader effects to cursor (trails, glow, etc.)");
            ui.add_space(4.0);

            // Cursor shader selection dropdown
            ui.horizontal(|ui| {
                ui.label("Shader:");
                let selected_text = if settings.temp_cursor_shader.is_empty() {
                    "(none)".to_string()
                } else {
                    settings.temp_cursor_shader.clone()
                };

                let mut shader_changed = false;
                egui::ComboBox::from_id_salt("cursor_shader_select")
                    .selected_text(&selected_text)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        // Option to select none
                        if ui
                            .selectable_label(settings.temp_cursor_shader.is_empty(), "(none)")
                            .clicked()
                        {
                            settings.temp_cursor_shader.clear();
                            settings.config.cursor_shader = None;
                            shader_changed = true;
                        }

                        // List available cursor shaders (only cursor_* shaders)
                        for shader in &settings.cursor_shaders() {
                            let is_selected = settings.temp_cursor_shader == *shader;
                            if ui.selectable_label(is_selected, shader).clicked() {
                                settings.temp_cursor_shader = shader.clone();
                                settings.config.cursor_shader = Some(shader.clone());
                                shader_changed = true;
                            }
                        }
                    });

                if shader_changed {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                // Refresh button
                if ui
                    .button("↻")
                    .on_hover_text("Refresh shader list")
                    .clicked()
                {
                    settings.refresh_shaders();
                }
            });

            // Browse button for cursor shader
            ui.horizontal(|ui| {
                if ui
                    .button("Browse...")
                    .on_hover_text("Browse for external shader file")
                    .clicked()
                    && let Some(path) = settings.pick_file_path("Select cursor shader file")
                {
                    settings.temp_cursor_shader = path.clone();
                    settings.config.cursor_shader = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Show cursor shader compilation error if any
            if let Some(error) = &settings.cursor_shader_editor_error {
                let shader_path =
                    par_term_config::Config::shader_path(&settings.temp_cursor_shader);
                let full_error = format!("File: {}\n\n{}", shader_path.display(), error);
                let error_display = error.clone();

                ui.add_space(4.0);
                egui::Frame::default()
                    .fill(Color32::from_rgb(80, 20, 20))
                    .inner_margin(8.0)
                    .outer_margin(0.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                Color32::from_rgb(255, 100, 100),
                                "⚠ Cursor Shader Error",
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Copy").clicked()
                                        && let Ok(mut clipboard) = Clipboard::new()
                                    {
                                        let _ = clipboard.set_text(full_error.clone());
                                    }
                                },
                            );
                        });
                        // Show shader path on its own line
                        ui.label(format!("File: {}", shader_path.display()));
                        ui.separator();
                        // Show error details with word wrap
                        ui.add(
                            egui::TextEdit::multiline(&mut error_display.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(3)
                                .interactive(false),
                        );
                    });
                ui.add_space(4.0);
            }

            // Show cursor shader metadata and per-shader settings if a shader is selected
            if !settings.temp_cursor_shader.is_empty() {
                show_cursor_shader_metadata_and_settings(
                    ui,
                    settings,
                    changes_this_frame,
                    collapsed,
                );
            }

            if ui
                .checkbox(
                    &mut settings.config.cursor_shader_enabled,
                    "Enable cursor shader",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.cursor_shader_animation,
                    "Enable cursor shader animation",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Animation speed:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_shader_animation_speed,
                        0.0..=5.0,
                    ))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
            .checkbox(
                &mut settings.config.cursor_shader_hides_cursor,
                "Hide default cursor (let shader handle it)",
            )
            .on_hover_text("When enabled, the normal cursor is not drawn, allowing the cursor shader to fully replace cursor rendering")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            if ui
            .checkbox(
                &mut settings.config.cursor_shader_disable_in_alt_screen,
                "Disable cursor shader in alt screen (vim/less/htop)",
            )
            .on_hover_text("When enabled, cursor shader effects pause while an application is using the alt screen")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            ui.add_space(8.0);
            ui.label("Cursor Shader Parameters:");

            ui.horizontal(|ui| {
                ui.label("Cursor color:");
                let mut color = settings.config.cursor_shader_color;
                if ui
                    .color_edit_button_srgb(&mut color)
                    .on_hover_text("Color passed to cursor shader via iCursorShaderColor uniform")
                    .changed()
                {
                    settings.config.cursor_shader_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Trail duration:");
                if ui
                    .add(
                        egui::Slider::new(
                            &mut settings.config.cursor_shader_trail_duration,
                            0.0..=2.0,
                        )
                        .suffix(" s"),
                    )
                    .on_hover_text(
                        "Duration of cursor trail effect in seconds (iCursorTrailDuration)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Glow radius:");
                if ui
                    .add(
                        egui::Slider::new(
                            &mut settings.config.cursor_shader_glow_radius,
                            0.0..=200.0,
                        )
                        .suffix(" px"),
                    )
                    .on_hover_text("Radius of cursor glow effect in pixels (iCursorGlowRadius)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Glow intensity:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_shader_glow_intensity,
                        0.0..=1.0,
                    ))
                    .on_hover_text("Intensity of cursor glow effect (iCursorGlowIntensity)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);

            // Edit Shader button - only enabled when a shader path is set
            let has_shader_path = !settings.temp_cursor_shader.is_empty();
            ui.horizontal(|ui| {
                let edit_button =
                    ui.add_enabled(has_shader_path, egui::Button::new("Edit Cursor Shader..."));
                if edit_button.clicked() {
                    // Load shader source from file
                    let shader_path =
                        par_term_config::Config::shader_path(&settings.temp_cursor_shader);
                    match std::fs::read_to_string(&shader_path) {
                        Ok(source) => {
                            settings.cursor_shader_editor_source = source.clone();
                            settings.cursor_shader_editor_original = source;
                            settings.cursor_shader_editor_error = None;
                            settings.cursor_shader_editor_visible = true;
                        }
                        Err(e) => {
                            settings.cursor_shader_editor_error = Some(format!(
                                "Failed to read cursor shader file '{}': {}",
                                shader_path.display(),
                                e
                            ));
                        }
                    }
                }
                if !has_shader_path {
                    ui.label("(set shader path first)");
                }
            });
        },
    );
}
