//! Default values for window and visual-appearance settings.

pub fn cols() -> usize {
    80
}

pub fn rows() -> usize {
    24
}

pub fn window_title() -> String {
    "par-term".to_string()
}

pub fn theme() -> String {
    "dark-background".to_string()
}

pub fn light_theme() -> String {
    "light-background".to_string()
}

pub fn dark_theme() -> String {
    "dark-background".to_string()
}

pub fn screenshot_format() -> String {
    "png".to_string()
}

pub fn max_fps() -> u32 {
    60
}

pub fn window_padding() -> f32 {
    0.0
}

pub fn window_opacity() -> f32 {
    1.0 // Fully opaque by default
}

pub fn background_image_opacity() -> f32 {
    1.0 // Fully opaque by default
}

pub fn pane_background_darken() -> f32 {
    0.0 // No darkening by default
}

pub fn background_color() -> [u8; 3] {
    [30, 30, 30] // Dark gray
}

pub fn text_opacity() -> f32 {
    1.0 // Fully opaque text by default
}

pub fn tab_bar_height() -> f32 {
    28.0 // Default tab bar height in pixels
}

pub fn tab_bar_width() -> f32 {
    160.0 // Default tab bar width in pixels (for left position)
}

pub fn unfocused_fps() -> u32 {
    30 // Reduced FPS when window is not focused
}

pub fn inactive_tab_fps() -> u32 {
    2 // Very low FPS for inactive (non-visible) tabs - just enough for activity detection
}

pub fn inactive_tab_opacity() -> f32 {
    0.6 // Default opacity for inactive tabs (60%)
}

pub fn tab_min_width() -> f32 {
    120.0 // Minimum tab width in pixels before scrolling kicks in
}

pub fn tab_stretch_to_fill() -> bool {
    true // Tabs stretch to share available width by default
}

pub fn tab_html_titles() -> bool {
    false // Render tab titles as plain text unless explicitly enabled
}

pub fn tab_border_width() -> f32 {
    1.0 // 1 pixel border
}

pub fn cubemap_enabled() -> bool {
    true // Cubemap sampling enabled by default when a path is configured
}

pub fn use_background_as_channel0() -> bool {
    false // By default, use configured channel0 texture, not background image
}
