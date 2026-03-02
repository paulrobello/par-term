//! Configuration types for the prettifier pipeline.

use super::super::boundary::DetectionScope;

/// Configuration for the `PrettifierPipeline`.
#[derive(Debug, Clone)]
pub struct PrettifierConfig {
    /// Whether the prettifier is enabled.
    pub enabled: bool,
    /// Whether to respect alternate-screen transitions as boundaries.
    pub respect_alternate_screen: bool,
    /// Minimum confidence for a detection to be accepted.
    pub confidence_threshold: f32,
    /// Maximum lines to accumulate before forcing emission.
    pub max_scan_lines: usize,
    /// Milliseconds of inactivity before emitting a block.
    pub debounce_ms: u64,
    /// When to detect boundaries.
    pub detection_scope: DetectionScope,
}

impl Default for PrettifierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            respect_alternate_screen: true,
            confidence_threshold: 0.6,
            max_scan_lines: 500,
            debounce_ms: 100,
            detection_scope: DetectionScope::All,
        }
    }
}
