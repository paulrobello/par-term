use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// State related to debug metrics and FPS overlay
pub(crate) struct DebugState {
    pub(crate) frame_times: VecDeque<Duration>, // Last 60 frame times for FPS calculation
    pub(crate) cell_gen_time: Duration,         // Time spent generating cells last frame
    pub(crate) render_time: Duration,           // Time spent rendering last frame
    pub(crate) cache_hit: bool,                 // Whether last frame used cached cells
    pub(crate) last_frame_start: Option<Instant>, // Start time of last frame
    pub(crate) render_start: Option<Instant>, // Start time of current render frame (for end-of-frame timing)
    pub(crate) show_fps_overlay: bool,        // Whether to show FPS overlay (toggle with F3)
    pub(crate) fps_value: f64,                // Current FPS value for overlay display
    pub(crate) last_egui_time: Duration,      // Time spent on egui overlay rendering last frame
}

impl Default for DebugState {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugState {
    pub(crate) fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(60),
            cell_gen_time: Duration::ZERO,
            render_time: Duration::ZERO,
            cache_hit: false,
            last_frame_start: None,
            render_start: None,
            show_fps_overlay: false,
            fps_value: 0.0,
            last_egui_time: Duration::ZERO,
        }
    }
}
