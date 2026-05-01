//! egui widget rendering for each `ShaderControlKind` variant.

use crate::SettingsUI;
use par_term_config;

use super::uniform_helpers::{
    cached_shader_controls_for_settings, clear_shader_uniform_override,
    color32_to_shader_color_value, float_uniform_value, integral_uniform_value,
    normalize_vec2_components, normalized_effective_uniform_value, set_shader_uniform_override,
    shader_color_value_to_color32, snap_i32_to_step,
};
use super::utils::show_reset_button;

pub(super) fn show_shader_uniform_controls(
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
    let groups = parsed.groups.clone();
    let mut current_group: Option<String> = None;

    for control in parsed.controls {
        let group = groups
            .get(&control.name)
            .cloned()
            .unwrap_or_else(|| "General".to_string());
        if current_group.as_deref() != Some(group.as_str()) {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&group).strong());
            current_group = Some(group);
        }
        let has_uniform_override = current_override
            .as_ref()
            .is_some_and(|config| config.uniforms.contains_key(&control.name));
        let value = normalized_effective_uniform_value(
            &control,
            current_override.as_ref(),
            metadata.as_ref(),
        );

        ui.horizontal(|ui| match &control.kind {
            par_term_config::ShaderControlKind::Slider {
                min,
                max,
                step,
                scale,
                label,
            } => {
                let mut slider_value = float_uniform_value(&value)
                    .unwrap_or(*min)
                    .clamp(*min, *max);
                let response = ui.add(
                    egui::Slider::new(&mut slider_value, *min..=*max)
                        .step_by((*step).max(f32::EPSILON) as f64)
                        .logarithmic(matches!(scale, par_term_config::SliderScale::Log))
                        .text(label.as_deref().unwrap_or(&control.name)),
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
            par_term_config::ShaderControlKind::Checkbox { label } => {
                let mut checked = matches!(value, par_term_config::ShaderUniformValue::Bool(true));
                if ui
                    .checkbox(&mut checked, label.as_deref().unwrap_or(&control.name))
                    .changed()
                {
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
            par_term_config::ShaderControlKind::Int {
                min,
                max,
                step,
                label,
            } => {
                let mut int_value = integral_uniform_value(&value).unwrap_or(*min);
                int_value = snap_i32_to_step(int_value, *min, *max, *step);
                let response = ui.add(
                    egui::Slider::new(&mut int_value, *min..=*max)
                        .step_by(i64::from((*step).max(1)) as f64)
                        .text(label.as_deref().unwrap_or(&control.name)),
                );
                if response.changed() {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Int(snap_i32_to_step(
                            int_value, *min, *max, *step,
                        )),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Select { options, label } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let max_index = options.len().saturating_sub(1) as i32;
                let mut selected = integral_uniform_value(&value)
                    .unwrap_or(0)
                    .clamp(0, max_index);
                let before = selected;
                let selected_text = options
                    .get(selected as usize)
                    .map(String::as_str)
                    .unwrap_or("No options");
                egui::ComboBox::from_id_salt(format!("shader_uniform_select_{}", control.name))
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        for (index, option) in options.iter().enumerate() {
                            ui.selectable_value(&mut selected, index as i32, option);
                        }
                    });
                if selected != before {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Int(selected),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Vec2 {
                min,
                max,
                step,
                label,
            } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let mut components = match value {
                    par_term_config::ShaderUniformValue::Vec2(value) => {
                        normalize_vec2_components(value, *min, *max)
                    }
                    _ => [*min, *min],
                };
                let x_response = ui.label("X:");
                x_response.on_hover_text("X component");
                let x_changed = ui
                    .add(
                        egui::DragValue::new(&mut components[0])
                            .range(*min..=*max)
                            .speed((*step).max(f32::EPSILON) as f64),
                    )
                    .changed();
                ui.label("Y:");
                let y_changed = ui
                    .add(
                        egui::DragValue::new(&mut components[1])
                            .range(*min..=*max)
                            .speed((*step).max(f32::EPSILON) as f64),
                    )
                    .changed();
                if x_changed || y_changed {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Vec2(normalize_vec2_components(
                            components, *min, *max,
                        )),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Point { label } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let mut point = match value {
                    par_term_config::ShaderUniformValue::Vec2(value) => {
                        normalize_vec2_components(value, 0.0, 1.0)
                    }
                    _ => [0.5, 0.5],
                };
                ui.label("X:");
                let x_changed = ui
                    .add(egui::Slider::new(&mut point[0], 0.0..=1.0))
                    .changed();
                ui.label("Y:");
                let y_changed = ui
                    .add(egui::Slider::new(&mut point[1], 0.0..=1.0))
                    .changed();
                let center_clicked = ui.button("Center").clicked();
                if center_clicked {
                    point = [0.5, 0.5];
                }
                if x_changed || y_changed || center_clicked {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Vec2(normalize_vec2_components(
                            point, 0.0, 1.0,
                        )),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Range {
                min,
                max,
                step,
                label,
            } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let mut range = match value {
                    par_term_config::ShaderUniformValue::Vec2(value) => {
                        normalize_vec2_components(value, *min, *max)
                    }
                    _ => [*min, *max],
                };
                range = [range[0].min(range[1]), range[0].max(range[1])];
                ui.label("Low:");
                let low_changed = ui
                    .add(
                        egui::Slider::new(&mut range[0], *min..=*max)
                            .step_by((*step).max(f32::EPSILON) as f64),
                    )
                    .changed();
                ui.label("High:");
                let high_changed = ui
                    .add(
                        egui::Slider::new(&mut range[1], *min..=*max)
                            .step_by((*step).max(f32::EPSILON) as f64),
                    )
                    .changed();
                if low_changed || high_changed {
                    let normalized = normalize_vec2_components(range, *min, *max);
                    let sorted = [
                        normalized[0].min(normalized[1]),
                        normalized[0].max(normalized[1]),
                    ];
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Vec2(sorted),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Angle { unit, label } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let mut angle = float_uniform_value(&value).unwrap_or(0.0);
                let suffix = match unit {
                    par_term_config::AngleUnit::Degrees => "\u{00B0}",
                    par_term_config::AngleUnit::Radians => " rad",
                };
                if ui
                    .add(egui::DragValue::new(&mut angle).speed(1.0).suffix(suffix))
                    .changed()
                {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Float(angle),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Channel { options, label } => {
                ui.label(label.as_deref().unwrap_or(&control.name));
                let fallback = options.first().copied().unwrap_or(0);
                let mut selected = integral_uniform_value(&value).unwrap_or(fallback);
                if !options.contains(&selected) {
                    selected = fallback;
                }
                let before = selected;
                egui::ComboBox::from_id_salt(format!("shader_uniform_channel_{}", control.name))
                    .selected_text(format!("iChannel{}", selected))
                    .show_ui(ui, |ui| {
                        for option in options {
                            ui.selectable_value(
                                &mut selected,
                                *option,
                                format!("iChannel{}", option),
                            );
                        }
                    });
                if selected != before {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Int(selected),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_uniform_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
        });
    }
}
