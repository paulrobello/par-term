use crate::types::shader::ShaderUniformValue;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderControlKind {
    Slider { min: f32, max: f32, step: f32 },
    Checkbox,
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
}

const MAX_SHADER_FLOAT_CONTROLS: usize = 16;
const MAX_SHADER_BOOL_CONTROLS: usize = 16;

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

fn parse_key_values(tokens: &[&str]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut key_values = BTreeMap::new();
    let mut malformed_tokens = Vec::new();

    for token in tokens {
        match token.split_once('=') {
            Some((key, value)) if !key.is_empty() && !value.is_empty() => {
                key_values.insert(key.to_string(), value.to_string());
            }
            _ => malformed_tokens.push((*token).to_string()),
        }
    }

    (key_values, malformed_tokens)
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
        warnings.push(ShaderControlWarning {
            line,
            message: format!("Malformed control token `{}`", token),
        });
    }

    for key in key_values.keys() {
        if !allowed_fields.contains(&key.as_str()) {
            warnings.push(ShaderControlWarning {
                line,
                message: format!("Unknown {} control field `{}`", control_type, key),
            });
        }
    }
}

pub fn parse_shader_controls(source: &str) -> ShaderControlParseResult {
    let lines: Vec<&str> = source.lines().collect();
    let mut controls = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    let mut float_count = 0usize;
    let mut bool_count = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("// control ") else {
            continue;
        };

        let line_number = index + 1;
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let Some(control_type) = tokens.first().copied() else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment is missing a control type".to_string(),
            });
            continue;
        };

        let Some(next_line) = lines.get(index + 1) else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment must be immediately followed by a uniform declaration"
                    .to_string(),
            });
            continue;
        };

        let Some((uniform_type, uniform_name)) = parse_uniform_declaration(next_line) else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment must be immediately followed by a uniform declaration"
                    .to_string(),
            });
            continue;
        };

        let (key_values, malformed_tokens) = parse_key_values(&tokens[1..]);
        let kind = match control_type {
            "slider" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &["min", "max", "step"],
                );

                if uniform_type != "float" {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!(
                            "Slider control for `{}` must attach to `uniform float`",
                            uniform_name
                        ),
                    });
                    continue;
                }

                let parse_required = |key: &str| -> Result<f32, String> {
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
                };

                let min = match parse_required("min") {
                    Ok(value) => value,
                    Err(error) => {
                        warnings.push(ShaderControlWarning {
                            line: line_number,
                            message: format!("Slider `{}` {}", uniform_name, error),
                        });
                        continue;
                    }
                };
                let max = match parse_required("max") {
                    Ok(value) => value,
                    Err(error) => {
                        warnings.push(ShaderControlWarning {
                            line: line_number,
                            message: format!("Slider `{}` {}", uniform_name, error),
                        });
                        continue;
                    }
                };
                let step = match parse_required("step") {
                    Ok(value) => value,
                    Err(error) => {
                        warnings.push(ShaderControlWarning {
                            line: line_number,
                            message: format!("Slider `{}` {}", uniform_name, error),
                        });
                        continue;
                    }
                };

                if max < min || step <= 0.0 {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!(
                            "Slider `{}` must have max >= min and step > 0",
                            uniform_name
                        ),
                    });
                    continue;
                }

                ShaderControlKind::Slider { min, max, step }
            }
            "checkbox" => {
                warn_for_unrecognized_fields(
                    &mut warnings,
                    line_number,
                    control_type,
                    &key_values,
                    &malformed_tokens,
                    &[],
                );

                if uniform_type != "bool" {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!(
                            "Checkbox control for `{}` must attach to `uniform bool`",
                            uniform_name
                        ),
                    });
                    continue;
                }
                ShaderControlKind::Checkbox
            }
            other => {
                warnings.push(ShaderControlWarning {
                    line: line_number,
                    message: format!("Unsupported control type `{}`", other),
                });
                continue;
            }
        };

        if !seen.insert(uniform_name.to_string()) {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: format!("Duplicate control for uniform `{}` ignored", uniform_name),
            });
            continue;
        }

        match &kind {
            ShaderControlKind::Slider { .. } => {
                if float_count >= MAX_SHADER_FLOAT_CONTROLS {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!(
                            "Only the first {} slider controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_FLOAT_CONTROLS, uniform_name
                        ),
                    });
                    continue;
                }
                float_count += 1;
            }
            ShaderControlKind::Checkbox => {
                if bool_count >= MAX_SHADER_BOOL_CONTROLS {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!(
                            "Only the first {} checkbox controls are active; ignoring over-limit control `{}`",
                            MAX_SHADER_BOOL_CONTROLS, uniform_name
                        ),
                    });
                    continue;
                }
                bool_count += 1;
            }
        }

        controls.push(ShaderControl {
            name: uniform_name.to_string(),
            kind,
        });
    }

    ShaderControlParseResult { controls, warnings }
}

pub fn fallback_value_for_control(control: &ShaderControl) -> ShaderUniformValue {
    match control.kind {
        ShaderControlKind::Slider { min, .. } => ShaderUniformValue::Float(min),
        ShaderControlKind::Checkbox => ShaderUniformValue::Bool(false),
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
                },
            }]
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
                kind: ShaderControlKind::Checkbox,
            }]
        );
    }

    #[test]
    fn warns_and_skips_unsupported_control_type() {
        let source = r#"
// control color min=0 max=1 step=0.1
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
        assert!(result.warnings[0].message.contains("color"));
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
