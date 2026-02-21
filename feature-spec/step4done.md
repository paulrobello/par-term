# Step 4: Renderer Registry & Detection Pipeline

## Summary

Implement the `RendererRegistry` that maps format IDs to `ContentRenderer` implementations, and the `DetectionPipeline` that orchestrates running detectors against content blocks, picking the best match, and dispatching to the appropriate renderer. This is the central orchestrator of the prettifier system.

## Dependencies

- **Step 1**: Core types and traits (`ContentDetector`, `ContentRenderer`, `DetectionResult`, `ContentBlock`)
- **Step 2**: `RegexDetector` implementation
- **Step 3**: `BoundaryDetector` and `ContentBlock` emission

## What to Implement

### New File: `src/prettifier/registry.rs`

```rust
use std::collections::HashMap;

/// Central registry mapping format IDs to detectors and renderers.
pub struct RendererRegistry {
    /// Registered detectors, sorted by priority (highest first)
    detectors: Vec<(i32, Box<dyn ContentDetector>)>,
    /// Registered renderers, keyed by format_id
    renderers: HashMap<String, Box<dyn ContentRenderer>>,
}

impl RendererRegistry {
    pub fn new() -> Self { ... }

    /// Register a detector with a priority. Higher priority = checked first.
    pub fn register_detector(&mut self, priority: i32, detector: Box<dyn ContentDetector>) { ... }

    /// Register a renderer for a format_id.
    pub fn register_renderer(&mut self, format_id: &str, renderer: Box<dyn ContentRenderer>) { ... }

    /// Get a renderer by format_id.
    pub fn get_renderer(&self, format_id: &str) -> Option<&dyn ContentRenderer> { ... }

    /// List all registered format IDs and display names (for Settings UI subtitle).
    pub fn registered_formats(&self) -> Vec<(&str, &str)> { ... }

    /// Run all detectors against a content block. Returns the best match
    /// (highest confidence above threshold), or None.
    pub fn detect(&self, content: &ContentBlock) -> Option<DetectionResult> { ... }
}
```

**Detection algorithm** in `detect()`:

1. For each detector (sorted by priority, highest first):
   a. Call `quick_match()` on the first 5 lines — skip if false
   b. Call `detect()` — collect result if Some
2. From all detection results, pick the one with highest confidence
3. If confidence >= global threshold, return it; otherwise return None

This implements the left-hand path ("Content Boundary Detector → Regex-Based Format Detection Pipeline → Renderer Registry") from the pipeline diagram (spec lines 564–625).

### New File: `src/prettifier/pipeline.rs`

The pipeline orchestrator ties boundary detection, format detection, and rendering together.

```rust
/// Orchestrates the full prettifier pipeline:
/// boundary detection → format detection → rendering → caching
pub struct PrettifierPipeline {
    boundary_detector: BoundaryDetector,
    registry: RendererRegistry,
    /// Track prettified blocks for toggle, copy, and display
    active_blocks: Vec<PrettifiedBlock>,
    /// Master enable/disable
    enabled: bool,
    /// Session-level toggle (from Cmd+Shift+M)
    session_override: Option<bool>,
}

/// A content block that has been detected and (optionally) rendered.
pub struct PrettifiedBlock {
    pub content: ContentBlock,
    pub detection: DetectionResult,
    pub rendered: Option<RenderedContent>,
    pub view_mode: ViewMode,
    /// Unique ID for this block
    pub block_id: u64,
}

impl PrettifierPipeline {
    pub fn new(config: PrettifierConfig, registry: RendererRegistry) -> Self { ... }

    /// Called when new terminal output arrives. Feeds lines to boundary detector,
    /// runs detection on emitted blocks, triggers rendering.
    pub fn process_output(&mut self, line: &str, row: usize) { ... }

    /// Called on OSC 133 markers.
    pub fn on_command_start(&mut self, command: &str) { ... }
    pub fn on_command_end(&mut self) { ... }

    /// Called on alternate screen transitions.
    pub fn on_alt_screen_change(&mut self, entering: bool) { ... }

    /// Periodic check for debounce (call from event loop).
    pub fn check_debounce(&mut self) { ... }

    /// Handle a trigger-based prettify action (bypasses confidence scoring).
    pub fn trigger_prettify(&mut self, format_id: &str, content: ContentBlock) { ... }

    /// Toggle global prettifying on/off (session-level, Cmd+Shift+M).
    pub fn toggle_global(&mut self) { ... }

    /// Toggle a specific block between rendered and source view.
    pub fn toggle_block(&mut self, block_id: u64) { ... }

    /// Get all active prettified blocks (for rendering).
    pub fn active_blocks(&self) -> &[PrettifiedBlock] { ... }

    /// Get a specific block by row range (for gutter display, click handling).
    pub fn block_at_row(&self, row: usize) -> Option<&PrettifiedBlock> { ... }

    /// Check if prettifying is currently enabled (considering master toggle + session override).
    pub fn is_enabled(&self) -> bool { ... }
}
```

**Internal flow when a block is emitted from boundary detection:**

1. If `!self.is_enabled()`, discard
2. Call `registry.detect(&block)`
3. If detection succeeds, create a `PrettifiedBlock` with `view_mode: ViewMode::Rendered`
4. Call `registry.get_renderer(format_id)` and invoke `render()`
5. Store the result in `active_blocks`
6. (In later steps, this will trigger a display update)

### Placeholder Config

```rust
pub struct PrettifierConfig {
    pub enabled: bool,
    pub respect_alternate_screen: bool,
    pub confidence_threshold: f32,
    pub max_scan_lines: usize,
    pub debounce_ms: u64,
    pub detection_scope: DetectionScope,
}
```

This will be expanded in Step 6 (Config Integration) to be loaded from `config.yaml`.

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/registry.rs` |
| Create | `src/prettifier/pipeline.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod registry; pub mod pipeline;`) |

## Relevant Spec Sections

- **Lines 564–625**: Full detection pipeline diagram with both trigger and auto-detection paths
- **Lines 601–609**: Renderer Registry box — maps format_id to ContentRenderer, checks capabilities, applies profile overrides
- **Lines 610–618**: Render & Cache Manager — async rendering, caching, placeholder display, dual view
- **Lines 793–827**: Per-renderer enable/disable and priority settings
- **Lines 1334**: Phase 0 framework description — registry, dual-view buffer management, toggle
- **Lines 1346–1348**: Performance — detection at content boundaries, quick_match() fast rejection

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `RendererRegistry::detect()` runs detectors in priority order
- [ ] `quick_match()` pre-filter is applied before full detection
- [ ] Highest-confidence result wins when multiple detectors match
- [ ] Global confidence threshold is respected
- [ ] `PrettifierPipeline` correctly wires boundary detector → registry → rendering
- [ ] `trigger_prettify()` bypasses confidence scoring and dispatches directly
- [ ] `toggle_global()` and `toggle_block()` correctly switch view modes
- [ ] `is_enabled()` respects both master toggle and session override
- [ ] `block_at_row()` finds the correct block for a given terminal row
- [ ] Unit tests for registry detection ordering, pipeline flow, and toggle behavior
