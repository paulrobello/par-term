//! Default values for font and text-rendering settings.

pub fn font_size() -> f32 {
    12.0
}

pub fn font_family() -> String {
    "JetBrains Mono".to_string()
}

pub fn line_spacing() -> f32 {
    1.0 // Default line height multiplier
}

pub fn char_spacing() -> f32 {
    1.0 // Default character width multiplier
}

pub fn text_shaping() -> bool {
    true // Enabled by default - OpenType features now properly configured via Feature::from_str()
}

pub fn minimum_contrast() -> f32 {
    1.0 // Disabled by default (1.0 = no adjustment)
}

pub fn blur_radius() -> u32 {
    8 // Default blur radius in points (macOS only)
}

pub fn light_tab_style() -> crate::types::TabStyle {
    crate::types::TabStyle::Light
}

pub fn dark_tab_style() -> crate::types::TabStyle {
    crate::types::TabStyle::Dark
}

pub fn badge_font() -> String {
    "Helvetica".to_string()
}
