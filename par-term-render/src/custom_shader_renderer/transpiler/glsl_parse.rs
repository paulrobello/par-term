//! GLSL source preprocessing and custom control uniform handling.
//!
//! This module handles:
//! - Shadertoy `fragCoord` Y-flip preprocessing
//! - Custom shader control (`// control ...`) uniform extraction and replacement
//! - Safe fallback generation for malformed or over-limit controls

fn format_glsl_float_literal(value: f32) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }

    let mut literal = value.to_string();
    if !literal.contains('.') && !literal.contains('e') && !literal.contains('E') {
        literal.push_str(".0");
    }
    literal
}

/// Pre-process GLSL to make Shadertoy `fragCoord` use the flipped Y convention **inside**
/// `mainImage`, avoiding cross-function `var<private>` writes that Metal is dropping.
///
/// Steps:
/// 1) Rename the `in vec2 <name>` parameter to `_fc_raw` (raw @builtin(position) coords).
/// 2) Inject at the start of `mainImage` a flipped local `vec2 fragCoord` and set
///    `gl_FragCoord_st` to that flipped value so shaders that read the global also see it.
pub(crate) fn preprocess_glsl_for_shadertoy(glsl_source: &str) -> String {
    let mut source = glsl_source.to_string();

    if let Some(main_pos) = source.find("void mainImage") {
        // Locate parameter list boundaries.
        if let (Some(paren_start), Some(paren_end)) =
            (source[main_pos..].find('('), source[main_pos..].find(')'))
        {
            let abs_start = main_pos + paren_start + 1;
            let abs_end = main_pos + paren_end;
            let params = &source[abs_start..abs_end];

            // Find the first `in vec2` parameter and rename its identifier to `_fc_raw`.
            if let Some(in_pos) = params.find("in vec2") {
                let ident_start = abs_start
                    + in_pos
                    + "in vec2".len()
                    + params[in_pos + "in vec2".len()..]
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .count();

                let mut ident_end = ident_start;
                for ch in source[ident_start..abs_end].chars() {
                    if ch.is_alphanumeric() || ch == '_' {
                        ident_end += ch.len_utf8();
                    } else {
                        break;
                    }
                }

                source.replace_range(ident_start..ident_end, "_fc_raw");
            }
        }

        // Find the first '{' after the mainImage declaration to inject our prologue.
        if let Some(rel_brace) = source[main_pos..].find('{') {
            let inject_pos = main_pos + rel_brace + 1; // after '{'
            // Flip once here for Shadertoy convention (y=0 at bottom).
            let inject = "\n    vec2 fragCoord = vec2(_fc_raw.x, iResolution.y - _fc_raw.y);\n    gl_FragCoord_st = fragCoord;\n";
            source.insert_str(inject_pos, inject);
        }
    }

    source
}

// ---- Custom control uniform types and helpers ----

#[derive(Debug, Clone, Copy)]
pub(crate) enum ActiveCustomControl {
    Float { index: usize },
    Bool { index: usize },
    Color { index: usize },
    Int { index: usize },
    Vec2 { index: usize },
}

pub(crate) fn active_custom_controls(
    controls: &[par_term_config::ShaderControl],
) -> std::collections::HashMap<String, ActiveCustomControl> {
    let mut float_index = 0usize;
    let mut bool_index = 0usize;
    let mut color_index = 0usize;
    let mut int_index = 0usize;
    let mut vec2_index = 0usize;
    let mut active_controls = std::collections::HashMap::new();

    for control in controls {
        match &control.kind {
            par_term_config::ShaderControlKind::Slider { .. }
            | par_term_config::ShaderControlKind::Angle { .. } => {
                active_controls.insert(
                    control.name.clone(),
                    ActiveCustomControl::Float { index: float_index },
                );
                float_index += 1;
            }
            par_term_config::ShaderControlKind::Checkbox { .. } => {
                active_controls.insert(
                    control.name.clone(),
                    ActiveCustomControl::Bool { index: bool_index },
                );
                bool_index += 1;
            }
            par_term_config::ShaderControlKind::Color { .. } => {
                active_controls.insert(
                    control.name.clone(),
                    ActiveCustomControl::Color { index: color_index },
                );
                color_index += 1;
            }
            par_term_config::ShaderControlKind::Int { .. }
            | par_term_config::ShaderControlKind::Select { .. }
            | par_term_config::ShaderControlKind::Channel { .. } => {
                active_controls.insert(
                    control.name.clone(),
                    ActiveCustomControl::Int { index: int_index },
                );
                int_index += 1;
            }
            par_term_config::ShaderControlKind::Vec2 { .. }
            | par_term_config::ShaderControlKind::Point { .. }
            | par_term_config::ShaderControlKind::Range { .. } => {
                active_controls.insert(
                    control.name.clone(),
                    ActiveCustomControl::Vec2 { index: vec2_index },
                );
                vec2_index += 1;
            }
        }
    }

    active_controls
}

