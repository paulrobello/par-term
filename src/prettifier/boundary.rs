//! Content boundary detection for the Content Prettifier framework.
//!
//! `BoundaryDetector` identifies where one content block ends and another begins
//! in the terminal output stream. It accumulates lines and emits `ContentBlock`
//! instances at natural boundaries such as OSC 133 command markers, blank-line
//! runs, or debounce timeouts.

use std::time::Instant;

use super::types::ContentBlock;

/// Controls *when* boundaries are detected.
///
/// Distinct from `RuleScope` (which controls *where* regexes apply within a block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionScope {
    /// Only accumulate between OSC 133 C→D markers (requires shell integration).
    CommandOutput,
    /// Accumulate all output; use blank-line heuristic + debounce as boundaries.
    All,
    /// Never auto-emit; only `flush()` produces blocks.
    ManualOnly,
}

/// Configuration for the `BoundaryDetector`.
#[derive(Debug, Clone)]
pub struct BoundaryConfig {
    /// When to detect boundaries.
    pub scope: DetectionScope,
    /// Maximum lines to accumulate before forcing emission.
    pub max_scan_lines: usize,
    /// Milliseconds of inactivity before emitting a block.
    pub debounce_ms: u64,
    /// Consecutive blank lines required to trigger a boundary in `All` mode.
    pub blank_line_threshold: usize,
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            scope: DetectionScope::All,
            max_scan_lines: 500,
            debounce_ms: 100,
            blank_line_threshold: 2,
        }
    }
}

/// Detects content block boundaries in the terminal output stream.
///
/// Accumulates lines and emits `ContentBlock` instances when a natural boundary
/// is reached (command end marker, blank-line run, max lines, or debounce timeout).
pub struct BoundaryDetector {
    /// Accumulated lines for the current block.
    current_lines: Vec<String>,
    /// The command that preceded this output (from OSC 133 C marker).
    current_command: Option<String>,
    /// Row where the current block started.
    block_start_row: usize,
    /// Configuration.
    config: BoundaryConfig,
    /// Timestamp of the last `push_line` call (for debounce).
    last_output_time: Instant,
    /// Whether we are between OSC 133 C and D markers.
    in_command_output: bool,
    /// Consecutive blank lines seen (for blank-line heuristic in `All` mode).
    consecutive_blank_lines: usize,
    /// Whether we are inside a fenced code block (``` or ~~~).
    /// Suppresses blank-line boundaries to keep markdown content together.
    in_fenced_block: bool,
    /// The fence marker character ('`' or '~') for the current fenced block.
    fence_char: Option<char>,
}

impl BoundaryDetector {
    /// Create a new `BoundaryDetector` with the given configuration.
    pub fn new(config: BoundaryConfig) -> Self {
        Self {
            current_lines: Vec::new(),
            current_command: None,
            block_start_row: 0,
            config,
            last_output_time: Instant::now(),
            in_command_output: false,
            consecutive_blank_lines: 0,
            in_fenced_block: false,
            fence_char: None,
        }
    }

