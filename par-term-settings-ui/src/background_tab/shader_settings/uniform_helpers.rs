//! Value normalization, cache management, and color conversion helpers
//! for shader uniform controls.

use std::collections::HashMap;

use crate::SettingsUI;
use par_term_config;

// ---------------------------------------------------------------------------
// Uniform override set / clear
// ---------------------------------------------------------------------------

pub fn set_shader_uniform_override(
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

pub fn clear_shader_uniform_override(
    settings: &mut SettingsUI,
    shader_name: &str,
    uniform_name: &str,
) {
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

// ---------------------------------------------------------------------------
// Shader controls cache
// ---------------------------------------------------------------------------

pub fn cached_shader_controls(
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

pub fn invalidate_cached_shader_controls(
    cache: &mut HashMap<String, par_term_config::ShaderControlParseResult>,
    shader_name: &str,
) {
    cache.remove(shader_name);
}

pub fn cached_shader_controls_for_settings(
    settings: &mut SettingsUI,
    shader_name: &str,
) -> Option<par_term_config::ShaderControlParseResult> {
    let shader_path = par_term_config::Config::shader_path(shader_name);
    cached_shader_controls(&mut settings.shader_controls_cache, shader_name, || {
        std::fs::read_to_string(&shader_path)
    })
}

// ---------------------------------------------------------------------------
// Effective value resolution
// ---------------------------------------------------------------------------

pub fn normalized_effective_uniform_value(
    control: &par_term_config::ShaderControl,
    current_override: Option<&par_term_config::ShaderConfig>,
    metadata: Option<&par_term_config::ShaderMetadata>,
) -> par_term_config::ShaderUniformValue {
    let raw = current_override
        .and_then(|config| config.uniforms.get(&control.name))
        .or_else(|| metadata.and_then(|meta| meta.defaults.uniforms.get(&control.name)));

    raw.and_then(|value| normalize_uniform_value_for_control(control, value))
        .unwrap_or_else(|| par_term_config::fallback_value_for_control(control))
}

pub fn normalize_uniform_value_for_control(
    control: &par_term_config::ShaderControl,
    value: &par_term_config::ShaderUniformValue,
) -> Option<par_term_config::ShaderUniformValue> {
    match &control.kind {
        par_term_config::ShaderControlKind::Slider { min, max, .. } => float_uniform_value(value)
            .map(|value| par_term_config::ShaderUniformValue::Float(value.clamp(*min, *max))),
        par_term_config::ShaderControlKind::Checkbox { .. } => match value {
            par_term_config::ShaderUniformValue::Bool(value) => {
                Some(par_term_config::ShaderUniformValue::Bool(*value))
            }
            _ => None,
        },
        par_term_config::ShaderControlKind::Color { alpha, .. } => match value {
            par_term_config::ShaderUniformValue::Color(value) => {
                Some(par_term_config::ShaderUniformValue::Color(
                    normalized_shader_color_value(value.0, *alpha),
                ))
            }
            _ => None,
        },
        par_term_config::ShaderControlKind::Int { min, max, step, .. } => {
            integral_uniform_value(value).map(|value| {
                par_term_config::ShaderUniformValue::Int(snap_i32_to_step(value, *min, *max, *step))
            })
        }
        par_term_config::ShaderControlKind::Select { options, .. } => {
            let max = options.len().saturating_sub(1) as i32;
            integral_uniform_value(value)
                .map(|value| par_term_config::ShaderUniformValue::Int(value.clamp(0, max)))
        }
        par_term_config::ShaderControlKind::Channel { options, .. } => {
            let fallback = options.first().copied().unwrap_or(0);
            integral_uniform_value(value).map(|value| {
                par_term_config::ShaderUniformValue::Int(if options.contains(&value) {
                    value
                } else {
                    fallback
                })
            })
        }
        par_term_config::ShaderControlKind::Vec2 { min, max, .. } => match value {
            par_term_config::ShaderUniformValue::Vec2(value) => {
                Some(par_term_config::ShaderUniformValue::Vec2(
                    normalize_vec2_components(*value, *min, *max),
                ))
            }
            _ => None,
        },
        par_term_config::ShaderControlKind::Point { .. } => match value {
            par_term_config::ShaderUniformValue::Vec2(value) => {
                Some(par_term_config::ShaderUniformValue::Vec2(
                    normalize_vec2_components(*value, 0.0, 1.0),
                ))
            }
            _ => None,
        },
        par_term_config::ShaderControlKind::Range { min, max, .. } => match value {
            par_term_config::ShaderUniformValue::Vec2(value) => {
                let normalized = normalize_vec2_components(*value, *min, *max);
                Some(par_term_config::ShaderUniformValue::Vec2([
                    normalized[0].min(normalized[1]),
                    normalized[0].max(normalized[1]),
                ]))
            }
            _ => None,
        },
        par_term_config::ShaderControlKind::Angle { .. } => {
            float_uniform_value(value).map(par_term_config::ShaderUniformValue::Float)
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive value extraction
// ---------------------------------------------------------------------------

pub fn integral_uniform_value(value: &par_term_config::ShaderUniformValue) -> Option<i32> {
    match value {
        par_term_config::ShaderUniformValue::Int(value) => Some(*value),
        par_term_config::ShaderUniformValue::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && f64::from(*value) >= f64::from(i32::MIN)
                && f64::from(*value) <= f64::from(i32::MAX) =>
        {
            Some(*value as i32)
        }
        _ => None,
    }
}

pub fn float_uniform_value(value: &par_term_config::ShaderUniformValue) -> Option<f32> {
    match value {
        par_term_config::ShaderUniformValue::Float(value) if value.is_finite() => Some(*value),
        par_term_config::ShaderUniformValue::Int(value) => Some(*value as f32),
        _ => None,
    }
}

pub fn snap_i32_to_step(value: i32, min: i32, max: i32, step: i32) -> i32 {
    let clamped = value.clamp(min, max);
    let min_i64 = i64::from(min);
    let max_i64 = i64::from(max);
    let step_i64 = i64::from(step.max(1));
    let offset = i64::from(clamped) - min_i64;
    let steps_from_min = (offset + step_i64 / 2) / step_i64;
    let candidate = min_i64 + steps_from_min * step_i64;

    candidate.clamp(min_i64, max_i64) as i32
}

pub fn normalize_vec2_components(value: [f32; 2], min: f32, max: f32) -> [f32; 2] {
    value.map(|component| {
        if component.is_finite() {
            component.clamp(min, max)
        } else {
            min
        }
    })
}

// ---------------------------------------------------------------------------
// Color conversion
// ---------------------------------------------------------------------------

pub fn normalized_shader_color_value(
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

pub fn shader_color_value_to_color32(
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

pub fn color32_to_shader_color_value(
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
