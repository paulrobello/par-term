//! Unit tests for shader control parsing.
//!
//! Kept in a sibling file so `mod.rs` stays focused on the parse pipeline.

use super::*;

#[test]
fn parses_slider_attached_to_float_uniform() {
    let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
    assert_eq!(
        result.controls,
        vec![ShaderControl {
            name: "iGlow".to_string(),
            kind: ShaderControlKind::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.01,
                scale: SliderScale::Linear,
                label: None,
            },
        }]
    );
}

#[test]
fn parses_control_group_field() {
    let source = r#"
// control slider min=0 max=1 step=0.01 group="Palette"
uniform float iGlow;
// control checkbox group="Performance"
uniform bool iFast;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
    assert_eq!(
        result.groups.get("iGlow").map(String::as_str),
        Some("Palette")
    );
    assert_eq!(
        result.groups.get("iFast").map(String::as_str),
        Some("Performance")
    );
}

#[test]
fn parses_checkbox_attached_to_bool_uniform() {
    let source = r#"
// control checkbox
uniform bool iEnabled;
"#;

    let result = parse_shader_controls(source);

    assert!(result.warnings.is_empty());
    assert_eq!(
        result.controls,
        vec![ShaderControl {
            name: "iEnabled".to_string(),
            kind: ShaderControlKind::Checkbox { label: None },
        }]
    );
}

#[test]
fn parses_new_numeric_control_types() {
    let source = r#"
// control slider min=0.01 max=100 step=0.01 scale=log label="Frequency"
uniform float iFrequency;
// control int min=1 max=12 step=2 label="Octaves"
uniform int iOctaves;
// control select options="soft,hard,screen" label="Blend Mode"
uniform int iBlendMode;
// control vec2 min=-1 max=1 step=0.05 label="Flow"
uniform vec2 iFlow;
// control point label="Origin"
uniform vec2 iOrigin;
// control range min=0 max=1 step=0.01 label="Band"
uniform vec2 iBand;
// control angle unit=radians label="Rotation"
uniform float iRotation;
// control channel options="0,2,4" label="Source"
uniform int iSourceChannel;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
    assert_eq!(
        result.controls,
        vec![
            ShaderControl {
                name: "iFrequency".to_string(),
                kind: ShaderControlKind::Slider {
                    min: 0.01,
                    max: 100.0,
                    step: 0.01,
                    scale: SliderScale::Log,
                    label: Some("Frequency".to_string()),
                },
            },
            ShaderControl {
                name: "iOctaves".to_string(),
                kind: ShaderControlKind::Int {
                    min: 1,
                    max: 12,
                    step: 2,
                    label: Some("Octaves".to_string()),
                },
            },
            ShaderControl {
                name: "iBlendMode".to_string(),
                kind: ShaderControlKind::Select {
                    options: vec!["soft".to_string(), "hard".to_string(), "screen".to_string()],
                    label: Some("Blend Mode".to_string()),
                },
            },
            ShaderControl {
                name: "iFlow".to_string(),
                kind: ShaderControlKind::Vec2 {
                    min: -1.0,
                    max: 1.0,
                    step: 0.05,
                    label: Some("Flow".to_string()),
                },
            },
            ShaderControl {
                name: "iOrigin".to_string(),
                kind: ShaderControlKind::Point {
                    label: Some("Origin".to_string()),
                },
            },
            ShaderControl {
                name: "iBand".to_string(),
                kind: ShaderControlKind::Range {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    label: Some("Band".to_string()),
                },
            },
            ShaderControl {
                name: "iRotation".to_string(),
                kind: ShaderControlKind::Angle {
                    unit: AngleUnit::Radians,
                    label: Some("Rotation".to_string()),
                },
            },
            ShaderControl {
                name: "iSourceChannel".to_string(),
                kind: ShaderControlKind::Channel {
                    options: vec![0, 2, 4],
                    label: Some("Source".to_string()),
                },
            },
        ]
    );
}