    /// Accumulate a line of terminal output.
    ///
    /// May emit a `ContentBlock` if:
    /// - `max_scan_lines` is reached
    /// - The blank-line heuristic triggers (in `All` scope only)
    ///
    /// In `CommandOutput` scope, lines outside C→D region are ignored.
    /// In `ManualOnly` scope, never auto-emits.
    pub fn push_line(&mut self, line: &str, row: usize) -> Option<ContentBlock> {
        match self.config.scope {
            DetectionScope::CommandOutput => {
                if !self.in_command_output {
                    crate::debug_trace!(
                        "PRETTIFIER",
                        "boundary::push_line IGNORED (CommandOutput, not in_cmd) row={}: {:?}",
                        row,
                        &line[..line.len().min(80)]
                    );
                    return None;
                }
                crate::debug_trace!(
                    "PRETTIFIER",
                    "boundary::push_line (CommandOutput, in_cmd=true) row={}: {:?}",
                    row,
                    &line[..line.len().min(80)]
                );
            }
            DetectionScope::ManualOnly => {
                self.last_output_time = Instant::now();
                if self.current_lines.is_empty() {
                    self.block_start_row = row;
                }
                self.current_lines.push(line.to_string());
                self.consecutive_blank_lines = 0;
                crate::debug_trace!(
                    "PRETTIFIER",
                    "boundary::push_line (ManualOnly) row={}, accumulated={}: {:?}",
                    row,
                    self.current_lines.len(),
                    &line[..line.len().min(80)]
                );
                return None;
            }
            DetectionScope::All => {
                crate::debug_trace!(
                    "PRETTIFIER",
                    "boundary::push_line (All) row={}, accumulated={}: {:?}",
                    row,
                    self.current_lines.len(),
                    &line[..line.len().min(80)]
                );
            }
        }

        self.last_output_time = Instant::now();

        if self.current_lines.is_empty() {
            self.block_start_row = row;
        }

        // Track fenced code blocks (``` or ~~~) to suppress blank-line
        // boundaries inside them. This keeps markdown content together.
        self.update_fence_state(line);

        // Blank-line heuristic (All scope only).
        // Suppressed inside fenced code blocks to avoid splitting markdown.
        if self.config.scope == DetectionScope::All
            && line.trim().is_empty()
            && !self.in_fenced_block
        {
            self.consecutive_blank_lines += 1;
            if self.consecutive_blank_lines >= self.config.blank_line_threshold {
                crate::debug_log!(
                    "PRETTIFIER",
                    "boundary: blank-line boundary triggered at row={} (consecutive_blanks={}, threshold={})",
                    row,
                    self.consecutive_blank_lines,
                    self.config.blank_line_threshold
                );
                // Emit the non-blank lines accumulated before this blank run.
                let block = self.emit_block();
                // Discard the blank lines — don't add them to the new accumulator.
                self.consecutive_blank_lines = 0;
                return block;
            }
            self.current_lines.push(line.to_string());
            return None;
        }

        if !self.in_fenced_block {
            self.consecutive_blank_lines = 0;
        }
        self.current_lines.push(line.to_string());

        // Max-lines boundary.
        if self.current_lines.len() >= self.config.max_scan_lines {
            crate::debug_log!(
                "PRETTIFIER",
                "boundary: max_scan_lines boundary triggered at row={} (lines={}, max={})",
                row,
                self.current_lines.len(),
                self.config.max_scan_lines
            );
            return self.emit_block();
        }

        None
    }

    /// Signal that a command is starting (OSC 133 C marker).
    ///
    /// Sets command context, enables accumulation in `CommandOutput` scope,
    /// and clears any pre-command noise.
    pub fn on_command_start(&mut self, command: &str) {
        crate::debug_info!(
            "PRETTIFIER",
            "on_command_start: {:?}",
            &command[..command.len().min(80)]
        );
        self.current_command = Some(command.to_string());
        self.in_command_output = true;
        self.current_lines.clear();
        self.consecutive_blank_lines = 0;
    }

    /// Signal that a command has ended (OSC 133 D marker).
    ///
    /// Emits accumulated lines as a `ContentBlock`. In `ManualOnly` scope,
    /// returns `None`.
    pub fn on_command_end(&mut self) -> Option<ContentBlock> {
        crate::debug_info!(
            "PRETTIFIER",
            "on_command_end: accumulated {} lines",
            self.current_lines.len()
        );
        self.in_command_output = false;
        if self.config.scope == DetectionScope::ManualOnly {
            return None;
        }
        self.emit_block()
    }

    /// Signal that the terminal entered or exited the alternate screen.
    ///
    /// Emits the current block on either transition. In `ManualOnly` scope,
    /// returns `None`.
    pub fn on_alt_screen_change(&mut self, _entering: bool) -> Option<ContentBlock> {
        if self.config.scope == DetectionScope::ManualOnly {
            return None;
        }
        self.emit_block()
    }

