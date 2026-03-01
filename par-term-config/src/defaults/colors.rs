//! Default values for color settings across all UI components.

// Scrollbar colors
pub fn scrollbar_thumb_color() -> [f32; 4] {
    [0.4, 0.4, 0.4, 0.95] // Medium gray, nearly opaque
}

pub fn scrollbar_track_color() -> [f32; 4] {
    [0.15, 0.15, 0.15, 0.6] // Dark gray, semi-transparent
}

// Command separator colors
pub fn command_separator_color() -> [u8; 3] {
    [128, 128, 128] // Medium gray
}

// Link color
pub fn link_highlight_color() -> [u8; 3] {
    [79, 195, 247] // Bright cyan (#4FC3F7)
}

// Cursor color
pub fn cursor_color() -> [u8; 3] {
    [255, 255, 255] // White cursor
}

// Tab bar colors
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

pub fn tab_border_color() -> [u8; 3] {
    [80, 80, 80] // Subtle gray border between tabs
}

// Progress bar colors
pub fn progress_bar_normal_color() -> [u8; 3] {
    [80, 180, 255] // Blue for normal progress
}

pub fn progress_bar_warning_color() -> [u8; 3] {
    [255, 200, 50] // Yellow for warning
}

pub fn progress_bar_error_color() -> [u8; 3] {
    [255, 80, 80] // Red for error
}

pub fn progress_bar_indeterminate_color() -> [u8; 3] {
    [150, 150, 150] // Gray for indeterminate
}

// Cursor enhancement colors
pub fn cursor_guide_color() -> [u8; 4] {
    [255, 255, 255, 20] // Subtle white highlight
}

pub fn cursor_shadow_color() -> [u8; 4] {
    [0, 0, 0, 128] // Semi-transparent black
}

pub fn cursor_boost_color() -> [u8; 3] {
    [255, 255, 255] // White glow
}

// Badge color
pub fn badge_color() -> [u8; 3] {
    [255, 0, 0] // Red text (matches iTerm2 default)
}

// Status bar colors
pub fn status_bar_bg_color() -> [u8; 3] {
    [30, 30, 30]
}

pub fn status_bar_fg_color() -> [u8; 3] {
    [200, 200, 200]
}

// Pane colors
pub fn pane_divider_color() -> [u8; 3] {
    [80, 80, 80] // Subtle gray divider
}

pub fn pane_divider_hover_color() -> [u8; 3] {
    [120, 150, 200] // Brighter color on hover for resize feedback
}

pub fn pane_focus_color() -> [u8; 3] {
    [100, 150, 255] // Blue highlight for focused pane
}

pub fn pane_title_color() -> [u8; 3] {
    [200, 200, 200] // Light gray text for pane titles
}

pub fn pane_title_bg_color() -> [u8; 3] {
    [40, 40, 50] // Dark background for pane title bars
}

// Search highlight colors
pub fn search_highlight_color() -> [u8; 4] {
    [255, 200, 0, 180] // Yellow with some transparency
}

pub fn search_current_highlight_color() -> [u8; 4] {
    [255, 100, 0, 220] // Orange, more visible for current match
}
