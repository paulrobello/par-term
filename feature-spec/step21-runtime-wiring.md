# Step 21: Runtime Wiring — Connect Prettifier Pipeline to Live Terminal

## Summary

Steps 1–19 built a complete, well-tested Content Prettifier library (590 unit tests, 11 detectors, 11 renderers, config, settings UI, trigger integration). Step 20 verified that the library works correctly in isolation but is **functionally disconnected** from the live terminal.

Step 21 wires the three remaining integration gaps so the prettifier produces visible results at runtime.

## Current State (post step 20)

| Component | Status |
|-----------|--------|
| Pipeline instantiation (`Tab::new`, `Tab::from_config`) | **Wired** — `create_pipeline_from_config(config)` |
| Alt-screen transitions (`on_alt_screen_change`) | **Wired** — `window_state.rs:2687` |
| Debounce check (`check_debounce`) | **Wired** — `window_state.rs:2694` |
| Global toggle keybinding (`toggle_prettifier`) | **Wired** — `input_events.rs:1214` |
| Gutter click handling | **Wired** — `mouse_events.rs:350-371` |
| Copy integration (`get_prettifier_copy_text`) | **Wired** — `text_selection.rs:347`, `input_events.rs:278` |
| Trigger dispatch (`Prettify` action) | **Wired** — `triggers.rs:327-429` |
| **Terminal output → `process_output()`** | **NOT WIRED** |
| **OSC 133 markers → `on_command_start()`/`on_command_end()`** | **NOT WIRED** |
| **Cell renderer → display rendered content** | **NOT WIRED** |

---

## Gap A: Feed Terminal Output to Pipeline

### Problem

`pipeline.process_output(line, row)` is defined in `src/prettifier/pipeline.rs:155` but never called from production code. The prettifier boundary detector accumulates lines and emits content blocks at boundaries — without feeding it lines, no detection or rendering ever occurs.

### Where to Wire

**File**: `src/app/window_state.rs`, in the render loop after `update_scrollback_metadata` (currently around line 2758) and before the existing prettifier pipeline update block (lines 2681–2697).

### Implementation

After the terminal lock is released (around line 2679), extract text lines from the freshly-generated `cells` array and feed them to the pipeline. The cells are a flat `Vec<StyledCell>` representing the visible grid; extract one text line per row.

```rust
// --- Feed terminal output to prettifier ---
// Extract text lines from the visible cells and feed new lines to the
// pipeline. Only feed lines that have changed since the last frame to
// avoid redundant detection work.
if let Some(tab) = self.tab_manager.active_tab_mut() {
    if let Some(ref mut pipeline) = tab.prettifier {
        if pipeline.is_enabled() && !is_alt_screen {
            let cols = self.renderer.as_ref().map(|r| r.grid_size().0).unwrap_or(80);
            let rows = self.renderer.as_ref().map(|r| r.grid_size().1).unwrap_or(24);
            let scroll_offset = tab.scroll_state.offset;

            for row_idx in 0..rows {
                // Build text line from cells for this row
                let start = row_idx * cols;
                let end = (start + cols).min(cells.len());
                if start >= cells.len() {
                    break;
                }
                let line: String = cells[start..end]
                    .iter()
                    .map(|c| if c.c == '\0' { ' ' } else { c.c })
                    .collect::<String>()
                    .trim_end()
                    .to_string();

                // Absolute row = scrollback_len - scroll_offset + row_idx
                // This matches the absolute line numbers used by scrollback marks
                let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;
                pipeline.process_output(&line, absolute_row);
            }
        }
    }
}
```

### Important Notes

- **Only feed when not in alt-screen**: TUI apps (vim, htop) use the alternate screen buffer; prettifying makes no sense there. The pipeline already has `respect_alternate_screen` config, but skipping the feed entirely is more efficient.
- **Deduplication**: The boundary detector already handles duplicate lines (it accumulates and emits at boundaries). Feeding the same visible lines every frame is cheap — `process_output` just calls `push_line` on the boundary detector, which is a string copy + timestamp check. However, for performance, consider tracking a "last fed generation" counter and only feeding when `current_generation != last_fed_generation`.
- **Cell text extraction**: `StyledCell.c` is the character. Null chars (`\0`) represent empty cells and should be treated as spaces. Trailing whitespace should be trimmed.