#[test]
fn warns_and_skips_invalid_new_control_types() {
    let source = r#"
// control slider min=0 max=10 step=1 scale=log
uniform float iBadLog;
// control int min=10 max=1
uniform int iBadInt;
// control select options=",,"
uniform int iBadSelect;
// control vec2 min=0 max=1 step=0
uniform vec2 iBadVec2;
// control angle unit=turns
uniform float iBadAngle;
// control channel options="0,5"
uniform int iBadChannel;
// control point x=1 label="Origin"
uniform vec2 iOrigin;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(
        result.controls[0],
        ShaderControl {
            name: "iOrigin".to_string(),
            kind: ShaderControlKind::Point {
                label: Some("Origin".to_string()),
            },
        }
    );
    assert_eq!(result.warnings.len(), 7);
    assert!(result.warnings.iter().any(|w| w.message.contains("log")));
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("max >= min"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("options"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("step > 0"))
    );
    assert!(result.warnings.iter().any(|w| w.message.contains("unit")));
    assert!(result.warnings.iter().any(|w| w.message.contains("0..=4")));
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("Unknown") && w.message.contains("x"))
    );
}

#[test]
fn warns_and_skips_select_with_empty_option_segment() {
    let source = r#"
// control select options="soft,,hard"
uniform int iBlendMode;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("options"));
    assert!(result.warnings[0].message.contains("empty"));
}

#[test]
fn fallback_values_for_new_control_types() {
    let cases = vec![
        (
            ShaderControlKind::Slider {
                min: 0.01,
                max: 100.0,
                step: 0.01,
                scale: SliderScale::Log,
                label: None,
            },
            ShaderUniformValue::Float(0.01),
        ),
        (
            ShaderControlKind::Int {
                min: 2,
                max: 8,
                step: 2,
                label: None,
            },
            ShaderUniformValue::Int(2),
        ),
        (
            ShaderControlKind::Select {
                options: vec!["a".to_string()],
                label: None,
            },
            ShaderUniformValue::Int(0),
        ),
        (
            ShaderControlKind::Vec2 {
                min: -1.0,
                max: 1.0,
                step: 0.1,
                label: None,
            },
            ShaderUniformValue::Vec2([-1.0, -1.0]),
        ),
        (
            ShaderControlKind::Point { label: None },
            ShaderUniformValue::Vec2([0.5, 0.5]),
        ),
        (
            ShaderControlKind::Range {
                min: 0.2,
                max: 0.8,
                step: 0.01,
                label: None,
            },
            ShaderUniformValue::Vec2([0.2, 0.8]),
        ),
        (
            ShaderControlKind::Angle {
                unit: AngleUnit::Degrees,
                label: None,
            },
            ShaderUniformValue::Float(0.0),
        ),
        (
            ShaderControlKind::Channel {
                options: vec![2, 4],
                label: None,
            },
            ShaderUniformValue::Int(2),
        ),
    ];

    for (kind, expected) in cases {
        let control = ShaderControl {
            name: "iValue".to_string(),
            kind,
        };
        assert_eq!(fallback_value_for_control(&control), expected);
    }
}

#[test]
fn warns_and_skips_unsupported_control_type() {
    let source = r#"
// control knob min=0 max=1 step=0.1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0]
            .message
            .contains("Unsupported control type")
    );
    assert!(result.warnings[0].message.contains("knob"));
}

#[test]
fn parses_color_vec3_with_label_and_default_alpha_false() {
    let source = r#"
// control color label="Tint"
uniform vec3 iTint;
"#;

    let result = parse_shader_controls(source);

    assert!(result.warnings.is_empty());
    assert_eq!(
        result.controls,
        vec![ShaderControl {
            name: "iTint".to_string(),
            kind: ShaderControlKind::Color {
                alpha: false,
                label: Some("Tint".to_string()),
            },
        }]
    );
}

