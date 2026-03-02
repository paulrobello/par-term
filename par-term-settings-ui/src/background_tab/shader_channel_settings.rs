//! Per-shader channel texture and cubemap override UI.
//!
//! Provides per-shader overrides for iChannel0-3 textures and the cubemap.
//! Metadata save/build logic lives in [`super::shader_metadata`].
//! Global channel controls live in [`super::global_channels`].

use crate::SettingsUI;
use par_term_config::ShaderConfig;

use super::shader_settings::{make_path_relative_to_shaders, show_reset_button};

// Re-export save_settings_to_shader_metadata so shader_settings.rs can use it unchanged.
pub(super) use super::shader_metadata::save_settings_to_shader_metadata;

// Re-export global channel controls so background_tab/mod.rs import is unchanged.
pub(super) use super::global_channels::{show_cubemap_controls, show_global_channel_textures};

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
                settings.config.shader.custom_shader_channel0.clone(),
            ),
            1 => (
                current_override.as_ref().and_then(|o| o.channel1.clone()),
                meta_defaults.and_then(|m| m.channel1.clone()),
                settings.config.shader.custom_shader_channel1.clone(),
            ),
            2 => (
                current_override.as_ref().and_then(|o| o.channel2.clone()),
                meta_defaults.and_then(|m| m.channel2.clone()),
                settings.config.shader.custom_shader_channel2.clone(),
            ),
            3 => (
                current_override.as_ref().and_then(|o| o.channel3.clone()),
                meta_defaults.and_then(|m| m.channel3.clone()),
                settings.config.shader.custom_shader_channel3.clone(),
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
    let cubemap_global = settings.config.shader.custom_shader_cubemap.clone();

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
        .unwrap_or(settings.config.shader.custom_shader_cubemap_enabled);
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
