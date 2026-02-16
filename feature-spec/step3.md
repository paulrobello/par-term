# Step 3: Content Boundary Detection

## Summary

Implement the content boundary detection system that identifies discrete blocks of terminal output to feed into the detection pipeline. This uses OSC 133 shell integration markers, alternate screen transitions, blank-line heuristics, and process change events to determine where one "content block" ends and another begins.

## Dependencies

- **Step 1**: `ContentBlock` type

## What to Implement

### New File: `src/prettifier/boundary.rs`

The boundary detector sits between the terminal output stream and the detection pipeline. It accumulates lines and emits `ContentBlock` instances at natural boundaries.

```rust
/// Detects content block boundaries in terminal output.
pub struct BoundaryDetector {
    /// Lines accumulated for the current block
    current_lines: Vec<String>,
    /// The command that produced the current output (from OSC 133)
    current_command: Option<String>,
    /// Row where the current block started
    block_start_row: usize,
    /// Configuration
    config: BoundaryConfig,
    /// Debounce timer — wait for output to settle
    last_output_time: std::time::Instant,
}

pub struct BoundaryConfig {
    /// How to detect boundaries
    pub scope: DetectionScope,
    /// Maximum lines to accumulate before forcing a boundary
    pub max_scan_lines: usize,
    /// Wait this long after last output before triggering detection
    pub debounce_ms: u64,
}

pub enum DetectionScope {
    /// Detect only within command output boundaries (OSC 133)
    CommandOutput,
    /// Detect in all terminal output
    All,
    /// Only detect when manually triggered
    ManualOnly,
}
```

**Boundary events** that trigger block emission:

1. **OSC 133 command end marker** (`\x1b]133;D\x07`) — the primary signal. When shell integration is active, this precisely marks the end of a command's output. The preceding `\x1b]133;C\x07` (command start) marks the beginning.
2. **Alternate screen enter/exit** — content before entering alt screen (e.g., opening vim) forms a block; content after leaving alt screen forms a new block.
3. **Blank line heuristic** — when shell integration is not available, use multiple consecutive blank lines as a boundary signal.
4. **Process change** — when the foreground process changes (detected via PTY), emit the current block.
5. **Max lines reached** — if `max_scan_lines` is reached without a boundary, force-emit the block.
6. **Debounce timeout** — after `debounce_ms` milliseconds of no new output, emit the current block.

```rust
impl BoundaryDetector {
    pub fn new(config: BoundaryConfig) -> Self { ... }

    /// Feed a new line of terminal output. Returns a ContentBlock if a boundary was detected.
    pub fn push_line(&mut self, line: &str, row: usize) -> Option<ContentBlock> { ... }

    /// Notify of an OSC 133 command start (C marker).
    pub fn on_command_start(&mut self, command: &str) { ... }

    /// Notify of an OSC 133 command end (D marker). Emits the accumulated block.
    pub fn on_command_end(&mut self) -> Option<ContentBlock> { ... }

    /// Notify of alternate screen transition.
    pub fn on_alt_screen_change(&mut self, entering: bool) -> Option<ContentBlock> { ... }

    /// Check if debounce timer has expired. Call periodically from the event loop.
    pub fn check_debounce(&mut self) -> Option<ContentBlock> { ... }

    /// Force-emit the current block (for manual trigger).
    pub fn flush(&mut self) -> Option<ContentBlock> { ... }

    /// Reset state (e.g., on terminal clear).
    pub fn reset(&mut self) { ... }
}
```

### Integration Points

This step defines the boundary detector but does NOT integrate it into the main app event loop (that happens in Step 4). However, the design must be compatible with the data flow in `src/app/handler.rs` where terminal output is read.

The boundary detector will be called from the main event loop in Step 4:
- After `TerminalManager::read()` produces new output
- On OSC 133 markers (already parsed by par-term-emu-core-rust)
- On alternate screen transitions
- Periodically for debounce checks in `about_to_wait()`

### Interaction with `src/scrollback_metadata.rs`

If this file exists, the boundary detector should leverage any existing command boundary metadata. If not, the boundary detector is self-contained.

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/boundary.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod boundary;`) |

## Relevant Spec Sections

- **Lines 564–625**: Detection pipeline diagram — shows Content Boundary Detector as the entry point
- **Lines 779–785**: Detection settings — `scope`, `max_scan_lines`, `debounce_ms`
- **Lines 1324**: Shell integration (OSC 133) command boundaries
- **Lines 1346–1348**: Performance — detection runs only at content boundaries, never per-byte
- **Lines 23–25**: Existing features — shell integration OSC markers, command separator lines

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `BoundaryDetector` correctly emits `ContentBlock` on OSC 133 command end
- [ ] `BoundaryDetector` emits block on alternate screen transition
- [ ] Blank-line heuristic works when OSC 133 is not available
- [ ] `max_scan_lines` limit forces block emission
- [ ] Debounce timer correctly delays emission until output settles
- [ ] `preceding_command` is correctly populated from OSC 133 command start marker
- [ ] `row_range` correctly tracks the start and end rows of each block
- [ ] `DetectionScope::ManualOnly` suppresses all automatic block emission
- [ ] `flush()` emits the current accumulated block regardless of boundary signals
- [ ] Unit tests for each boundary event type