---

## Gap B: Forward OSC 133 Shell Markers to Pipeline

### Problem

`pipeline.on_command_start(command)` and `pipeline.on_command_end()` are defined but never called. The boundary detector needs these to know when commands start/end for `detection_scope: "command_output"` mode (the default).

### Where to Wire

**File**: `par-term-terminal/src/terminal/mod.rs`, inside `update_scrollback_metadata()` (line 141+), where shell integration events are already being processed.

The function already iterates over `shell_events` and matches `event_type` strings. The prettifier hooks should be called at the same points.

### Implementation

The challenge is that `update_scrollback_metadata` runs inside `Terminal` (in the `par-term-terminal` crate), which has no access to the `Tab` or `PrettifierPipeline`. There are two approaches:

**Approach A (recommended): Event queue + drain in render loop**

Add a small event queue to `Terminal` that records shell lifecycle events. Drain it in the render loop (where `tab.prettifier` is accessible).

In `par-term-terminal/src/terminal/mod.rs`:
```rust
/// Shell lifecycle events for the prettifier pipeline.
#[derive(Debug, Clone)]
pub enum ShellLifecycleEvent {
    CommandStarted { command: String, absolute_line: usize },
    CommandFinished { absolute_line: usize },
}

// Add to Terminal struct:
pub(crate) shell_lifecycle_events: Vec<ShellLifecycleEvent>,

// In update_scrollback_metadata, after processing each shell event:
match event_type.as_str() {
    "command_executed" => {
        let cmd_text = event_command.clone()
            .or_else(|| self.captured_command_text.as_ref().map(|(_, t)| t.clone()))
            .unwrap_or_default();
        // ... existing code ...
        // NEW: Queue for prettifier
        self.shell_lifecycle_events.push(
            ShellLifecycleEvent::CommandStarted {
                command: cmd_text.clone(),
                absolute_line: abs_line,
            }
        );
    }
    "command_finished" => {
        // ... existing code ...
        // NEW: Queue for prettifier
        self.shell_lifecycle_events.push(
            ShellLifecycleEvent::CommandFinished {
                absolute_line: abs_line,
            }
        );
    }
    _ => {}
}

// Add drain method:
pub fn drain_shell_lifecycle_events(&mut self) -> Vec<ShellLifecycleEvent> {
    std::mem::take(&mut self.shell_lifecycle_events)
}
```

In `src/app/window_state.rs`, inside the terminal lock block (around line 2758), after `update_scrollback_metadata`:
```rust
// Drain shell lifecycle events for prettifier
let shell_events_for_prettifier = term.drain_shell_lifecycle_events();
```

Then after the lock is released, in the prettifier update block (lines 2681–2697):
```rust
// Forward shell lifecycle events to prettifier
if let Some(ref mut pipeline) = tab.prettifier {
    for event in &shell_events_for_prettifier {
        match event {
            ShellLifecycleEvent::CommandStarted { command, .. } => {
                pipeline.on_command_start(command);
            }
            ShellLifecycleEvent::CommandFinished { .. } => {
                pipeline.on_command_end();
            }
        }
    }
}
```

**Approach B (simpler but coarser): Derive from scrollback marks**

Instead of adding an event queue, detect command boundaries by comparing the current `scrollback_marks` against the previously-seen set. When a new mark appears with a `command` field, call `on_command_start`; when its `exit_code` becomes `Some`, call `on_command_end`. This is simpler but introduces a one-frame delay and doesn't capture the exact command text at the right moment.

**Recommendation**: Approach A is cleaner, more precise, and only adds ~20 lines of code to the terminal crate.

---

## Gap C: Display Rendered Content in Cell Renderer

### Problem

