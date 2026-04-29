use crate::SettingsUI;
use crate::section::{collapsing_section, collapsing_section_with_state};
use std::collections::{HashMap, HashSet};
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
            .unwrap_or(settings.config.shader.custom_shader_animation_speed);
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
            .unwrap_or(settings.config.shader.custom_shader_brightness);
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
            .unwrap_or(settings.config.shader.custom_shader_text_opacity);
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
            .unwrap_or(settings.config.shader.custom_shader_full_content);
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
            .unwrap_or(
                settings
                    .config
                    .shader
                    .custom_shader_use_background_as_channel0,
            );
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

    show_shader_uniform_controls(ui, settings, shader_name, metadata, changes_this_frame);

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

fn show_shader_uniform_controls(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    shader_name: &str,
    metadata: &Option<par_term_config::ShaderMetadata>,
    changes_this_frame: &mut bool,
) {
    let Some(parsed) = cached_shader_controls_for_settings(settings, shader_name) else {
        return;
    };

    if parsed.controls.is_empty() && parsed.warnings.is_empty() {
        return;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label("Shader Controls");

    for warning in parsed.warnings {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 80),
            format!("Line {}: {}", warning.line, warning.message),
        );
    }

    let current_override = settings.config.shader_configs.get(shader_name).cloned();

    for control in parsed.controls {
        let has_uniform_override = current_override
            .as_ref()
            .is_some_and(|config| config.uniforms.contains_key(&control.name));
        let value = normalized_effective_uniform_value(
            &control,
            current_override.as_ref(),
            metadata.as_ref(),
        );

        ui.horizontal(|ui| match &control.kind {
            par_term_config::ShaderControlKind::Slider { min, max, step } => {
                let mut slider_value = match value {
                    par_term_config::ShaderUniformValue::Float(value) => value.clamp(*min, *max),
                    _ => *min,
                };
                let response = ui.add(
                    egui::Slider::new(&mut slider_value, *min..=*max)
                        .step_by(*step as f64)
                        .text(&control.name),
                );
                if response.changed() {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Float(slider_value),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Checkbox => {
                let mut checked = matches!(value, par_term_config::ShaderUniformValue::Bool(true));
                if ui.checkbox(&mut checked, &control.name).changed() {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Bool(checked),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Color { alpha, label } => {
                let color_label = label.as_deref().unwrap_or(&control.name);
                let par_term_config::ShaderUniformValue::Color(color) = value else {
                    unreachable!("color controls normalize to color uniform values");
                };
                ui.label(color_label);

                let response = if *alpha {
                    let mut srgba = shader_color_value_to_color32(color, true);
                    let response = egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut srgba,
                        egui::color_picker::Alpha::OnlyBlend,
                    );
                    if response.changed() {
                        set_shader_uniform_override(
                            settings,
                            shader_name,
                            &control.name,
                            par_term_config::ShaderUniformValue::Color(
                                color32_to_shader_color_value(srgba, true),
                            ),
                        );
                        *changes_this_frame = true;
                    }
                    response
                } else {
                    let mut srgb = shader_color_value_to_color32(color, false);
                    let response = egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut srgb,
                        egui::color_picker::Alpha::Opaque,
                    );
                    if response.changed() {
                        set_shader_uniform_override(
                            settings,
                            shader_name,
                            &control.name,
                            par_term_config::ShaderUniformValue::Color(
                                color32_to_shader_color_value(srgb, false),
                            ),
                        );
                        *changes_this_frame = true;
                    }
                    response
                };

                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }

                response.on_hover_text("Pick shader uniform color");
            }
        });
    }
}

fn set_shader_uniform_override(
    settings: &mut SettingsUI,
    shader_name: &str,
    uniform_name: &str,
    value: par_term_config::ShaderUniformValue,
) {
    let override_entry = settings.config.get_or_create_shader_override(shader_name);
    override_entry
        .uniforms
        .insert(uniform_name.to_string(), value);
    settings.has_changes = true;
}

fn clear_shader_uniform_override(settings: &mut SettingsUI, shader_name: &str, uniform_name: &str) {
    let should_prune =
        if let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name) {
            override_entry.uniforms.remove(uniform_name);
            *override_entry == par_term_config::ShaderConfig::default()
        } else {
            false
        };

    if should_prune {
        settings.config.shader_configs.remove(shader_name);
    }

    settings.has_changes = true;
}

pub(super) fn cached_shader_controls(
    cache: &mut HashMap<String, par_term_config::ShaderControlParseResult>,
    shader_name: &str,
    load_source: impl FnOnce() -> std::io::Result<String>,
) -> Option<par_term_config::ShaderControlParseResult> {
    if let Some(cached) = cache.get(shader_name) {
        return Some(cached.clone());
    }

    let source = load_source().ok()?;
    let parsed = par_term_config::parse_shader_controls(&source);
    cache.insert(shader_name.to_string(), parsed.clone());
    Some(parsed)
}

