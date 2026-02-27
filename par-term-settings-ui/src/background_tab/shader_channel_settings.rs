use crate::SettingsUI;
use par_term_config::{ShaderConfig, ShaderMetadata};

use super::shader_settings::{
    find_cubemap_prefix, make_path_relative_to_shaders, show_reset_button,
};

/// Save current effective settings to the shader file's metadata block.
pub(super) fn save_settings_to_shader_metadata(
    settings: &mut SettingsUI,
    shader_name: &str,
    existing_metadata: &Option<par_term_config::ShaderMetadata>,
) {
    // Get the shader file path
    let shader_path = par_term_config::Config::shader_path(shader_name);

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
    match par_term_config::update_shader_metadata_file(&shader_path, &new_metadata) {
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
pub(super) fn show_per_shader_channel_settings(
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
                settings.config.custom_shader_channel0 = if settings.temp_shader_channel0.is_empty()
                {
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
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel1)
                .changed()
            {
                settings.config.custom_shader_channel1 = if settings.temp_shader_channel1.is_empty()
                {
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
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel2)
                .changed()
            {
                settings.config.custom_shader_channel2 = if settings.temp_shader_channel2.is_empty()
                {
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
            if ui
                .text_edit_singleline(&mut settings.temp_shader_channel3)
                .changed()
            {
                settings.config.custom_shader_channel3 = if settings.temp_shader_channel3.is_empty()
                {
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
}