#[test]
fn parses_color_vec4_with_alpha_true_and_label() {
    let source = r#"
// control color alpha=true label="Overlay"
uniform vec4 iOverlay;
"#;

    let result = parse_shader_controls(source);

    assert!(result.warnings.is_empty());
    assert_eq!(
        result.controls,
        vec![ShaderControl {
            name: "iOverlay".to_string(),
            kind: ShaderControlKind::Color {
                alpha: true,
                label: Some("Overlay".to_string()),
            },
        }]
    );
}

#[test]
fn parses_color_vec4_alpha_false_for_rgb_picker() {
    let source = r#"
// control color alpha=false
uniform vec4 iOverlay;
"#;

    let result = parse_shader_controls(source);

    assert!(result.warnings.is_empty());
    assert_eq!(
        result.controls,
        vec![ShaderControl {
            name: "iOverlay".to_string(),
            kind: ShaderControlKind::Color {
                alpha: false,
                label: None,
            },
        }]
    );
}

#[test]
fn warns_and_skips_color_alpha_true_on_vec3() {
    let source = r#"
// control color alpha=true label="Tint"
uniform vec3 iTint;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("alpha=true"));
    assert!(result.warnings[0].message.contains("vec3"));
}

#[test]
fn warns_for_unknown_and_malformed_color_fields_but_keeps_valid_control() {
    let source = r#"
// control color label="Tint" junk=1 unexpected-token
uniform vec3 iTint;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.warnings.len(), 2);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("Unknown") && w.message.contains("junk"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("Malformed") && w.message.contains("unexpected-token"))
    );
}

#[test]
fn limits_color_controls_to_16() {
    let mut source = String::new();
    for index in 0..17 {
        source.push_str(&format!(
            "// control color label=\"Color {index}\"\nuniform vec3 iColor{index};\n"
        ));
    }

    let result = parse_shader_controls(&source);

    assert_eq!(result.controls.len(), 16);
    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0]
            .message
            .contains("Only the first 16 color controls")
    );
    assert!(result.warnings[0].message.contains("iColor16"));
}

#[test]
fn fallback_for_color_control_is_opaque_white() {
    let control = ShaderControl {
        name: "iTint".to_string(),
        kind: ShaderControlKind::Color {
            alpha: false,
            label: None,
        },
    };

    assert_eq!(
        fallback_value_for_control(&control),
        ShaderUniformValue::Color(crate::types::shader::ShaderColorValue([1.0, 1.0, 1.0, 1.0]))
    );
}

#[test]
fn warns_for_unknown_slider_field_but_keeps_valid_control() {
    let source = r#"
// control slider min=0 max=1 step=0.1 junk=1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("Unknown"));
    assert!(result.warnings[0].message.contains("junk"));
}

#[test]
fn warns_for_unknown_checkbox_field_but_keeps_valid_control() {
    let source = r#"
// control checkbox default=true
uniform bool iEnabled;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("Unknown"));
    assert!(result.warnings[0].message.contains("default"));
}

#[test]
fn warns_for_malformed_control_token_but_keeps_valid_control() {
    let source = r#"
// control slider min=0 max=1 step=0.1 unexpected-token
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("Malformed"));
    assert!(result.warnings[0].message.contains("unexpected-token"));
}

#[test]
fn warns_and_skips_slider_missing_min() {
    let source = r#"
// control slider max=1 step=0.01
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("min"));
}

#[test]
fn warns_and_skips_slider_missing_max() {
    let source = r#"
// control slider min=0 step=0.01
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("max"));
}

#[test]
fn warns_and_skips_slider_missing_step() {
    let source = r#"
// control slider min=0 max=1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("step"));
}

#[test]
fn warns_and_skips_slider_with_non_finite_min() {
    let source = r#"
// control slider min=NaN max=1 step=0.1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("finite"));
    assert!(result.warnings[0].message.contains("min"));
}

