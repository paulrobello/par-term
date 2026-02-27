//! Color conversion helper functions.

// ============================================================================
// Color Conversion Helpers
// ============================================================================

/// Convert a `[u8; 3]` RGB color to `[f32; 3]` normalized to 0.0..1.0.
#[inline]
pub fn color_u8_to_f32(rgb: [u8; 3]) -> [f32; 3] {
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    ]
}

/// Convert a `[u8; 3]` RGB color to `[f32; 4]` normalized to 0.0..1.0 with
/// the given alpha value appended.
#[inline]
pub fn color_u8_to_f32_a(rgb: [u8; 3], alpha: f32) -> [f32; 4] {
    [
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
        alpha,
    ]
}

/// Convert a `[u8; 4]` RGBA color to `[f32; 4]` normalized to 0.0..1.0.
#[inline]
pub fn color_u8x4_to_f32(rgba: [u8; 4]) -> [f32; 4] {
    [
        rgba[0] as f32 / 255.0,
        rgba[1] as f32 / 255.0,
        rgba[2] as f32 / 255.0,
        rgba[3] as f32 / 255.0,
    ]
}

/// Convert the RGB channels of a `[u8; 4]` color to `[f32; 3]` normalized to
/// 0.0..1.0, discarding the alpha channel.
#[inline]
pub fn color_u8x4_rgb_to_f32(rgba: [u8; 4]) -> [f32; 3] {
    [
        rgba[0] as f32 / 255.0,
        rgba[1] as f32 / 255.0,
        rgba[2] as f32 / 255.0,
    ]
}

/// Convert the RGB channels of a `[u8; 4]` color to `[f32; 4]` normalized to
/// 0.0..1.0, replacing the alpha channel with the provided value.
#[inline]
pub fn color_u8x4_rgb_to_f32_a(rgba: [u8; 4], alpha: f32) -> [f32; 4] {
    [
        rgba[0] as f32 / 255.0,
        rgba[1] as f32 / 255.0,
        rgba[2] as f32 / 255.0,
        alpha,
    ]
}

/// Convert an `(u8, u8, u8)` RGB tuple to `[f32; 4]` normalized to 0.0..1.0
/// with the given alpha value appended.
#[inline]
pub fn color_tuple_to_f32_a(r: u8, g: u8, b: u8, alpha: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, alpha]
}
