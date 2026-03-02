//! Global (non-per-shader) iChannel0-3 texture inputs and cubemap controls.

use crate::SettingsUI;

use super::shader_settings::{find_cubemap_prefix, make_path_relative_to_shaders};

/// Render the global (top-level) cubemap selection and controls.
pub(super) fn show_cubemap_controls(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Cubemap:");
        let selected_text = if settings.temp_cubemap_path.is_empty() {
            "(none)".to_string()
        } else {
            // Show just the cubemap name, not full path
            settings
                .temp_cubemap_path
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
                if ui
                    .selectable_label(settings.temp_cubemap_path.is_empty(), "(none)")
                    .clicked()
                {
                    settings.temp_cubemap_path.clear();
                    settings.config.shader.custom_shader_cubemap = None;
                    cubemap_changed = true;
                }

                // List available cubemaps
                for cubemap in &settings.available_cubemaps.clone() {
                    let display_name = cubemap.rsplit('/').next().unwrap_or(cubemap);
                    let is_selected = settings.temp_cubemap_path == *cubemap;
                    if ui.selectable_label(is_selected, display_name).clicked() {
                        settings.temp_cubemap_path = cubemap.clone();
                        settings.config.shader.custom_shader_cubemap = Some(cubemap.clone());
                        cubemap_changed = true;
                    }
                }
            });

        if cubemap_changed {
            log::info!(
                "Cubemap changed in UI: path={:?}",
                settings.config.shader.custom_shader_cubemap
            );
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
                settings.config.shader.custom_shader_cubemap = Some(relative_path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        }

        if ui.button("Clear").clicked() && !settings.temp_cubemap_path.is_empty() {
            settings.temp_cubemap_path.clear();
            settings.config.shader.custom_shader_cubemap = None;
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });

    if ui
        .checkbox(
            &mut settings.config.shader.custom_shader_cubemap_enabled,
            "Enable cubemap",
        )
        .on_hover_text("Enable iCubemap uniform for environment mapping in shaders")
        .changed()
    {
        settings.has_changes = true;
        *changes_this_frame = true;
    }
}

/// Render the global (non-per-shader) iChannel0-3 texture input controls.
pub(super) fn show_global_channel_textures(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.collapsing("Shader Channel Textures (iChannel0-3)", |ui| {
        ui.label("Provide texture inputs to shaders via iChannel0-3 (Shadertoy compatible)");
        ui.add_space(4.0);

        // iChannel0
        ui.horizontal(|ui| {
            ui.label("iChannel0:");
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel0)
                .changed()
            {
                settings.config.shader.custom_shader_channel0 =
                    if settings.temp_shader_channel0.is_empty() {
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
                settings.config.shader.custom_shader_channel0 = Some(relative_path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.temp_shader_channel0.is_empty() && ui.button("×").clicked() {
                settings.temp_shader_channel0.clear();
                settings.config.shader.custom_shader_channel0 = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // iChannel1
        ui.horizontal(|ui| {
            ui.label("iChannel1:");
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel1)
                .changed()
            {
                settings.config.shader.custom_shader_channel1 =
                    if settings.temp_shader_channel1.is_empty() {
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
                settings.config.shader.custom_shader_channel1 = Some(relative_path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.temp_shader_channel1.is_empty() && ui.button("×").clicked() {
                settings.temp_shader_channel1.clear();
                settings.config.shader.custom_shader_channel1 = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // iChannel2
        ui.horizontal(|ui| {
            ui.label("iChannel2:");
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel2)
                .changed()
            {
                settings.config.shader.custom_shader_channel2 =
                    if settings.temp_shader_channel2.is_empty() {
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
                settings.config.shader.custom_shader_channel2 = Some(relative_path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.temp_shader_channel2.is_empty() && ui.button("×").clicked() {
                settings.temp_shader_channel2.clear();
                settings.config.shader.custom_shader_channel2 = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // iChannel3
        ui.horizontal(|ui| {
            ui.label("iChannel3:");
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel3)
                .changed()
            {
                settings.config.shader.custom_shader_channel3 =
                    if settings.temp_shader_channel3.is_empty() {
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
                settings.config.shader.custom_shader_channel3 = Some(relative_path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.temp_shader_channel3.is_empty() && ui.button("×").clicked() {
                settings.temp_shader_channel3.clear();
                settings.config.shader.custom_shader_channel3 = None;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);
        ui.label("Textures are available in shaders as iChannel0-3");
        ui.label("Terminal content is available as iChannel4");
        ui.label("Use iChannelResolution[n].xy for texture dimensions");
    });
}