#[test]
fn warns_and_skips_slider_with_non_finite_max() {
    let source = r#"
// control slider min=0 max=inf step=0.1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("finite"));
    assert!(result.warnings[0].message.contains("max"));
}

#[test]
fn warns_and_skips_slider_with_non_finite_step() {
    let source = r#"
// control slider min=0 max=1 step=-inf
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("finite"));
    assert!(result.warnings[0].message.contains("step"));
}

#[test]
fn warns_and_skips_slider_with_max_less_than_min() {
    let source = r#"
// control slider min=2 max=1 step=0.1
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("max >= min"));
}

#[test]
fn warns_and_skips_slider_with_non_positive_step() {
    let source = r#"
// control slider min=0 max=1 step=0
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("step > 0"));
}

#[test]
fn warns_and_skips_slider_on_bool_uniform() {
    let source = r#"
// control slider min=0 max=1 step=0.1
uniform bool iGlow;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("uniform float"));
}

#[test]
fn warns_and_skips_duplicate_uniform_control() {
    let source = r#"
// control slider min=0 max=1 step=0.1
uniform float iGlow;
// control slider min=0 max=2 step=0.2
uniform float iGlow;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("Duplicate"));
}

#[test]
fn warns_and_skips_control_not_followed_by_uniform() {
    let source = r#"
// control checkbox
vec3 not_a_uniform;
"#;

    let result = parse_shader_controls(source);

    assert!(result.controls.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("uniform"));
}

/// Every control kind, parsed from one source.
///
/// Guards the dispatch table against a kind silently dropping out of it: the
/// per-kind parsers live in four separate modules, so a missing match arm
/// would otherwise only show up as one absent widget in the settings UI.
#[test]
fn parses_every_control_kind() {
    let source = r#"
// control slider min=0 max=1 step=0.1
uniform float aSlider;
// control checkbox
uniform bool aCheckbox;
// control color
uniform vec4 aColor;
// control int min=0 max=9
uniform int anInt;
// control select options="a,b"
uniform int aSelect;
// control vec2 min=0 max=1 step=0.1
uniform vec2 aVec2;
// control point
uniform vec2 aPoint;
// control range min=0 max=1 step=0.1
uniform vec2 aRange;
// control angle
uniform float anAngle;
// control channel
uniform int aChannel;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
    let kinds: Vec<&ShaderControlKind> = result.controls.iter().map(|c| &c.kind).collect();
    assert!(matches!(kinds[0], ShaderControlKind::Slider { .. }));
    assert!(matches!(kinds[1], ShaderControlKind::Checkbox { .. }));
    assert!(matches!(
        kinds[2],
        ShaderControlKind::Color { alpha: true, .. }
    ));
    assert!(matches!(kinds[3], ShaderControlKind::Int { .. }));
    assert!(matches!(kinds[4], ShaderControlKind::Select { .. }));
    assert!(matches!(kinds[5], ShaderControlKind::Vec2 { .. }));
    assert!(matches!(kinds[6], ShaderControlKind::Point { .. }));
    assert!(matches!(kinds[7], ShaderControlKind::Range { .. }));
    assert!(matches!(kinds[8], ShaderControlKind::Angle { .. }));
    assert!(matches!(kinds[9], ShaderControlKind::Channel { .. }));
    assert_eq!(kinds.len(), 10);
}

/// `angle` spends a float slot, so sliders and angles share one 16-control
/// budget rather than getting 16 each.
#[test]
fn slider_and_angle_share_the_float_capacity_budget() {
    let mut source = String::new();
    for index in 0..10 {
        source.push_str(&format!(
            "// control slider min=0 max=1 step=0.1\nuniform float s{index};\n"
        ));
    }
    for index in 0..8 {
        source.push_str(&format!("// control angle\nuniform float a{index};\n"));
    }

    let result = parse_shader_controls(&source);

    // 10 sliders + 6 angles fill the budget; the last two angles are dropped.
    assert_eq!(result.controls.len(), 16);
    assert_eq!(result.warnings.len(), 2);
    for warning in &result.warnings {
        assert!(
            warning.message.contains("Only the first 16 float controls"),
            "unexpected warning: {}",
            warning.message
        );
    }
}

