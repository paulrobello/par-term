use crate::types::shader::ShaderUniformValue;
use std::collections::{BTreeMap, HashSet};

/// Scale mode for slider controls.
///
/// `Linear` maps the slider range uniformly. `Log` applies logarithmic
/// scaling, which is useful for frequency or exponential parameters where
/// small values need fine-grained control.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SliderScale {
    /// Linear mapping from min to max.
    Linear,
    /// Logarithmic mapping (requires 0 < min < max).
    Log,
}

/// Unit for angle controls.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AngleUnit {
    /// Degrees (0-360).
    Degrees,
    /// Radians (0-2*pi).
    Radians,
}

/// Kind of UI control parsed from a `// control` comment in a shader file.
///
/// Each variant corresponds to a different widget type in the settings UI,
/// paired with the GLSL uniform type it can attach to.
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderControlKind {
    /// Float slider control attached to `uniform float`.
    Slider {
        min: f32,
        max: f32,
        step: f32,
        scale: SliderScale,
        label: Option<String>,
    },
    /// Boolean checkbox attached to `uniform bool`.
    Checkbox { label: Option<String> },
    /// Color picker attached to `uniform vec3` or `uniform vec4`.
    Color { alpha: bool, label: Option<String> },
    /// Integer slider attached to `uniform int`.
    Int {
        min: i32,
        max: i32,
        step: i32,
        label: Option<String>,
    },
    /// Dropdown selector attached to `uniform int` (selected by index).
    Select {
        options: Vec<String>,
        label: Option<String>,
    },
    /// Two-component float slider attached to `uniform vec2`.
    Vec2 {
        min: f32,
        max: f32,
        step: f32,
        label: Option<String>,
    },
    /// Normalized 2D point picker attached to `uniform vec2` (range 0-1).
    Point { label: Option<String> },
    /// Two-handle range slider attached to `uniform vec2`.
    Range {
        min: f32,
        max: f32,
        step: f32,
        label: Option<String>,
    },
    /// Angle dial attached to `uniform float`.
    Angle {
        unit: AngleUnit,
        label: Option<String>,
    },
    /// iChannel selector dropdown attached to `uniform int`.
    Channel {
        options: Vec<i32>,
        label: Option<String>,
    },
}

/// A parsed `// control` declaration bound to a shader uniform.
///
/// Maps a uniform name to its control kind (slider, checkbox, color, etc.)
/// for rendering in the settings UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControl {
    /// Name of the GLSL uniform this control attaches to.
    pub name: String,
    /// The kind of UI control (slider, checkbox, color picker, etc.).
    pub kind: ShaderControlKind,
}

/// A non-fatal warning emitted during shader control parsing.
///
/// Warnings are displayed in the shader settings UI but do not prevent
/// the shader from loading. They indicate malformed or unrecognized
/// control declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControlWarning {
    /// 1-based line number where the warning was detected.
    pub line: usize,
    /// Human-readable description of the issue.
    pub message: String,
}

/// Result of parsing all `// control` declarations from a shader source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderControlParseResult {
    /// Successfully parsed controls, ordered by appearance in the source.
    pub controls: Vec<ShaderControl>,
    /// Non-fatal warnings for malformed or unrecognized declarations.
    pub warnings: Vec<ShaderControlWarning>,
    /// Map of uniform name to group name (for grouping controls in the UI).
    pub groups: BTreeMap<String, String>,
}

const MAX_SHADER_FLOAT_CONTROLS: usize = 16;
const MAX_SHADER_BOOL_CONTROLS: usize = 16;
const MAX_SHADER_COLOR_CONTROLS: usize = 16;
const MAX_SHADER_INT_CONTROLS: usize = 16;
const MAX_SHADER_VEC2_CONTROLS: usize = 16;

mod float_kinds;
mod int_kinds;
mod parse_helpers;
mod simple_kinds;
mod vec2_kinds;

#[cfg(test)]
mod tests;

use parse_helpers::{
    ControlDirective, check_and_push_capacity_warning, parse_key_values, parse_quoted_label,
    parse_uniform_declaration, push_warning, tokenize_control_directive,
};

/// The five capacity budgets, one per uniform storage class.
///
/// Kinds share budgets rather than owning them: an `angle` spends a float
/// slot, `select` and `channel` spend int slots, and `point` and `range` spend
/// vec2 slots. Sixteen sliders therefore leave no room for a single angle
/// control.
#[derive(Default)]
struct CapacityCounters {
    float: usize,
    boolean: usize,
    color: usize,
    int: usize,
    vec2: usize,
}

impl CapacityCounters {
    /// Claim a slot for `kind`, or warn and return `false` when the budget it
    /// draws on is already full.
    fn claim(
        &mut self,
        kind: &ShaderControlKind,
        warnings: &mut Vec<ShaderControlWarning>,
        line: usize,
        uniform_name: &str,
    ) -> bool {
        let (type_label, limit, count) = match kind {
            ShaderControlKind::Slider { .. } | ShaderControlKind::Angle { .. } => {
                ("float", MAX_SHADER_FLOAT_CONTROLS, &mut self.float)
            }
            ShaderControlKind::Checkbox { .. } => {
                ("checkbox", MAX_SHADER_BOOL_CONTROLS, &mut self.boolean)
            }
            ShaderControlKind::Color { .. } => {
                ("color", MAX_SHADER_COLOR_CONTROLS, &mut self.color)
            }
            ShaderControlKind::Int { .. }
            | ShaderControlKind::Select { .. }
            | ShaderControlKind::Channel { .. } => ("int", MAX_SHADER_INT_CONTROLS, &mut self.int),
            ShaderControlKind::Vec2 { .. }
            | ShaderControlKind::Point { .. }
            | ShaderControlKind::Range { .. } => ("vec2", MAX_SHADER_VEC2_CONTROLS, &mut self.vec2),
        };

        if check_and_push_capacity_warning(warnings, line, type_label, limit, *count, uniform_name)
        {
            return false;
        }
        *count += 1;
        true
    }
}

