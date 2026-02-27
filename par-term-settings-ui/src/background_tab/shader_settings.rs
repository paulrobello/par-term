use crate::SettingsUI;
use crate::section::{collapsing_section, collapsing_section_with_state};
use std::collections::HashSet;
use std::path::Path;

use super::shader_channel_settings::{
    save_settings_to_shader_metadata, show_per_shader_channel_settings,
};

/// Convert an absolute path to a path relative to the shaders directory if possible.
/// If the path is within the shaders directory, returns a relative path.
/// Otherwise, returns the original path unchanged.
/// Always uses forward slashes for cross-platform compatibility.
pub(super) fn make_path_relative_to_shaders(absolute_path: &str) -> String {
    let shaders_dir = par_term_config::Config::shaders_dir();
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

/// Show a reset button that's only visible/enabled when there's an override
pub(super) fn show_reset_button(ui: &mut egui::Ui, has_override: bool) -> bool {
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
pub(super) fn find_cubemap_prefix(folder: &std::path::Path) -> Option<std::path::PathBuf> {
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

/// Show shader metadata and per-shader settings section
pub fn show_shader_metadata_and_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
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

    collapsing_section_with_state(
        ui,
        &header_text,
        "shader_settings",
        true,
        collapsed,
        |ui, collapsed| {
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

            show_per_shader_settings(
                ui,
                settings,
                &shader_name,
                &metadata,
                changes_this_frame,
                collapsed,
            );
        },
    );
}

/// Show shader metadata info (name, author, description, version)
fn show_shader_metadata_info(ui: &mut egui::Ui, metadata: &par_term_config::ShaderMetadata) {
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
    metadata: &Option<par_term_config::ShaderMetadata>,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
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
    collapsing_section(
        ui,
        "Channel Textures",
        "per_shader_channels",
        false,
        collapsed,
        |ui| {
            show_per_shader_channel_settings(
                ui,
                settings,
                shader_name,
                meta_defaults_for_channels.as_ref(),
                changes_this_frame,
            );
        },
    );

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
