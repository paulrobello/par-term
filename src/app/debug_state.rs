use std::time::{Duration, Instant};

/// State related to debug metrics and FPS overlay
pub struct DebugState {
    pub frame_times: Vec<Duration>, // Last 60 frame times for FPS calculation
    pub cell_gen_time: Duration,    // Time spent generating cells last frame
    pub render_time: Duration,      // Time spent rendering last frame
    pub cache_hit: bool,            // Whether last frame used cached cells
    pub last_frame_start: Option<Instant>, // Start time of last frame
    pub show_fps_overlay: bool,     // Whether to show FPS overlay (toggle with F3)
    pub fps_value: f64,             // Current FPS value for overlay display
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(60),
            cell_gen_time: Duration::ZERO,
            render_time: Duration::ZERO,
            cache_hit: false,
            last_frame_start: None,
            show_fps_overlay: false,
            fps_value: 0.0,
        }
    }
}