/// Route a directive to the parser for its control type.
///
/// `None` means the directive is dropped: the caller must skip it entirely,
/// which is what keeps a rejected control from consuming a duplicate-name
/// entry, a capacity slot, or a group assignment.
fn parse_control_kind(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    match directive.control_type {
        "slider" => float_kinds::parse_slider(directive, warnings),
        "checkbox" => simple_kinds::parse_checkbox(directive, warnings),
        "color" => simple_kinds::parse_color(directive, warnings),
        "int" => int_kinds::parse_int(directive, warnings),
        "select" => int_kinds::parse_select(directive, warnings),
        "vec2" => vec2_kinds::parse_vec2(directive, warnings),
        "point" => vec2_kinds::parse_point(directive, warnings),
        "range" => vec2_kinds::parse_range(directive, warnings),
        "angle" => float_kinds::parse_angle(directive, warnings),
        "channel" => int_kinds::parse_channel(directive, warnings),
        other => {
            directive.warn(warnings, format!("Unsupported control type `{}`", other));
            None
        }
    }
}

/// Extract every `// control ...` declaration from a shader source.
///
/// A control is a comment immediately followed by the `uniform` it decorates.
/// Anything malformed produces a warning and is skipped -- parsing never
/// fails, because a bad control comment must not stop a shader from loading.
///
/// Each accepted control must clear four gates in order: its own per-kind
/// parse, a duplicate-name check, the capacity budget for its uniform type,
/// and finally group assignment. Failing any gate drops the control before it
/// reaches the later ones, so an over-capacity control never claims a group.
pub fn parse_shader_controls(source: &str) -> ShaderControlParseResult {
    let lines: Vec<&str> = source.lines().collect();
    let mut controls = Vec::new();
    let mut warnings = Vec::new();
    let mut groups = BTreeMap::new();
    let mut seen = HashSet::new();
    let mut capacity = CapacityCounters::default();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("// control ") else {
            continue;
        };

        let line_number = index + 1;
        let (tokens, tokenization_warnings) = tokenize_control_directive(rest);
        let Some(control_type) = tokens.first().map(String::as_str) else {
            push_warning(
                &mut warnings,
                line_number,
                "Control comment is missing a control type".to_string(),
            );
            continue;
        };

        let Some(next_line) = lines.get(index + 1) else {
            push_warning(
                &mut warnings,
                line_number,
                "Control comment must be immediately followed by a uniform declaration".to_string(),
            );
            continue;
        };

        let Some((uniform_type, uniform_name)) = parse_uniform_declaration(next_line) else {
            push_warning(
                &mut warnings,
                line_number,
                "Control comment must be immediately followed by a uniform declaration".to_string(),
            );
            continue;
        };

        let (key_values, mut malformed_tokens) = parse_key_values(&tokens[1..]);
        malformed_tokens.extend(tokenization_warnings);

        let directive = ControlDirective {
            line: line_number,
            control_type,
            uniform_type,
            uniform_name,
            key_values: &key_values,
            malformed_tokens: &malformed_tokens,
        };

        let Some(kind) = parse_control_kind(&directive, &mut warnings) else {
            continue;
        };

        if !seen.insert(uniform_name.to_string()) {
            push_warning(
                &mut warnings,
                line_number,
                format!("Duplicate control for uniform `{}` ignored", uniform_name),
            );
            continue;
        }

        if !capacity.claim(&kind, &mut warnings, line_number, uniform_name) {
            continue;
        }

        if let Some(group) = parse_quoted_label(
            &mut warnings,
            line_number,
            "Control group",
            uniform_name,
            key_values.get("group"),
        ) {
            groups.insert(uniform_name.to_string(), group);
        }

        controls.push(ShaderControl {
            name: uniform_name.to_string(),
            kind,
        });
    }

    ShaderControlParseResult {
        controls,
        warnings,
        groups,
    }
}

pub fn fallback_value_for_control(control: &ShaderControl) -> ShaderUniformValue {
    match &control.kind {
        ShaderControlKind::Slider { min, .. } => ShaderUniformValue::Float(*min),
        ShaderControlKind::Checkbox { .. } => ShaderUniformValue::Bool(false),
        ShaderControlKind::Color { .. } => {
            ShaderUniformValue::Color(crate::types::shader::ShaderColorValue([1.0, 1.0, 1.0, 1.0]))
        }
        ShaderControlKind::Int { min, .. } => ShaderUniformValue::Int(*min),
        ShaderControlKind::Select { .. } => ShaderUniformValue::Int(0),
        ShaderControlKind::Vec2 { min, .. } => ShaderUniformValue::Vec2([*min, *min]),
        ShaderControlKind::Point { .. } => ShaderUniformValue::Vec2([0.5, 0.5]),
        ShaderControlKind::Range { min, max, .. } => ShaderUniformValue::Vec2([*min, *max]),
        ShaderControlKind::Angle { .. } => ShaderUniformValue::Float(0.0),
        ShaderControlKind::Channel { options, .. } => {
            ShaderUniformValue::Int(options.first().copied().unwrap_or(0))
        }
    }
}
