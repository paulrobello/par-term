use anyhow::Result;
use std::path::Path;

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL
///
/// Supports standard Shadertoy uniforms:
/// - `iTime`: Current time in seconds
/// - `iResolution`: Viewport resolution (width, height, 1.0)
/// - `iMouse`: Mouse state (xy=current, zw=click)
/// - `iChannel0`: Terminal content texture
/// - `iChannel1-4`: User-defined texture channels
/// - `iChannelResolution[0-4]`: Channel texture resolutions
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
    float _pad1;           // offset 76, size 4 - Padding

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
}};                            // total: 256 bytes

// Shadertoy-compatible iChannelResolution array accessor
// Usage: iChannelResolution[0].xyz, iChannelResolution[1].xy, etc.
vec3 iChannelResolution[5] = vec3[5](
    iChannelResolution0.xyz,
    iChannelResolution1.xyz,
    iChannelResolution2.xyz,
    iChannelResolution3.xyz,
    iChannelResolution4.xyz
);

// Terminal content texture (iChannel0)
layout(set = 0, binding = 1) uniform texture2D _iChannel0Tex;
layout(set = 0, binding = 2) uniform sampler _iChannel0Sampler;

// User-defined texture channels (iChannel1-4)
layout(set = 0, binding = 3) uniform texture2D _iChannel1Tex;
layout(set = 0, binding = 4) uniform sampler _iChannel1Sampler;
layout(set = 0, binding = 5) uniform texture2D _iChannel2Tex;
layout(set = 0, binding = 6) uniform sampler _iChannel2Sampler;
layout(set = 0, binding = 7) uniform texture2D _iChannel3Tex;
layout(set = 0, binding = 8) uniform sampler _iChannel3Sampler;
layout(set = 0, binding = 9) uniform texture2D _iChannel4Tex;
layout(set = 0, binding = 10) uniform sampler _iChannel4Sampler;

// Combined samplers for texture() calls
#define iChannel0 sampler2D(_iChannel0Tex, _iChannel0Sampler)
#define iChannel1 sampler2D(_iChannel1Tex, _iChannel1Sampler)
#define iChannel2 sampler2D(_iChannel2Tex, _iChannel2Sampler)
#define iChannel3 sampler2D(_iChannel3Tex, _iChannel3Sampler)
#define iChannel4 sampler2D(_iChannel4Tex, _iChannel4Sampler)

// Input from vertex shader
layout(location = 0) in vec2 v_uv;

// Output color
layout(location = 0) out vec4 outColor;

// ============ User shader code begins ============

{glsl_source}

// ============ User shader code ends ============

void main() {{
    vec2 fragCoord = v_uv * iResolution;
    vec4 shaderColor;
    mainImage(shaderColor, fragCoord);

    // Apply brightness multiplier to shader background (not text)
    vec3 dimmedShaderRgb = shaderColor.rgb * iBrightness;

    if (iFullContent > 0.5) {{
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel0
        // Apply window opacity to the shader's alpha output
        outColor = vec4(dimmedShaderRgb * iOpacity, shaderColor.a * iOpacity);
    }} else {{
        // Background-only mode: text is composited cleanly on top
        // Sample terminal to detect text pixels
        vec4 terminalColor = texture(iChannel0, v_uv);
        float hasText = step(0.01, terminalColor.a);

        // Text pixels: use terminal color with text opacity
        // Background pixels: use dimmed shader output with window opacity
        vec3 textCol = terminalColor.rgb;
        vec3 bgCol = dimmedShaderRgb;

        // Composite: text over shader background
        float textA = hasText * iTextOpacity;
        float bgA = (1.0 - hasText) * iOpacity;

        vec3 finalRgb = textCol * textA + bgCol * bgA;
        float finalA = textA + bgA;

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

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

    // Full screen in NDC
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
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
    float _pad1;           // offset 76, size 4 - Padding

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
}};                            // total: 256 bytes

// Shadertoy-compatible iChannelResolution array accessor
// Usage: iChannelResolution[0].xyz, iChannelResolution[1].xy, etc.
vec3 iChannelResolution[5] = vec3[5](
    iChannelResolution0.xyz,
    iChannelResolution1.xyz,
    iChannelResolution2.xyz,
    iChannelResolution3.xyz,
    iChannelResolution4.xyz
);

// Terminal content texture (iChannel0)
layout(set = 0, binding = 1) uniform texture2D _iChannel0Tex;
layout(set = 0, binding = 2) uniform sampler _iChannel0Sampler;

// User-defined texture channels (iChannel1-4)
layout(set = 0, binding = 3) uniform texture2D _iChannel1Tex;
layout(set = 0, binding = 4) uniform sampler _iChannel1Sampler;
layout(set = 0, binding = 5) uniform texture2D _iChannel2Tex;
layout(set = 0, binding = 6) uniform sampler _iChannel2Sampler;
layout(set = 0, binding = 7) uniform texture2D _iChannel3Tex;
layout(set = 0, binding = 8) uniform sampler _iChannel3Sampler;
layout(set = 0, binding = 9) uniform texture2D _iChannel4Tex;
layout(set = 0, binding = 10) uniform sampler _iChannel4Sampler;

// Combined samplers for texture() calls
#define iChannel0 sampler2D(_iChannel0Tex, _iChannel0Sampler)
#define iChannel1 sampler2D(_iChannel1Tex, _iChannel1Sampler)
#define iChannel2 sampler2D(_iChannel2Tex, _iChannel2Sampler)
#define iChannel3 sampler2D(_iChannel3Tex, _iChannel3Sampler)
#define iChannel4 sampler2D(_iChannel4Tex, _iChannel4Sampler)

// Input from vertex shader
layout(location = 0) in vec2 v_uv;

// Output color
layout(location = 0) out vec4 outColor;

// ============ User shader code begins ============

{glsl_source}

// ============ User shader code ends ============

void main() {{
    vec2 fragCoord = v_uv * iResolution;
    vec4 shaderColor;
    mainImage(shaderColor, fragCoord);

    // Apply brightness multiplier to shader background (not text)
    vec3 dimmedShaderRgb = shaderColor.rgb * iBrightness;

    if (iFullContent > 0.5) {{
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel0
        // Apply window opacity to the shader's alpha output
        outColor = vec4(dimmedShaderRgb * iOpacity, shaderColor.a * iOpacity);
    }} else {{
        // Background-only mode: text is composited cleanly on top
        // Sample terminal to detect text pixels
        vec4 terminalColor = texture(iChannel0, v_uv);
        float hasText = step(0.01, terminalColor.a);

        // Text pixels: use terminal color with text opacity
        // Background pixels: use dimmed shader output with window opacity
        vec3 textCol = terminalColor.rgb;
        vec3 bgCol = dimmedShaderRgb;

        // Composite: text over shader background
        float textA = hasText * iTextOpacity;
        float bgA = (1.0 - hasText) * iOpacity;

        vec3 finalRgb = textCol * textA + bgCol * bgA;
        float finalA = textA + bgA;

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

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

    // Full screen in NDC
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
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
