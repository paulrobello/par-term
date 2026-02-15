use super::SettingsUI;
use crate::config::{
    BackgroundImageMode, BackgroundMode, CursorShaderConfig, CursorShaderMetadata, ShaderConfig,
    ShaderMetadata,
};
use arboard::Clipboard;
use egui::Color32;
use std::path::Path;

/// Convert an absolute path to a path relative to the shaders directory if possible.
/// If the path is within the shaders directory, returns a relative path.
/// Otherwise, returns the original path unchanged.
/// Always uses forward slashes for cross-platform compatibility.
fn make_path_relative_to_shaders(absolute_path: &str) -> String {
    let shaders_dir = crate::config::Config::shaders_dir();
    let path = Path::new(absolute_path);

    // Try to make it relative to the shaders directory
    if let Ok(relative) = path.strip_prefix(&shaders_dir) {
        // Use forward slashes for cross-platform compatibility
        let relative_str = relative.display().to_string();
        relative_str.replace('\\', "/")
    } else {
        // Path is outside shaders directory, keep as-is
        absolute_path.to_string()
    }
}

pub fn show_background(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    egui::CollapsingHeader::new("Background & Effects")
        .default_open(true)
        .show(ui, |ui| {
        // Background mode selector
        ui.horizontal(|ui| {
            ui.label("Background mode:");
            let current = match settings.config.background_mode {
                BackgroundMode::Default => 0,
                BackgroundMode::Color => 1,
                BackgroundMode::Image => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("bg_source_mode")
                .selected_text(match current {
                    0 => "Default (Theme)",
                    1 => "Solid Color",
                    2 => "Image",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Default (Theme)");
                    ui.selectable_value(&mut selected, 1, "Solid Color");
                    ui.selectable_value(&mut selected, 2, "Image");
                });
            if selected != current {
                settings.config.background_mode = match selected {
                    0 => BackgroundMode::Default,
                    1 => BackgroundMode::Color,
                    2 => BackgroundMode::Image,
                    _ => BackgroundMode::Default,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        // Mode-specific settings
        match settings.config.background_mode {
            BackgroundMode::Default => {
                ui.label("Using theme background color.");
            }
            BackgroundMode::Color => {
                // Solid color settings
                ui.horizontal(|ui| {
                    ui.label("Background color:");
                    // Convert [u8; 3] to egui Color32 for color picker
                    let mut color = Color32::from_rgb(
                        settings.temp_background_color[0],
                        settings.temp_background_color[1],
                        settings.temp_background_color[2],
                    );
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        settings.temp_background_color = [color.r(), color.g(), color.b()];
                        settings.config.background_color = settings.temp_background_color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    // Show hex value
                    ui.label(format!(
                        "#{:02X}{:02X}{:02X}",
                        settings.temp_background_color[0],
                        settings.temp_background_color[1],
                        settings.temp_background_color[2]
                    ));
                });
                ui.label("Transparency controlled by Window Opacity setting.");
            }
            BackgroundMode::Image => {
                // Image settings
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
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

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

        // Show shader metadata and per-shader settings if a shader is selected
        if !settings.temp_custom_shader.is_empty() {
            show_shader_metadata_and_settings(ui, settings, changes_this_frame);
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

        if ui
            .checkbox(
                &mut settings.config.shader_hot_reload,
                "Enable shader hot reload",
            )
            .on_hover_text("Automatically reload shaders when files change on disk")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.shader_hot_reload {
            ui.horizontal(|ui| {
                ui.label("Hot reload delay:");
                // Convert u64 to u32 for slider
                let mut delay = settings.config.shader_hot_reload_delay as u32;
                if ui
                    .add(
                        egui::Slider::new(&mut delay, 50..=1000)
                            .suffix(" ms"),
                    )
                    .on_hover_text("Debounce delay before reloading shader after file change (helps avoid multiple reloads)")
                    .changed()
                {
                    settings.config.shader_hot_reload_delay = delay as u64;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

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

        // Cubemap settings
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Cubemap:");
            let selected_text = if settings.temp_cubemap_path.is_empty() {
                "(none)".to_string()
            } else {
                // Show just the cubemap name, not full path
                settings.temp_cubemap_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&settings.temp_cubemap_path)
                    .to_string()
            };

            let mut cubemap_changed = false;
            egui::ComboBox::from_id_salt("cubemap_select")
                .selected_text(&selected_text)
                .width(200.0)
                .show_ui(ui, |ui| {
                    // Option to select none
                    if ui.selectable_label(settings.temp_cubemap_path.is_empty(), "(none)").clicked() {
                        settings.temp_cubemap_path.clear();
                        settings.config.custom_shader_cubemap = None;
                        cubemap_changed = true;
                    }

                    // List available cubemaps
                    for cubemap in &settings.available_cubemaps.clone() {
                        let display_name = cubemap.rsplit('/').next().unwrap_or(cubemap);
                        let is_selected = settings.temp_cubemap_path == *cubemap;
                        if ui.selectable_label(is_selected, display_name).clicked() {
                            settings.temp_cubemap_path = cubemap.clone();
                            settings.config.custom_shader_cubemap = Some(cubemap.clone());
                            cubemap_changed = true;
                        }
                    }
                });

            if cubemap_changed {
                log::info!(
                    "Cubemap changed in UI: path={:?}",
                    settings.config.custom_shader_cubemap
                );
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Refresh button
            if ui.button("↻").on_hover_text("Refresh cubemap list").clicked() {
                settings.refresh_cubemaps();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Browse folder...").clicked()
                && let Some(folder) = rfd::FileDialog::new()
                    .set_title("Select folder containing cubemap faces")
                    .pick_folder()
            {
                // Look for common cubemap prefixes in the selected folder
                if let Some(prefix) = find_cubemap_prefix(&folder) {
                    // Convert to relative path like texture channels
                    let relative_path = make_path_relative_to_shaders(&prefix.to_string_lossy());
                    settings.temp_cubemap_path = relative_path.clone();
                    settings.config.custom_shader_cubemap = Some(relative_path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }

            if ui.button("Clear").clicked() && !settings.temp_cubemap_path.is_empty() {
                settings.temp_cubemap_path.clear();
                settings.config.custom_shader_cubemap = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.custom_shader_cubemap_enabled,
                "Enable cubemap",
            )
            .on_hover_text("Enable iCubemap uniform for environment mapping in shaders")
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

        // Shader channel textures (iChannel0-3) section
        ui.add_space(8.0);
        ui.collapsing("Shader Channel Textures (iChannel0-3)", |ui| {
            ui.label("Provide texture inputs to shaders via iChannel0-3 (Shadertoy compatible)");
            ui.add_space(4.0);

            // iChannel0
            ui.horizontal(|ui| {
                ui.label("iChannel0:");
                if ui.text_edit_singleline(&mut settings.temp_shader_channel0).changed() {
                    settings.config.custom_shader_channel0 = if settings.temp_shader_channel0.is_empty() {
                        None
                    } else {
                        Some(settings.temp_shader_channel0.clone())
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse…").clicked()
                    && let Some(path) = settings.pick_file_path("Select iChannel0 texture")
                {
                    let relative_path = make_path_relative_to_shaders(&path);
                    settings.temp_shader_channel0 = relative_path.clone();
                    settings.config.custom_shader_channel0 = Some(relative_path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !settings.temp_shader_channel0.is_empty() && ui.button("×").clicked() {
                    settings.temp_shader_channel0.clear();
                    settings.config.custom_shader_channel0 = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

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
                    let relative_path = make_path_relative_to_shaders(&path);
                    settings.temp_shader_channel1 = relative_path.clone();
                    settings.config.custom_shader_channel1 = Some(relative_path);
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
                    let relative_path = make_path_relative_to_shaders(&path);
                    settings.temp_shader_channel2 = relative_path.clone();
                    settings.config.custom_shader_channel2 = Some(relative_path);
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
                    let relative_path = make_path_relative_to_shaders(&path);
                    settings.temp_shader_channel3 = relative_path.clone();
                    settings.config.custom_shader_channel3 = Some(relative_path);
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

            ui.add_space(4.0);
            ui.label("Textures are available in shaders as iChannel0-3");
            ui.label("Terminal content is available as iChannel4");
            ui.label("Use iChannelResolution[n].xy for texture dimensions");
        });

        // Use background as iChannel0
        ui.add_space(8.0);
        if ui
            .checkbox(
                &mut settings.config.custom_shader_use_background_as_channel0,
                "Use background as iChannel0",
            )
            .on_hover_text(
                "When enabled, the app's background (image or solid color) is bound as iChannel0 instead of a separate texture file.\n\
                This allows shaders to incorporate the background without requiring a separate texture."
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

pub fn show_pane_backgrounds(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    egui::CollapsingHeader::new("Per-Pane Background")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Override the global background for individual split panes.");
            ui.add_space(4.0);

            // Identify Panes button + Pane index selector
            ui.horizontal(|ui| {
                if ui
                    .button("Identify Panes")
                    .on_hover_text("Flash pane indices on the terminal window for 3 seconds")
                    .clicked()
                {
                    settings.identify_panes_requested = true;
                }
            });

            ui.add_space(4.0);

            // Initialize temp fields from pane 0 config on first render
            if settings.temp_pane_bg_index.is_none() {
                settings.temp_pane_bg_index = Some(0);
                if let Some(pb) = settings.config.get_pane_background(0) {
                    settings.temp_pane_bg_path = pb.image_path.unwrap_or_default();
                    settings.temp_pane_bg_mode = pb.mode;
                    settings.temp_pane_bg_opacity = pb.opacity;
                }
            }

            // Pane index selector
            ui.horizontal(|ui| {
                ui.label("Pane index:");
                let mut index = settings.temp_pane_bg_index.unwrap_or(0);
                if ui
                    .add(egui::DragValue::new(&mut index).range(0..=9))
                    .changed()
                {
                    settings.temp_pane_bg_index = Some(index);
                    // Load existing config for this pane index if available
                    if let Some(pb) = settings.config.get_pane_background(index) {
                        settings.temp_pane_bg_path = pb.image_path.unwrap_or_default();
                        settings.temp_pane_bg_mode = pb.mode;
                        settings.temp_pane_bg_opacity = pb.opacity;
                    } else {
                        settings.temp_pane_bg_path.clear();
                        settings.temp_pane_bg_mode = BackgroundImageMode::default();
                        settings.temp_pane_bg_opacity = 1.0;
                    }
                }
            });

            // Image path
            ui.horizontal(|ui| {
                ui.label("Image path:");
                if ui
                    .text_edit_singleline(&mut settings.temp_pane_bg_path)
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse\u{2026}").clicked()
                    && let Some(path) = settings.pick_file_path("Select pane background image")
                {
                    settings.temp_pane_bg_path = path;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Mode dropdown
            ui.horizontal(|ui| {
                ui.label("Mode:");
                let current = settings.temp_pane_bg_mode as usize;
                let mut selected = current;
                egui::ComboBox::from_id_salt("pane_bg_mode")
                    .selected_text(match current {
                        0 => "Fit",
                        1 => "Fill",
                        2 => "Stretch",
                        3 => "Tile",
                        4 => "Center",
                        _ => "Stretch",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, 0, "Fit");
                        ui.selectable_value(&mut selected, 1, "Fill");
                        ui.selectable_value(&mut selected, 2, "Stretch");
                        ui.selectable_value(&mut selected, 3, "Tile");
                        ui.selectable_value(&mut selected, 4, "Center");
                    });
                if selected != current {
                    settings.temp_pane_bg_mode = match selected {
                        0 => BackgroundImageMode::Fit,
                        1 => BackgroundImageMode::Fill,
                        2 => BackgroundImageMode::Stretch,
                        3 => BackgroundImageMode::Tile,
                        4 => BackgroundImageMode::Center,
                        _ => BackgroundImageMode::default(),
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Opacity slider
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.temp_pane_bg_opacity,
                        0.0..=1.0,
                    ))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(4.0);

            // Apply and Clear buttons
            ui.horizontal(|ui| {
                if ui.button("Apply to pane").clicked() {
                    let index = settings.temp_pane_bg_index.unwrap_or(0);
                    // Remove existing config for this index
                    settings
                        .config
                        .pane_backgrounds
                        .retain(|pb| pb.index != index);
                    // Add new config if path is not empty
                    if !settings.temp_pane_bg_path.is_empty() {
                        settings.config.pane_backgrounds.push(
                            crate::config::PaneBackgroundConfig {
                                index,
                                image: settings.temp_pane_bg_path.clone(),
                                mode: settings.temp_pane_bg_mode,
                                opacity: settings.temp_pane_bg_opacity,
                            },
                        );
                    }
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if ui.button("Clear pane background").clicked() {
                    let index = settings.temp_pane_bg_index.unwrap_or(0);
                    settings
                        .config
                        .pane_backgrounds
                        .retain(|pb| pb.index != index);
                    settings.temp_pane_bg_path.clear();
                    settings.temp_pane_bg_mode = BackgroundImageMode::default();
                    settings.temp_pane_bg_opacity = 1.0;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Show configured pane backgrounds
            if !settings.config.pane_backgrounds.is_empty() {
                ui.add_space(4.0);
                ui.label("Configured pane backgrounds:");
                for pb in &settings.config.pane_backgrounds {
                    ui.label(format!(
                        "  Pane {}: {} ({:?}, opacity: {:.1})",
                        pb.index, pb.image, pb.mode, pb.opacity
                    ));
                }
            }
        });
}

pub fn show_cursor_shader(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    egui::CollapsingHeader::new("Cursor Shader")
        .default_open(true)
        .show(ui, |ui| {
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

        // Show cursor shader metadata and per-shader settings if a shader is selected
        if !settings.temp_cursor_shader.is_empty() {
            show_cursor_shader_metadata_and_settings(ui, settings, changes_this_frame);
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
                    egui::Slider::new(&mut settings.config.cursor_shader_trail_duration, 0.0..=2.0)
                        .suffix(" s"),
                )
                .on_hover_text("Duration of cursor trail effect in seconds (iCursorTrailDuration)")
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
                    egui::Slider::new(&mut settings.config.cursor_shader_glow_radius, 0.0..=200.0)
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

/// Show shader metadata and per-shader settings section
fn show_shader_metadata_and_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    let shader_name = settings.temp_custom_shader.clone();

    // Get metadata for current shader (cached)
    let metadata = settings.shader_metadata_cache.get(&shader_name).cloned();

    // Show collapsible section for shader info and per-shader settings
    ui.add_space(4.0);
    let header_text = if let Some(ref meta) = metadata {
        if let Some(ref name) = meta.name {
            format!("Shader Settings: {}", name)
        } else {
            format!("Shader Settings: {}", shader_name)
        }
    } else {
        format!("Shader Settings: {}", shader_name)
    };

    egui::CollapsingHeader::new(header_text)
        .id_salt("shader_settings")
        .default_open(settings.shader_settings_expanded)
        .show(ui, |ui| {
            settings.shader_settings_expanded = true;

            // Show metadata if available
            if let Some(ref meta) = metadata {
                show_shader_metadata_info(ui, meta);
                ui.add_space(4.0);
                ui.separator();
            }

            // Per-shader settings with override controls
            ui.add_space(4.0);
            ui.label("Per-shader overrides (takes precedence over global settings):");
            ui.add_space(4.0);

            show_per_shader_settings(ui, settings, &shader_name, &metadata, changes_this_frame);
        });
}

/// Show shader metadata info (name, author, description, version)
fn show_shader_metadata_info(ui: &mut egui::Ui, metadata: &crate::config::ShaderMetadata) {
    egui::Grid::new("shader_metadata_grid")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            if let Some(ref name) = metadata.name {
                ui.label("Name:");
                ui.label(name);
                ui.end_row();
            }

            if let Some(ref author) = metadata.author {
                ui.label("Author:");
                ui.label(author);
                ui.end_row();
            }

            if let Some(ref version) = metadata.version {
                ui.label("Version:");
                ui.label(version);
                ui.end_row();
            }

            if let Some(ref description) = metadata.description {
                ui.label("Description:");
                ui.label(description);
                ui.end_row();
            }
        });
}

/// Show per-shader settings controls with reset buttons
fn show_per_shader_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    shader_name: &str,
    metadata: &Option<crate::config::ShaderMetadata>,
    changes_this_frame: &mut bool,
) {
    // Get current override or create empty one for display
    let has_override = settings.config.shader_configs.contains_key(shader_name);

    // Get metadata defaults (if any) - clone to avoid borrow issues
    let meta_defaults = metadata.as_ref().map(|m| m.defaults.clone());

    // Clone current override to avoid borrow issues with closures
    let current_override = settings.config.shader_configs.get(shader_name).cloned();

    // Animation Speed
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.animation_speed)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.animation_speed))
            .unwrap_or(settings.config.custom_shader_animation_speed);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.animation_speed)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            ui.label("Animation speed:");
            let response = ui.add(egui::Slider::new(&mut value, 0.0..=5.0));

            if response.changed() {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                override_entry.animation_speed = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                override_entry.animation_speed = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Brightness
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.brightness)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.brightness))
            .unwrap_or(settings.config.custom_shader_brightness);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.brightness)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            ui.label("Brightness:");
            let response = ui.add(
                egui::Slider::new(&mut value, 0.05..=1.0)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );

            if response.changed() {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                override_entry.brightness = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                override_entry.brightness = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Text Opacity
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.text_opacity)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.text_opacity))
            .unwrap_or(settings.config.custom_shader_text_opacity);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.text_opacity)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            ui.label("Text opacity:");
            let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0));

            if response.changed() {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                override_entry.text_opacity = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                override_entry.text_opacity = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Full Content Mode
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.full_content)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.full_content))
            .unwrap_or(settings.config.custom_shader_full_content);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.full_content)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut value, "Full content mode")
                .on_hover_text("Shader receives and can manipulate full terminal content")
                .changed()
            {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                override_entry.full_content = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                override_entry.full_content = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Use Background Image as iChannel0
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.use_background_as_channel0)
            .or_else(|| {
                meta_defaults
                    .as_ref()
                    .and_then(|m| m.use_background_as_channel0)
            })
            .unwrap_or(settings.config.custom_shader_use_background_as_channel0);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.use_background_as_channel0)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut value, "Use background as iChannel0")
                .on_hover_text(
                    "Use the app's background (image or solid color) as iChannel0 instead of a separate texture file",
                )
                .changed()
            {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                override_entry.use_background_as_channel0 = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                override_entry.use_background_as_channel0 = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Show channel texture overrides in a sub-collapsible
    ui.add_space(4.0);
    let meta_defaults_for_channels = meta_defaults.clone();
    egui::CollapsingHeader::new("Channel Textures")
        .id_salt("per_shader_channels")
        .default_open(false)
        .show(ui, |ui| {
            show_per_shader_channel_settings(
                ui,
                settings,
                shader_name,
                meta_defaults_for_channels.as_ref(),
                changes_this_frame,
            );
        });

    // Reset all overrides button
    if has_override {
        ui.add_space(8.0);
        if ui
            .button("Reset All Overrides")
            .on_hover_text("Remove all per-shader overrides and use defaults")
            .clicked()
        {
            settings.config.remove_shader_override(shader_name);
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    }

    // Save to Shader button - writes current effective settings as defaults in the shader file
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui
            .button("💾 Save Defaults to Shader")
            .on_hover_text(
                "Write the current effective settings as defaults in the shader file's metadata block.\n\
                This will update or create the /*! par-term shader metadata ... */ block.",
            )
            .clicked()
        {
            save_settings_to_shader_metadata(settings, shader_name, metadata);
        }
    });
}

/// Save current effective settings to the shader file's metadata block.
fn save_settings_to_shader_metadata(
    settings: &mut SettingsUI,
    shader_name: &str,
    existing_metadata: &Option<crate::config::ShaderMetadata>,
) {
    // Get the shader file path
    let shader_path = crate::config::Config::shader_path(shader_name);

    if !shader_path.exists() {
        log::error!(
            "Cannot save metadata: shader file not found: {}",
            shader_path.display()
        );
        settings.shader_editor_error = Some(format!(
            "Cannot save metadata: shader file not found:\n{}",
            shader_path.display()
        ));
        return;
    }

    // Build the new metadata from current effective settings
    let new_metadata = build_metadata_from_settings(settings, shader_name, existing_metadata);

    // Update the shader file
    match crate::config::update_shader_metadata_file(&shader_path, &new_metadata) {
        Ok(()) => {
            log::info!("Saved metadata to shader: {}", shader_path.display());
            // Invalidate the cache so the new metadata is picked up
            settings.shader_metadata_cache.invalidate(shader_name);
            // Clear any previous error
            settings.shader_editor_error = None;
        }
        Err(e) => {
            log::error!("Failed to save metadata to shader: {}", e);
            settings.shader_editor_error = Some(format!("Failed to save metadata:\n{}", e));
        }
    }
}

/// Build a ShaderMetadata struct from the current effective settings.
fn build_metadata_from_settings(
    settings: &SettingsUI,
    shader_name: &str,
    existing_metadata: &Option<ShaderMetadata>,
) -> ShaderMetadata {
    // Start with existing metadata info (name, author, description, version) or defaults
    let mut metadata = existing_metadata.clone().unwrap_or_else(|| ShaderMetadata {
        name: Some(shader_name.trim_end_matches(".glsl").to_string()),
        author: None,
        description: None,
        version: Some("1.0.0".to_string()),
        defaults: ShaderConfig::default(),
    });

    // Get the current override and metadata defaults
    let current_override = settings.config.shader_configs.get(shader_name);
    let meta_defaults = existing_metadata.as_ref().map(|m| &m.defaults);

    // Build the new defaults from effective values
    // For each field, use: override -> existing meta default -> global default
    // But we only save non-default values to keep the metadata clean

    let mut new_defaults = ShaderConfig::default();

    // Animation speed
    let effective_speed = current_override
        .and_then(|o| o.animation_speed)
        .or_else(|| meta_defaults.and_then(|m| m.animation_speed))
        .unwrap_or(settings.config.custom_shader_animation_speed);
    if (effective_speed - 1.0).abs() > 0.001 {
        new_defaults.animation_speed = Some(effective_speed);
    }

    // Brightness
    let effective_brightness = current_override
        .and_then(|o| o.brightness)
        .or_else(|| meta_defaults.and_then(|m| m.brightness))
        .unwrap_or(settings.config.custom_shader_brightness);
    if (effective_brightness - 1.0).abs() > 0.001 {
        new_defaults.brightness = Some(effective_brightness);
    }

    // Text opacity
    let effective_text_opacity = current_override
        .and_then(|o| o.text_opacity)
        .or_else(|| meta_defaults.and_then(|m| m.text_opacity))
        .unwrap_or(settings.config.custom_shader_text_opacity);
    if (effective_text_opacity - 1.0).abs() > 0.001 {
        new_defaults.text_opacity = Some(effective_text_opacity);
    }

    // Full content mode
    let effective_full_content = current_override
        .and_then(|o| o.full_content)
        .or_else(|| meta_defaults.and_then(|m| m.full_content))
        .unwrap_or(settings.config.custom_shader_full_content);
    if effective_full_content {
        new_defaults.full_content = Some(true);
    }

    // Channel textures - only save if set
    let effective_channel0 = current_override
        .and_then(|o| o.channel0.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel0.clone()))
        .or_else(|| settings.config.custom_shader_channel0.clone());
    if effective_channel0.is_some() {
        new_defaults.channel0 = effective_channel0;
    }

    let effective_channel1 = current_override
        .and_then(|o| o.channel1.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel1.clone()))
        .or_else(|| settings.config.custom_shader_channel1.clone());
    if effective_channel1.is_some() {
        new_defaults.channel1 = effective_channel1;
    }

    let effective_channel2 = current_override
        .and_then(|o| o.channel2.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel2.clone()))
        .or_else(|| settings.config.custom_shader_channel2.clone());
    if effective_channel2.is_some() {
        new_defaults.channel2 = effective_channel2;
    }

    let effective_channel3 = current_override
        .and_then(|o| o.channel3.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel3.clone()))
        .or_else(|| settings.config.custom_shader_channel3.clone());
    if effective_channel3.is_some() {
        new_defaults.channel3 = effective_channel3;
    }

    // Cubemap
    let effective_cubemap = current_override
        .and_then(|o| o.cubemap.clone())
        .or_else(|| meta_defaults.and_then(|m| m.cubemap.clone()))
        .or_else(|| settings.config.custom_shader_cubemap.clone());
    if effective_cubemap.is_some() {
        new_defaults.cubemap = effective_cubemap;
    }

    // Cubemap enabled - only save if false (true is default)
    let effective_cubemap_enabled = current_override
        .and_then(|o| o.cubemap_enabled)
        .or_else(|| meta_defaults.and_then(|m| m.cubemap_enabled))
        .unwrap_or(settings.config.custom_shader_cubemap_enabled);
    if !effective_cubemap_enabled {
        new_defaults.cubemap_enabled = Some(false);
    }

    // Use background as channel0 - only save if true (false is default)
    let effective_use_background = current_override
        .and_then(|o| o.use_background_as_channel0)
        .or_else(|| meta_defaults.and_then(|m| m.use_background_as_channel0))
        .unwrap_or(settings.config.custom_shader_use_background_as_channel0);
    if effective_use_background {
        new_defaults.use_background_as_channel0 = Some(true);
    }

    metadata.defaults = new_defaults;
    metadata
}

/// Show per-shader channel texture settings
fn show_per_shader_channel_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    shader_name: &str,
    meta_defaults: Option<&ShaderConfig>,
    changes_this_frame: &mut bool,
) {
    // Clone current override to avoid borrow issues
    let current_override = settings.config.shader_configs.get(shader_name).cloned();

    // Show each channel
    for channel_num in 0..4u8 {
        let (override_val, meta_val, global_val) = match channel_num {
            0 => (
                current_override.as_ref().and_then(|o| o.channel0.clone()),
                meta_defaults.and_then(|m| m.channel0.clone()),
                settings.config.custom_shader_channel0.clone(),
            ),
            1 => (
                current_override.as_ref().and_then(|o| o.channel1.clone()),
                meta_defaults.and_then(|m| m.channel1.clone()),
                settings.config.custom_shader_channel1.clone(),
            ),
            2 => (
                current_override.as_ref().and_then(|o| o.channel2.clone()),
                meta_defaults.and_then(|m| m.channel2.clone()),
                settings.config.custom_shader_channel2.clone(),
            ),
            3 => (
                current_override.as_ref().and_then(|o| o.channel3.clone()),
                meta_defaults.and_then(|m| m.channel3.clone()),
                settings.config.custom_shader_channel3.clone(),
            ),
            _ => continue,
        };

        // Check if override is explicitly empty (cleared)
        let is_explicitly_cleared = override_val.as_ref().is_some_and(|v| v.is_empty());

        // For display, show empty if explicitly cleared, otherwise show effective value
        let effective_value = if is_explicitly_cleared {
            None
        } else {
            override_val
                .clone()
                .or(meta_val.clone())
                .or(global_val.clone())
        };
        let mut display_value = effective_value.clone().unwrap_or_default();
        let has_override = override_val.is_some();

        // Can clear if there's a value from metadata or global (and not already cleared)
        let has_default_value = meta_val.is_some() || global_val.is_some();
        let can_clear = has_default_value && !is_explicitly_cleared;

        ui.horizontal(|ui| {
            ui.label(format!("iChannel{}:", channel_num));

            // Show "(cleared)" placeholder when explicitly cleared
            let response = if is_explicitly_cleared {
                ui.add(egui::TextEdit::singleline(&mut display_value).hint_text("(cleared)"))
            } else {
                ui.text_edit_singleline(&mut display_value)
            };

            if response.changed() {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                // When typing, set the value (empty removes override, non-empty sets it)
                let new_val = if display_value.is_empty() {
                    None
                } else {
                    Some(display_value.clone())
                };
                match channel_num {
                    0 => override_entry.channel0 = new_val,
                    1 => override_entry.channel1 = new_val,
                    2 => override_entry.channel2 = new_val,
                    3 => override_entry.channel3 = new_val,
                    _ => {}
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui.button("Browse…").clicked()
                && let Some(path) =
                    settings.pick_file_path(&format!("Select iChannel{} texture", channel_num))
            {
                let relative_path = make_path_relative_to_shaders(&path);
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                match channel_num {
                    0 => override_entry.channel0 = Some(relative_path),
                    1 => override_entry.channel1 = Some(relative_path),
                    2 => override_entry.channel2 = Some(relative_path),
                    3 => override_entry.channel3 = Some(relative_path),
                    _ => {}
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Clear button - sets explicit empty override to disable default texture
            if can_clear
                && ui
                    .button("×")
                    .on_hover_text("Clear texture (override default)")
                    .clicked()
            {
                let override_entry = settings.config.get_or_create_shader_override(shader_name);
                match channel_num {
                    0 => override_entry.channel0 = Some(String::new()),
                    1 => override_entry.channel1 = Some(String::new()),
                    2 => override_entry.channel2 = Some(String::new()),
                    3 => override_entry.channel3 = Some(String::new()),
                    _ => {}
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Reset button - removes override to restore default
            if show_reset_button(ui, has_override)
                && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
            {
                match channel_num {
                    0 => override_entry.channel0 = None,
                    1 => override_entry.channel1 = None,
                    2 => override_entry.channel2 = None,
                    3 => override_entry.channel3 = None,
                    _ => {}
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Cubemap override
    ui.add_space(4.0);
    let cubemap_override = current_override.as_ref().and_then(|o| o.cubemap.clone());
    let cubemap_meta = meta_defaults.and_then(|m| m.cubemap.clone());
    let cubemap_global = settings.config.custom_shader_cubemap.clone();

    // Check if override is explicitly empty (cleared)
    let is_cubemap_cleared = cubemap_override.as_ref().is_some_and(|v| v.is_empty());

    let effective_cubemap = if is_cubemap_cleared {
        None
    } else {
        cubemap_override
            .clone()
            .or(cubemap_meta.clone())
            .or(cubemap_global.clone())
    };
    let has_cubemap_override = cubemap_override.is_some();

    // Check if there's a default value that can be cleared
    let has_cubemap_default = cubemap_meta.is_some() || cubemap_global.is_some();

    ui.horizontal(|ui| {
        ui.label("Cubemap:");

        // Determine display text for dropdown
        let selected_text = if is_cubemap_cleared {
            "(cleared)".to_string()
        } else if let Some(ref path) = effective_cubemap {
            if path.is_empty() {
                "(none)".to_string()
            } else {
                // Show just the cubemap name, not full path
                path.rsplit('/').next().unwrap_or(path).to_string()
            }
        } else {
            "(none)".to_string()
        };

        let mut cubemap_changed = false;
        let combo_id = format!("cubemap_override_{}", shader_name);
        egui::ComboBox::from_id_salt(&combo_id)
            .selected_text(&selected_text)
            .width(150.0)
            .show_ui(ui, |ui| {
                // Option to use default (remove override)
                let using_default = !has_cubemap_override;
                if ui
                    .selectable_label(using_default, "(use default)")
                    .clicked()
                    && !using_default
                {
                    if let Some(override_entry) =
                        settings.config.shader_configs.get_mut(shader_name)
                    {
                        override_entry.cubemap = None;
                    }
                    cubemap_changed = true;
                }

                // Option to clear (explicit empty override)
                if has_cubemap_default
                    && ui
                        .selectable_label(is_cubemap_cleared, "(none/clear)")
                        .clicked()
                    && !is_cubemap_cleared
                {
                    let override_entry = settings.config.get_or_create_shader_override(shader_name);
                    override_entry.cubemap = Some(String::new());
                    cubemap_changed = true;
                }

                ui.separator();

                // List available cubemaps
                for cubemap in &settings.available_cubemaps.clone() {
                    let display_name = cubemap.rsplit('/').next().unwrap_or(cubemap);
                    let is_selected = effective_cubemap.as_ref().is_some_and(|c| c == cubemap);
                    if ui.selectable_label(is_selected, display_name).clicked() {
                        let override_entry =
                            settings.config.get_or_create_shader_override(shader_name);
                        override_entry.cubemap = Some(cubemap.clone());
                        cubemap_changed = true;
                    }
                }
            });

        if cubemap_changed {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Refresh button
        if ui
            .button("↻")
            .on_hover_text("Refresh cubemap list")
            .clicked()
        {
            settings.refresh_cubemaps();
        }

        // Reset button
        if show_reset_button(ui, has_cubemap_override)
            && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
        {
            override_entry.cubemap = None;
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });

    // Cubemap enabled
    let cubemap_enabled_override = current_override.as_ref().and_then(|o| o.cubemap_enabled);
    let cubemap_enabled_meta = meta_defaults.and_then(|m| m.cubemap_enabled);
    let effective_cubemap_enabled = cubemap_enabled_override
        .or(cubemap_enabled_meta)
        .unwrap_or(settings.config.custom_shader_cubemap_enabled);
    let has_cubemap_enabled_override = cubemap_enabled_override.is_some();

    let mut value = effective_cubemap_enabled;
    ui.horizontal(|ui| {
        if ui.checkbox(&mut value, "Enable cubemap").changed() {
            let override_entry = settings.config.get_or_create_shader_override(shader_name);
            override_entry.cubemap_enabled = Some(value);
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if show_reset_button(ui, has_cubemap_enabled_override)
            && let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name)
        {
            override_entry.cubemap_enabled = None;
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

/// Show a reset button that's only visible/enabled when there's an override
fn show_reset_button(ui: &mut egui::Ui, has_override: bool) -> bool {
    if has_override {
        ui.button("↺").on_hover_text("Reset to default").clicked()
    } else {
        // Show disabled placeholder to maintain layout
        ui.add_enabled(false, egui::Button::new("↺"))
            .on_hover_text("Using default value");
        false
    }
}

/// Find a cubemap prefix in a folder by looking for standard face naming patterns
fn find_cubemap_prefix(folder: &std::path::Path) -> Option<std::path::PathBuf> {
    // Look for files matching common cubemap naming patterns
    let suffixes = ["px", "nx", "py", "ny", "pz", "nz"];
    let extensions = ["png", "jpg", "jpeg", "hdr"];

    // Try to find any file that matches *-px.* pattern
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Check if this file ends with a face suffix
                for suffix in &suffixes {
                    let pattern = format!("-{}", suffix);
                    if stem.ends_with(&pattern) {
                        // Found a face file, extract the prefix
                        let prefix = &stem[..stem.len() - pattern.len()];
                        // Verify all 6 faces exist
                        let mut all_found = true;
                        for check_suffix in &suffixes {
                            let mut found = false;
                            for ext in &extensions {
                                let face_name = format!("{}-{}.{}", prefix, check_suffix, ext);
                                if folder.join(&face_name).exists() {
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                all_found = false;
                                break;
                            }
                        }
                        if all_found {
                            return Some(folder.join(prefix));
                        }
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// Cursor Shader Metadata Functions
// ============================================================================

/// Show cursor shader metadata and per-shader settings section
fn show_cursor_shader_metadata_and_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    let shader_name = settings.temp_cursor_shader.clone();

    // Get metadata for current cursor shader (cached)
    let metadata = settings
        .cursor_shader_metadata_cache
        .get(&shader_name)
        .cloned();

    // Show collapsible section for shader info and per-shader settings
    ui.add_space(4.0);
    let header_text = if let Some(ref meta) = metadata {
        if let Some(ref name) = meta.name {
            format!("Cursor Shader Settings: {}", name)
        } else {
            format!("Cursor Shader Settings: {}", shader_name)
        }
    } else {
        format!("Cursor Shader Settings: {}", shader_name)
    };

    egui::CollapsingHeader::new(header_text)
        .id_salt("cursor_shader_settings")
        .default_open(settings.cursor_shader_settings_expanded)
        .show(ui, |ui| {
            settings.cursor_shader_settings_expanded = true;

            // Show metadata if available
            if let Some(ref meta) = metadata {
                show_cursor_shader_metadata_info(ui, meta);
                ui.add_space(4.0);
                ui.separator();
            }

            // Per-shader settings with override controls
            ui.add_space(4.0);
            ui.label("Per-shader overrides (takes precedence over global settings):");
            ui.add_space(4.0);

            show_per_cursor_shader_settings(
                ui,
                settings,
                &shader_name,
                &metadata,
                changes_this_frame,
            );
        });
}

/// Show cursor shader metadata info (name, author, description, version)
fn show_cursor_shader_metadata_info(ui: &mut egui::Ui, metadata: &CursorShaderMetadata) {
    egui::Grid::new("cursor_shader_metadata_grid")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            if let Some(ref name) = metadata.name {
                ui.label("Name:");
                ui.label(name);
                ui.end_row();
            }

            if let Some(ref author) = metadata.author {
                ui.label("Author:");
                ui.label(author);
                ui.end_row();
            }

            if let Some(ref version) = metadata.version {
                ui.label("Version:");
                ui.label(version);
                ui.end_row();
            }

            if let Some(ref description) = metadata.description {
                ui.label("Description:");
                ui.label(description);
                ui.end_row();
            }
        });
}

/// Show per-cursor-shader settings controls with reset buttons
fn show_per_cursor_shader_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    shader_name: &str,
    metadata: &Option<CursorShaderMetadata>,
    changes_this_frame: &mut bool,
) {
    // Get current override or create empty one for display
    let has_override = settings
        .config
        .cursor_shader_configs
        .contains_key(shader_name);

    // Get metadata defaults (if any) - clone to avoid borrow issues
    let meta_defaults = metadata.as_ref().map(|m| m.defaults.clone());

    // Clone current override to avoid borrow issues with closures
    let current_override = settings
        .config
        .cursor_shader_configs
        .get(shader_name)
        .cloned();

    // Animation Speed (universal setting applicable to all shaders)
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.base.animation_speed)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.base.animation_speed))
            .unwrap_or(settings.config.cursor_shader_animation_speed);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.base.animation_speed)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            ui.label("Animation speed:");
            let response = ui.add(egui::Slider::new(&mut value, 0.0..=5.0));

            if response.changed() {
                let override_entry = settings
                    .config
                    .get_or_create_cursor_shader_override(shader_name);
                override_entry.base.animation_speed = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) =
                    settings.config.cursor_shader_configs.get_mut(shader_name)
            {
                override_entry.base.animation_speed = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Hide Default Cursor (per-shader override)
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.hides_cursor)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.hides_cursor))
            .unwrap_or(settings.config.cursor_shader_hides_cursor);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.hides_cursor)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut value, "Hide default cursor")
                .on_hover_text(
                    "When enabled, the normal cursor is not drawn, allowing the shader to fully replace cursor rendering",
                )
                .changed()
            {
                let override_entry = settings
                    .config
                    .get_or_create_cursor_shader_override(shader_name);
                override_entry.hides_cursor = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) =
                    settings.config.cursor_shader_configs.get_mut(shader_name)
            {
                override_entry.hides_cursor = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Disable in Alt Screen (per-shader override)
    {
        let effective_value = current_override
            .as_ref()
            .and_then(|o| o.disable_in_alt_screen)
            .or_else(|| meta_defaults.as_ref().and_then(|m| m.disable_in_alt_screen))
            .unwrap_or(settings.config.cursor_shader_disable_in_alt_screen);
        let has_override_val = current_override
            .as_ref()
            .and_then(|o| o.disable_in_alt_screen)
            .is_some();

        let mut value = effective_value;
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut value, "Disable in alt screen")
                .on_hover_text(
                    "When enabled, the cursor shader is paused in alt-screen apps like vim, less, and htop",
                )
                .changed()
            {
                let override_entry = settings
                    .config
                    .get_or_create_cursor_shader_override(shader_name);
                override_entry.disable_in_alt_screen = Some(value);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if show_reset_button(ui, has_override_val)
                && let Some(override_entry) =
                    settings.config.cursor_shader_configs.get_mut(shader_name)
            {
                override_entry.disable_in_alt_screen = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }

    // Reset all overrides button
    if has_override {
        ui.add_space(8.0);
        if ui
            .button("Reset All Overrides")
            .on_hover_text("Remove all per-shader overrides and use defaults")
            .clicked()
        {
            settings.config.remove_cursor_shader_override(shader_name);
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    }

    // Save to Shader button - writes current effective settings as defaults in the shader file
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui
            .button("💾 Save Defaults to Shader")
            .on_hover_text(
                "Write the current effective settings as defaults in the shader file's metadata block.\n\
                This will update or create the /*! par-term shader metadata ... */ block.",
            )
            .clicked()
        {
            save_cursor_settings_to_shader_metadata(settings, shader_name, metadata);
        }
    });
}

/// Save current effective cursor shader settings to the shader file's metadata block.
fn save_cursor_settings_to_shader_metadata(
    settings: &mut SettingsUI,
    shader_name: &str,
    existing_metadata: &Option<CursorShaderMetadata>,
) {
    // Get the shader file path
    let shader_path = crate::config::Config::shader_path(shader_name);

    if !shader_path.exists() {
        log::error!(
            "Cannot save metadata: cursor shader file not found: {}",
            shader_path.display()
        );
        settings.cursor_shader_editor_error = Some(format!(
            "Cannot save metadata: shader file not found:\n{}",
            shader_path.display()
        ));
        return;
    }

    // Build the new metadata from current effective settings
    let new_metadata =
        build_cursor_metadata_from_settings(settings, shader_name, existing_metadata);

    // Update the shader file
    match crate::config::update_cursor_shader_metadata_file(&shader_path, &new_metadata) {
        Ok(()) => {
            log::info!("Saved metadata to cursor shader: {}", shader_path.display());
            // Invalidate the cache so the new metadata is picked up
            settings
                .cursor_shader_metadata_cache
                .invalidate(shader_name);
            // Clear any previous error
            settings.cursor_shader_editor_error = None;
        }
        Err(e) => {
            log::error!("Failed to save metadata to cursor shader: {}", e);
            settings.cursor_shader_editor_error = Some(format!("Failed to save metadata:\n{}", e));
        }
    }
}

/// Build a CursorShaderMetadata struct from the current effective settings.
fn build_cursor_metadata_from_settings(
    settings: &SettingsUI,
    shader_name: &str,
    existing_metadata: &Option<CursorShaderMetadata>,
) -> CursorShaderMetadata {
    // Start with existing metadata info (name, author, description, version) or defaults
    let mut metadata = existing_metadata
        .clone()
        .unwrap_or_else(|| CursorShaderMetadata {
            name: Some(shader_name.trim_end_matches(".glsl").to_string()),
            author: None,
            description: None,
            version: Some("1.0.0".to_string()),
            defaults: CursorShaderConfig::default(),
        });

    // Get the current override and metadata defaults
    let current_override = settings.config.cursor_shader_configs.get(shader_name);
    let meta_defaults = existing_metadata.as_ref().map(|m| &m.defaults);

    // Start with existing defaults to preserve shader-specific settings
    let mut new_defaults = meta_defaults.cloned().unwrap_or_default();

    // Update animation_speed (universal setting shown in UI)
    let effective_speed = current_override
        .and_then(|o| o.base.animation_speed)
        .or_else(|| meta_defaults.and_then(|m| m.base.animation_speed))
        .unwrap_or(settings.config.cursor_shader_animation_speed);
    if (effective_speed - 1.0).abs() > 0.001 {
        new_defaults.base.animation_speed = Some(effective_speed);
    } else {
        new_defaults.base.animation_speed = None;
    }

    // Update hides_cursor (per-shader override shown in UI)
    let effective_hides_cursor = current_override
        .and_then(|o| o.hides_cursor)
        .or_else(|| meta_defaults.and_then(|m| m.hides_cursor))
        .unwrap_or(settings.config.cursor_shader_hides_cursor);
    // Only save if true (false is the default)
    if effective_hides_cursor {
        new_defaults.hides_cursor = Some(true);
    } else {
        new_defaults.hides_cursor = None;
    }

    // Update disable_in_alt_screen (per-shader override shown in UI)
    let effective_disable_in_alt_screen = current_override
        .and_then(|o| o.disable_in_alt_screen)
        .or_else(|| meta_defaults.and_then(|m| m.disable_in_alt_screen))
        .unwrap_or(settings.config.cursor_shader_disable_in_alt_screen);
    // Only save if false (true is the default)
    if !effective_disable_in_alt_screen {
        new_defaults.disable_in_alt_screen = Some(false);
    } else {
        new_defaults.disable_in_alt_screen = None;
    }

    metadata.defaults = new_defaults;
    metadata
}
