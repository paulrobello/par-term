//! Tests for shader settings uniform helpers and cache management.
//!
//! Kept in a separate file so `mod.rs` stays focused on UI rendering.

use super::*;
use super::uniform_helpers::*;

#[test]
fn shader_uniform_override_set_shader_uniform_override_creates_per_shader_entry() {
    let mut settings = SettingsUI::new_for_tests(par_term_config::Config::default());

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
    let mut settings = SettingsUI::new_for_tests(par_term_config::Config::default());
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
    let mut settings = SettingsUI::new_for_tests(par_term_config::Config::default());
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
            scale: par_term_config::SliderScale::Linear,
            label: None,
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
            scale: par_term_config::SliderScale::Linear,
            label: None,
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
        par_term_config::ShaderUniformValue::Float(0.1)
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

fn int_control(name: &str, min: i32, max: i32, step: i32) -> par_term_config::ShaderControl {
    par_term_config::ShaderControl {
        name: name.to_string(),
        kind: par_term_config::ShaderControlKind::Int {
            min,
            max,
            step,
            label: None,
        },
    }
}

fn checkbox_control(name: &str) -> par_term_config::ShaderControl {
    par_term_config::ShaderControl {
        name: name.to_string(),
        kind: par_term_config::ShaderControlKind::Checkbox { label: None },
    }
}

fn vec2_components(value: par_term_config::ShaderUniformValue) -> [f32; 2] {
    let par_term_config::ShaderUniformValue::Vec2(actual) = value else {
        panic!("expected vec2 uniform value, got {value:?}");
    };
    actual
}

#[test]
fn shader_uniform_override_normalizes_slider_int_and_checkbox_values() {
    let slider_control = par_term_config::ShaderControl {
        name: "iGlow".to_string(),
        kind: par_term_config::ShaderControlKind::Slider {
            min: 0.25,
            max: 2.0,
            step: 0.25,
            scale: par_term_config::SliderScale::Log,
            label: Some("Glow".to_string()),
        },
    };
    assert_eq!(
        normalize_uniform_value_for_control(
            &slider_control,
            &par_term_config::ShaderUniformValue::Int(3),
        ),
        Some(par_term_config::ShaderUniformValue::Float(2.0))
    );

    let checkbox_control = checkbox_control("iEnabled");
    assert_eq!(
        normalize_uniform_value_for_control(
            &checkbox_control,
            &par_term_config::ShaderUniformValue::Bool(true),
        ),
        Some(par_term_config::ShaderUniformValue::Bool(true))
    );
}

#[test]
fn shader_uniform_override_normalizes_int_select_channel_and_angle_values() {
    let int_control = int_control("iCount", -10, 10, 3);
    assert_eq!(
        normalize_uniform_value_for_control(
            &int_control,
            &par_term_config::ShaderUniformValue::Float(9.0),
        ),
        Some(par_term_config::ShaderUniformValue::Int(8))
    );

    let mut override_config = par_term_config::ShaderConfig::default();
    override_config.uniforms.insert(
        "iCount".to_string(),
        par_term_config::ShaderUniformValue::Bool(true),
    );
    let mut metadata = par_term_config::ShaderMetadata::default();
    metadata.defaults.uniforms.insert(
        "iCount".to_string(),
        par_term_config::ShaderUniformValue::Int(8),
    );
    assert_eq!(
        normalized_effective_uniform_value(
            &int_control,
            Some(&override_config),
            Some(&metadata)
        ),
        par_term_config::ShaderUniformValue::Int(-10)
    );

    let select_control = par_term_config::ShaderControl {
        name: "iMode".to_string(),
        kind: par_term_config::ShaderControlKind::Select {
            options: vec!["Off".to_string(), "Low".to_string(), "High".to_string()],
            label: None,
        },
    };
    assert_eq!(
        normalize_uniform_value_for_control(
            &select_control,
            &par_term_config::ShaderUniformValue::Float(9.0),
        ),
        Some(par_term_config::ShaderUniformValue::Int(2))
    );

    let channel_control = par_term_config::ShaderControl {
        name: "iSource".to_string(),
        kind: par_term_config::ShaderControlKind::Channel {
            options: vec![1, 3, 5],
            label: None,
        },
    };
    assert_eq!(
        normalize_uniform_value_for_control(
            &channel_control,
            &par_term_config::ShaderUniformValue::Int(3),
        ),
        Some(par_term_config::ShaderUniformValue::Int(3))
    );
    assert_eq!(
        normalize_uniform_value_for_control(
            &channel_control,
            &par_term_config::ShaderUniformValue::Float(7.0),
        ),
        Some(par_term_config::ShaderUniformValue::Int(1))
    );

    let angle_control = par_term_config::ShaderControl {
        name: "iRotation".to_string(),
        kind: par_term_config::ShaderControlKind::Angle {
            unit: par_term_config::AngleUnit::Degrees,
            label: None,
        },
    };
    assert_eq!(
        normalize_uniform_value_for_control(
            &angle_control,
            &par_term_config::ShaderUniformValue::Int(45),
        ),
        Some(par_term_config::ShaderUniformValue::Float(45.0))
    );
}

#[test]
fn shader_uniform_override_normalizes_vec2_point_and_range_values() {
    let vec2_control = par_term_config::ShaderControl {
        name: "iOffset".to_string(),
        kind: par_term_config::ShaderControlKind::Vec2 {
            min: -1.0,
            max: 1.0,
            step: 0.1,
            label: None,
        },
    };
    assert_eq!(
        vec2_components(
            normalize_uniform_value_for_control(
                &vec2_control,
                &par_term_config::ShaderUniformValue::Vec2([-2.0, 2.0]),
            )
            .expect("vec2 should normalize"),
        ),
        [-1.0, 1.0]
    );

    let point_control = par_term_config::ShaderControl {
        name: "iCenter".to_string(),
        kind: par_term_config::ShaderControlKind::Point { label: None },
    };
    assert_eq!(
        vec2_components(
            normalize_uniform_value_for_control(
                &point_control,
                &par_term_config::ShaderUniformValue::Vec2([-0.25, 1.25]),
            )
            .expect("point should normalize"),
        ),
        [0.0, 1.0]
    );

    let range_control = par_term_config::ShaderControl {
        name: "iBand".to_string(),
        kind: par_term_config::ShaderControlKind::Range {
            min: 0.0,
            max: 5.0,
            step: 0.25,
            label: None,
        },
    };
    assert_eq!(
        vec2_components(
            normalize_uniform_value_for_control(
                &range_control,
                &par_term_config::ShaderUniformValue::Vec2([6.0, -1.0]),
            )
            .expect("range should normalize"),
        ),
        [0.0, 5.0]
    );
}

#[test]
fn shader_uniform_override_snap_i32_to_step_uses_wide_arithmetic_for_extremes() {
    assert_eq!(
        snap_i32_to_step(i32::MAX, i32::MIN, i32::MAX, i32::MAX),
        i32::MAX - 1
    );
    assert_eq!(snap_i32_to_step(i32::MIN, i32::MIN, i32::MAX, 2), i32::MIN);
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