The cell renderer (`src/cell_renderer.rs`) and GPU renderer (`src/renderer.rs`) have zero references to any prettifier type. Even if detection and rendering ran, the results would never appear on screen.

### Design Considerations

This is the most architecturally significant gap. The prettifier produces `StyledLine` objects (text + fg/bg/bold/italic/underline/link per segment), but the cell renderer expects `StyledCell` arrays. There are two strategies:

**Strategy 1 (recommended): Cell substitution in the render loop**

Before passing `cells` to `renderer.update_cells()`, check if any visible rows belong to prettified blocks. If so, replace the raw `StyledCell` values for those rows with cells derived from the prettifier's `StyledLine` output.

This keeps the change contained in `window_state.rs` and requires no changes to the cell renderer or GPU shaders.

**Strategy 2: Overlay rendering via egui**

Render prettified content as an egui overlay on top of the terminal grid. This allows richer formatting (variable-height content, images) but is more complex and may have z-ordering issues.

### Implementation (Strategy 1)

**File**: `src/app/window_state.rs`, between cell generation (line ~2658) and `renderer.update_cells(&cells)` (line ~3509).

```rust
// --- Prettifier cell substitution ---
// Replace raw terminal cells with rendered content for prettified rows.
if let Some(tab) = self.tab_manager.active_tab() {
    if let Some(ref pipeline) = tab.prettifier {
        if pipeline.is_enabled() {
            let cols = self.renderer.as_ref().map(|r| r.grid_size().0).unwrap_or(80);
            let scroll_offset = tab.scroll_state.offset;
            let viewport_rows = self.renderer.as_ref().map(|r| r.grid_size().1).unwrap_or(24);

            for viewport_row in 0..viewport_rows {
                let absolute_row = scrollback_len.saturating_sub(scroll_offset) + viewport_row;

                if let Some(block) = pipeline.block_at_row(absolute_row) {
                    if !block.has_rendered() {
                        continue; // Detection ran but rendering failed — show raw
                    }

                    let display_lines = block.buffer.display_lines();
                    // Map absolute_row to offset within the block
                    let block_start = block.content().start_row;
                    let line_offset = absolute_row.saturating_sub(block_start);

                    if let Some(styled_line) = display_lines.get(line_offset) {
                        let cell_start = viewport_row * cols;
                        let cell_end = (cell_start + cols).min(cells.len());

                        // Clear the row first
                        for cell in &mut cells[cell_start..cell_end] {
                            cell.c = ' ';
                            cell.fg = [0xFF, 0xFF, 0xFF, 0xFF]; // default fg
                            cell.bg = [0x00, 0x00, 0x00, 0x00]; // default bg
                            cell.flags = 0;
                        }

                        // Write styled segments into cells
                        let mut col = 0;
                        for segment in &styled_line.segments {
                            for ch in segment.text.chars() {
                                if col >= cols {
                                    break;
                                }
                                let idx = cell_start + col;
                                if idx < cells.len() {
                                    cells[idx].c = ch;
                                    // Map StyledSegment colors to cell fg/bg
                                    if let Some([r, g, b]) = segment.fg {
                                        cells[idx].fg = [r, g, b, 0xFF];
                                    }
                                    if let Some([r, g, b]) = segment.bg {
                                        cells[idx].bg = [r, g, b, 0xFF];
                                    }
                                    // Map style flags
                                    if segment.bold {
                                        cells[idx].flags |= BOLD_FLAG;
                                    }
                                    if segment.italic {
                                        cells[idx].flags |= ITALIC_FLAG;
                                    }
                                    if segment.underline {
                                        cells[idx].flags |= UNDERLINE_FLAG;
                                    }
                                }
                                col += 1;
                            }
                        }
                    }
                }
            }
        }
    }
}
```

### Gutter Indicator Rendering

The `GutterManager` already computes indicator positions and handles hit-testing. To actually draw gutter badges (e.g., `MD`, `{}`, `±`), reserve the leftmost 2–3 columns of prettified rows for the badge text, rendered with a distinct background color.

