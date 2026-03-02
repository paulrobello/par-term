//! ANSI terminal color palette constants and 256-color conversion.
//!
//! Centralises the 16-color ANSI palette and the 256-color-to-RGB mapping so
//! that multiple subsystems (the prettifier ANSI parser, future terminal
//! renderers, etc.) can share the same authoritative values.
//!
//! # Contents
//!
//! - [`ANSI_COLORS`] — standard 8-color palette (indices 0–7)
//! - [`ANSI_BRIGHT`] — bright 8-color palette (indices 8–15)
//! - [`color_256_to_rgb`] — convert any 256-color index to `[u8; 3]` RGB

// ---------------------------------------------------------------------------
// 8-color ANSI palette (SGR 30–37 / 40–47)
// ---------------------------------------------------------------------------

/// Standard ANSI 8-color foreground/background palette (indices 0–7).
///
/// Maps to SGR codes 30–37 (foreground) and 40–47 (background).
pub const ANSI_COLORS: [[u8; 3]; 8] = [
    [0, 0, 0],       // 0  Black
    [170, 0, 0],     // 1  Red
    [0, 170, 0],     // 2  Green
    [170, 85, 0],    // 3  Yellow
    [0, 0, 170],     // 4  Blue
    [170, 0, 170],   // 5  Magenta
    [0, 170, 170],   // 6  Cyan
    [192, 192, 192], // 7  White
];

/// Bright ANSI 8-color palette (indices 8–15).
///
/// Maps to SGR codes 90–97 (bright foreground) and 100–107 (bright background).
pub const ANSI_BRIGHT: [[u8; 3]; 8] = [
    [85, 85, 85],    // 8   Bright black  (dark grey)
    [255, 85, 85],   // 9   Bright red
    [85, 255, 85],   // 10  Bright green
    [255, 255, 85],  // 11  Bright yellow
    [85, 85, 255],   // 12  Bright blue
    [255, 85, 255],  // 13  Bright magenta
    [85, 255, 255],  // 14  Bright cyan
    [255, 255, 255], // 15  Bright white
];

// ---------------------------------------------------------------------------
// 256-color to RGB
// ---------------------------------------------------------------------------

/// Convert a 256-color terminal palette index to an `[r, g, b]` triple.
///
/// # Palette layout
///
/// | Index range | Palette segment |
/// |-------------|----------------|
/// | 0 – 7       | Standard ANSI colors ([`ANSI_COLORS`]) |
/// | 8 – 15      | Bright ANSI colors ([`ANSI_BRIGHT`]) |
/// | 16 – 231    | 6×6×6 RGB color cube |
/// | 232 – 255   | 24-step grayscale ramp |
pub fn color_256_to_rgb(idx: u8) -> [u8; 3] {
    match idx {
        0..=7 => ANSI_COLORS[idx as usize],
        8..=15 => ANSI_BRIGHT[(idx - 8) as usize],
        16..=231 => {
            // 6×6×6 color cube
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            [to_val(r), to_val(g), to_val(b)]
        }
        232..=255 => {
            // 24-step grayscale ramp: index 232 → rgb(8,8,8), 255 → rgb(238,238,238)
            let v = 8 + 10 * (idx - 232);
            [v, v, v]
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_colors_black() {
        assert_eq!(ANSI_COLORS[0], [0, 0, 0]);
    }

    #[test]
    fn test_ansi_bright_white() {
        assert_eq!(ANSI_BRIGHT[7], [255, 255, 255]);
    }

    #[test]
    fn test_color_256_standard_palette() {
        // Index 0 maps to ANSI_COLORS[0]
        assert_eq!(color_256_to_rgb(0), ANSI_COLORS[0]);
        // Index 7 maps to ANSI_COLORS[7]
        assert_eq!(color_256_to_rgb(7), ANSI_COLORS[7]);
    }

    #[test]
    fn test_color_256_bright_palette() {
        // Index 8 maps to ANSI_BRIGHT[0]
        assert_eq!(color_256_to_rgb(8), ANSI_BRIGHT[0]);
        // Index 15 maps to ANSI_BRIGHT[7]
        assert_eq!(color_256_to_rgb(15), ANSI_BRIGHT[7]);
    }

    #[test]
    fn test_color_256_cube_black() {
        // Index 16 is the first cube entry: rgb(0,0,0)
        assert_eq!(color_256_to_rgb(16), [0, 0, 0]);
    }

    #[test]
    fn test_color_256_cube_white() {
        // Index 231 is the last cube entry: rgb(255,255,255)
        assert_eq!(color_256_to_rgb(231), [255, 255, 255]);
    }

    #[test]
    fn test_color_256_grayscale_first() {
        // Index 232 → rgb(8,8,8)
        assert_eq!(color_256_to_rgb(232), [8, 8, 8]);
    }

    #[test]
    fn test_color_256_grayscale_last() {
        // Index 255 → rgb(238,238,238)
        assert_eq!(color_256_to_rgb(255), [238, 238, 238]);
    }
}
