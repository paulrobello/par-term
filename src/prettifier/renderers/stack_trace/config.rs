//! Configuration for the stack trace renderer.

/// Configuration for the stack trace renderer.
#[derive(Clone, Debug)]
pub struct StackTraceRendererConfig {
    /// Package name prefixes considered "application" code (bright styling).
    /// Frames not matching these are "framework" (dimmed).
    pub app_packages: Vec<String>,
    /// Maximum frames to show before collapsing (default: 5).
    pub max_visible_frames: usize,
    /// Always keep the last N frames visible (for "Caused by" chains, default: 1).
    pub keep_tail_frames: usize,
}

impl Default for StackTraceRendererConfig {
    fn default() -> Self {
        Self {
            app_packages: Vec::new(),
            max_visible_frames: 5,
            keep_tail_frames: 1,
        }
    }
}
