//! Default values for shader and render-pipeline settings.

/// Default background shader animation speed multiplier.
pub fn custom_shader_speed() -> f32 {
    1.0 // Normal animation speed
}

/// Default background shader brightness (0.0–1.0; lower values improve text readability).
pub fn custom_shader_brightness() -> f32 {
    0.15 // 15% brightness by default for better text readability
}

/// Default cursor shader effect color as RGB bytes.
pub fn cursor_shader_color() -> [u8; 3] {
    [255, 255, 255] // White cursor for shader effects
}

/// Default cursor trail effect duration in seconds.
pub fn cursor_trail_duration() -> f32 {
    0.5 // 500ms trail duration
}

/// Default cursor glow effect radius in pixels.
pub fn cursor_glow_radius() -> f32 {
    80.0 // 80 pixel glow radius
}

/// Default cursor glow effect intensity (0.0–1.0).
pub fn cursor_glow_intensity() -> f32 {
    0.3 // 30% glow intensity
}

/// Default flag disabling the cursor shader when the terminal is in alt screen mode.
pub fn cursor_shader_disable_in_alt_screen() -> bool {
    true // Preserve current behavior: disable cursor shader in alt screen by default
}

/// Default shader hot-reload debounce delay in milliseconds.
pub fn shader_hot_reload_delay() -> u64 {
    100 // Debounce delay in milliseconds
}

/// Default for reduce_flicker option
pub fn reduce_flicker() -> bool {
    true
}

/// Default delay in milliseconds for reduce_flicker
pub fn reduce_flicker_delay_ms() -> u32 {
    16 // ~1 frame at 60fps
}

/// Default for maximize_throughput option
pub fn maximize_throughput() -> bool {
    false // Off by default
}

/// Default render interval in milliseconds when maximize_throughput is enabled
pub fn throughput_render_interval_ms() -> u32 {
    100 // 100ms default (~10 fps during bulk output)
}