/// `select` and `channel` spend int slots alongside `int`.
#[test]
fn int_select_and_channel_share_the_int_capacity_budget() {
    let mut source = String::new();
    for index in 0..8 {
        source.push_str(&format!(
            "// control int min=0 max=1\nuniform int i{index};\n"
        ));
    }
    for index in 0..8 {
        source.push_str(&format!(
            "// control select options=\"a,b\"\nuniform int e{index};\n"
        ));
    }
    source.push_str("// control channel\nuniform int overflow;\n");

    let result = parse_shader_controls(&source);

    assert_eq!(result.controls.len(), 16);
    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0]
            .message
            .contains("Only the first 16 int controls are active")
    );
}

/// `point` and `range` spend vec2 slots alongside `vec2`.
#[test]
fn vec2_point_and_range_share_the_vec2_capacity_budget() {
    let mut source = String::new();
    for index in 0..8 {
        source.push_str(&format!(
            "// control vec2 min=0 max=1 step=0.1\nuniform vec2 v{index};\n"
        ));
    }
    for index in 0..8 {
        source.push_str(&format!("// control point\nuniform vec2 p{index};\n"));
    }
    source.push_str("// control range min=0 max=1 step=0.1\nuniform vec2 overflow;\n");

    let result = parse_shader_controls(&source);

    assert_eq!(result.controls.len(), 16);
    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0]
            .message
            .contains("Only the first 16 vec2 controls are active")
    );
}

/// An unquoted label warns but must NOT reject the control.
///
/// Easy to invert when the per-kind branches become functions returning
/// `Option`: a warning path that returns `None` would silently delete the
/// control instead of just dropping its label.
#[test]
fn unquoted_label_warns_but_keeps_the_control_for_every_kind() {
    for (directive, uniform) in [
        ("slider min=0 max=1 step=0.1", "uniform float u;"),
        ("checkbox", "uniform bool u;"),
        ("color", "uniform vec4 u;"),
        ("int min=0 max=1", "uniform int u;"),
        ("select options=\"a,b\"", "uniform int u;"),
        ("vec2 min=0 max=1 step=0.1", "uniform vec2 u;"),
        ("point", "uniform vec2 u;"),
        ("range min=0 max=1 step=0.1", "uniform vec2 u;"),
        ("angle", "uniform float u;"),
        ("channel", "uniform int u;"),
    ] {
        let source = format!("// control {directive} label=Unquoted\n{uniform}\n");
        let result = parse_shader_controls(&source);

        assert_eq!(
            result.controls.len(),
            1,
            "`{directive}` dropped the control instead of just the label"
        );
        assert_eq!(result.warnings.len(), 1, "for `{directive}`");
        assert!(
            result.warnings[0].message.contains("label must be quoted"),
            "for `{directive}`: {}",
            result.warnings[0].message
        );
    }
}

/// Groups are assigned after the capacity check, so a control that loses its
/// capacity slot must not leave a stray entry in the group map.
#[test]
fn over_capacity_control_does_not_claim_a_group() {
    let mut source = String::new();
    for index in 0..16 {
        source.push_str(&format!("// control checkbox\nuniform bool b{index};\n"));
    }
    source.push_str("// control checkbox group=\"Dropped\"\nuniform bool overflow;\n");

    let result = parse_shader_controls(&source);

    assert_eq!(result.controls.len(), 16);
    assert!(
        result.groups.is_empty(),
        "over-capacity control leaked a group entry: {:?}",
        result.groups
    );
}
