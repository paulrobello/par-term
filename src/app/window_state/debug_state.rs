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
    /// Timestamp of the last `about_to_wait` call (for inter-frame gap detection).
    pub(crate) last_about_to_wait: Option<Instant>,
    /// Gather-path lock misses since window start: the terminal `try_read`
    /// failed and the frame fell back to cached cells (card 01a198054b5d).
    pub(crate) gather_lock_misses: u64,
    /// Terminal content generation of the last frame that actually rendered.
    pub(crate) last_rendered_generation: Option<u64>,
    /// Gate skips observed while the pane content generation had advanced
    /// past the last rendered generation — the lost-invalidation signature
    /// the hunt is after (card 01a198054b5d).
    pub(crate) skips_with_advanced_generation: u64,
    /// Wall-clock time of the last winit `Resized` event — the trigger of
    /// the surface reconfigure whose stale-pixel corruption persisted until
    /// a pane-level resize (card 01a198054b5d sighting 3).
    pub(crate) last_window_resize_at: Option<Instant>,
    /// The last 64 frames as (time, focused-pane content generation,
    /// rendered?) — the timeline dumped when a render-fault canary fires or
    /// on the first post-resize present, so a stale-pixel sighting carries
    /// its own frame history (card 01a198054b5d).
    pub(crate) frame_history: VecDeque<(Instant, Option<u64>, bool)>,
    /// Whether the first post-resize present has already been announced for
    /// the current resize (one line per resize, not per frame).
    pub(crate) announced_post_resize_present: bool,
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
            last_frame_start: None,
            render_start: None,
            show_fps_overlay: false,
            fps_value: 0.0,
            last_egui_time: Duration::ZERO,
            last_about_to_wait: None,
            gather_lock_misses: 0,
            last_rendered_generation: None,
            skips_with_advanced_generation: 0,
            last_window_resize_at: None,
            frame_history: VecDeque::with_capacity(64),
            announced_post_resize_present: false,
        }
    }
}
