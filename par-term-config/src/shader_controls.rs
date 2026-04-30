use crate::types::shader::ShaderUniformValue;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SliderScale {
    Linear,
    Log,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AngleUnit {
    Degrees,
    Radians,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderControlKind {
    Slider {
        min: f32,
        max: f32,
        step: f32,
        scale: SliderScale,
        label: Option<String>,
    },
    Checkbox {
        label: Option<String>,
    },
    Color {
        alpha: bool,
        label: Option<String>,
    },
    Int {
        min: i32,
        max: i32,
        step: i32,
        label: Option<String>,
    },
    Select {
        options: Vec<String>,
        label: Option<String>,
    },
    Vec2 {
        min: f32,
        max: f32,
        step: f32,
        label: Option<String>,
    },
    Point {
        label: Option<String>,
    },
    Range {
        min: f32,
        max: f32,
        step: f32,
        label: Option<String>,
    },
    Angle {
        unit: AngleUnit,
        label: Option<String>,
    },
    Channel {
        options: Vec<i32>,
        label: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControl {
    pub name: String,
    pub kind: ShaderControlKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControlWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderControlParseResult {
    pub controls: Vec<ShaderControl>,
    pub warnings: Vec<ShaderControlWarning>,
    pub groups: BTreeMap<String, String>,
}

const MAX_SHADER_FLOAT_CONTROLS: usize = 16;
const MAX_SHADER_BOOL_CONTROLS: usize = 16;
const MAX_SHADER_COLOR_CONTROLS: usize = 16;
const MAX_SHADER_INT_CONTROLS: usize = 16;
const MAX_SHADER_VEC2_CONTROLS: usize = 16;

fn parse_uniform_declaration(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("uniform ") || !trimmed.ends_with(';') {
        return None;
    }

    let without_semicolon = trimmed.trim_end_matches(';').trim();
    let mut parts = without_semicolon.split_whitespace();
    let uniform = parts.next()?;
    let ty = parts.next()?;
    let name = parts.next()?;

    if uniform != "uniform" || parts.next().is_some() {
        return None;
    }

    Some((ty, name))
}

fn tokenize_control_directive(rest: &str) -> (Vec<String>, Vec<String>) {
    let mut tokens = Vec::new();
    let mut malformed_tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in rest.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
                current.push(character);
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }

    if !current.is_empty() {
        if in_quotes {
            malformed_tokens.push(current.clone());
        }
        tokens.push(current);
    }

    (tokens, malformed_tokens)
}

fn parse_key_values(tokens: &[String]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut key_values = BTreeMap::new();
    let mut malformed_tokens = Vec::new();

    for token in tokens {
        match token.split_once('=') {
            Some((key, value)) if !key.is_empty() && !value.is_empty() => {
                key_values.insert(key.to_string(), value.to_string());
            }
            _ => malformed_tokens.push(token.to_string()),
        }
    }

    (key_values, malformed_tokens)
}

fn push_warning(warnings: &mut Vec<ShaderControlWarning>, line: usize, message: String) {
    warnings.push(ShaderControlWarning { line, message });
}

fn unquote(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
}

fn parse_quoted_label(
    warnings: &mut Vec<ShaderControlWarning>,
    line: usize,
    control_type: &str,
    uniform_name: &str,
    value: Option<&String>,
) -> Option<String> {
    let value = value?;
    match unquote(value) {
        Some(label) => Some(label.to_string()),
        None => {
            push_warning(
                warnings,
                line,
                format!("{} `{}` label must be quoted", control_type, uniform_name),
            );
            None
        }
    }
}

fn warn_for_unrecognized_fields(
    warnings: &mut Vec<ShaderControlWarning>,
    line: usize,
    control_type: &str,
    key_values: &BTreeMap<String, String>,
    malformed_tokens: &[String],
    allowed_fields: &[&str],
) {
    for token in malformed_tokens {
        push_warning(
            warnings,
            line,
            format!("Malformed control token `{}`", token),
        );
    }

    for key in key_values.keys() {
        if !allowed_fields.contains(&key.as_str()) {
            push_warning(
                warnings,
                line,
                format!("Unknown {} control field `{}`", control_type, key),
            );
        }
    }
}

fn parse_required_f32(key_values: &BTreeMap<String, String>, key: &str) -> Result<f32, String> {
    let value = key_values
        .get(key)
        .ok_or_else(|| format!("missing `{}`", key))?
        .parse::<f32>()
        .map_err(|_| format!("invalid `{}`", key))?;

    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("`{}` must be finite", key))
    }
}

fn parse_required_i32(key_values: &BTreeMap<String, String>, key: &str) -> Result<i32, String> {
    key_values
        .get(key)
        .ok_or_else(|| format!("missing `{}`", key))?
        .parse::<i32>()
        .map_err(|_| format!("invalid `{}`", key))
}

fn parse_float_range_control(
    control_label: &str,
    uniform_name: &str,
    key_values: &BTreeMap<String, String>,
) -> Result<(f32, f32, f32), String> {
    let min = parse_required_f32(key_values, "min")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;
    let max = parse_required_f32(key_values, "max")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;
    let step = parse_required_f32(key_values, "step")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;

    if max < min || step <= 0.0 {
        return Err(format!(
            "{} `{}` must have max >= min and step > 0",
            control_label, uniform_name
        ));
    }

    Ok((min, max, step))
}

fn parse_select_options(value: Option<&String>) -> Result<Vec<String>, String> {
    let value = value.ok_or_else(|| "missing `options`".to_string())?;
    let quoted = unquote(value).ok_or_else(|| "`options` must be quoted".to_string())?;
    let mut options = Vec::new();
    for option in quoted.split(',').map(str::trim) {
        if option.is_empty() {
            return Err("`options` must not contain empty labels".to_string());
        }
        options.push(option.to_string());
    }

    if options.is_empty() {
        Err("`options` must include at least one label".to_string())
    } else {
        Ok(options)
    }
}

fn parse_channel_options(value: Option<&String>) -> Result<Vec<i32>, String> {
    let Some(value) = value else {
        return Ok(vec![0, 1, 2, 3, 4]);
    };
    let quoted = unquote(value).ok_or_else(|| "`options` must be quoted".to_string())?;
    let mut options = Vec::new();
    for option in quoted.split(',').map(str::trim) {
        if option.is_empty() {
            return Err("`options` must contain channel numbers in 0..=4".to_string());
        }
        let channel = option
            .parse::<i32>()
            .map_err(|_| "`options` must contain channel numbers in 0..=4".to_string())?;
        if !(0..=4).contains(&channel) {
            return Err("`options` must contain channel numbers in 0..=4".to_string());
        }
        options.push(channel);
    }

    if options.is_empty() {
        Err("`options` must contain at least one channel in 0..=4".to_string())
    } else {
        Ok(options)
    }
}

pub fn parse_shader_controls(source: &str) -> ShaderControlParseResult {
    let lines: Vec<&str> = source.lines().collect();
    let mut controls = Vec::new();
    let mut warnings = Vec::new();
    let mut groups = BTreeMap::new();
    let mut seen = HashSet::new();
    let mut float_count = 0usize;
    let mut bool_count = 0usize;
    let mut color_count = 0usize;
    let mut int_count = 0usize;
    let mut vec2_count = 0usize;

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
        let kind = match control_type {
            "slider" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["min", "max", "step", "scale", "label", "group"],
                );

                if uniform_type != "float" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Slider control for `{}` must attach to `uniform float`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let (min, max, step) =
                    match parse_float_range_control("Slider", uniform_name, &key_values) {
                        Ok(values) => values,
                        Err(error) => {
                            push_warning(&mut warnings, line_number, error);
                            continue;
                        }
                    };

                let scale = match key_values.get("scale").map(String::as_str) {
                    Some("linear") | None => SliderScale::Linear,
                    Some("log") => SliderScale::Log,
                    Some(_) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!("Slider `{}` scale must be `linear` or `log`", uniform_name),
                        );
                        continue;
                    }
                };

                if scale == SliderScale::Log && !(0.0 < min && min < max) {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!("Slider `{}` scale=log requires 0 < min < max", uniform_name),
                    );
                    continue;
                }

                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Slider",
                    uniform_name,
                    key_values.get("label"),
                );

                ShaderControlKind::Slider {
                    min,
                    max,
                    step,
                    scale,
                    label,
                }
            }
            "checkbox" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["label", "group"],
                );

                if uniform_type != "bool" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Checkbox control for `{}` must attach to `uniform bool`",
                            uniform_name
                        ),
                    );
                    continue;
                }
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Checkbox",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Checkbox { label }
            }
            "color" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["alpha", "label", "group"],
                );

                if uniform_type != "vec3" && uniform_type != "vec4" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Color control for `{}` must attach to `uniform vec3` or `uniform vec4`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let default_alpha = uniform_type == "vec4";
                let alpha = match key_values.get("alpha").map(String::as_str) {
                    Some("true") => true,
                    Some("false") => false,
                    Some(_) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!(
                                "Color `{}` alpha must be `true` or `false`; using default",
                                uniform_name
                            ),
                        );
                        default_alpha
                    }
                    None => default_alpha,
                };

                if uniform_type == "vec3" && alpha {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Color control `{}` cannot use alpha=true with `uniform vec3`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Color",
                    uniform_name,
                    key_values.get("label"),
                );

                ShaderControlKind::Color { alpha, label }
            }
            "int" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["min", "max", "step", "label", "group"],
                );

                if uniform_type != "int" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Int control for `{}` must attach to `uniform int`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let min = match parse_required_i32(&key_values, "min") {
                    Ok(value) => value,
                    Err(error) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!("Int `{}` {}", uniform_name, error),
                        );
                        continue;
                    }
                };
                let max = match parse_required_i32(&key_values, "max") {
                    Ok(value) => value,
                    Err(error) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!("Int `{}` {}", uniform_name, error),
                        );
                        continue;
                    }
                };
                let step = match key_values.get("step") {
                    Some(_) => match parse_required_i32(&key_values, "step") {
                        Ok(value) => value,
                        Err(error) => {
                            push_warning(
                                &mut warnings,
                                line_number,
                                format!("Int `{}` {}", uniform_name, error),
                            );
                            continue;
                        }
                    },
                    None => 1,
                };

                if max < min || step <= 0 {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!("Int `{}` must have max >= min and step > 0", uniform_name),
                    );
                    continue;
                }

                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Int",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Int {
                    min,
                    max,
                    step,
                    label,
                }
            }
            "select" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["options", "label", "group"],
                );

                if uniform_type != "int" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Select control for `{}` must attach to `uniform int`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let options = match parse_select_options(key_values.get("options")) {
                    Ok(options) => options,
                    Err(error) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!("Select `{}` {}", uniform_name, error),
                        );
                        continue;
                    }
                };
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Select",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Select { options, label }
            }
            "vec2" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["min", "max", "step", "label", "group"],
                );

                if uniform_type != "vec2" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Vec2 control for `{}` must attach to `uniform vec2`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let (min, max, step) =
                    match parse_float_range_control("Vec2", uniform_name, &key_values) {
                        Ok(values) => values,
                        Err(error) => {
                            push_warning(&mut warnings, line_number, error);
                            continue;
                        }
                    };
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Vec2",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Vec2 {
                    min,
                    max,
                    step,
                    label,
                }
            }
            "point" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["label", "group"],
                );

                if uniform_type != "vec2" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Point control for `{}` must attach to `uniform vec2`",
                            uniform_name
                        ),
                    );
                    continue;
                }
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Point",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Point { label }
            }
            "range" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["min", "max", "step", "label", "group"],
                );

                if uniform_type != "vec2" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Range control for `{}` must attach to `uniform vec2`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let (min, max, step) =
                    match parse_float_range_control("Range", uniform_name, &key_values) {
                        Ok(values) => values,
                        Err(error) => {
                            push_warning(&mut warnings, line_number, error);
                            continue;
                        }
                    };
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Range",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Range {
                    min,
                    max,
                    step,
                    label,
                }
            }
            "angle" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["unit", "label", "group"],
                );

                if uniform_type != "float" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Angle control for `{}` must attach to `uniform float`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let unit = match key_values.get("unit").map(String::as_str) {
                    Some("degrees") | None => AngleUnit::Degrees,
                    Some("radians") => AngleUnit::Radians,
                    Some(_) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!(
                                "Angle `{}` unit must be `degrees` or `radians`",
                                uniform_name
                            ),
                        );
                        continue;
                    }
                };
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Angle",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Angle { unit, label }
            }
            "channel" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["options", "label", "group"],
                );

                if uniform_type != "int" {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Channel control for `{}` must attach to `uniform int`",
                            uniform_name
                        ),
                    );
                    continue;
                }

                let options = match parse_channel_options(key_values.get("options")) {
                    Ok(options) => options,
                    Err(error) => {
                        push_warning(
                            &mut warnings,
                            line_number,
                            format!("Channel `{}` {}", uniform_name, error),
                        );
                        continue;
                    }
                };
                let label = parse_quoted_label(
                    &mut warnings,
                    line_number,
                    "Channel",
                    uniform_name,
                    key_values.get("label"),
                );
                ShaderControlKind::Channel { options, label }
            }
            other => {
                push_warning(
                    &mut warnings,
                    line_number,
                    format!("Unsupported control type `{}`", other),
                );
                continue;
            }
        };

        if !seen.insert(uniform_name.to_string()) {
            push_warning(
                &mut warnings,
                line_number,
                format!("Duplicate control for uniform `{}` ignored", uniform_name),
            );
            continue;
        }

        match &kind {
            ShaderControlKind::Slider { .. } | ShaderControlKind::Angle { .. } => {
                if float_count >= MAX_SHADER_FLOAT_CONTROLS {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Only the first {} float controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_FLOAT_CONTROLS, uniform_name
                        ),
                    );
                    continue;
                }
                float_count += 1;
            }
            ShaderControlKind::Checkbox { .. } => {
                if bool_count >= MAX_SHADER_BOOL_CONTROLS {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Only the first {} checkbox controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_BOOL_CONTROLS, uniform_name
                        ),
                    );
                    continue;
                }
                bool_count += 1;
            }
            ShaderControlKind::Color { .. } => {
                if color_count >= MAX_SHADER_COLOR_CONTROLS {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Only the first {} color controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_COLOR_CONTROLS, uniform_name
                        ),
                    );
                    continue;
                }
                color_count += 1;
            }
            ShaderControlKind::Int { .. }
            | ShaderControlKind::Select { .. }
            | ShaderControlKind::Channel { .. } => {
                if int_count >= MAX_SHADER_INT_CONTROLS {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Only the first {} int controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_INT_CONTROLS, uniform_name
                        ),
                    );
                    continue;
                }
                int_count += 1;
            }
            ShaderControlKind::Vec2 { .. }
            | ShaderControlKind::Point { .. }
            | ShaderControlKind::Range { .. } => {
                if vec2_count >= MAX_SHADER_VEC2_CONTROLS {
                    push_warning(
                        &mut warnings,
                        line_number,
                        format!(
                            "Only the first {} vec2 controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_VEC2_CONTROLS, uniform_name
                        ),
                    );
                    continue;
                }
                vec2_count += 1;
            }
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

#[cfg(test)]
mod tests {
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
}
