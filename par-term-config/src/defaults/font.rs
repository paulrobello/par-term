//! Default values for font and text-rendering settings.

/// Default font size in points.
pub fn font_size() -> f32 {
    13.0
}

/// Default font family name.
pub fn font_family() -> String {
    "JetBrains Mono".to_string()
}

/// Default line spacing multiplier (1.0 = normal line height).
pub fn line_spacing() -> f32 {
    1.0 // Default line height multiplier
}

/// Default character spacing multiplier (1.0 = normal character width).
pub fn char_spacing() -> f32 {
    1.0 // Default character width multiplier
}

/// Default flag enabling HarfBuzz-based text shaping for ligatures and complex scripts.
pub fn text_shaping() -> bool {
    true // Enabled by default - OpenType features now properly configured via Feature::from_str()
}

/// Default minimum contrast ratio adjustment (0.0 = disabled).
pub fn minimum_contrast() -> f32 {
    0.0 // Disabled by default (0.0 = no adjustment, matching iTerm2 convention)
}

/// Default window blur radius in points (macOS only).
pub fn blur_radius() -> u32 {
    8 // Default blur radius in points (macOS only)
}

/// Default tab style for light color themes.
pub fn light_tab_style() -> crate::types::TabStyle {
    crate::types::TabStyle::Light
}

/// Default tab style for dark color themes.
pub fn dark_tab_style() -> crate::types::TabStyle {
    crate::types::TabStyle::Dark
}

/// Default font family used for badge text rendering.
pub fn badge_font() -> String {
    "Helvetica".to_string()
}
