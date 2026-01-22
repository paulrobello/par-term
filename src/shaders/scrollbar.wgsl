// Scrollbar shader

struct Uniforms {
    position: vec2<f32>,  // Position in NDC
    size: vec2<f32>,      // Size in NDC
    color: vec4<f32>,     // RGBA color
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Transform to scrollbar position and size
    let pos = vec2<f32>(
        uniforms.position.x + x * uniforms.size.x,
        uniforms.position.y + y * uniforms.size.y
    );

    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple rounded corners at left and right edges
    let edge_radius = 0.3;  // Radius for rounded corners
    let edge_x = min(in.uv.x, 1.0 - in.uv.x);  // Distance from left or right edge

    // Only apply rounding at the left and right edges
    var alpha = uniforms.color.a;
    if edge_x < edge_radius {
        let edge_dist = edge_x / edge_radius;
        alpha *= smoothstep(0.0, 0.5, edge_dist);
    }

    return vec4<f32>(uniforms.color.rgb, alpha);
}
