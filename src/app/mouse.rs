use crate::selection::Selection;
use crate::url_detection;
use std::time::Instant;

/// State related to mouse interaction, selection, and URL detection
pub struct MouseState {
    pub selection: Option<Selection>, // Current text selection
    pub is_selecting: bool,           // Whether currently dragging to select

    pub position: (f64, f64),   // Current mouse position in pixels
    pub button_pressed: bool, // Whether any mouse button is currently pressed (for motion tracking)
    pub last_click_time: Option<Instant>, // Time of last mouse click
    pub click_count: u32,           // Number of sequential clicks (1 = single, 2 = double, 3 = triple)
    pub click_position: Option<(usize, usize)>, // Position of last click in cell coordinates
    pub detected_urls: Vec<url_detection::DetectedUrl>, // URLs detected in visible terminal area
    pub hovered_url: Option<String>, // URL currently under mouse cursor
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            selection: None,
            is_selecting: false,
            position: (0.0, 0.0),
            button_pressed: false,
            last_click_time: None,
            click_count: 0,
            click_position: None,
            detected_urls: Vec::new(),
            hovered_url: None,
        }
    }
}
