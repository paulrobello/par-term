//! Global (non-per-shader) iChannel0-3 texture inputs and cubemap controls.

use crate::SettingsUI;

use super::shader_settings::{find_cubemap_prefix, make_path_relative_to_shaders};

pub(super) fn builtin_noise_choices() -> [&'static str; 5] {
    [
        "builtin://noise/value-128",
        "builtin://noise/value-256",
        "builtin://noise/fbm-256",
        "builtin://noise/fbm-512",
        "builtin://noise/cellular-256",
    ]
}

fn set_channel_path(settings: &mut SettingsUI, channel: usize, value: String) {
    match channel {
        0 => {
            settings.temp_shader_channel0 = value.clone();
            settings.config.shader.custom_shader_channel0 = Some(value);
        }
        1 => {
            settings.temp_shader_channel1 = value.clone();
            settings.config.shader.custom_shader_channel1 = Some(value);
        }
        2 => {
            settings.temp_shader_channel2 = value.clone();
            settings.config.shader.custom_shader_channel2 = Some(value);
        }
        3 => {
            settings.temp_shader_channel3 = value.clone();
            settings.config.shader.custom_shader_channel3 = Some(value);
        }
        _ => unreachable!("only iChannel0-3 are supported"),
    }
}

fn channel_path(settings: &SettingsUI, channel: usize) -> &str {
    match channel {
        0 => &settings.temp_shader_channel0,
        1 => &settings.temp_shader_channel1,
        2 => &settings.temp_shader_channel2,
        3 => &settings.temp_shader_channel3,
        _ => unreachable!("only iChannel0-3 are supported"),
    }
}

fn builtin_noise_selection_change(current_path: &str, choice: &str) -> Option<String> {
    (current_path != choice).then(|| choice.to_string())
}

fn show_builtin_noise_combo(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    channel: usize,
    changes_this_frame: &mut bool,
) {
    egui::ComboBox::from_id_salt(format!("shader_channel{channel}_builtin_noise"))
        .selected_text("Built-in noise…")
        .show_ui(ui, |ui| {
            for choice in builtin_noise_choices() {
                let current_path = channel_path(settings, channel).to_owned();
                let is_selected = current_path == choice;
                if ui.selectable_label(is_selected, choice).clicked()
                    && let Some(next_path) = builtin_noise_selection_change(&current_path, choice)
                {
                    set_channel_path(settings, channel, next_path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }
        })
        .response
        .on_hover_text("Use deterministic built-in noise texture for this iChannel");
}

/// Render the global background-as-channel0 controls.
pub(super) fn show_background_channel0_controls(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    if ui
        .checkbox(
            &mut settings.config.shader.custom_shader_use_background_as_channel0,
            "Use background as iChannel0",
        )
        .on_hover_text(
            "When enabled, the app's background (image or solid color) is bound as iChannel0 instead of a separate texture file.\n\
            This allows shaders to incorporate the background without requiring a separate texture.",
        )
        .changed()
    {
        settings.has_changes = true;
        *changes_this_frame = true;
    }

    ui.horizontal(|ui| {
        ui.label("Background blend mode:");
        egui::ComboBox::from_id_salt("background_channel0_blend_mode")
            .selected_text(
                settings
                    .config
                    .shader
                    .custom_shader_background_channel0_blend_mode
                    .display_name(),
            )
            .show_ui(ui, |ui| {
                for mode in par_term_config::ShaderBackgroundBlendMode::ALL {
                    if ui
                        .selectable_value(
                            &mut settings
                                .config
                                .shader
                                .custom_shader_background_channel0_blend_mode,
                            mode,
                            mode.display_name(),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                }
            });
    });
    ui.label("Shaders can read this hint via iBackgroundBlendMode.");
}

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

            show_builtin_noise_combo(ui, settings, 0, changes_this_frame);

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

            show_builtin_noise_combo(ui, settings, 1, changes_this_frame);

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

            show_builtin_noise_combo(ui, settings, 2, changes_this_frame);

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

            show_builtin_noise_combo(ui, settings, 3, changes_this_frame);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_noise_choices_are_stable() {
        assert_eq!(
            builtin_noise_choices(),
            [
                "builtin://noise/value-128",
                "builtin://noise/value-256",
                "builtin://noise/fbm-256",
                "builtin://noise/fbm-512",
                "builtin://noise/cellular-256",
            ]
        );
    }

    #[test]
    fn selecting_current_builtin_noise_is_not_a_change() {
        let current = "builtin://noise/fbm-256";

        assert_eq!(builtin_noise_selection_change(current, current), None);
    }

    #[test]
    fn selecting_different_builtin_noise_returns_new_path() {
        assert_eq!(
            builtin_noise_selection_change(
                "builtin://noise/fbm-256",
                "builtin://noise/cellular-256"
            ),
            Some("builtin://noise/cellular-256".to_string())
        );
    }

    #[test]
    fn blend_mode_labels_are_user_readable() {
        assert_eq!(
            par_term_config::ShaderBackgroundBlendMode::Overlay.display_name(),
            "Overlay"
        );
    }
}
