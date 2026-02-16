# Step 10: Gutter Indicators, Toggles & Copy

## Summary

Implement the visual gutter indicators that appear next to prettified blocks, the per-block toggle mechanism (click gutter to switch source/rendered), the global toggle keybinding (`Cmd+Shift+M` / `Ctrl+Shift+M`), and the copy behavior that differentiates between rendered and source text. This connects the prettifier system to the existing rendering and input handling.

## Dependencies

- **Step 4**: `PrettifierPipeline` (toggle methods, `block_at_row()`)
- **Step 5**: `DualViewBuffer` (source/rendered text, view mode toggle)
- **Step 8**: At least one renderer exists to produce prettified output

## What to Implement

### New File: `src/prettifier/gutter.rs`

The gutter system renders format badges and toggle indicators in the left margin of prettified blocks.

```rust
/// Manages gutter indicators for prettified content blocks.
pub struct GutterManager {
    /// Width of the gutter column in cells (typically 2-3)
    pub gutter_width: usize,
}

/// A gutter indicator for a single prettified block.
pub struct GutterIndicator {
    /// Row where this indicator should be displayed
    pub row: usize,
    /// Height in rows (spans the full block)
    pub height: usize,
    /// Format badge text (e.g., "📝", "{}", "📊")
    pub badge: String,
    /// Whether the block is in rendered or source view
    pub view_mode: ViewMode,
    /// Whether the mouse is hovering over the gutter area
    pub hovered: bool,
    /// The block ID for click handling
    pub block_id: u64,
}

impl GutterManager {
    /// Generate gutter indicators for all visible prettified blocks.
    pub fn indicators_for_viewport(
        &self,
        pipeline: &PrettifierPipeline,
        viewport_start_row: usize,
        viewport_height: usize,
    ) -> Vec<GutterIndicator> { ... }

    /// Check if a mouse position is within a gutter indicator.
    pub fn hit_test(&self, row: usize, col: usize, indicators: &[GutterIndicator]) -> Option<u64> { ... }
}
```

Format badges from spec (lines 1293–1302):

| Format | Badge |
|--------|-------|
| Markdown | `📝` |
| JSON | `{}` |
| Diagram | `📊` |
| YAML / TOML | `📋` |
| XML | `📄` |
| CSV | `📉` |
| Diff | `±` |
| Log | `📜` |
| Stack trace | `⚠️` |

### Modify: `src/cell_renderer/render.rs`

Add gutter rendering to the cell renderer:

1. Before rendering the main cell grid, check if any prettified blocks intersect the viewport
2. For each visible block, render the gutter indicator in the leftmost columns
3. The gutter uses a slightly different background to visually separate it from content
4. The format badge is centered vertically within the block's row span
5. On hover, show a tooltip-like indicator: "Click to toggle source/pretty"

### Modify: `src/app/mouse_events.rs`

Handle clicks on gutter indicators:

1. On mouse click in the gutter area (leftmost columns), check `GutterManager::hit_test()`
2. If a block is hit, call `PrettifierPipeline::toggle_block(block_id)`
3. Trigger a re-render to reflect the view mode change

### Modify: `src/app/input_events.rs`

Add the global toggle keybinding:

1. Handle `Cmd+Shift+M` (macOS) / `Ctrl+Shift+M` (Linux/Windows)
2. Call `PrettifierPipeline::toggle_global()`
3. This is a session-level toggle — does NOT persist to config (spec line 558)
4. Trigger a re-render for all visible blocks

### Copy Behavior Modification

Modify copy operations to respect prettifier source/rendered preference:

```rust
// In clipboard handling code:
fn get_copy_text(
    selection: &Selection,
    pipeline: &PrettifierPipeline,
    copy_modifier: CopyModifier,
) -> String {
    if let Some(block) = pipeline.block_at_row(selection.start_row) {
        match copy_modifier {
            CopyModifier::Normal => {
                // Copy rendered text (or source if in source view)
                block.buffer.display_text_for_range(selection)
            }
            CopyModifier::Shift => {
                // Always copy source text
                block.buffer.source_text_for_range(selection)
            }
        }
    } else {
        // Not a prettified block — copy raw terminal text as usual
        selection.text()
    }
}
```

Copy behavior from spec (lines 1303–1306):
- Normal copy (`Cmd+C`): copies rendered/formatted text
- `Cmd+Shift+C`: copies raw source
- Vi copy mode: operates on source text

### Modify: `src/app/handler.rs`

Wire the prettifier pipeline into the main event loop:

1. In `about_to_wait()`, after reading terminal output, feed lines to `PrettifierPipeline::process_output()`
2. On OSC 133 markers, call `on_command_start()` / `on_command_end()`
3. On alternate screen change, call `on_alt_screen_change()`
4. Periodically call `check_debounce()`

This is the step where the prettifier becomes "live" — actually processing terminal output and displaying results.

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/gutter.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod gutter;`) |
| Modify | `src/cell_renderer/render.rs` (gutter rendering) |
| Modify | `src/app/mouse_events.rs` (gutter click handling) |
| Modify | `src/app/input_events.rs` (global toggle keybinding) |
| Modify | `src/app/handler.rs` (wire pipeline into event loop) |
| Modify | `src/terminal/clipboard.rs` (source/rendered copy logic) |

## Relevant Spec Sections

- **Lines 1289–1313**: User interaction & toggle controls — global toggle, per-block toggle, gutter indicators, copy behavior
- **Lines 776–778**: Config — global_toggle_key, per_block_toggle
- **Lines 786–790**: Clipboard config — default_copy, source_copy_modifier, vi_copy_mode
- **Lines 1293–1302**: Format indicator gutter — badge characters for each format
- **Lines 556–558**: Cmd+Shift+M session-level toggle
- **Lines 1447–1449**: Acceptance criteria — global toggle, per-block toggle, gutter indicators, copy options

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] Gutter indicators display next to prettified blocks with correct format badges
- [ ] Clicking a gutter indicator toggles the block between rendered and source view
- [ ] `Cmd+Shift+M` / `Ctrl+Shift+M` toggles all prettifying on/off for the session
- [ ] Session toggle does not persist to config file
- [ ] Normal copy (`Cmd+C`) on prettified content copies rendered text
- [ ] `Cmd+Shift+C` on prettified content copies raw source text
- [ ] Non-prettified content copy behavior is unchanged
- [ ] Gutter hover shows visual feedback
- [ ] Pipeline is wired into the main event loop — terminal output is processed
- [ ] OSC 133 markers trigger correct boundary detection
- [ ] Multiple prettified blocks display with independent gutter indicators
- [ ] Toggling one block does not affect others