pub(crate) fn active_custom_control_define(
    name: &str,
    ty: &str,
    control: ActiveCustomControl,
) -> Option<String> {
    match (control, ty) {
        (ActiveCustomControl::Float { index }, "float") => Some(format!(
            "#define {} iCustomFloatUniforms[{}].{}\n",
            name,
            index / 4,
            ["x", "y", "z", "w"][index % 4]
        )),
        (ActiveCustomControl::Bool { index }, "bool") => Some(format!(
            "#define {} (iCustomBoolUniforms[{}].{} != 0)\n",
            name,
            index / 4,
            ["x", "y", "z", "w"][index % 4]
        )),
        (ActiveCustomControl::Color { index }, "vec3") => Some(format!(
            "#define {name} iCustomColorUniforms[{index}].rgb\n"
        )),
        (ActiveCustomControl::Color { index }, "vec4") => {
            Some(format!("#define {name} iCustomColorUniforms[{index}]\n"))
        }
        (ActiveCustomControl::Int { index }, "int") => Some(format!(
            "#define {} iCustomIntUniforms[{}].{}\n",
            name,
            index / 4,
            ["x", "y", "z", "w"][index % 4]
        )),
        (ActiveCustomControl::Vec2 { index }, "vec2") => {
            Some(format!("#define {name} iCustomVec2Uniforms[{index}].xy\n"))
        }
        _ => None,
    }
}

pub(crate) fn parse_control_uniform_declaration(line: &str) -> Option<(&str, &str)> {
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

pub(crate) fn parse_control_key_values<'a>(
    tokens: impl Iterator<Item = &'a str>,
) -> std::collections::HashMap<&'a str, &'a str> {
    tokens
        .filter_map(|token| token.split_once('='))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

pub(crate) fn tokenize_attached_control_directive(rest: &str) -> Vec<String> {
    let mut tokens = Vec::new();
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
        tokens.push(current);
    }

    tokens
}

