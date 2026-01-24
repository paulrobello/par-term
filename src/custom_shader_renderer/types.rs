/// Uniform data passed to custom shaders
/// Layout must match GLSL std140 rules:
/// - vec2 aligned to 8 bytes
/// - vec4 aligned to 16 bytes
/// - float aligned to 4 bytes
/// - struct size rounded to 16 bytes (largest alignment)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CustomShaderUniforms {
    /// Viewport resolution (iResolution.xy) - offset 0, size 8
    pub resolution: [f32; 2],
    /// Time in seconds since shader started (iTime) - offset 8, size 4
    pub time: f32,
    /// Time since last frame in seconds (iTimeDelta) - offset 12, size 4
    pub time_delta: f32,
    /// Mouse state (iMouse) - offset 16, size 16
    /// xy = current position (if dragging) or last drag position
    /// zw = click position (positive when held, negative when released)
    pub mouse: [f32; 4],
    /// Date/time (iDate) - offset 32, size 16
    /// x = year, y = month (0-11), z = day (1-31), w = seconds since midnight
    pub date: [f32; 4],
    /// Window opacity for transparency support - offset 48, size 4
    pub opacity: f32,
    /// Text opacity (separate from window opacity) - offset 52, size 4
    pub text_opacity: f32,
    /// Full content mode: 1.0 = shader receives and outputs full content, 0.0 = background only
    pub full_content_mode: f32,
    /// Frame counter (iFrame) - offset 60, size 4
    pub frame: f32,
    /// Current frame rate in FPS (iFrameRate) - offset 64, size 4
    pub frame_rate: f32,
    /// Pixel aspect ratio (iResolution.z) - offset 68, size 4, usually 1.0
    pub resolution_z: f32,
    /// Padding to reach 80 bytes (multiple of 16) - offset 72, size 8
    pub _pad1: [f32; 2],

    // ============ Cursor uniforms (Ghostty-compatible, v1.2.0+) ============
    // Offsets 80-159
    /// Current cursor position (xy) and size (zw) in pixels - offset 80, size 16
    pub current_cursor: [f32; 4],
    /// Previous cursor position (xy) and size (zw) in pixels - offset 96, size 16
    pub previous_cursor: [f32; 4],
    /// Current cursor RGBA color (with opacity baked into alpha) - offset 112, size 16
    pub current_cursor_color: [f32; 4],
    /// Previous cursor RGBA color - offset 128, size 16
    pub previous_cursor_color: [f32; 4],
    /// Time when cursor last moved (same timebase as iTime) - offset 144, size 4
    pub cursor_change_time: f32,

    // ============ Cursor shader configuration uniforms ============
    // Offsets 148-175
    /// Cursor trail duration in seconds - offset 148, size 4
    pub cursor_trail_duration: f32,
    /// Cursor glow radius in pixels - offset 152, size 4
    pub cursor_glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0) - offset 156, size 4
    pub cursor_glow_intensity: f32,
    /// User-configured cursor color for shader effects [R, G, B, 1.0] - offset 160, size 16
    /// (placed last because vec4 must be aligned to 16 bytes in std140)
    pub cursor_shader_color: [f32; 4],
}
// Total size: 176 bytes

// Compile-time assertion to ensure uniform struct size matches expectations
const _: () = assert!(
    std::mem::size_of::<CustomShaderUniforms>() == 176,
    "CustomShaderUniforms must be exactly 176 bytes for GPU compatibility"
);
