//! Gutter indicators for prettified content blocks.
//!
//! `GutterManager` tracks which screen rows have prettified content and provides
//! hit-testing for gutter clicks (to toggle per-block view mode).

use super::pipeline::PrettifierPipeline;
use super::types::ViewMode;

/// Width of the gutter area in terminal columns.
const GUTTER_WIDTH: usize = 2;

/// Manages gutter indicator state for prettified content blocks.
pub struct GutterManager {
    /// Width of the gutter area in terminal columns.
    pub gutter_width: usize,
}

/// A single gutter indicator for a visible prettified block.
#[derive(Debug, Clone)]
pub struct GutterIndicator {
    /// Screen row where this block starts.
    pub row: usize,
    /// Block height in screen rows.
    pub height: usize,
    /// Badge string to display (emoji or short text).
    pub badge: String,
    /// Current view mode of this block.
    pub view_mode: ViewMode,
    /// Whether the mouse is hovering over this indicator.
    pub hovered: bool,
    /// Unique block ID (for toggle operations).
    pub block_id: u64,
}

impl Default for GutterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GutterManager {
    /// Create a new gutter manager with the default gutter width.
    pub fn new() -> Self {
        Self {
            gutter_width: GUTTER_WIDTH,
        }
    }

    /// Map a format ID to a display badge.
    pub fn badge_for_format(format_id: &str) -> &str {
        match format_id {
            "markdown" => "\u{1F4DD}", // 📝
            "json" => "{}",
            "mermaid" | "diagram" => "\u{1F4CA}", // 📊
            "yaml" | "toml" => "\u{1F4CB}",       // 📋
            "xml" => "\u{1F4C4}",                 // 📄
            "csv" => "\u{1F4C9}",                 // 📉
            "diff" => "\u{00B1}",                 // ±
            "log" => "\u{1F4DC}",                 // 📜
            "stack_trace" => "\u{26A0}\u{FE0F}",  // ⚠️
            _ => "\u{2726}",                      // ✦
        }
    }

    /// Compute gutter indicators for blocks visible in the current viewport.
    ///
    /// Iterates `pipeline.active_blocks()`, filters to rows overlapping the
    /// viewport, and builds an indicator for each visible block.
    pub fn indicators_for_viewport(
        &self,
        pipeline: &PrettifierPipeline,
        viewport_start_row: usize,
        viewport_height: usize,
    ) -> Vec<GutterIndicator> {
        let viewport_end = viewport_start_row + viewport_height;
        let mut indicators = Vec::new();

        for block in pipeline.active_blocks() {
            let content = block.content();
            let block_start = content.start_row;
            let block_end = content.end_row;

            // Skip blocks entirely outside the viewport.
            if block_end <= viewport_start_row || block_start >= viewport_end {
                continue;
            }

            // Clamp to viewport bounds and convert to screen-relative rows.
            let visible_start = block_start.max(viewport_start_row);
            let visible_end = block_end.min(viewport_end);
            let screen_row = visible_start - viewport_start_row;
            let height = visible_end - visible_start;

            indicators.push(GutterIndicator {
                row: screen_row,
                height,
                badge: Self::badge_for_format(&block.detection.format_id).to_string(),
                view_mode: block.view_mode(),
                hovered: false,
                block_id: block.block_id,
            });
        }

        indicators
    }

    /// Hit-test a cell position against gutter indicators.
    ///
    /// Returns the `block_id` if `col < gutter_width` and `row` falls within
    /// an indicator's row range.
    pub fn hit_test(&self, col: usize, row: usize, indicators: &[GutterIndicator]) -> Option<u64> {
        if col >= self.gutter_width {
            return None;
        }
        indicators.iter().find_map(|ind| {
            if row >= ind.row && row < ind.row + ind.height {
                Some(ind.block_id)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::boundary::DetectionScope;
    use crate::prettifier::pipeline::{PrettifierConfig, PrettifierPipeline};
    use crate::prettifier::registry::RendererRegistry;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::ContentBlock;
    use std::time::SystemTime;

    fn empty_pipeline() -> PrettifierPipeline {
        PrettifierPipeline::new(
            PrettifierConfig {
                enabled: true,
                detection_scope: DetectionScope::All,
                ..PrettifierConfig::default()
            },
            RendererRegistry::new(0.5),
            RendererConfig::default(),
        )
    }

    #[test]
    fn test_badge_for_format() {
        assert_eq!(GutterManager::badge_for_format("markdown"), "\u{1F4DD}");
        assert_eq!(GutterManager::badge_for_format("json"), "{}");
        assert_eq!(GutterManager::badge_for_format("diff"), "\u{00B1}");
        assert_eq!(GutterManager::badge_for_format("unknown"), "\u{2726}");
    }

    #[test]
    fn test_indicators_empty_pipeline() {
        let gm = GutterManager::new();
        let pipeline = empty_pipeline();
        let indicators = gm.indicators_for_viewport(&pipeline, 0, 24);
        assert!(indicators.is_empty());
    }

    #[test]
    fn test_indicators_for_visible_block() {
        let gm = GutterManager::new();
        let mut pipeline = empty_pipeline();
        let content = ContentBlock {
            lines: vec!["# Hello".to_string(), "world".to_string()],
            preceding_command: None,
            start_row: 5,
            end_row: 7,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("markdown", content);

        let indicators = gm.indicators_for_viewport(&pipeline, 0, 24);
        assert_eq!(indicators.len(), 1);
        assert_eq!(indicators[0].row, 5);
        assert_eq!(indicators[0].height, 2);
        assert_eq!(indicators[0].block_id, 0);
    }

    #[test]
    fn test_indicators_block_outside_viewport() {
        let gm = GutterManager::new();
        let mut pipeline = empty_pipeline();
        let content = ContentBlock {
            lines: vec!["data".to_string()],
            preceding_command: None,
            start_row: 100,
            end_row: 101,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("json", content);

        let indicators = gm.indicators_for_viewport(&pipeline, 0, 24);
        assert!(indicators.is_empty());
    }

    #[test]
    fn test_indicators_partially_visible_block() {
        let gm = GutterManager::new();
        let mut pipeline = empty_pipeline();
        let content = ContentBlock {
            lines: (0..10).map(|i| format!("line {i}")).collect(),
            preceding_command: None,
            start_row: 20,
            end_row: 30,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("yaml", content);

        // Viewport starts at row 25, height 10 → sees rows 25..35
        let indicators = gm.indicators_for_viewport(&pipeline, 25, 10);
        assert_eq!(indicators.len(), 1);
        assert_eq!(indicators[0].row, 0); // screen-relative
        assert_eq!(indicators[0].height, 5); // rows 25..30 visible
    }

    #[test]
    fn test_hit_test_in_gutter() {
        let gm = GutterManager::new();
        let indicators = vec![GutterIndicator {
            row: 5,
            height: 3,
            badge: "{}".to_string(),
            view_mode: ViewMode::Rendered,
            hovered: false,
            block_id: 42,
        }];

        // Inside gutter and within block rows.
        assert_eq!(gm.hit_test(0, 5, &indicators), Some(42));
        assert_eq!(gm.hit_test(1, 7, &indicators), Some(42));

        // Outside gutter column.
        assert_eq!(gm.hit_test(2, 5, &indicators), None);

        // Outside block row range.
        assert_eq!(gm.hit_test(0, 4, &indicators), None);
        assert_eq!(gm.hit_test(0, 8, &indicators), None);
    }

    #[test]
    fn test_hit_test_empty() {
        let gm = GutterManager::new();
        assert_eq!(gm.hit_test(0, 0, &[]), None);
    }
}