    /// Signal that the foreground process changed.
    ///
    /// Emits the current block. In `ManualOnly` scope, returns `None`.
    pub fn on_process_change(&mut self) -> Option<ContentBlock> {
        if self.config.scope == DetectionScope::ManualOnly {
            return None;
        }
        self.emit_block()
    }

    /// Check whether the debounce timeout has elapsed.
    ///
    /// If elapsed >= `debounce_ms` and there are pending lines, emits a block.
    /// In `ManualOnly` scope, always returns `None`.
    pub fn check_debounce(&mut self) -> Option<ContentBlock> {
        if self.config.scope == DetectionScope::ManualOnly {
            return None;
        }
        if self.current_lines.is_empty() {
            return None;
        }
        let elapsed = self.last_output_time.elapsed().as_millis() as u64;
        if elapsed >= self.config.debounce_ms {
            crate::debug_log!(
                "PRETTIFIER",
                "boundary: debounce fired after {}ms (threshold={}ms), pending_lines={}",
                elapsed,
                self.config.debounce_ms,
                self.current_lines.len()
            );
            return self.emit_block();
        }
        None
    }

    /// Force-emit the current block regardless of scope.
    ///
    /// Works in all scopes including `ManualOnly`.
    pub fn flush(&mut self) -> Option<ContentBlock> {
        self.emit_block()
    }

    /// Get the configured detection scope.
    pub fn scope(&self) -> DetectionScope {
        self.config.scope
    }

    /// Clear all accumulated state.
    pub fn reset(&mut self) {
        self.current_lines.clear();
        self.current_command = None;
        self.block_start_row = 0;
        self.in_command_output = false;
        self.consecutive_blank_lines = 0;
        self.in_fenced_block = false;
        self.fence_char = None;
    }

