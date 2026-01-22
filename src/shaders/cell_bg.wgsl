// Cell background shader - renders colored quads for each cell

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(2) position: vec2<f32>,  // Cell position in NDC
    @location(3) size: vec2<f32>,      // Cell size in NDC
    @location(4) color: vec4<f32>,     // Background color
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices (triangle strip)
    let x = f32(input.vertex_index & 1u);
    let y = f32((input.vertex_index >> 1u) & 1u);

    // Calculate vertex position using actual glyph size in NDC
    let pos = vec2<f32>(
        input.position.x + x * input.size.x,
        input.position.y - y * input.size.y
    );

    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.color = input.color;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
