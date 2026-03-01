use crate::selection::Selection;
use crate::url_detection;
use std::time::Instant;

/// State related to mouse interaction, selection, and URL detection
pub struct MouseState {
    pub(crate) selection: Option<Selection>, // Current text selection
    pub(crate) is_selecting: bool,           // Whether currently dragging to select

    pub(crate) position: (f64, f64), // Current mouse position in pixels
    pub(crate) button_pressed: bool, // Whether any mouse button is currently pressed (for motion tracking)
    pub(crate) last_click_time: Option<Instant>, // Time of last mouse click
    pub(crate) click_count: u32, // Number of sequential clicks (1 = single, 2 = double, 3 = triple)
    pub(crate) click_position: Option<(usize, usize)>, // Position of last click in cell coordinates
    pub(crate) click_pixel_position: Option<(f64, f64)>, // Position of last click in pixels (for drag threshold)
    pub(crate) detected_urls: Vec<url_detection::DetectedUrl>, // URLs detected in visible terminal area
    pub(crate) hovered_url: Option<String>,                    // URL currently under mouse cursor

    // Divider drag state
    pub(crate) dragging_divider: Option<usize>, // Index of divider being dragged
    pub(crate) divider_hover: bool,             // Whether hovering over a divider
    pub(crate) hovered_divider_index: Option<usize>, // Index of the hovered divider
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseState {
    pub(crate) fn new() -> Self {
        Self {
            selection: None,
            is_selecting: false,
            position: (0.0, 0.0),
            button_pressed: false,
            last_click_time: None,
            click_count: 0,
            click_position: None,
            click_pixel_position: None,
            detected_urls: Vec::new(),
            hovered_url: None,
            dragging_divider: None,
            divider_hover: false,
            hovered_divider_index: None,
        }
    }
}
