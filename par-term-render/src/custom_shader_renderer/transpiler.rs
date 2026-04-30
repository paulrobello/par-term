use anyhow::Result;
use std::path::Path;

/// Pre-process GLSL to make Shadertoy `fragCoord` use the flipped Y convention **inside**
/// `mainImage`, avoiding cross-function `var<private>` writes that Metal is dropping.
///
/// Steps:
/// 1) Rename the `in vec2 <name>` parameter to `_fc_raw` (raw @builtin(position) coords).
/// 2) Inject at the start of `mainImage` a flipped local `vec2 fragCoord` and set
///    `gl_FragCoord_st` to that flipped value so shaders that read the global also see it.
fn preprocess_glsl_for_shadertoy(glsl_source: &str) -> String {
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

#[derive(Debug, Clone, Copy)]
enum ActiveCustomControl {
    Float { index: usize },
    Bool { index: usize },
    Color { index: usize },
    Int { index: usize },
    Vec2 { index: usize },
}

fn active_custom_controls(
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

fn active_custom_control_define(
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

fn parse_control_uniform_declaration(line: &str) -> Option<(&str, &str)> {
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

fn parse_control_key_values<'a>(
    tokens: impl Iterator<Item = &'a str>,
) -> std::collections::HashMap<&'a str, &'a str> {
    tokens
        .filter_map(|token| token.split_once('='))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

fn tokenize_attached_control_directive(rest: &str) -> Vec<String> {
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

fn valid_attached_control_fallback(comment_line: &str, ty: &str) -> Option<String> {
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

fn valid_quoted_csv(value: &str) -> bool {
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

fn first_valid_channel_option(value: &str) -> Option<i32> {
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

fn safe_fallback_define(comment_line: &str, ty: &str, name: &str) -> String {
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

fn attached_control_type(comment_line: &str) -> Option<&str> {
    comment_line
        .trim()
        .strip_prefix("// control ")?
        .split_whitespace()
        .next()
}

fn preprocess_custom_control_uniforms(source: &str) -> String {
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

/// The shared GLSL wrapper template injected around the user shader code.
///
/// The `{glsl_source}` placeholder is replaced with the user-provided (preprocessed) GLSL.
fn glsl_wrapper_template(glsl_source: &str) -> String {
    format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 368 bytes
layout(set = 0, binding = 0) uniform Uniforms {{
    vec2 iResolution;      // offset 0, size 8 - Viewport resolution
    float iTime;           // offset 8, size 4 - Time in seconds
    float iTimeDelta;      // offset 12, size 4 - Time since last frame
    vec4 iMouse;           // offset 16, size 16 - Mouse state (xy=current, zw=click)
    vec4 iDate;            // offset 32, size 16 - Date (year, month, day, seconds)
    float iOpacity;        // offset 48, size 4 - Window opacity
    float iTextOpacity;    // offset 52, size 4 - Text opacity
    float iFullContent;    // offset 56, size 4 - Full content mode (1.0 = enabled)
    float iFrame;          // offset 60, size 4 - Frame counter
    float iFrameRate;      // offset 64, size 4 - Current FPS
    float iResolutionZ;    // offset 68, size 4 - Pixel aspect ratio (usually 1.0)
    float iBrightness;     // offset 72, size 4 - Shader brightness multiplier (0.05-1.0)
    float iTimeKeyPress;   // offset 76, size 4 - Time when last key was pressed

    // Cursor uniforms (Ghostty-compatible, v1.2.0+)
    vec4 iCurrentCursor;       // offset 80, size 16 - xy=position, zw=size (pixels)
    vec4 iPreviousCursor;      // offset 96, size 16 - xy=previous position, zw=size
    vec4 iCurrentCursorColor;  // offset 112, size 16 - RGBA (opacity baked into alpha)
    vec4 iPreviousCursorColor; // offset 128, size 16 - RGBA previous color
    float iTimeCursorChange;   // offset 144, size 4 - Time when cursor last moved

    // Cursor shader configuration uniforms
    float iCursorTrailDuration;// offset 148, size 4 - Trail effect duration (seconds)
    float iCursorGlowRadius;   // offset 152, size 4 - Glow effect radius (pixels)
    float iCursorGlowIntensity;// offset 156, size 4 - Glow effect intensity (0-1)
    vec4 iCursorShaderColor;   // offset 160, size 16 - User-configured cursor color (aligned to 16)

    // Channel resolution uniforms (Shadertoy-compatible)
    vec4 iChannelResolution0;  // offset 176, size 16 - iChannel0 resolution [width, height, 1, 0]
    vec4 iChannelResolution1;  // offset 192, size 16 - iChannel1 resolution
    vec4 iChannelResolution2;  // offset 208, size 16 - iChannel2 resolution
    vec4 iChannelResolution3;  // offset 224, size 16 - iChannel3 resolution
    vec4 iChannelResolution4;  // offset 240, size 16 - iChannel4 resolution
    vec4 iCubemapResolution;   // offset 256, size 16 - Cubemap resolution [size, size, 1, 0]

    // Background color uniform
    vec4 iBackgroundColor;     // offset 272, size 16 - Solid background color [R, G, B, A]
                               // When A > 0, use this as background instead of shader output

    // Progress bar state
    vec4 iProgress;            // offset 288, size 16 - x=state(0-4), y=percent(0-1), z=isActive(0/1), w=activeCount

    // Terminal-aware context
    vec4 iCommand;             // offset 304, size 16 - x=state(0 unknown,1 running,2 success,3 failure), y=exitCode, z=eventTime, w=running
    vec4 iFocusedPane;         // offset 320, size 16 - xy=bottom-left pixel origin, zw=size of focused pane
    vec4 iScroll;              // offset 336, size 16 - x=scrollOffset, y=visibleLines, z=scrollbackLines, w=normalizedDepth
    vec4 iReadability;         // offset 352, size 16 - x=autoDimUnderText, y=autoDimStrength
    vec4 iBackgroundChannel;   // offset 368, size 16 - x=background-as-channel0 blend mode
}};                            // total: 384 bytes

#define iBackgroundBlendMode int(iBackgroundChannel.x + 0.5)
const int BACKGROUND_BLEND_REPLACE = 0;
const int BACKGROUND_BLEND_MULTIPLY = 1;
const int BACKGROUND_BLEND_SCREEN = 2;
const int BACKGROUND_BLEND_OVERLAY = 3;
const int BACKGROUND_BLEND_LUMINANCE_MASK = 4;

// Shadertoy-compatible iChannelResolution array accessor
// Usage: iChannelResolution[0].xyz, iChannelResolution[1].xy, etc.
vec3 iChannelResolution[5] = vec3[5](
    iChannelResolution0.xyz,
    iChannelResolution1.xyz,
    iChannelResolution2.xyz,
    iChannelResolution3.xyz,
    iChannelResolution4.xyz
);

// User-defined texture channels (iChannel0-3) - Shadertoy compatible
layout(set = 0, binding = 1) uniform texture2D _iChannel0Tex;
layout(set = 0, binding = 2) uniform sampler _iChannel0Sampler;
layout(set = 0, binding = 3) uniform texture2D _iChannel1Tex;
layout(set = 0, binding = 4) uniform sampler _iChannel1Sampler;
layout(set = 0, binding = 5) uniform texture2D _iChannel2Tex;
layout(set = 0, binding = 6) uniform sampler _iChannel2Sampler;
layout(set = 0, binding = 7) uniform texture2D _iChannel3Tex;
layout(set = 0, binding = 8) uniform sampler _iChannel3Sampler;

// Terminal content texture (iChannel4)
layout(set = 0, binding = 9) uniform texture2D _iChannel4Tex;
layout(set = 0, binding = 10) uniform sampler _iChannel4Sampler;

// Cubemap texture (iCubemap)
layout(set = 0, binding = 11) uniform textureCube _iCubemapTex;
layout(set = 0, binding = 12) uniform sampler _iCubemapSampler;

// Custom shader controls generated from `// control ...` comments.
layout(set = 0, binding = 13) uniform CustomShaderControls {{
    vec4 iCustomFloatUniforms[4];
    ivec4 iCustomBoolUniforms[4];
    vec4 iCustomColorUniforms[16];
    ivec4 iCustomIntUniforms[4];
    vec4 iCustomVec2Uniforms[16];
}};

// Combined samplers for texture() calls
#define iChannel0 sampler2D(_iChannel0Tex, _iChannel0Sampler)
#define iChannel1 sampler2D(_iChannel1Tex, _iChannel1Sampler)
#define iChannel2 sampler2D(_iChannel2Tex, _iChannel2Sampler)
#define iChannel3 sampler2D(_iChannel3Tex, _iChannel3Sampler)
#define iChannel4 sampler2D(_iChannel4Tex, _iChannel4Sampler)
#define iCubemap samplerCube(_iCubemapTex, _iCubemapSampler)

// Input from vertex shader
layout(location = 0) in vec2 v_uv;

// Output color
layout(location = 0) out vec4 outColor;

// Global fragCoord for Shadertoy compatibility (avoids WGSL parameter passing issues)
vec2 gl_FragCoord_st;

// ============ User shader code begins ============

{glsl_source}

// ============ User shader code ends ============

void main() {{
    // Populate iChannelResolution array at runtime (naga drops dynamic initializers)
    iChannelResolution[0] = iChannelResolution0.xyz;
    iChannelResolution[1] = iChannelResolution1.xyz;
    iChannelResolution[2] = iChannelResolution2.xyz;
    iChannelResolution[3] = iChannelResolution3.xyz;
    iChannelResolution[4] = iChannelResolution4.xyz;

    // Flip once here (wgpu y=0 top -> Shadertoy y=0 bottom).
    vec2 st_fragCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);
    gl_FragCoord_st = st_fragCoord;
    vec4 shaderColor;
    mainImage(shaderColor, st_fragCoord);

    // Apply brightness multiplier to shader background (not text)
    vec3 dimmedShaderRgb = shaderColor.rgb * iBrightness;

    if (iFullContent > 0.5) {{
        // Full content mode: shader has full control over terminal content
        // The shader receives terminal content via iChannel4 and returns processed output.
        // We use the shader's output directly - it has already done its own compositing.
        //
        // The shader output (shaderColor/dimmedShaderRgb) contains:
        // - CRT effects, distortion, scanlines, color grading, etc.
        // - The shader's own sampling and processing of iChannel4
        vec4 terminalColor = texture(iChannel4, vec2(v_uv.x, 1.0 - v_uv.y));
        float hasContent = step(0.01, terminalColor.a);

        // When keep_text_opaque is enabled (iTextOpacity = 1.0):
        //   - Content areas (text or colored bg): opacity = 1.0
        //   - Empty areas (default bg): opacity = iOpacity
        // When disabled (iTextOpacity = iOpacity):
        //   - Everything uses iOpacity
        float pixelOpacity = mix(iOpacity, iTextOpacity, hasContent);

        // Determine if we need to composite over a background color
        // This is needed for cursor shaders when no background shader is active
        float useSolidBg = step(0.01, iBackgroundColor.a);
        vec3 bgColor = iBackgroundColor.rgb * iBrightness;

        // Composite shader output over background color where there's no content
        // For areas with content (terminal text), use shader output directly
        // For empty areas, blend shader output over background
        float readabilityDim = mix(1.0, max(0.0, 1.0 - iReadability.y), iReadability.x * hasContent);
        vec3 readableShaderRgb = dimmedShaderRgb * readabilityDim;
        vec3 shaderOverBg = readableShaderRgb + bgColor * (1.0 - terminalColor.a);
        vec3 finalRgb = mix(readableShaderRgb, shaderOverBg, useSolidBg);

        // Detect chain mode (iOpacity ≈ 0 signals rendering to intermediate for another shader)
        float isChainMode = step(iOpacity, 0.001);

        // In chain mode: preserve full RGB, output hasContent as alpha for transparency detection
        // In final mode: apply pixelOpacity as normal (premultiplied output)
        vec3 chainRgb = finalRgb;
        float chainAlpha = hasContent;
        vec3 finalModeRgb = finalRgb * pixelOpacity;
        float finalModeAlpha = pixelOpacity;

        outColor = vec4(
            mix(finalModeRgb, chainRgb, isChainMode),
            mix(finalModeAlpha, chainAlpha, isChainMode)
        );
    }} else {{
        // Background-only mode: text is composited cleanly on top of shader background
        vec4 terminalColor = texture(iChannel4, vec2(v_uv.x, 1.0 - v_uv.y));

        // Terminal texture is premultiplied alpha (rgb already multiplied by alpha)
        // from GPU blending onto transparent background.
        // Scale by iTextOpacity to allow fading terminal content.
        vec3 srcPremul = terminalColor.rgb * iTextOpacity;
        float srcA = terminalColor.a * iTextOpacity;

        // Determine background color:
        // - If iBackgroundColor.a > 0, use it as solid background (with brightness applied)
        // - Otherwise, use shader output (dimmedShaderRgb) as background
        float useSolidBg = step(0.01, iBackgroundColor.a);
        float hasContent = step(0.01, terminalColor.a);
        float readabilityDim = mix(1.0, max(0.0, 1.0 - iReadability.y), iReadability.x * hasContent);
        vec3 readableShaderRgb = dimmedShaderRgb * readabilityDim;
        vec3 bgColor = mix(readableShaderRgb, iBackgroundColor.rgb * iBrightness, useSolidBg);

        // Detect chain mode (iOpacity ≈ 0 signals rendering to intermediate for another shader)
        float isChainMode = step(iOpacity, 0.001);

        // In chain mode: use full background color for RGB, terminal-only alpha
        // In final mode: use premultiplied background with full alpha compositing
        vec3 bgPremul = bgColor * iOpacity;
        float bgA = iOpacity;

        // RGB: in chain mode use full bgColor, in final mode use premultiplied
        vec3 effectiveBgRgb = mix(bgPremul, bgColor, isChainMode);

        // Standard "over" compositing with the effective background
        vec3 finalRgb = srcPremul + effectiveBgRgb * (1.0 - srcA);

        // Alpha: in chain mode preserve terminal alpha only (for transparency detection)
        // In final mode, composite with background opacity
        float finalA_chain = srcA;
        float finalA_final = srcA + bgA * (1.0 - srcA);
        float finalA = mix(finalA_final, finalA_chain, isChainMode);

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    )
}

/// Controls how the `@builtin(position)` parameter is injected into the generated WGSL
/// `fs_main` function signature.
///
/// The two public transpile functions differ only in where they place the builtin relative
/// to the `@location(0) v_uv` parameter, which is determined by which naga output pattern
/// they were originally tuned against.
#[derive(Debug, Clone, Copy)]
enum BuiltinPositionOrder {
    /// `@location(0) v_uv, @builtin(position) frag_pos` — append after location param
    After,
    /// `@builtin(position) frag_pos, @location(0) v_uv` — prepend before location param
    Before,
}

/// Perform the string replacement and return an error if the target was not found.
///
/// This validates that the naga-generated WGSL contains the exact pattern we expect so
/// that a naga version change does not silently produce a broken shader.
fn replace_required(source: &str, from: &str, to: &str, context: &str) -> Result<String> {
    if !source.contains(from) {
        return Err(anyhow::anyhow!(
            "WGSL post-processing failed: {} — target pattern not found in naga output.\n\
             Expected pattern: {:?}",
            context,
            from
        ));
    }
    Ok(source.replace(from, to))
}

/// Core transpilation logic shared by both public entry points.
///
/// # Arguments
/// * `glsl_source` – The raw (not yet preprocessed) user GLSL shader source.
/// * `name` – A human-readable name used in error messages (file path or synthetic name).
/// * `debug_glsl_filename` – Filename (not full path) for the optional debug GLSL dump.
/// * `builtin_order` – Controls `@builtin(position)` placement in the WGSL signature.
fn transpile_impl(
    glsl_source: &str,
    name: &str,
    debug_glsl_filename: &str,
    builtin_order: BuiltinPositionOrder,
) -> Result<String> {
    let glsl_source = preprocess_custom_control_uniforms(glsl_source);
    let glsl_source = preprocess_glsl_for_shadertoy(&glsl_source);
    let wrapped_glsl = glsl_wrapper_template(&glsl_source);

    // DEBUG: Write wrapped GLSL to file for inspection (debug builds only)
    #[cfg(debug_assertions)]
    {
        let debug_path = std::env::temp_dir().join(debug_glsl_filename);
        // Use restricted permissions (0o600) to prevent world-readable access on multi-user systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&debug_path)
                .and_then(|mut file| std::io::Write::write_all(&mut file, wrapped_glsl.as_bytes()));
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::write(&debug_path, &wrapped_glsl);
        }
    }
    // Suppress unused-variable warning in release builds
    #[cfg(not(debug_assertions))]
    let _ = debug_glsl_filename;

    // Parse GLSL using naga
    let mut parser = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);

    let module = parser.parse(&options, &wrapped_glsl).map_err(|errors| {
        let error_messages: Vec<String> = errors
            .errors
            .iter()
            .map(|e| format!("  {:?}", e.kind))
            .collect();
        anyhow::anyhow!(
            "GLSL parse error in '{}'. Errors:\n{}",
            name,
            error_messages.join("\n")
        )
    })?;

    // Validate the module
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| anyhow::anyhow!("Shader validation failed for '{}': {:?}", name, e))?;

    // Generate WGSL output for fragment shader
    let mut fragment_wgsl = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut fragment_wgsl, naga::back::wgsl::WriterFlags::empty());

    writer
        .write(&module, &info)
        .map_err(|e| anyhow::anyhow!("WGSL generation failed for '{}': {:?}", name, e))?;

    // Rename main() → fs_main() (naga always emits "main" for the entry point)
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Inject @builtin(position) into fs_main's parameter list.
    //
    // Naga may emit the @fragment attribute and fn on a single line or with a newline
    // between them. We handle both variants. Each replacement is validated — if the
    // expected pattern is absent, naga's output format changed and we return an error
    // rather than silently producing a broken shader (M-11 fix).
    let builtin_param = "@builtin(position) frag_pos: vec4<f32>";
    let location_param = "@location(0) v_uv: vec2<f32>";

    let (with_space_before, with_space_after, without_space_before, without_space_after) =
        match builtin_order {
            BuiltinPositionOrder::After => (
                // with newline between @fragment and fn
                format!("@fragment \nfn fs_main({location_param}) -> FragmentOutput {{",),
                format!(
                    "@fragment \nfn fs_main({location_param}, {builtin_param}) -> FragmentOutput {{",
                ),
                // without newline
                format!("@fragment\nfn fs_main({location_param}) -> FragmentOutput {{",),
                format!(
                    "@fragment\nfn fs_main({location_param}, {builtin_param}) -> FragmentOutput {{",
                ),
            ),
            BuiltinPositionOrder::Before => (
                format!("@fragment \nfn fs_main({location_param}) -> FragmentOutput {{",),
                format!(
                    "@fragment \nfn fs_main({builtin_param}, {location_param}) -> FragmentOutput {{",
                ),
                format!("@fragment\nfn fs_main({location_param}) -> FragmentOutput {{",),
                format!(
                    "@fragment\nfn fs_main({builtin_param}, {location_param}) -> FragmentOutput {{",
                ),
            ),
        };

    // Try the "with space" variant first; if not found, try the "without space" variant.
    // At least one must succeed — both failing means naga changed its output format.
    let fragment_wgsl = if fragment_wgsl.contains(&with_space_before) {
        replace_required(
            &fragment_wgsl,
            &with_space_before,
            &with_space_after,
            "@builtin(position) injection target not found in naga output",
        )?
    } else {
        replace_required(
            &fragment_wgsl,
            &without_space_before,
            &without_space_after,
            "@builtin(position) injection target not found in naga output",
        )?
    };

    // Seed gl_FragCoord_st with the raw @builtin(position) coordinates immediately after
    // the v_uv assignment. The actual Y-flip is applied inside mainImage.
    let uv_assign_target = "v_uv_1 = v_uv;";
    let uv_assign_replacement = "v_uv_1 = v_uv;\n    // Seed gl_FragCoord_st with raw @builtin(position)\n    gl_FragCoord_st = vec2<f32>(frag_pos.x, frag_pos.y);";
    let fragment_wgsl = replace_required(
        &fragment_wgsl,
        uv_assign_target,
        uv_assign_replacement,
        "gl_FragCoord_st seeding target ('v_uv_1 = v_uv;') not found in naga output",
    )?;

    // Build the complete shader with vertex shader
    let full_wgsl = format!(
        r#"// Auto-generated WGSL from GLSL shader: {name}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {{
    var out: VertexOutput;

    // Generate full-screen quad vertices (triangle strip)
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Full screen in NDC - standard orientation
    // Y-flip is handled in fragment shader via gl_FragCoord_st
    out.position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}}

// ============ Fragment shader (transpiled from GLSL) ============

{fragment_wgsl}
"#,
    );

    Ok(full_wgsl)
}

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL
///
/// Supports standard Shadertoy uniforms:
/// - `iTime`: Current time in seconds
/// - `iResolution`: Viewport resolution (width, height, 1.0)
/// - `iMouse`: Mouse state (xy=current, zw=click)
/// - `iChannel0-3`: User-defined texture channels (Shadertoy compatible)
/// - `iChannel4`: Terminal content texture
/// - `iChannelResolution[0-4]`: Channel texture resolutions
///
/// Key press uniform (par-term specific):
/// - `iTimeKeyPress`: Time when last key was pressed (same timebase as iTime)
///
/// Ghostty-compatible cursor uniforms (v1.2.0+):
/// - `iCurrentCursor`: xy=position, zw=size (pixels)
/// - `iPreviousCursor`: xy=previous position, zw=size
/// - `iCurrentCursorColor`: RGBA (opacity baked into alpha)
/// - `iPreviousCursorColor`: RGBA previous color
/// - `iTimeCursorChange`: Time when cursor last moved
///
/// Cursor shader configuration uniforms (par-term specific):
/// - `iCursorShaderColor`: User-configured cursor color for effects (RGBA)
/// - `iCursorTrailDuration`: Trail effect duration in seconds
/// - `iCursorGlowRadius`: Glow effect radius in pixels
/// - `iCursorGlowIntensity`: Glow effect intensity (0.0-1.0)
///
/// Terminal-aware context uniforms (par-term specific):
/// - `iCommand`: x=state(0 unknown, 1 running, 2 success, 3 failure), y=exit code, z=event time, w=running flag
/// - `iFocusedPane`: xy=focused pane bottom-left pixel origin, zw=focused pane size
/// - `iScroll`: x=scroll offset, y=visible lines, z=scrollback lines, w=normalized depth
pub(crate) fn transpile_glsl_to_wgsl(glsl_source: &str, shader_path: &Path) -> Result<String> {
    transpile_impl(
        glsl_source,
        &shader_path.display().to_string(),
        "par_term_debug_wrapped.glsl",
        BuiltinPositionOrder::After,
    )
}

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL from source string
///
/// Same as `transpile_glsl_to_wgsl` but takes a source string and name instead of a file path.
pub(crate) fn transpile_glsl_to_wgsl_source(glsl_source: &str, name: &str) -> Result<String> {
    transpile_impl(
        glsl_source,
        name,
        "par_term_debug_wrapped_source.glsl",
        BuiltinPositionOrder::Before,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_exposes_background_blend_mode_uniform_and_constants() {
        let source = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(float(iBackgroundBlendMode + BACKGROUND_BLEND_REPLACE + BACKGROUND_BLEND_MULTIPLY + BACKGROUND_BLEND_SCREEN + BACKGROUND_BLEND_OVERLAY + BACKGROUND_BLEND_LUMINANCE_MASK));
}
"#;
        let wgsl = transpile_glsl_to_wgsl_source(source, "background_blend_mode_test")
            .expect("transpile should succeed");

        assert!(wgsl.contains("iBackgroundBlendMode") || wgsl.contains("iBackgroundChannel"));
    }

    #[test]
    fn builtin_terminal_context_uniforms_are_declared_in_wrapper() {
        let wgsl = transpile_glsl_to_wgsl_source(
            r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iCommand.x + iFocusedPane.z + iScroll.w);
}
"#,
            "terminal_context_uniforms_test",
        )
        .expect("shader should transpile with built-in terminal context uniforms");

        assert!(wgsl.contains("iCommand"));
        assert!(wgsl.contains("iFocusedPane"));
        assert!(wgsl.contains("iScroll"));
    }

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
    fn transpiled_controlled_uniform_shader_mentions_custom_uniform_block() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), 1.0);
}
"#;

        let wgsl = transpile_glsl_to_wgsl_source(source, "controlled_test").unwrap();

        assert!(wgsl.contains("iCustomFloatUniforms") || wgsl.contains("custom"));
    }

    #[test]
    fn transpiled_malformed_attached_controlled_uniform_uses_safe_fallback() {
        let source = r#"
// control slider min=0 max=1
uniform float iGlow;
// control radio
uniform bool iEnabled;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), iEnabled ? 1.0 : 0.0);
}
"#;

        let wgsl = transpile_glsl_to_wgsl_source(source, "malformed_controlled_test").unwrap();

        assert!(wgsl.contains("0.0") || wgsl.contains("false"));
    }

    #[test]
    fn transpiled_over_limit_controlled_uniform_uses_safe_fallback() {
        let mut source = String::new();
        for index in 0..17 {
            source.push_str("// control slider min=0.25 max=1 step=0.01\n");
            source.push_str(&format!("uniform float iFloat{index};\n"));
        }
        source.push_str(
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(vec3(iFloat16), 1.0); }\n",
        );

        let wgsl = transpile_glsl_to_wgsl_source(&source, "over_limit_controlled_test").unwrap();

        assert!(wgsl.contains("0.25"));
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

        let wgsl = transpile_glsl_to_wgsl_source(&source, "over_limit_color_test").unwrap();

        assert!(wgsl.contains("vec3") || wgsl.contains("1.0"));
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

        let wgsl = transpile_glsl_to_wgsl_source(source, "malformed_color_test").unwrap();

        assert!(wgsl.contains("vec3") || wgsl.contains("1.0"));
    }
}
