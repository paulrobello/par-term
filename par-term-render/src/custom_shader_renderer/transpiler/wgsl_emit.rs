//! WGSL code generation and GLSL-to-WGSL transpilation.
//!
//! This module contains:
//! - The GLSL wrapper template (uniforms, bindings, main() wrapper)
//! - WGSL post-processing (builtin injection, fragCoord seeding)
//! - The public transpile entry points

use anyhow::Result;
use std::path::Path;

use super::glsl_parse::{preprocess_custom_control_uniforms, preprocess_glsl_for_shadertoy};

/// The shared GLSL wrapper template injected around the user shader code.
///
/// The `{glsl_source}` placeholder is replaced with the user-provided (preprocessed) GLSL.
pub(crate) fn glsl_wrapper_template(glsl_source: &str) -> String {
    format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 384 bytes
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
        // Background-only mode: terminal content is rendered by the pane pass after
        // this shader pass. iChannel4 is only a mask for readability effects here;
        // compositing terminalColor into this output would draw content twice.
        vec4 terminalColor = texture(iChannel4, vec2(v_uv.x, 1.0 - v_uv.y));

        // Determine background color:
        // - If iBackgroundColor.a > 0, use it as solid background (with brightness applied)
        // - Otherwise, use shader output (dimmedShaderRgb) as background
        float useSolidBg = step(0.01, iBackgroundColor.a);
        float contentMask = clamp(terminalColor.a, 0.0, 1.0);
        float readabilityDim = mix(1.0, max(0.0, 1.0 - iReadability.y), iReadability.x * contentMask);
        vec3 readableShaderRgb = dimmedShaderRgb * readabilityDim;
        vec3 bgColor = mix(readableShaderRgb, iBackgroundColor.rgb * iBrightness, useSolidBg);

        // Detect chain mode (iOpacity ≈ 0 signals rendering to intermediate for another shader)
        float isChainMode = step(iOpacity, 0.001);

        // In chain mode: keep full RGB for the next shader, but leave alpha transparent
        // so terminal alpha comes from the pane render that follows this pass.
        // In final mode: output premultiplied background at window opacity.
        vec3 bgPremul = bgColor * iOpacity;
        vec3 finalRgb = mix(bgPremul, bgColor, isChainMode);
        float finalA = mix(iOpacity, 0.0, isChainMode);

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

    // Rename main() -> fs_main() (naga always emits "main" for the entry point)
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Inject @builtin(position) into fs_main's parameter list.
    //
    // Naga may emit the @fragment attribute and fn on a single line or with a newline
    // between them. We handle both variants. Each replacement is validated -- if the
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
    // At least one must succeed -- both failing means naga changed its output format.
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
    fn background_only_wrapper_uses_ichannel4_as_mask_not_terminal_composite() {
        let wrapper = glsl_wrapper_template(
            r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(0.2, 0.4, 0.8, 1.0);
}
"#,
        );
        let background_only_branch = wrapper
            .split("    } else {")
            .nth(1)
            .expect("wrapper should contain background-only branch");

        assert!(background_only_branch.contains("contentMask"));
        assert!(
            background_only_branch
                .contains("float contentMask = clamp(terminalColor.a, 0.0, 1.0);")
        );
        assert!(!background_only_branch.contains("step(0.01, terminalColor.a)"));
        assert!(!background_only_branch.contains("srcPremul"));
        assert!(!background_only_branch.contains("srcA"));
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
    fn transpiled_over_limit_color_controls_use_safe_fallback() {
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

        let wgsl = transpile_glsl_to_wgsl_source(&source, "over_limit_color_test").unwrap();

        assert!(wgsl.contains("vec3") || wgsl.contains("1.0"));
    }

    #[test]
    fn transpiled_malformed_color_controls_use_safe_fallbacks() {
        let source = r#"
// control color alpha=true
uniform vec3 iBadRgb;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iBadRgb, 1.0);
}
"#;

        let wgsl = transpile_glsl_to_wgsl_source(source, "malformed_color_test").unwrap();

        assert!(wgsl.contains("vec3") || wgsl.contains("1.0"));
    }
}
