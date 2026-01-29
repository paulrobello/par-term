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
pub(crate) fn transpile_glsl_to_wgsl(glsl_source: &str, shader_path: &Path) -> Result<String> {
    let glsl_source = preprocess_glsl_for_shadertoy(glsl_source);

    // Wrap the Shadertoy-style shader in a proper GLSL fragment shader
    // We need to:
    // 1. Add version and precision qualifiers
    // 2. Declare uniforms and samplers
    // 3. Add input/output declarations
    // 4. Add a main() that calls mainImage()

    let wrapped_glsl = format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 256 bytes
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
}};                            // total: 288 bytes

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
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel4
        //
        // For keep_text_opaque (iTextOpacity = 1.0):
        // - Where terminal has content (text or colored bg): keep opaque
        // - Where terminal is empty (default bg): apply window opacity
        //
        // The terminal texture already has correct alpha values:
        // - Default backgrounds: alpha ≈ 0 (skipped by cell renderer)
        // - Colored backgrounds: alpha = 1.0 (when transparency_affects_only_default_background)
        // - Text: alpha = 1.0
        vec4 terminalColor = texture(iChannel4, vec2(v_uv.x, 1.0 - v_uv.y));
        float hasContent = step(0.01, terminalColor.a);

        // When keep_text_opaque is enabled (iTextOpacity = 1.0):
        //   - Content areas (text + colored bg): opacity = 1.0
        //   - Empty areas (default bg): opacity = iOpacity
        // When disabled (iTextOpacity = iOpacity):
        //   - Everything uses iOpacity
        float pixelOpacity = mix(iOpacity, iTextOpacity, hasContent);

        // Determine background: solid color, image (iChannel0), or none
        float useSolidBg = step(0.01, iBackgroundColor.a);
        // Check if background image is in iChannel0 (resolution > 1x1 means real texture)
        float useImageBg = step(2.0, iChannelResolution0.x) * (1.0 - useSolidBg);

        // Sample background image if available
        vec3 imageBgRgb = texture(iChannel0, vec2(v_uv.x, 1.0 - v_uv.y)).rgb * iBrightness;
        vec3 solidBgRgb = iBackgroundColor.rgb * iBrightness;

        // Select background: solid color takes priority, then image, then black
        vec3 bgRgb = mix(mix(vec3(0.0), imageBgRgb, useImageBg), solidBgRgb, useSolidBg);
        float hasBg = max(useSolidBg, useImageBg);

        // Properly composite terminal over background to fix text edge artifacts
        // Terminal texture is premultiplied alpha, so: out = term.rgb + bg * (1 - term.a)
        vec3 termOverBg = terminalColor.rgb + bgRgb * (1.0 - terminalColor.a);
        vec3 termOverBlack = terminalColor.rgb; // Original behavior when no background
        vec3 termComposited = mix(termOverBlack, termOverBg, hasBg);

        // Extract shader's glow effect (cursor glow, etc.)
        // Glow = shader output minus terminal contribution
        // This preserves cursor effects while allowing proper text compositing
        vec3 glowEffect = max(dimmedShaderRgb - terminalColor.rgb, vec3(0.0));

        // Final color: properly composited terminal + glow effects
        vec3 finalRgb = termComposited + glowEffect;

        outColor = vec4(finalRgb * pixelOpacity, pixelOpacity);
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
        vec3 bgColor = mix(dimmedShaderRgb, iBackgroundColor.rgb * iBrightness, useSolidBg);

        // Background with window opacity (premultiplied)
        vec3 bgPremul = bgColor * iOpacity;
        float bgA = iOpacity;

        // Standard "over" compositing with premultiplied source and dest:
        // out.rgb = src.rgb + dst.rgb * (1 - src.a)
        // out.a = src.a + dst.a * (1 - src.a)
        vec3 finalRgb = srcPremul + bgPremul * (1.0 - srcA);
        float finalA = srcA + bgA * (1.0 - srcA);

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

    // No post-replacements needed; coordinates stay raw and are flipped in mainImage.

    // DEBUG: Write wrapped GLSL to file for inspection
    let _ = std::fs::write("/tmp/par_term_debug_wrapped.glsl", &wrapped_glsl);

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
            shader_path.display(),
            error_messages.join("\n")
        )
    })?;

    // Validate the module
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| {
        anyhow::anyhow!(
            "Shader validation failed for '{}': {:?}",
            shader_path.display(),
            e
        )
    })?;

    // Generate WGSL output for fragment shader
    let mut fragment_wgsl = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut fragment_wgsl, naga::back::wgsl::WriterFlags::empty());

    writer.write(&module, &info).map_err(|e| {
        anyhow::anyhow!(
            "WGSL generation failed for '{}': {:?}",
            shader_path.display(),
            e
        )
    })?;

    // The generated WGSL will have a main() function but we need to rename it to fs_main
    // and add a vertex shader
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Add @builtin(position) to fs_main and seed gl_FragCoord_st with raw coords
    // (wgpu origin: y=0 at top). mainImage will perform the flip locally.
    let fragment_wgsl = fragment_wgsl.replace(
        "@fragment \nfn fs_main(@location(0) v_uv: vec2<f32>) -> FragmentOutput {",
        "@fragment \nfn fs_main(@location(0) v_uv: vec2<f32>, @builtin(position) frag_pos: vec4<f32>) -> FragmentOutput {"
    );

    // Also handle the case without the newline
    let fragment_wgsl = fragment_wgsl.replace(
        "@fragment\nfn fs_main(@location(0) v_uv: vec2<f32>) -> FragmentOutput {",
        "@fragment\nfn fs_main(@location(0) v_uv: vec2<f32>, @builtin(position) frag_pos: vec4<f32>) -> FragmentOutput {"
    );

    // Insert code after v_uv_1 assignment to seed gl_FragCoord_st with raw coords
    // (no flip); mainImage handles the flip locally.
    let fragment_wgsl = fragment_wgsl.replace(
        "v_uv_1 = v_uv;",
        "v_uv_1 = v_uv;\n    // Seed gl_FragCoord_st with raw @builtin(position)\n    gl_FragCoord_st = vec2<f32>(frag_pos.x, frag_pos.y);"
    );

    // Y-flip is ensured in GLSL preprocessing (preprocess_glsl_for_shadertoy)

    // Build the complete shader with vertex shader
    let full_wgsl = format!(
        r#"// Auto-generated WGSL from GLSL shader: {}

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
        shader_path.display()
    );

    Ok(full_wgsl)
}

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL from source string
///
/// Same as `transpile_glsl_to_wgsl` but takes a source string and name instead of a file path.
pub(crate) fn transpile_glsl_to_wgsl_source(glsl_source: &str, name: &str) -> Result<String> {
    let glsl_source = preprocess_glsl_for_shadertoy(glsl_source);

    // Wrap the Shadertoy-style shader in a proper GLSL fragment shader
    let wrapped_glsl = format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 256 bytes
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
}};                            // total: 288 bytes

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

    // Flip once here (wgpu y=0 top -> Shadertoy y=0 bottom)
    vec2 st_fragCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);
    gl_FragCoord_st = st_fragCoord;
    vec4 shaderColor;
    mainImage(shaderColor, st_fragCoord);

    // Apply brightness multiplier to shader background (not text)
    vec3 dimmedShaderRgb = shaderColor.rgb * iBrightness;

    if (iFullContent > 0.5) {{
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel4
        //
        // For keep_text_opaque (iTextOpacity = 1.0):
        // - Where terminal has content (text or colored bg): keep opaque
        // - Where terminal is empty (default bg): apply window opacity
        //
        // The terminal texture already has correct alpha values:
        // - Default backgrounds: alpha ≈ 0 (skipped by cell renderer)
        // - Colored backgrounds: alpha = 1.0 (when transparency_affects_only_default_background)
        // - Text: alpha = 1.0
        vec4 terminalColor = texture(iChannel4, vec2(v_uv.x, 1.0 - v_uv.y));
        float hasContent = step(0.01, terminalColor.a);

        // When keep_text_opaque is enabled (iTextOpacity = 1.0):
        //   - Content areas (text + colored bg): opacity = 1.0
        //   - Empty areas (default bg): opacity = iOpacity
        // When disabled (iTextOpacity = iOpacity):
        //   - Everything uses iOpacity
        float pixelOpacity = mix(iOpacity, iTextOpacity, hasContent);

        // Determine background: solid color, image (iChannel0), or none
        float useSolidBg = step(0.01, iBackgroundColor.a);
        // Check if background image is in iChannel0 (resolution > 1x1 means real texture)
        float useImageBg = step(2.0, iChannelResolution0.x) * (1.0 - useSolidBg);

        // Sample background image if available
        vec3 imageBgRgb = texture(iChannel0, vec2(v_uv.x, 1.0 - v_uv.y)).rgb * iBrightness;
        vec3 solidBgRgb = iBackgroundColor.rgb * iBrightness;

        // Select background: solid color takes priority, then image, then black
        vec3 bgRgb = mix(mix(vec3(0.0), imageBgRgb, useImageBg), solidBgRgb, useSolidBg);
        float hasBg = max(useSolidBg, useImageBg);

        // Properly composite terminal over background to fix text edge artifacts
        // Terminal texture is premultiplied alpha, so: out = term.rgb + bg * (1 - term.a)
        vec3 termOverBg = terminalColor.rgb + bgRgb * (1.0 - terminalColor.a);
        vec3 termOverBlack = terminalColor.rgb; // Original behavior when no background
        vec3 termComposited = mix(termOverBlack, termOverBg, hasBg);

        // Extract shader's glow effect (cursor glow, etc.)
        // Glow = shader output minus terminal contribution
        // This preserves cursor effects while allowing proper text compositing
        vec3 glowEffect = max(dimmedShaderRgb - terminalColor.rgb, vec3(0.0));

        // Final color: properly composited terminal + glow effects
        vec3 finalRgb = termComposited + glowEffect;

        outColor = vec4(finalRgb * pixelOpacity, pixelOpacity);
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
        vec3 bgColor = mix(dimmedShaderRgb, iBackgroundColor.rgb * iBrightness, useSolidBg);

        // Background with window opacity (premultiplied)
        vec3 bgPremul = bgColor * iOpacity;
        float bgA = iOpacity;

        // Standard "over" compositing with premultiplied source and dest:
        // out.rgb = src.rgb + dst.rgb * (1 - src.a)
        // out.a = src.a + dst.a * (1 - src.a)
        vec3 finalRgb = srcPremul + bgPremul * (1.0 - srcA);
        float finalA = srcA + bgA * (1.0 - srcA);

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

    // DEBUG: Write wrapped GLSL to file for inspection
    let _ = std::fs::write("/tmp/par_term_debug_wrapped.glsl", &wrapped_glsl);

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

    // The generated WGSL will have a main() function but we need to rename it to fs_main
    // and add a vertex shader
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Add @builtin(position) to fs_main and seed gl_FragCoord_st with raw coords
    // (wgpu origin: y=0 at top). mainImage will perform the flip locally.
    let fragment_wgsl = fragment_wgsl.replace(
        "@fragment \nfn fs_main(@location(0) v_uv: vec2<f32>) -> FragmentOutput {",
        "@fragment \nfn fs_main(@builtin(position) frag_pos: vec4<f32>, @location(0) v_uv: vec2<f32>) -> FragmentOutput {"
    );

    // Also handle the case without the newline
    let fragment_wgsl = fragment_wgsl.replace(
        "@fragment\nfn fs_main(@location(0) v_uv: vec2<f32>) -> FragmentOutput {",
        "@fragment\nfn fs_main(@builtin(position) frag_pos: vec4<f32>, @location(0) v_uv: vec2<f32>) -> FragmentOutput {"
    );

    // Insert code after v_uv_1 assignment to seed gl_FragCoord_st with raw coords
    // (no flip here; flip happens inside mainImage).
    let fragment_wgsl = fragment_wgsl.replace(
        "v_uv_1 = v_uv;",
        "v_uv_1 = v_uv;\n    // Seed gl_FragCoord_st with raw @builtin(position)\n    gl_FragCoord_st = vec2<f32>(frag_pos.x, frag_pos.y);"
    );

    // Y-flip is handled in GLSL preprocessing (preprocess_glsl_for_shadertoy)

    // Build the complete shader with vertex shader
    let full_wgsl = format!(
        r#"// Auto-generated WGSL from GLSL shader: {}

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
        name
    );

    Ok(full_wgsl)
}
