// Background image shader - renders a background image with various display modes

struct Uniforms {
    // Image dimensions (original)
    image_size: vec2<f32>,
    // Window/pane dimensions (pane size for per-pane, window size for global)
    window_size: vec2<f32>,
    // Display mode: 0=fit, 1=fill, 2=stretch, 3=tile, 4=center
    mode: u32,
    // Opacity
    opacity: f32,
    // Pane offset in pixels (0,0 for global background)
    pane_offset: vec2<f32>,
    // Surface (window) size in pixels (same as window_size for global)
    surface_size: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@group(0) @binding(0)
var bg_texture: texture_2d<f32>;

@group(0) @binding(1)
var bg_sampler: sampler;

@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices (triangle strip)
    // x and y go 0..1 across the pane area
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Convert pane pixel coordinates to NDC (-1..1)
    // pane_offset is (0,0) for global backgrounds, so this reduces to full-screen
    let px = uniforms.pane_offset.x + x * uniforms.window_size.x;
    let py = uniforms.pane_offset.y + y * uniforms.window_size.y;
    let ndc_x = (px / uniforms.surface_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / uniforms.surface_size.y) * 2.0;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    // Calculate texture coordinates based on mode
    // x and y are 0..1 across the pane, so UV math works correctly
    let mode = uniforms.mode;
    let img_aspect = uniforms.image_size.x / uniforms.image_size.y;
    let win_aspect = uniforms.window_size.x / uniforms.window_size.y;

    if mode == 0u {
        // Fit: scale to fit, maintaining aspect ratio (may have letterboxing)
        if win_aspect > img_aspect {
            // Window is wider than image - letterbox on sides
            let scale = img_aspect / win_aspect;
            let offset = (1.0 - scale) / 2.0;
            out.tex_coord = vec2<f32>((x - offset) / scale, y);
        } else {
            // Window is taller than image - letterbox on top/bottom
            let scale = win_aspect / img_aspect;
            let offset = (1.0 - scale) / 2.0;
            out.tex_coord = vec2<f32>(x, (y - offset) / scale);
        }
    } else if mode == 1u {
        // Fill: scale to fill, maintaining aspect ratio (may crop)
        if win_aspect > img_aspect {
            // Window is wider - crop top/bottom
            let scale = win_aspect / img_aspect;
            let offset = (scale - 1.0) / 2.0 / scale;
            out.tex_coord = vec2<f32>(x, y / scale + offset);
        } else {
            // Window is taller - crop sides
            let scale = img_aspect / win_aspect;
            let offset = (scale - 1.0) / 2.0 / scale;
            out.tex_coord = vec2<f32>(x / scale + offset, y);
        }
    } else if mode == 2u {
        // Stretch: ignore aspect ratio
        out.tex_coord = vec2<f32>(x, y);
    } else if mode == 3u {
        // Tile: repeat at original size
        out.tex_coord = vec2<f32>(
            x * uniforms.window_size.x / uniforms.image_size.x,
            y * uniforms.window_size.y / uniforms.image_size.y
        );
    } else {
        // Center: original size, centered
        let scale_x = uniforms.image_size.x / uniforms.window_size.x;
        let scale_y = uniforms.image_size.y / uniforms.window_size.y;
        let offset_x = (1.0 - scale_x) / 2.0;
        let offset_y = (1.0 - scale_y) / 2.0;
        out.tex_coord = vec2<f32>((x - offset_x) / scale_x, (y - offset_y) / scale_y);
    }

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Check if texture coordinates are outside [0, 1] range (for modes that don't tile)
    let mode = uniforms.mode;
    if mode != 3u {
        // Not tiling mode - check bounds
        if input.tex_coord.x < 0.0 || input.tex_coord.x > 1.0 ||
           input.tex_coord.y < 0.0 || input.tex_coord.y > 1.0 {
            // Outside image bounds - return transparent
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    // Sample the texture (tiling mode uses repeat sampler)
    var tex_coord = input.tex_coord;
    if mode == 3u {
        // For tiling, wrap coordinates manually
        tex_coord = fract(tex_coord);
    }

    let color = textureSample(bg_texture, bg_sampler, tex_coord);

    // Apply opacity and output premultiplied colors for PreMultiplied composite alpha mode
    let final_alpha = color.a * uniforms.opacity;
    return vec4<f32>(color.rgb * final_alpha, final_alpha);
}
