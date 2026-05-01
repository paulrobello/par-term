//! Shader settings UI for the background tab.
//!
//! Organized into focused sub-modules:
//!
//!   `utils`              — Path helpers, reset button, cubemap detection
//!   `uniform_helpers`    — Value normalization, cache management, color conversion
//!   `uniform_controls`   — egui widget rendering for each `ShaderControlKind`
//!   `per_shader_settings`— Per-shader override controls, lint output, save/reset buttons

mod per_shader_settings;
mod uniform_controls;
mod uniform_helpers;
mod utils;

// Re-export utilities used by sibling modules within background_tab.
pub(super) use uniform_helpers::{
    invalidate_cached_shader_controls, normalized_effective_uniform_value,
};

// Re-export utilities used by sibling modules within background_tab.
pub(super) use utils::{find_cubemap_prefix, make_path_relative_to_shaders, show_reset_button};

use std::collections::{BTreeSet, HashSet};

use crate::SettingsUI;
use crate::section::collapsing_section_with_state;

use per_shader_settings::show_per_shader_settings;

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
    let badges = shader_safety_badges(metadata);
    if !badges.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Safety:");
            for badge in badges {
                ui.label(
                    egui::RichText::new(badge.label())
                        .small()
                        .background_color(egui::Color32::from_rgb(45, 55, 70)),
                );
            }
        });
        ui.add_space(4.0);
    }

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

fn shader_safety_badges(
    metadata: &par_term_config::ShaderMetadata,
) -> Vec<par_term_config::ShaderSafetyBadge> {
    use par_term_config::ShaderSafetyBadge;
    let mut badges: BTreeSet<&'static str> = BTreeSet::new();
    let mut result = Vec::new();

    let mut push_badge = |badge: ShaderSafetyBadge| {
        if badges.insert(badge.label()) {
            result.push(badge);
        }
    };

    for badge in &metadata.safety_badges {
        push_badge(*badge);
    }
    if metadata.defaults.full_content == Some(true) {
        push_badge(ShaderSafetyBadge::FullContent);
    }
    if metadata.defaults.channel0.is_some()
        || metadata.defaults.channel1.is_some()
        || metadata.defaults.channel2.is_some()
        || metadata.defaults.channel3.is_some()
    {
        push_badge(ShaderSafetyBadge::UsesTextures);
    }
    if metadata.defaults.cubemap.is_some() || metadata.defaults.cubemap_enabled == Some(true) {
        push_badge(ShaderSafetyBadge::UsesCubemap);
    }

    result
}

#[cfg(test)]
mod tests;