pub(crate) fn valid_attached_control_fallback(comment_line: &str, ty: &str) -> Option<String> {
    let rest = comment_line.trim().strip_prefix("// control ")?;
    let tokens = tokenize_attached_control_directive(rest);
    let control_type = tokens.first()?.as_str();
    let key_values = parse_control_key_values(tokens[1..].iter().map(String::as_str));

    match control_type {
        "slider" if ty == "float" => {
            let min = key_values.get("min")?.parse::<f32>().ok()?;
            let max = key_values.get("max")?.parse::<f32>().ok()?;
            let step = key_values.get("step")?.parse::<f32>().ok()?;
            if min.is_finite() && max.is_finite() && step.is_finite() && max >= min && step > 0.0 {
                Some(format_glsl_float_literal(min))
            } else {
                None
            }
        }
        "angle" if ty == "float" => Some("0.0".to_string()),
        "checkbox" if ty == "bool" => Some("false".to_string()),
        "color" if ty == "vec3" => Some("vec3(1.0)".to_string()),
        "color" if ty == "vec4" => Some("vec4(1.0)".to_string()),
        "int" if ty == "int" => {
            let min = key_values.get("min")?.parse::<i32>().ok()?;
            let max = key_values.get("max")?.parse::<i32>().ok()?;
            let step = key_values
                .get("step")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(1);
            if max >= min && step > 0 {
                Some(min.to_string())
            } else {
                None
            }
        }
        "select" if ty == "int" => {
            valid_quoted_csv(key_values.get("options")?).then(|| "0".to_string())
        }
        "channel" if ty == "int" => match key_values.get("options") {
            Some(options) => first_valid_channel_option(options).map(|channel| channel.to_string()),
            None => Some("0".to_string()),
        },
        "vec2" if ty == "vec2" => {
            let min = key_values.get("min")?.parse::<f32>().ok()?;
            let max = key_values.get("max")?.parse::<f32>().ok()?;
            let step = key_values.get("step")?.parse::<f32>().ok()?;
            if min.is_finite() && max.is_finite() && step.is_finite() && max >= min && step > 0.0 {
                Some(format!("vec2({})", format_glsl_float_literal(min)))
            } else {
                None
            }
        }
        "point" if ty == "vec2" => Some("vec2(0.5)".to_string()),
        "range" if ty == "vec2" => {
            let min = key_values.get("min")?.parse::<f32>().ok()?;
            let max = key_values.get("max")?.parse::<f32>().ok()?;
            let step = key_values.get("step")?.parse::<f32>().ok()?;
            if min.is_finite() && max.is_finite() && step.is_finite() && max >= min && step > 0.0 {
                Some(format!(
                    "vec2({}, {})",
                    format_glsl_float_literal(min),
                    format_glsl_float_literal(max)
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn valid_quoted_csv(value: &str) -> bool {
    let Some(quoted) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return false;
    };

    let mut saw_option = false;
    for option in quoted.split(',').map(str::trim) {
        if option.is_empty() {
            return false;
        }
        saw_option = true;
    }
    saw_option
}

pub(crate) fn first_valid_channel_option(value: &str) -> Option<i32> {
    let quoted = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))?;
    let mut first = None;
    for option in quoted.split(',').map(str::trim) {
        let channel = option.parse::<i32>().ok()?;
        if !(0..=4).contains(&channel) {
            return None;
        }
        first.get_or_insert(channel);
    }
    first
}

pub(crate) fn safe_fallback_define(comment_line: &str, ty: &str, name: &str) -> String {
    let fallback = valid_attached_control_fallback(comment_line, ty).unwrap_or_else(|| match ty {
        "float" => "0.0".to_string(),
        "bool" => "false".to_string(),
        "int" => "0".to_string(),
        "vec2" => "vec2(0.0)".to_string(),
        "vec3" => "vec3(1.0)".to_string(),
        "vec4" => "vec4(1.0)".to_string(),
        _ => unreachable!("fallback requested only for supported custom control uniforms"),
    });

    format!("#define {name} {fallback}\n")
}

pub(crate) fn attached_control_type(comment_line: &str) -> Option<&str> {
    comment_line
        .trim()
        .strip_prefix("// control ")?
        .split_whitespace()
        .next()
}

pub(crate) fn preprocess_custom_control_uniforms(source: &str) -> String {
    let parse_result = par_term_config::parse_shader_controls(source);
    let mut active_controls = active_custom_controls(&parse_result.controls);
    let lines: Vec<&str> = source.lines().collect();
    let mut strip_line_indices = std::collections::HashSet::new();
    let mut control_defines = String::new();
    let mut defined_names = std::collections::HashSet::new();

    for (index, line) in lines.iter().enumerate() {
        if !line.trim().starts_with("// control ") {
            continue;
        }

        let Some(next_line) = lines.get(index + 1) else {
            continue;
        };
        let Some((ty, name)) = parse_control_uniform_declaration(next_line) else {
            continue;
        };
        let control_type = attached_control_type(line);
        let should_strip = ty == "float"
            || ty == "bool"
            || ty == "int"
            || ty == "vec2"
            || (control_type == Some("color") && (ty == "vec3" || ty == "vec4"));
        if !should_strip {
            continue;
        }

        strip_line_indices.insert(index + 1);
        if !defined_names.insert(name.to_string()) {
            continue;
        }

        if let Some(define) = active_controls
            .remove(name)
            .and_then(|control| active_custom_control_define(name, ty, control))
        {
            control_defines.push_str(&define);
        } else {
            control_defines.push_str(&safe_fallback_define(line, ty, name));
        }
    }

    let mut output = String::new();
    output.push_str(&control_defines);

    for (index, line) in lines.iter().enumerate() {
        if !strip_line_indices.contains(&index) {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_uniform_declarations_are_replaced_with_custom_block_macros() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
// control checkbox
uniform bool iEnabled;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), iEnabled ? 1.0 : 0.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(!preprocessed.contains("uniform float iGlow;"));
        assert!(!preprocessed.contains("uniform bool iEnabled;"));
        assert!(preprocessed.contains("#define iGlow iCustomFloatUniforms[0].x"));
        assert!(preprocessed.contains("#define iEnabled (iCustomBoolUniforms[0].x != 0)"));
    }

    #[test]
    fn controlled_uniform_new_declarations_are_replaced_with_custom_macros() {
        let source = r#"
// control angle unit=radians
uniform float iAngle;
// control int min=1 max=9 step=2
uniform int iCount;
// control select options="Low,Medium,High"
uniform int iChoice;
// control channel options="1,3"
uniform int iChannel;
// control vec2 min=-1 max=1 step=0.1
uniform vec2 iOffset;
// control point
uniform vec2 iPoint;
// control range min=0 max=10 step=0.5
uniform vec2 iRange;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iOffset + iPoint + iRange, float(iCount + iChoice + iChannel) + iAngle, 1.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(!preprocessed.contains("uniform float iAngle;"));
        assert!(!preprocessed.contains("uniform int iCount;"));
        assert!(!preprocessed.contains("uniform int iChoice;"));
        assert!(!preprocessed.contains("uniform int iChannel;"));
        assert!(!preprocessed.contains("uniform vec2 iOffset;"));
        assert!(!preprocessed.contains("uniform vec2 iPoint;"));
        assert!(!preprocessed.contains("uniform vec2 iRange;"));
        assert!(preprocessed.contains("#define iAngle iCustomFloatUniforms[0].x"));
        assert!(preprocessed.contains("#define iCount iCustomIntUniforms[0].x"));
        assert!(preprocessed.contains("#define iChoice iCustomIntUniforms[0].y"));
        assert!(preprocessed.contains("#define iChannel iCustomIntUniforms[0].z"));
        assert!(preprocessed.contains("#define iOffset iCustomVec2Uniforms[0].xy"));
        assert!(preprocessed.contains("#define iPoint iCustomVec2Uniforms[1].xy"));
        assert!(preprocessed.contains("#define iRange iCustomVec2Uniforms[2].xy"));
    }

    #[test]
    fn controlled_uniform_declarations_with_whitespace_are_stripped() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow ;
// control checkbox
uniform   bool   iEnabled   ;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), iEnabled ? 1.0 : 0.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(!preprocessed.contains("uniform float iGlow ;"));
        assert!(!preprocessed.contains("uniform   bool   iEnabled   ;"));
        assert!(preprocessed.contains("#define iGlow iCustomFloatUniforms[0].x"));
        assert!(preprocessed.contains("#define iEnabled (iCustomBoolUniforms[0].x != 0)"));
    }

    #[test]
    fn controlled_uniform_color_declarations_are_replaced_with_color_macros() {
        let source = r#"
// control color label="Tint"
uniform vec3 iTint;
// control color alpha=true label="Overlay"
uniform vec4 iOverlay;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iTint, 1.0) * iOverlay;
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(!preprocessed.contains("uniform vec3 iTint;"));
        assert!(!preprocessed.contains("uniform vec4 iOverlay;"));
        assert!(preprocessed.contains("#define iTint iCustomColorUniforms[0].rgb"));
        assert!(preprocessed.contains("#define iOverlay iCustomColorUniforms[1]"));
    }

    #[test]
    fn controlled_uniforms_over_float_limit_are_replaced_with_safe_fallback() {
        let mut source = String::new();
        for index in 0..17 {
            source.push_str("// control slider min=0.25 max=1 step=0.01\n");
            source.push_str(&format!("uniform float iFloat{index};\n"));
        }
        source.push_str(
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(iFloat16); }\n",
        );

        let preprocessed = preprocess_custom_control_uniforms(&source);

        assert!(preprocessed.contains("#define iFloat15 iCustomFloatUniforms[3].w"));
        assert!(preprocessed.contains("#define iFloat16 0.25"));
        assert!(!preprocessed.contains("uniform float iFloat16;"));
    }

    #[test]
    fn malformed_attached_controlled_uniform_is_replaced_with_safe_fallback() {
        let source = r#"
// control slider min=0 max=1
uniform float iGlow;
// control radio
uniform bool iEnabled;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), iEnabled ? 1.0 : 0.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(preprocessed.contains("#define iGlow 0.0"));
        assert!(preprocessed.contains("#define iEnabled false"));
        assert!(!preprocessed.contains("uniform float iGlow;"));
        assert!(!preprocessed.contains("uniform bool iEnabled;"));
    }

    #[test]
    fn attached_control_fallbacks_parse_quoted_options_with_spaces() {
        assert_eq!(
            valid_attached_control_fallback("// control select options=\"Low, Medium\"", "int"),
            Some("0".to_string())
        );
        assert_eq!(
            valid_attached_control_fallback("// control channel options=\"1, 3\"", "int"),
            Some("1".to_string())
        );
    }

    #[test]
    fn malformed_and_over_limit_new_controls_use_safe_fallbacks() {
        let source = r#"
// control int min=5
uniform int iMalformedInt;
// control radio
uniform vec2 iUnsupportedVec2;
"#;
        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(preprocessed.contains("#define iMalformedInt 0"));
        assert!(preprocessed.contains("#define iUnsupportedVec2 vec2(0.0)"));
        assert!(!preprocessed.contains("uniform int iMalformedInt;"));
        assert!(!preprocessed.contains("uniform vec2 iUnsupportedVec2;"));

        let mut over_limit = String::new();
        for index in 0..16 {
            over_limit.push_str("// control vec2 min=-1 max=1 step=0.1\n");
            over_limit.push_str(&format!("uniform vec2 iVec{index};\n"));
        }
        over_limit.push_str("// control range min=2 max=8 step=1\n");
        over_limit.push_str("uniform vec2 iRange16;\n");
        over_limit.push_str("// control point\n");
        over_limit.push_str("uniform vec2 iPoint17;\n");
        let preprocessed = preprocess_custom_control_uniforms(&over_limit);
        assert!(preprocessed.contains("#define iVec15 iCustomVec2Uniforms[15].xy"));
        assert!(preprocessed.contains("#define iRange16 vec2(2.0, 8.0)"));
        assert!(preprocessed.contains("#define iPoint17 vec2(0.5)"));
        assert!(!preprocessed.contains("uniform vec2 iRange16;"));
        assert!(!preprocessed.contains("uniform vec2 iPoint17;"));
    }

    #[test]
    fn controlled_uniform_parser_warns_and_ignores_over_limit_controls() {
        let mut source = String::new();
        for index in 0..17 {
            source.push_str("// control checkbox\n");
            source.push_str(&format!("uniform bool iBool{index};\n"));
        }

        let result = par_term_config::parse_shader_controls(&source);

        assert_eq!(result.controls.len(), 16);
        assert!(
            result
                .controls
                .iter()
                .all(|control| control.name != "iBool16")
        );
        assert!(result.warnings.iter().any(|warning| {
            warning
                .message
                .contains("Only the first 16 checkbox controls")
        }));
    }

    #[test]
    fn transpiled_over_limit_controlled_uniform_color_controls_use_safe_fallback() {
        let mut source = String::new();
        for index in 0..16 {
            source.push_str("// control color\n");
            source.push_str(&format!("uniform vec3 iColor{index};\n"));
        }
        source.push_str("// control color\n");
        source.push_str("uniform vec4 iColor16;\n");
        source.push_str(
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = iColor16; }\n",
        );

        let preprocessed = preprocess_custom_control_uniforms(&source);
        assert!(preprocessed.contains("#define iColor16 vec4(1.0)"));
        assert!(!preprocessed.contains("uniform vec4 iColor16;"));
    }

    #[test]
    fn transpiled_malformed_controlled_uniform_color_controls_use_safe_fallbacks() {
        let source = r#"
// control color alpha=true
uniform vec3 iBadRgb;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iBadRgb, 1.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);
        assert!(preprocessed.contains("#define iBadRgb vec3(1.0)"));
        assert!(!preprocessed.contains("uniform vec3 iBadRgb;"));
    }
}
