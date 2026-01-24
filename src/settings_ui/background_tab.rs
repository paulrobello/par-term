use super::SettingsUI;
use crate::config::BackgroundImageMode;
use arboard::Clipboard;
use egui::Color32;

pub fn show_background(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.collapsing("Background & Effects", |ui| {
        ui.horizontal(|ui| {
            ui.label("Background image path:");
            if ui
                .text_edit_singleline(&mut settings.temp_background_image)
                .changed()
            {
                settings.config.background_image =
                    if settings.temp_background_image.is_empty() {
                        None
                    } else {
                        Some(settings.temp_background_image.clone())
                    };
                settings.has_changes = true;
            }

            if ui.button("Browse…").clicked()
                && let Some(path) =
                    settings.pick_file_path("Select background image")
            {
                settings.temp_background_image = path.clone();
                settings.config.background_image = Some(path);
                settings.has_changes = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.background_image_enabled,
                "Enable background image",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Background image mode:");
            let current = match settings.config.background_image_mode {
                BackgroundImageMode::Fit => 0,
                BackgroundImageMode::Fill => 1,
                BackgroundImageMode::Stretch => 2,
                BackgroundImageMode::Tile => 3,
                BackgroundImageMode::Center => 4,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("bg_mode")
                .selected_text(match current {
                    0 => "Fit",
                    1 => "Fill",
                    2 => "Stretch",
                    3 => "Tile",
                    4 => "Center",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Fit");
                    ui.selectable_value(&mut selected, 1, "Fill");
                    ui.selectable_value(&mut selected, 2, "Stretch");
                    ui.selectable_value(&mut selected, 3, "Tile");
                    ui.selectable_value(&mut selected, 4, "Center");
                });
            if selected != current {
                settings.config.background_image_mode = match selected {
                    0 => BackgroundImageMode::Fit,
                    1 => BackgroundImageMode::Fill,
                    2 => BackgroundImageMode::Stretch,
                    3 => BackgroundImageMode::Tile,
                    4 => BackgroundImageMode::Center,
                    _ => BackgroundImageMode::Stretch,
                };
                settings.has_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Background image opacity:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.background_image_opacity,
                    0.0..=1.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Shader selection dropdown
        ui.horizontal(|ui| {
            ui.label("Shader:");
            let selected_text = if settings.temp_custom_shader.is_empty() {
                "(none)".to_string()
            } else {
                settings.temp_custom_shader.clone()
            };

            let mut shader_changed = false;
            egui::ComboBox::from_id_salt("shader_select")
                .selected_text(&selected_text)
                .width(200.0)
                .show_ui(ui, |ui| {
                    // Option to select none
                    if ui.selectable_label(settings.temp_custom_shader.is_empty(), "(none)").clicked() {
                        settings.temp_custom_shader.clear();
                        settings.config.custom_shader = None;
                        shader_changed = true;
                    }

                    // List available background shaders (excludes cursor_* shaders)
                    for shader in &settings.background_shaders() {
                        let is_selected = settings.temp_custom_shader == *shader;
                        if ui.selectable_label(is_selected, shader).clicked() {
                            settings.temp_custom_shader = shader.clone();
                            settings.config.custom_shader = Some(shader.clone());
                            shader_changed = true;
                        }
                    }
                });

            if shader_changed {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Refresh button
            if ui.button("↻").on_hover_text("Refresh shader list").clicked() {
                settings.refresh_shaders();
            }
        });

        // Create and Delete buttons
        ui.horizontal(|ui| {
            if ui.button("Create New...").clicked() {
                settings.new_shader_name.clear();
                settings.show_create_shader_dialog = true;
            }

            let has_shader = !settings.temp_custom_shader.is_empty();
            if ui.add_enabled(has_shader, egui::Button::new("Delete")).clicked() {
                settings.show_delete_shader_dialog = true;
            }

            if ui.button("Browse...").on_hover_text("Browse for external shader file").clicked()
                && let Some(path) = settings.pick_file_path("Select shader file")
            {
                settings.temp_custom_shader = path.clone();
                settings.config.custom_shader = Some(path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Show shader compilation error if any
        if let Some(error) = &settings.shader_editor_error {
            let shader_path = crate::config::Config::shader_path(&settings.temp_custom_shader);
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
                        ui.colored_label(Color32::from_rgb(255, 100, 100), "⚠ Shader Error");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Copy").clicked()
                                && let Ok(mut clipboard) = Clipboard::new()
                            {
                                let _ = clipboard.set_text(full_error.clone());
                            }
                        });
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
                            .interactive(false)
                    );
                });
            ui.add_space(4.0);
        }

        if ui
            .checkbox(
                &mut settings.config.custom_shader_enabled,
                "Enable custom shader",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.custom_shader_animation,
                "Enable shader animation",
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
                    &mut settings.config.custom_shader_animation_speed,
                    0.0..=5.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shader brightness:");
            if ui
                .add(
                    egui::Slider::new(
                        &mut settings.config.custom_shader_brightness,
                        0.05..=1.0,
                    )
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                )
                .on_hover_text("Dim the shader background to improve text readability")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shader text opacity:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.custom_shader_text_opacity,
                    0.0..=1.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.custom_shader_full_content,
                "Full content mode",
            )
            .on_hover_text("When enabled, shader receives and can manipulate the full terminal content (text + background). When disabled, shader only provides background and text is composited cleanly on top.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Edit Shader button - only enabled when a shader path is set
        let has_shader_path = !settings.temp_custom_shader.is_empty();
        ui.horizontal(|ui| {
            let edit_button = ui.add_enabled(
                has_shader_path,
                egui::Button::new("Edit Shader..."),
            );
            if edit_button.clicked() {
                // Load shader source from file
                let shader_path = crate::config::Config::shader_path(&settings.temp_custom_shader);
                match std::fs::read_to_string(&shader_path) {
                    Ok(source) => {
                        settings.shader_editor_source = source.clone();
                        settings.shader_editor_original = source;
                        settings.shader_editor_error = None;
                        settings.shader_editor_visible = true;
                    }
                    Err(e) => {
                        settings.shader_editor_error = Some(format!(
                            "Failed to read shader file '{}': {}",
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

        // Shader channel textures (iChannel1-4) section
        ui.add_space(8.0);
        ui.collapsing("Shader Channel Textures (iChannel1-4)", |ui| {
            ui.label("Provide texture inputs to shaders via iChannel1-4");
            ui.add_space(4.0);

            // iChannel1
            ui.horizontal(|ui| {
                ui.label("iChannel1:");
                if ui.text_edit_singleline(&mut settings.temp_shader_channel1).changed() {
                    settings.config.custom_shader_channel1 = if settings.temp_shader_channel1.is_empty() {
                        None
                    } else {
                        Some(settings.temp_shader_channel1.clone())
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse…").clicked()
                    && let Some(path) = settings.pick_file_path("Select iChannel1 texture")
                {
                    settings.temp_shader_channel1 = path.clone();
                    settings.config.custom_shader_channel1 = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !settings.temp_shader_channel1.is_empty() && ui.button("×").clicked() {
                    settings.temp_shader_channel1.clear();
                    settings.config.custom_shader_channel1 = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // iChannel2
            ui.horizontal(|ui| {
                ui.label("iChannel2:");
                if ui.text_edit_singleline(&mut settings.temp_shader_channel2).changed() {
                    settings.config.custom_shader_channel2 = if settings.temp_shader_channel2.is_empty() {
                        None
                    } else {
                        Some(settings.temp_shader_channel2.clone())
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse…").clicked()
                    && let Some(path) = settings.pick_file_path("Select iChannel2 texture")
                {
                    settings.temp_shader_channel2 = path.clone();
                    settings.config.custom_shader_channel2 = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !settings.temp_shader_channel2.is_empty() && ui.button("×").clicked() {
                    settings.temp_shader_channel2.clear();
                    settings.config.custom_shader_channel2 = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // iChannel3
            ui.horizontal(|ui| {
                ui.label("iChannel3:");
                if ui.text_edit_singleline(&mut settings.temp_shader_channel3).changed() {
                    settings.config.custom_shader_channel3 = if settings.temp_shader_channel3.is_empty() {
                        None
                    } else {
                        Some(settings.temp_shader_channel3.clone())
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse…").clicked()
                    && let Some(path) = settings.pick_file_path("Select iChannel3 texture")
                {
                    settings.temp_shader_channel3 = path.clone();
                    settings.config.custom_shader_channel3 = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !settings.temp_shader_channel3.is_empty() && ui.button("×").clicked() {
                    settings.temp_shader_channel3.clear();
                    settings.config.custom_shader_channel3 = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // iChannel4
            ui.horizontal(|ui| {
                ui.label("iChannel4:");
                if ui.text_edit_singleline(&mut settings.temp_shader_channel4).changed() {
                    settings.config.custom_shader_channel4 = if settings.temp_shader_channel4.is_empty() {
                        None
                    } else {
                        Some(settings.temp_shader_channel4.clone())
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse…").clicked()
                    && let Some(path) = settings.pick_file_path("Select iChannel4 texture")
                {
                    settings.temp_shader_channel4 = path.clone();
                    settings.config.custom_shader_channel4 = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !settings.temp_shader_channel4.is_empty() && ui.button("×").clicked() {
                    settings.temp_shader_channel4.clear();
                    settings.config.custom_shader_channel4 = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(4.0);
            ui.label("Textures are available in shaders as iChannel1-4");
            ui.label("Use iChannelResolution[n].xy for texture dimensions");
        });
    });
}

pub fn show_cursor_shader(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.collapsing("Cursor Shader", |ui| {
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
            let shader_path = crate::config::Config::shader_path(&settings.temp_cursor_shader);
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
                        ui.colored_label(Color32::from_rgb(255, 100, 100), "⚠ Cursor Shader Error");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Copy").clicked()
                                && let Ok(mut clipboard) = Clipboard::new()
                            {
                                let _ = clipboard.set_text(full_error.clone());
                            }
                        });
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

        // Edit Shader button - only enabled when a shader path is set
        let has_shader_path = !settings.temp_cursor_shader.is_empty();
        ui.horizontal(|ui| {
            let edit_button =
                ui.add_enabled(has_shader_path, egui::Button::new("Edit Cursor Shader..."));
            if edit_button.clicked() {
                // Load shader source from file
                let shader_path = crate::config::Config::shader_path(&settings.temp_cursor_shader);
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
    });
}
