//! Default value functions for configuration.

pub fn cols() -> usize {
    80
}

pub fn rows() -> usize {
    24
}

pub fn font_size() -> f32 {
    13.0
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

pub fn scrollback() -> usize {
    10000
}

pub fn window_title() -> String {
    "par-term".to_string()
}

pub fn theme() -> String {
    "dark-background".to_string()
}

pub fn screenshot_format() -> String {
    "png".to_string()
}

pub fn max_fps() -> u32 {
    60
}

pub fn window_padding() -> f32 {
    10.0
}

pub fn login_shell() -> bool {
    true
}

pub fn scrollbar_position() -> String {
    "right".to_string()
}

pub fn scrollbar_width() -> f32 {
    15.0
}

pub fn scrollbar_thumb_color() -> [f32; 4] {
    [0.4, 0.4, 0.4, 0.95] // Medium gray, nearly opaque
}

pub fn scrollbar_track_color() -> [f32; 4] {
    [0.15, 0.15, 0.15, 0.6] // Dark gray, semi-transparent
}

pub fn clipboard_max_sync_events() -> usize {
    64 // Aligned with sister project
}

pub fn clipboard_max_event_bytes() -> usize {
    2048 // Aligned with sister project
}

pub fn activity_threshold() -> u64 {
    10 // Aligned with sister project (10 seconds)
}

pub fn silence_threshold() -> u64 {
    300 // 5 minutes
}

pub fn notification_max_buffer() -> usize {
    64 // Aligned with sister project
}

pub fn scroll_speed() -> f32 {
    3.0 // Lines per scroll tick
}

pub fn double_click_threshold() -> u64 {
    500 // 500 milliseconds
}

pub fn triple_click_threshold() -> u64 {
    500 // 500 milliseconds (same as double-click)
}

pub fn cursor_blink_interval() -> u64 {
    500 // 500 milliseconds (blink twice per second)
}

pub fn cursor_color() -> [u8; 3] {
    [255, 255, 255] // White cursor
}

pub fn scrollbar_autohide_delay() -> u64 {
    0 // 0 = never auto-hide (always visible when scrollback exists)
}

pub fn window_opacity() -> f32 {
    1.0 // Fully opaque by default
}

pub fn background_image_opacity() -> f32 {
    1.0 // Fully opaque by default
}

pub fn background_color() -> [u8; 3] {
    [30, 30, 30] // Dark gray
}

pub fn bool_false() -> bool {
    false
}

pub fn bool_true() -> bool {
    true
}

pub fn text_opacity() -> f32 {
    1.0 // Fully opaque text by default
}

pub fn custom_shader_speed() -> f32 {
    1.0 // Normal animation speed
}

pub fn custom_shader_brightness() -> f32 {
    1.0 // Full brightness by default
}

pub fn cursor_shader_color() -> [u8; 3] {
    [255, 255, 255] // White cursor for shader effects
}

pub fn cursor_trail_duration() -> f32 {
    0.5 // 500ms trail duration
}

pub fn cursor_glow_radius() -> f32 {
    80.0 // 80 pixel glow radius
}

pub fn cursor_glow_intensity() -> f32 {
    0.3 // 30% glow intensity
}

pub fn cursor_shader_disable_in_alt_screen() -> bool {
    true // Preserve current behavior: disable cursor shader in alt screen by default
}

pub fn bell_sound() -> u8 {
    50 // Default to 50% volume
}

pub fn tab_bar_height() -> f32 {
    28.0 // Default tab bar height in pixels
}

pub fn zero() -> usize {
    0
}

pub fn unfocused_fps() -> u32 {
    30 // Reduced FPS when window is not focused
}

pub fn shader_hot_reload_delay() -> u64 {
    100 // Debounce delay in milliseconds
}

// Tab bar color defaults
pub fn tab_bar_background() -> [u8; 3] {
    [40, 40, 40] // Dark gray background
}

pub fn tab_active_background() -> [u8; 3] {
    [60, 60, 60] // Slightly lighter for active tab
}

pub fn tab_inactive_background() -> [u8; 3] {
    [40, 40, 40] // Same as bar background
}

pub fn tab_hover_background() -> [u8; 3] {
    [50, 50, 50] // Between inactive and active
}

pub fn tab_active_text() -> [u8; 3] {
    [255, 255, 255] // White text for active tab
}

pub fn tab_inactive_text() -> [u8; 3] {
    [180, 180, 180] // Gray text for inactive tabs
}

pub fn tab_active_indicator() -> [u8; 3] {
    [100, 150, 255] // Blue underline for active tab
}

pub fn tab_activity_indicator() -> [u8; 3] {
    [100, 180, 255] // Light blue activity dot
}

pub fn tab_bell_indicator() -> [u8; 3] {
    [255, 200, 100] // Orange/yellow bell icon
}

pub fn tab_close_button() -> [u8; 3] {
    [150, 150, 150] // Gray close button
}

pub fn tab_close_button_hover() -> [u8; 3] {
    [255, 100, 100] // Red on hover
}

pub fn cubemap_enabled() -> bool {
    true // Cubemap sampling enabled by default when a path is configured
}

pub fn inactive_tab_opacity() -> f32 {
    0.6 // Default opacity for inactive tabs (60%)
}

pub fn tab_min_width() -> f32 {
    120.0 // Minimum tab width in pixels before scrolling kicks in
}

pub fn tab_border_color() -> [u8; 3] {
    [80, 80, 80] // Subtle gray border between tabs
}

pub fn tab_border_width() -> f32 {
    1.0 // 1 pixel border
}

pub fn blur_radius() -> u32 {
    8 // Default blur radius in points (macOS only)
}

pub fn use_background_as_channel0() -> bool {
    false // By default, use configured channel0 texture, not background image
}

pub fn keybindings() -> Vec<super::types::KeyBinding> {
    vec![
        super::types::KeyBinding {
            key: "CmdOrCtrl+Shift+B".to_string(),
            action: "toggle_background_shader".to_string(),
        },
        super::types::KeyBinding {
            key: "CmdOrCtrl+Shift+U".to_string(),
            action: "toggle_cursor_shader".to_string(),
        },
    ]
}

// Cursor enhancement defaults
pub fn cursor_guide_color() -> [u8; 4] {
    [255, 255, 255, 20] // Subtle white highlight
}

pub fn cursor_shadow_color() -> [u8; 4] {
    [0, 0, 0, 128] // Semi-transparent black
}

pub fn cursor_shadow_offset() -> [f32; 2] {
    [2.0, 2.0] // 2 pixels offset in both directions
}

pub fn cursor_shadow_blur() -> f32 {
    3.0 // 3 pixel blur radius
}

pub fn cursor_boost() -> f32 {
    0.0 // Disabled by default
}

pub fn cursor_boost_color() -> [u8; 3] {
    [255, 255, 255] // White glow
}

pub fn update_check_frequency() -> super::types::UpdateCheckFrequency {
    super::types::UpdateCheckFrequency::Weekly
}

// Search defaults
pub fn search_highlight_color() -> [u8; 4] {
    [255, 200, 0, 180] // Yellow with some transparency
}

pub fn search_current_highlight_color() -> [u8; 4] {
    [255, 100, 0, 220] // Orange, more visible for current match
}
