//! `PrettifiedBlock` — a content block that has been through the detection
//! and rendering pipeline.

use super::super::buffer::DualViewBuffer;
use super::super::types::{ContentBlock, DetectionResult, ViewMode};

/// A content block that has been through the detection and rendering pipeline.
///
/// Wraps a `DualViewBuffer` for source/rendered dual-view management.
#[derive(Debug)]
pub struct PrettifiedBlock {
    /// Dual-view buffer managing source + rendered content.
    pub buffer: DualViewBuffer,
    /// The detection result that matched this block.
    pub detection: DetectionResult,
    /// Unique identifier for this block within the session.
    pub block_id: u64,
}

impl PrettifiedBlock {
    /// Get the original content block.
    pub fn content(&self) -> &ContentBlock {
        self.buffer.source()
    }

    /// Get the current view mode.
    pub fn view_mode(&self) -> ViewMode {
        *self.buffer.view_mode()
    }

    /// Whether rendered content is available.
    pub fn has_rendered(&self) -> bool {
        self.buffer.rendered().is_some()
    }
}
