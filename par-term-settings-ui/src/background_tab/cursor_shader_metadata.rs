use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::{CursorShaderConfig, CursorShaderMetadata};
use std::collections::HashSet;

use super::shader_settings::show_reset_button;

/// Show cursor shader metadata and per-shader settings section
pub(super) fn show_cursor_shader_metadata_and_settings(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
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

    collapsing_section(
        ui,
        &header_text,
        "cursor_shader_settings",
        true,
        collapsed,
        |ui| {
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
        },
    );
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
    let shader_path = par_term_config::Config::shader_path(shader_name);

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
    match par_term_config::update_cursor_shader_metadata_file(&shader_path, &new_metadata) {
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
