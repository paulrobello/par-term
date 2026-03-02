// Opaque-surface alpha pass: forces alpha=1.0 across the entire surface
// without modifying RGB values.
//
// On macOS with CompositeAlphaMode::PreMultiplied, any pixel with alpha < 1.0
// becomes translucent through to the desktop. Several rendering passes can
// inadvertently reduce alpha (anti-aliased text blending, overlay compositing).
// This single full-screen triangle stamps alpha=1.0 after all rendering,
// guaranteeing an opaque surface when window_opacity == 1.0.
//
// Cost: negligible — 3 vertices, no textures, no blending, alpha-only writes.

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Full-screen triangle (oversized to cover entire NDC viewport)
    // vertex 0: (-1, -1) — bottom-left
    // vertex 1: ( 3, -1) — far right (extends past viewport)
    // vertex 2: (-1,  3) — far top (extends past viewport)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // With ColorWrites::ALPHA write mask, only alpha=1.0 is written
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
