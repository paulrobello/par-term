// Sixel graphics shader - renders RGBA images at terminal cell positions

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec2<f32>,    // Image position in screen space (normalized 0-1)
    @location(1) tex_coords: vec4<f32>,  // Texture coordinates (x, y, w, h) - normalized 0-1
    @location(2) size: vec2<f32>,        // Image size in screen space (normalized 0-1)
    @location(3) alpha: f32,             // Global alpha multiplier (for fade effects)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) alpha: f32,
}

@group(0) @binding(0)
var sixel_texture: texture_2d<f32>;

@group(0) @binding(1)
var sixel_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices (triangle strip)
    let x = f32(input.vertex_index & 1u);
    let y = f32((input.vertex_index >> 1u) & 1u);

    // Calculate vertex position using actual image size in screen space
    let pos = vec2<f32>(
        input.position.x + x * input.size.x,
        input.position.y + y * input.size.y
    );

    // Convert to NDC (-1 to 1)
    out.position = vec4<f32>(pos.x * 2.0 - 1.0, 1.0 - pos.y * 2.0, 0.0, 1.0);

    // Calculate texture coordinates
    out.tex_coord = vec2<f32>(
        input.tex_coords.x + x * input.tex_coords.z,
        input.tex_coords.y + y * input.tex_coords.w
    );

    out.alpha = input.alpha;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample sixel image (full RGBA)
    let color = textureSample(sixel_texture, sixel_sampler, input.tex_coord);

    // Apply global alpha multiplier and output premultiplied colors
    // for PreMultiplied composite alpha mode
    let final_alpha = color.a * input.alpha;
    return vec4<f32>(color.rgb * final_alpha, final_alpha);
}
