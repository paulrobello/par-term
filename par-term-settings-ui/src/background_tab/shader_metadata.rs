//! Shader metadata save/build logic.
//!
//! Serialises the current effective settings back into the shader file's
//! `@metadata` comment block.

use crate::SettingsUI;
use par_term_config::{ShaderConfig, ShaderMetadata};

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
        .unwrap_or(settings.config.shader.custom_shader_animation_speed);
    if (effective_speed - 1.0).abs() > 0.001 {
        new_defaults.animation_speed = Some(effective_speed);
    }

    // Brightness
    let effective_brightness = current_override
        .and_then(|o| o.brightness)
        .or_else(|| meta_defaults.and_then(|m| m.brightness))
        .unwrap_or(settings.config.shader.custom_shader_brightness);
    if (effective_brightness - 1.0).abs() > 0.001 {
        new_defaults.brightness = Some(effective_brightness);
    }

    // Text opacity
    let effective_text_opacity = current_override
        .and_then(|o| o.text_opacity)
        .or_else(|| meta_defaults.and_then(|m| m.text_opacity))
        .unwrap_or(settings.config.shader.custom_shader_text_opacity);
    if (effective_text_opacity - 1.0).abs() > 0.001 {
        new_defaults.text_opacity = Some(effective_text_opacity);
    }

    // Full content mode
    let effective_full_content = current_override
        .and_then(|o| o.full_content)
        .or_else(|| meta_defaults.and_then(|m| m.full_content))
        .unwrap_or(settings.config.shader.custom_shader_full_content);
    if effective_full_content {
        new_defaults.full_content = Some(true);
    }

    // Channel textures - only save if set
    let effective_channel0 = current_override
        .and_then(|o| o.channel0.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel0.clone()))
        .or_else(|| settings.config.shader.custom_shader_channel0.clone());
    if effective_channel0.is_some() {
        new_defaults.channel0 = effective_channel0;
    }

    let effective_channel1 = current_override
        .and_then(|o| o.channel1.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel1.clone()))
        .or_else(|| settings.config.shader.custom_shader_channel1.clone());
    if effective_channel1.is_some() {
        new_defaults.channel1 = effective_channel1;
    }

    let effective_channel2 = current_override
        .and_then(|o| o.channel2.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel2.clone()))
        .or_else(|| settings.config.shader.custom_shader_channel2.clone());
    if effective_channel2.is_some() {
        new_defaults.channel2 = effective_channel2;
    }

    let effective_channel3 = current_override
        .and_then(|o| o.channel3.clone())
        .or_else(|| meta_defaults.and_then(|m| m.channel3.clone()))
        .or_else(|| settings.config.shader.custom_shader_channel3.clone());
    if effective_channel3.is_some() {
        new_defaults.channel3 = effective_channel3;
    }

    // Cubemap
    let effective_cubemap = current_override
        .and_then(|o| o.cubemap.clone())
        .or_else(|| meta_defaults.and_then(|m| m.cubemap.clone()))
        .or_else(|| settings.config.shader.custom_shader_cubemap.clone());
    if effective_cubemap.is_some() {
        new_defaults.cubemap = effective_cubemap;
    }

    // Cubemap enabled - only save if false (true is default)
    let effective_cubemap_enabled = current_override
        .and_then(|o| o.cubemap_enabled)
        .or_else(|| meta_defaults.and_then(|m| m.cubemap_enabled))
        .unwrap_or(settings.config.shader.custom_shader_cubemap_enabled);
    if !effective_cubemap_enabled {
        new_defaults.cubemap_enabled = Some(false);
    }

    // Use background as channel0 - only save if true (false is default)
    let effective_use_background = current_override
        .and_then(|o| o.use_background_as_channel0)
        .or_else(|| meta_defaults.and_then(|m| m.use_background_as_channel0))
        .unwrap_or(
            settings
                .config
                .shader
                .custom_shader_use_background_as_channel0,
        );
    if effective_use_background {
        new_defaults.use_background_as_channel0 = Some(true);
    }

    metadata.defaults = new_defaults;
    metadata
}
