// Cell text shader - renders glyphs from texture atlas

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(2) position: vec2<f32>,    // Glyph position in NDC
    @location(3) size: vec2<f32>,        // Glyph size in NDC
    @location(4) tex_offset: vec2<f32>,  // Texture offset (normalized 0-1)
    @location(5) tex_size: vec2<f32>,    // Texture size (normalized 0-1)
    @location(6) color: vec4<f32>,       // Foreground color
    @location(7) is_colored: u32,        // 1 if colored (emoji), 0 if monochrome
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) is_colored: u32,
}

@group(0) @binding(0)
var glyph_texture: texture_2d<f32>;

@group(0) @binding(1)
var glyph_sampler: sampler;

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

    // Calculate texture coordinates
    out.tex_coord = vec2<f32>(
        input.tex_offset.x + x * input.tex_size.x,
        input.tex_offset.y + y * input.tex_size.y
    );

    out.color = input.color;
    out.is_colored = input.is_colored;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph from atlas (RGBA for colored emoji support)
    let glyph = textureSample(glyph_texture, glyph_sampler, input.tex_coord);

    // Output straight (non-premultiplied) colors for PostMultiplied alpha mode
    if (input.is_colored == 1u) {
        // Colored glyph (emoji) - use glyph color with combined alpha
        return vec4<f32>(glyph.rgb, glyph.a * input.color.a);
    } else {
        // Monochrome glyph - use foreground color with glyph alpha mask
        return vec4<f32>(input.color.rgb, input.color.a * glyph.a);
    }
}