pub(super) fn invalidate_cached_shader_controls(
    cache: &mut HashMap<String, par_term_config::ShaderControlParseResult>,
    shader_name: &str,
) {
    cache.remove(shader_name);
}

pub(super) fn cached_shader_controls_for_settings(
    settings: &mut SettingsUI,
    shader_name: &str,
) -> Option<par_term_config::ShaderControlParseResult> {
    let shader_path = par_term_config::Config::shader_path(shader_name);
    cached_shader_controls(&mut settings.shader_controls_cache, shader_name, || {
        std::fs::read_to_string(&shader_path)
    })
}

pub(super) fn normalized_effective_uniform_value(
    control: &par_term_config::ShaderControl,
    current_override: Option<&par_term_config::ShaderConfig>,
    metadata: Option<&par_term_config::ShaderMetadata>,
) -> par_term_config::ShaderUniformValue {
    current_override
        .and_then(|config| config.uniforms.get(&control.name))
        .and_then(|value| normalize_uniform_value_for_control(control, value))
        .or_else(|| {
            metadata
                .and_then(|meta| meta.defaults.uniforms.get(&control.name))
                .and_then(|value| normalize_uniform_value_for_control(control, value))
        })
        .unwrap_or_else(|| par_term_config::fallback_value_for_control(control))
}

fn normalize_uniform_value_for_control(
    control: &par_term_config::ShaderControl,
    value: &par_term_config::ShaderUniformValue,
) -> Option<par_term_config::ShaderUniformValue> {
    match (&control.kind, value) {
        (
            par_term_config::ShaderControlKind::Slider { min, max, .. },
            par_term_config::ShaderUniformValue::Float(value),
        ) => Some(par_term_config::ShaderUniformValue::Float(
            value.clamp(*min, *max),
        )),
        (
            par_term_config::ShaderControlKind::Checkbox,
            par_term_config::ShaderUniformValue::Bool(value),
        ) => Some(par_term_config::ShaderUniformValue::Bool(*value)),
        (
            par_term_config::ShaderControlKind::Color { alpha, .. },
            par_term_config::ShaderUniformValue::Color(value),
        ) => Some(par_term_config::ShaderUniformValue::Color(
            normalized_shader_color_value(value.0, *alpha),
        )),
        _ => None,
    }
}

fn normalized_shader_color_value(
    mut rgba: [f32; 4],
    preserve_alpha: bool,
) -> par_term_config::ShaderColorValue {
    for component in &mut rgba {
        *component = if component.is_finite() {
            component.clamp(0.0, 1.0)
        } else {
            1.0
        };
    }

    if !preserve_alpha {
        rgba[3] = 1.0;
    }

    par_term_config::ShaderColorValue(rgba)
}

fn normalized_color_component_to_u8(component: f32) -> u8 {
    let component = if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        1.0
    };
    (component * 255.0).round() as u8
}

fn shader_color_value_to_color32(
    color: par_term_config::ShaderColorValue,
    preserve_alpha: bool,
) -> egui::Color32 {
    let color = normalized_shader_color_value(color.0, preserve_alpha);
    egui::Color32::from_rgba_unmultiplied(
        normalized_color_component_to_u8(color.0[0]),
        normalized_color_component_to_u8(color.0[1]),
        normalized_color_component_to_u8(color.0[2]),
        normalized_color_component_to_u8(color.0[3]),
    )
}

