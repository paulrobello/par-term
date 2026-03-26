use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Timing metrics and FPS overlay state for the window.
///
/// Tracks per-frame timing data used to compute and display the live FPS
/// overlay (toggled with F3). All durations are reset at the start of each
/// frame and updated during the render pass.
pub(crate) struct DebugState {
    /// Last 60 frame durations for rolling FPS calculation.
    pub(crate) frame_times: VecDeque<Duration>,
    /// Time spent building the styled cell buffer for the current frame.
    pub(crate) cell_gen_time: Duration,
    /// Time spent executing the GPU render pass for the current frame.
    pub(crate) render_time: Duration,
    /// Whether the current frame reused the cached cell buffer (no PTY output).
    pub(crate) cache_hit: bool,
    /// Wall-clock start time of the current frame (set at the top of the event loop).
    pub(crate) last_frame_start: Option<Instant>,
    /// Wall-clock start of the GPU render phase (for end-of-frame timing).
    pub(crate) render_start: Option<Instant>,
    /// Whether the FPS/timing overlay is visible. Toggled with F3.
    pub(crate) show_fps_overlay: bool,
    /// Current smoothed FPS value displayed by the overlay.
    pub(crate) fps_value: f64,
    /// Time spent on the egui overlay render pass during the last frame.
    pub(crate) last_egui_time: Duration,
}

impl Default for DebugState {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugState {
    /// Create a zeroed `DebugState` with a 60-frame time ring allocated.
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