This can be done in the same cell substitution pass:
```rust
// Draw gutter indicator in first 2–3 columns of the block's first row
if line_offset == 0 {
    let badge = block.detection.format_id.as_str(); // or block.format_badge()
    let badge_chars: Vec<char> = match badge {
        "markdown" => "MD".chars().collect(),
        "json" => "{}".chars().collect(),
        "diff" => "± ".chars().collect(),
        _ => badge.chars().take(2).collect(),
    };
    for (i, ch) in badge_chars.iter().enumerate() {
        if i < cols {
            let idx = cell_start + i;
            cells[idx].c = *ch;
            cells[idx].fg = [0x80, 0x80, 0x80, 0xFF]; // dim
            cells[idx].bg = [0x30, 0x30, 0x30, 0xFF]; // subtle bg
        }
    }
}
```

---

## Gap D: Missing "Test Rules" UI Feature

### Problem

Spec extensibility criterion #7 requires a "Test rules" feature in Settings UI — users should be able to paste sample content and see which regex rules fire, their weights, and the resulting confidence score. This is not implemented.

### Implementation

**File**: `par-term-settings-ui/src/prettifier_tab.rs`, in the renderers section (inside or adjacent to each per-renderer card).

Add a collapsible "Test Detection" panel per renderer:

1. A multiline `TextEdit` for pasting sample content
2. A "Test" button that:
   - Creates a `ContentBlock` from the pasted text
   - Runs the detector for this format against it
   - Displays: which rules matched, their individual weights, total confidence score
3. A results display showing:
   - Table of matched rules (ID, pattern excerpt, weight, scope)
   - Total confidence score with pass/fail indicator (vs threshold)
   - "Detected: Yes/No" summary

This requires the settings UI to have access to the detector instances, which means either:
- Passing detector references into the settings UI (complex, breaks crate boundaries)
- Providing a static `test_detection(format_id, text) -> TestResult` function in `prettifier::config_bridge` that creates a temporary detector and runs it

The second approach is cleaner:
```rust
// In src/prettifier/config_bridge.rs
pub struct DetectionTestResult {
    pub matched_rules: Vec<(String, f32)>,  // (rule_id, weight)
    pub total_confidence: f32,
    pub detected: bool,
    pub threshold: f32,
}

pub fn test_detection(format_id: &str, sample_text: &str, config: &Config) -> Option<DetectionTestResult> {
    // Build a temporary detector for this format from config
    // Run detection against a ContentBlock built from sample_text
    // Return rule match details
}
```

---

## Execution Order

1. **Gap B first** — Add `ShellLifecycleEvent` queue to `par-term-terminal` and drain in render loop. This is a cross-crate change and should be done first to unblock command-scoped detection.
2. **Gap A second** — Feed terminal output lines to `process_output()`. With Gap B in place, the pipeline can properly scope detection to command output.
3. **Gap C third** — Cell substitution to display rendered content. This is the most visible change and depends on A+B producing prettified blocks.
4. **Gap D last** — "Test rules" UI is a nice-to-have that doesn't block runtime functionality.

## Testing

After wiring:
1. Run `make run` and execute `echo "# Hello World"` — should see rendered markdown header
2. Run `cat` on a JSON file — should see syntax-highlighted JSON
3. Run `git diff` — should see colored diff output
4. Open Settings > Content Prettifier — verify all 7 sections render correctly
5. Toggle prettifier with `Ctrl+Shift+P` — rendered blocks should revert to source
6. Click a gutter badge — that block should toggle between source/rendered

## Files Modified

| File | Change |
|------|--------|
| `par-term-terminal/src/terminal/mod.rs` | Add `ShellLifecycleEvent` enum, event queue, drain method |
| `src/app/window_state.rs` | Feed output to `process_output()`, drain shell events, cell substitution |
| `src/prettifier/config_bridge.rs` | Add `test_detection()` for Settings UI |
| `par-term-settings-ui/src/prettifier_tab.rs` | Add "Test Detection" panel per renderer |