fn color32_to_shader_color_value(
    color: egui::Color32,
    preserve_alpha: bool,
) -> par_term_config::ShaderColorValue {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    normalized_shader_color_value(
        [
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        ],
        preserve_alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_uniform_override_set_shader_uniform_override_creates_per_shader_entry() {
        let mut settings = SettingsUI::new(par_term_config::Config::default());

        set_shader_uniform_override(
            &mut settings,
            "controlled.glsl",
            "iGlow",
            par_term_config::ShaderUniformValue::Float(0.75),
        );

        assert_eq!(
            settings
                .config
                .shader_configs
                .get("controlled.glsl")
                .and_then(|config| config.uniforms.get("iGlow")),
            Some(&par_term_config::ShaderUniformValue::Float(0.75))
        );
        assert!(settings.has_changes);
    }

    #[test]
    fn shader_uniform_override_clear_shader_uniform_override_removes_only_uniform_value() {
        let mut settings = SettingsUI::new(par_term_config::Config::default());
        settings
            .config
            .get_or_create_shader_override("controlled.glsl")
            .brightness = Some(0.5);
        set_shader_uniform_override(
            &mut settings,
            "controlled.glsl",
            "iGlow",
            par_term_config::ShaderUniformValue::Float(0.75),
        );

        settings.has_changes = false;
        clear_shader_uniform_override(&mut settings, "controlled.glsl", "iGlow");

        let override_config = settings
            .config
            .shader_configs
            .get("controlled.glsl")
            .expect("non-uniform override should keep shader override entry");
        assert_eq!(override_config.brightness, Some(0.5));
        assert!(!override_config.uniforms.contains_key("iGlow"));
        assert!(settings.has_changes);
    }

    #[test]
    fn shader_uniform_override_clear_shader_uniform_override_removes_empty_shader_entry() {
        let mut settings = SettingsUI::new(par_term_config::Config::default());
        set_shader_uniform_override(
            &mut settings,
            "controlled.glsl",
            "iGlow",
            par_term_config::ShaderUniformValue::Float(0.75),
        );

        settings.has_changes = false;
        clear_shader_uniform_override(&mut settings, "controlled.glsl", "iGlow");

        assert!(
            !settings
                .config
                .shader_configs
                .contains_key("controlled.glsl")
        );
        assert!(settings.has_changes);
    }

    #[test]
    fn shader_uniform_override_color32_helpers_roundtrip_srgb_u8_color() {
        let shader_color =
            par_term_config::ShaderColorValue([1.0, 136.0 / 255.0, 0.0, 204.0 / 255.0]);

        let color32 = shader_color_value_to_color32(shader_color, true);
        assert_eq!(
            color32,
            egui::Color32::from_rgba_unmultiplied(0xff, 0x88, 0x00, 0xcc)
        );

        assert_eq!(color32_to_shader_color_value(color32, true), shader_color);
    }

    #[test]
    fn shader_uniform_override_effective_uniform_value_prefers_override_then_metadata_then_fallback()
     {
        let control = par_term_config::ShaderControl {
            name: "iGlow".to_string(),
            kind: par_term_config::ShaderControlKind::Slider {
                min: 0.1,
                max: 1.0,
                step: 0.05,
            },
        };
        let mut override_config = par_term_config::ShaderConfig::default();
        override_config.uniforms.insert(
            "iGlow".to_string(),
            par_term_config::ShaderUniformValue::Float(0.75),
        );
        let mut metadata = par_term_config::ShaderMetadata::default();
        metadata.defaults.uniforms.insert(
            "iGlow".to_string(),
            par_term_config::ShaderUniformValue::Float(0.4),
        );

        assert_eq!(
            normalized_effective_uniform_value(&control, Some(&override_config), Some(&metadata)),
            par_term_config::ShaderUniformValue::Float(0.75)
        );
        assert_eq!(
            normalized_effective_uniform_value(&control, None, Some(&metadata)),
            par_term_config::ShaderUniformValue::Float(0.4)
        );
        assert_eq!(
            normalized_effective_uniform_value(&control, None, None),
            par_term_config::ShaderUniformValue::Float(0.1)
        );
    }

    #[test]
    fn shader_uniform_override_normalized_value_clamps_slider_and_falls_back_on_wrong_type() {
        let control = par_term_config::ShaderControl {
            name: "iGlow".to_string(),
            kind: par_term_config::ShaderControlKind::Slider {
                min: 0.1,
                max: 1.0,
                step: 0.05,
            },
        };
        let mut override_config = par_term_config::ShaderConfig::default();
        override_config.uniforms.insert(
            "iGlow".to_string(),
            par_term_config::ShaderUniformValue::Bool(true),
        );
        let mut metadata = par_term_config::ShaderMetadata::default();
        metadata.defaults.uniforms.insert(
            "iGlow".to_string(),
            par_term_config::ShaderUniformValue::Float(1.5),
        );

        assert_eq!(
            normalized_effective_uniform_value(&control, Some(&override_config), Some(&metadata)),
            par_term_config::ShaderUniformValue::Float(1.0)
        );

        metadata.defaults.uniforms.insert(
            "iGlow".to_string(),
            par_term_config::ShaderUniformValue::Bool(false),
        );
        assert_eq!(
            normalized_effective_uniform_value(&control, Some(&override_config), Some(&metadata)),
            par_term_config::ShaderUniformValue::Float(0.1)
        );
    }

    fn color_control(name: &str, alpha: bool) -> par_term_config::ShaderControl {
        par_term_config::ShaderControl {
            name: name.to_string(),
            kind: par_term_config::ShaderControlKind::Color { alpha, label: None },
        }
    }

    fn assert_color_value(value: par_term_config::ShaderUniformValue, expected: [f32; 4]) {
        let par_term_config::ShaderUniformValue::Color(actual) = value else {
            panic!("expected color uniform value, got {value:?}");
        };

        for (actual, expected) in actual.0.iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "expected {expected:?}, got {actual:?}"
            );
        }
    }

    #[test]
    fn shader_uniform_override_color_metadata_hex_default_resolves_to_normalized_value() {
        let control = color_control("iTint", true);
        let metadata: par_term_config::ShaderMetadata = serde_yaml_ng::from_str(
            r##"
defaults:
  uniforms:
    iTint: "#33669980"
"##,
        )
        .expect("metadata should parse");

        assert_color_value(
            normalized_effective_uniform_value(&control, None, Some(&metadata)),
            [0.2, 0.4, 0.6, 128.0 / 255.0],
        );
    }

    #[test]
    fn shader_uniform_override_color_metadata_array_default_resolves_to_normalized_value() {
        let control = color_control("iTint", true);
        let metadata: par_term_config::ShaderMetadata = serde_yaml_ng::from_str(
            r#"
defaults:
  uniforms:
    iTint: [1.0, 0.5, 0.0, 0.25]
"#,
        )
        .expect("metadata should parse");

        assert_color_value(
            normalized_effective_uniform_value(&control, None, Some(&metadata)),
            [1.0, 0.5, 0.0, 0.25],
        );
    }

    #[test]
    fn shader_uniform_override_color_explicit_default_resolves_to_normalized_value() {
        let control = color_control("iTint", true);
        let mut metadata = par_term_config::ShaderMetadata::default();
        metadata.defaults.uniforms.insert(
            "iTint".to_string(),
            par_term_config::ShaderUniformValue::Color(par_term_config::ShaderColorValue([
                0.1, 0.2, 0.3, 0.4,
            ])),
        );

        assert_color_value(
            normalized_effective_uniform_value(&control, None, Some(&metadata)),
            [0.1, 0.2, 0.3, 0.4],
        );
    }

    #[test]
    fn shader_uniform_override_color_wrong_type_falls_back_to_opaque_white() {
        let control = color_control("iTint", true);
        let mut metadata = par_term_config::ShaderMetadata::default();
        metadata.defaults.uniforms.insert(
            "iTint".to_string(),
            par_term_config::ShaderUniformValue::Bool(true),
        );

        assert_color_value(
            normalized_effective_uniform_value(&control, None, Some(&metadata)),
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    #[test]
    fn shader_uniform_override_color_alpha_false_forces_alpha_to_opaque() {
        let control = color_control("iTint", false);
        let mut override_config = par_term_config::ShaderConfig::default();
        override_config.uniforms.insert(
            "iTint".to_string(),
            par_term_config::ShaderUniformValue::Color(par_term_config::ShaderColorValue([
                0.1, 0.2, 0.3, 0.4,
            ])),
        );

        assert_color_value(
            normalized_effective_uniform_value(&control, Some(&override_config), None),
            [0.1, 0.2, 0.3, 1.0],
        );
    }

    #[test]
    fn shader_uniform_override_color_override_beats_metadata_default() {
        let control = color_control("iTint", true);
        let mut override_config = par_term_config::ShaderConfig::default();
        override_config.uniforms.insert(
            "iTint".to_string(),
            par_term_config::ShaderUniformValue::Color(par_term_config::ShaderColorValue([
                0.9, 0.8, 0.7, 0.6,
            ])),
        );
        let mut metadata = par_term_config::ShaderMetadata::default();
        metadata.defaults.uniforms.insert(
            "iTint".to_string(),
            par_term_config::ShaderUniformValue::Color(par_term_config::ShaderColorValue([
                0.1, 0.2, 0.3, 0.4,
            ])),
        );

        assert_color_value(
            normalized_effective_uniform_value(&control, Some(&override_config), Some(&metadata)),
            [0.9, 0.8, 0.7, 0.6],
        );
    }

    #[test]
    fn shader_uniform_override_cached_shader_controls_reuses_cached_parse_until_invalidated() {
        let mut cache = std::collections::HashMap::new();
        let load_calls = std::cell::Cell::new(0);
        let source = "// control slider min=0.0 max=1.0 step=0.1\nuniform float iGlow;";

        let first = cached_shader_controls(&mut cache, "controlled.glsl", || {
            load_calls.set(load_calls.get() + 1);
            Ok(source.to_string())
        })
        .expect("first parse should load source");
        let second = cached_shader_controls(&mut cache, "controlled.glsl", || {
            load_calls.set(load_calls.get() + 1);
            Ok(String::new())
        })
        .expect("second parse should use cache");

        assert_eq!(first, second);
        assert_eq!(load_calls.get(), 1);

        invalidate_cached_shader_controls(&mut cache, "controlled.glsl");
        let after_invalidate = cached_shader_controls(&mut cache, "controlled.glsl", || {
            load_calls.set(load_calls.get() + 1);
            Ok("".to_string())
        })
        .expect("invalidated cache should reload source");

        assert!(after_invalidate.controls.is_empty());
        assert_eq!(load_calls.get(), 2);
    }
}