    /// Track fenced code block boundaries (``` or ~~~).
    ///
    /// An opening fence is ``` or ~~~ (3+ chars) optionally followed by a
    /// language tag. A closing fence is the same character (3+ chars) with
    /// no trailing content except whitespace.
    fn update_fence_state(&mut self, line: &str) {
        let trimmed = line.trim();

        if self.in_fenced_block {
            // Look for a closing fence: same character, 3+ repetitions, no other content.
            if let Some(ch) = self.fence_char {
                let fence_len = trimmed.len() - trimmed.trim_start_matches(ch).len();
                if fence_len >= 3 && trimmed[fence_len..].trim().is_empty() {
                    crate::debug_trace!(
                        "PRETTIFIER",
                        "boundary: closing fence detected ('{}'x{})",
                        ch,
                        fence_len
                    );
                    self.in_fenced_block = false;
                    self.fence_char = None;
                }
            }
        } else {
            // Look for an opening fence: ``` or ~~~ (3+ chars) with optional language tag.
            let ch = if trimmed.starts_with("```") {
                Some('`')
            } else if trimmed.starts_with("~~~") {
                Some('~')
            } else {
                None
            };

            if let Some(ch) = ch {
                let fence_len = trimmed.len() - trimmed.trim_start_matches(ch).len();
                let rest = trimmed[fence_len..].trim();
                // Opening fence: rest must be empty or a valid language tag
                // (alphanumeric, hyphens, underscores, plus signs).
                if rest.is_empty()
                    || rest
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '+')
                {
                    crate::debug_trace!(
                        "PRETTIFIER",
                        "boundary: opening fence detected ('{}'x{}, lang={:?})",
                        ch,
                        fence_len,
                        if rest.is_empty() { None } else { Some(rest) }
                    );
                    self.in_fenced_block = true;
                    self.fence_char = Some(ch);
                }
            }
        }
    }

    /// Whether there are accumulated lines waiting to be emitted.
    pub fn has_pending_lines(&self) -> bool {
        !self.current_lines.is_empty()
    }

    /// Number of accumulated lines.
    pub fn pending_line_count(&self) -> usize {
        self.current_lines.len()
    }

    /// Build a `ContentBlock` from accumulated state and reset the accumulator.
    ///
    /// Trims trailing blank lines. Returns `None` if no non-blank content remains.
    fn emit_block(&mut self) -> Option<ContentBlock> {
        if self.current_lines.is_empty() {
            crate::debug_trace!("PRETTIFIER", "boundary::emit_block: no lines accumulated, returning None");
            return None;
        }

        let original_count = self.current_lines.len();
        let mut lines = std::mem::take(&mut self.current_lines);
        let command = self.current_command.take();
        let start_row = self.block_start_row;

        // Trim trailing blank lines.
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }

        if lines.is_empty() {
            // All content was blank — nothing to emit.
            crate::debug_log!(
                "PRETTIFIER",
                "boundary::emit_block: all {} lines were blank, returning None",
                original_count
            );
            self.consecutive_blank_lines = 0;
            return None;
        }

        let end_row = start_row + lines.len();

        crate::debug_info!(
            "PRETTIFIER",
            "boundary::emit_block: emitting {} lines (trimmed from {}), rows={}..{}, cmd={:?}, first={:?}",
            lines.len(),
            original_count,
            start_row,
            end_row,
            command.as_deref().map(|c| &c[..c.len().min(40)]),
            lines.first().map(|l| &l[..l.len().min(80)])
        );

        self.block_start_row = 0;
        self.consecutive_blank_lines = 0;

        Some(ContentBlock {
            lines,
            preceding_command: command,
            start_row,
            end_row,
            timestamp: std::time::SystemTime::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a `BoundaryConfig` for `CommandOutput` scope.
    fn command_output_config() -> BoundaryConfig {
        BoundaryConfig {
            scope: DetectionScope::CommandOutput,
            ..BoundaryConfig::default()
        }
    }

    /// Helper: create a `BoundaryConfig` for `All` scope.
    fn all_scope_config() -> BoundaryConfig {
        BoundaryConfig {
            scope: DetectionScope::All,
            ..BoundaryConfig::default()
        }
    }

    /// Helper: create a `BoundaryConfig` for `ManualOnly` scope.
    fn manual_only_config() -> BoundaryConfig {
        BoundaryConfig {
            scope: DetectionScope::ManualOnly,
            ..BoundaryConfig::default()
        }
    }

    // -----------------------------------------------------------------------
    // CommandOutput scope tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_command_output_basic_flow() {
        let mut det = BoundaryDetector::new(command_output_config());

        det.on_command_start("git log");
        assert!(det.push_line("commit abc123", 10).is_none());
        assert!(det.push_line("Author: Alice", 11).is_none());
        assert!(det.push_line("", 12).is_none());
        assert!(det.push_line("  fix: resolve crash", 13).is_none());

        let block = det.on_command_end();
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.preceding_command.as_deref(), Some("git log"));
        assert_eq!(block.start_row, 10);
        assert_eq!(block.lines.len(), 4);
        assert_eq!(block.lines[0], "commit abc123");
        assert_eq!(block.lines[3], "  fix: resolve crash");
        assert_eq!(block.end_row, 14);
    }

    #[test]
    fn test_command_output_ignores_outside() {
        let mut det = BoundaryDetector::new(command_output_config());

        // No C marker — lines should be ignored.
        assert!(det.push_line("stray output", 0).is_none());
        assert!(det.push_line("more noise", 1).is_none());
        assert!(!det.has_pending_lines());
    }

    #[test]
    fn test_command_output_empty_command() {
        let mut det = BoundaryDetector::new(command_output_config());

        det.on_command_start("ls");
        // D marker with no lines pushed.
        let block = det.on_command_end();
        assert!(block.is_none());
    }

    // -----------------------------------------------------------------------
    // All scope tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_scope_blank_line_heuristic() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        assert!(det.push_line("# Title", 0).is_none());
        assert!(det.push_line("body text", 1).is_none());
        // First blank line — not enough.
        assert!(det.push_line("", 2).is_none());
        // Second blank line — triggers boundary.
        let block = det.push_line("", 3);
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.lines, vec!["# Title", "body text"]);
        assert_eq!(block.start_row, 0);
        assert_eq!(block.end_row, 2);
    }

    #[test]
    fn test_all_scope_blank_line_insufficient() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        assert!(det.push_line("line1", 0).is_none());
        // Only 1 blank line — should not trigger.
        assert!(det.push_line("", 1).is_none());
        assert!(det.push_line("line2", 2).is_none());

        assert!(det.has_pending_lines());
        assert_eq!(det.pending_line_count(), 3);
    }

    #[test]
    fn test_max_scan_lines() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            max_scan_lines: 3,
            ..all_scope_config()
        });

        assert!(det.push_line("line1", 0).is_none());
        assert!(det.push_line("line2", 1).is_none());
        // Third line hits the limit.
        let block = det.push_line("line3", 2);
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.lines.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Alt screen / process change tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_alt_screen_enter_emits() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("some output", 0);
        det.push_line("more output", 1);

        let block = det.on_alt_screen_change(true);
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.lines, vec!["some output", "more output"]);
    }

    #[test]
    fn test_alt_screen_exit_no_lines() {
        let mut det = BoundaryDetector::new(all_scope_config());

        // No accumulated lines — nothing to emit.
        let block = det.on_alt_screen_change(false);
        assert!(block.is_none());
    }

    #[test]
    fn test_process_change_emits() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("output A", 5);
        det.push_line("output B", 6);

        let block = det.on_process_change();
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.lines, vec!["output A", "output B"]);
        assert_eq!(block.start_row, 5);
        assert_eq!(block.end_row, 7);
    }

    // -----------------------------------------------------------------------
    // ManualOnly scope tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_manual_only_never_auto_emits() {
        let mut det = BoundaryDetector::new(manual_only_config());

        // push_line never auto-emits.
        assert!(det.push_line("line1", 0).is_none());
        assert!(det.push_line("line2", 1).is_none());
        assert!(det.has_pending_lines());

        // on_command_end returns None.
        det.on_command_start("test");
        det.push_line("cmd output", 2);
        assert!(det.on_command_end().is_none());

        // check_debounce returns None.
        assert!(det.check_debounce().is_none());

        // on_alt_screen_change returns None.
        assert!(det.on_alt_screen_change(true).is_none());

        // on_process_change returns None.
        assert!(det.on_process_change().is_none());

        // But flush() works.
        let block = det.flush();
        assert!(block.is_some());
    }

    // -----------------------------------------------------------------------
    // Flush / reset tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_flush_emits_and_clears() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("content", 0);
        let block = det.flush();
        assert!(block.is_some());
        assert_eq!(block.unwrap().lines, vec!["content"]);

        // Second flush returns None.
        assert!(det.flush().is_none());
        assert!(!det.has_pending_lines());
    }

    #[test]
    fn test_reset_clears_state() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("line1", 0);
        det.push_line("line2", 1);
        assert!(det.has_pending_lines());

        det.reset();

        assert!(!det.has_pending_lines());
        assert_eq!(det.pending_line_count(), 0);
        assert!(det.flush().is_none());
    }

    // -----------------------------------------------------------------------
    // Debounce tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_debounce_not_ready() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            debounce_ms: 1000, // Long debounce.
            ..all_scope_config()
        });

        det.push_line("content", 0);
        // Immediately check — should not fire.
        assert!(det.check_debounce().is_none());
    }

    #[test]
    fn test_debounce_fires_after_timeout() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            debounce_ms: 10,
            ..all_scope_config()
        });

        det.push_line("content", 0);
        // Wait longer than debounce_ms.
        std::thread::sleep(std::time::Duration::from_millis(15));

        let block = det.check_debounce();
        assert!(block.is_some());
        assert_eq!(block.unwrap().lines, vec!["content"]);
    }

    #[test]
    fn test_debounce_manual_only_always_none() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            debounce_ms: 1, // Very short debounce.
            ..manual_only_config()
        });

        det.push_line("content", 0);
        std::thread::sleep(std::time::Duration::from_millis(5));

        // ManualOnly always returns None from check_debounce.
        assert!(det.check_debounce().is_none());
        assert!(det.has_pending_lines());
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_trailing_blank_lines_trimmed() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("content", 0);
        det.push_line("", 1);

        let block = det.flush();
        assert!(block.is_some());
        let block = block.unwrap();
        // Trailing blank line should be trimmed.
        assert_eq!(block.lines, vec!["content"]);
        assert_eq!(block.end_row, 1);
    }

    #[test]
    fn test_row_tracking() {
        let mut det = BoundaryDetector::new(command_output_config());

        det.on_command_start("echo hello");
        det.push_line("hello", 42);
        det.push_line("world", 43);

        let block = det.on_command_end().unwrap();
        assert_eq!(block.start_row, 42);
        assert_eq!(block.end_row, 44);
    }

    #[test]
    fn test_blank_only_content_not_emitted() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("", 0);
        det.push_line("   ", 1);
        det.push_line("", 2);

        // All content is blank — flush should return None.
        assert!(det.flush().is_none());
    }

    // -----------------------------------------------------------------------
    // Fence-aware boundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_fence_suppresses_blank_line_boundary() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        assert!(det.push_line("# Header", 0).is_none());
        assert!(det.push_line("```rust", 1).is_none());
        assert!(det.push_line("fn main() {}", 2).is_none());
        // Two blank lines inside a fenced block should NOT trigger boundary.
        assert!(det.push_line("", 3).is_none());
        assert!(det.push_line("", 4).is_none());
        assert!(det.push_line("let x = 1;", 5).is_none());
        assert!(det.push_line("```", 6).is_none());

        // All lines should still be accumulated (no boundary triggered).
        assert_eq!(det.pending_line_count(), 7);
        let block = det.flush().unwrap();
        assert_eq!(block.lines.len(), 7);
        assert_eq!(block.lines[0], "# Header");
        assert_eq!(block.lines[6], "```");
    }

    #[test]
    fn test_fence_boundary_after_close() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        assert!(det.push_line("```python", 0).is_none());
        assert!(det.push_line("print('hi')", 1).is_none());
        assert!(det.push_line("```", 2).is_none());
        // After closing fence, blank lines SHOULD trigger boundary again.
        assert!(det.push_line("", 3).is_none());
        let block = det.push_line("", 4);
        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.lines.len(), 3);
        assert_eq!(block.lines[0], "```python");
    }

    #[test]
    fn test_tilde_fence_suppresses_blank_line_boundary() {
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        assert!(det.push_line("~~~yaml", 0).is_none());
        assert!(det.push_line("key: value", 1).is_none());
        assert!(det.push_line("", 2).is_none());
        assert!(det.push_line("", 3).is_none());
        assert!(det.push_line("other: data", 4).is_none());
        assert!(det.push_line("~~~", 5).is_none());

        // All lines accumulated — fence suppressed blank-line boundary.
        assert_eq!(det.pending_line_count(), 6);
    }

    #[test]
    fn test_reset_clears_fence_state() {
        let mut det = BoundaryDetector::new(all_scope_config());

        det.push_line("```rust", 0);
        assert!(det.flush().is_some());
        // After reset, fence state should be cleared.
        det.reset();
        // Now blank lines should trigger boundaries normally.
        det.push_line("text", 0);
        det.push_line("", 1);
        let block = det.push_line("", 2);
        assert!(block.is_some());
    }

    #[test]
    fn test_fence_with_language_tag_spaces() {
        // Language tags with non-alphanumeric chars should not start a fence.
        let mut det = BoundaryDetector::new(BoundaryConfig {
            blank_line_threshold: 2,
            ..all_scope_config()
        });

        // "``` not a real fence" has spaces, so should NOT be treated as a fence.
        assert!(det.push_line("``` not a real fence", 0).is_none());
        assert!(det.push_line("content", 1).is_none());
        // Blank lines should still trigger boundary (not in a fence).
        assert!(det.push_line("", 2).is_none());
        let block = det.push_line("", 3);
        assert!(block.is_some());
    }
}
