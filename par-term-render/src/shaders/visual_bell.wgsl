// Visual bell shader
//
// Renders a fullscreen flash overlay for the terminal bell (\x07).
// Uses premultiplied alpha blending for smooth fade-out.

struct Uniforms {
    position: vec2<f32>,  // NDC position (-1, -1 for fullscreen)
    size: vec2<f32>,      // NDC size (2, 2 for fullscreen)
    color: vec4<f32>,     // RGBA (alpha = intensity 0.0-1.0)
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen quad vertices (triangle strip)
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Transform to position and size
    let pos = vec2<f32>(
        uniforms.position.x + x * uniforms.size.x,
        uniforms.position.y + y * uniforms.size.y
    );

    out.position = vec4<f32>(pos, 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Output premultiplied color for PreMultiplied composite alpha mode
    let alpha = uniforms.color.a;
    return vec4<f32>(uniforms.color.rgb * alpha, alpha);
}
