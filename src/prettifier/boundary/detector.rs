//! Content boundary detection algorithms.
//!
//! [`BoundaryDetector`] accumulates terminal output lines and emits
//! [`ContentBlock`] instances at natural boundaries: OSC 133 command
//! markers, blank-line runs, max-line limits, or debounce timeouts.

use std::time::Instant;

use crate::prettifier::types::ContentBlock;

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
                        &line[..line.floor_char_boundary(80)]
                    );
                    return None;
                }
                crate::debug_trace!(
                    "PRETTIFIER",
                    "boundary::push_line (CommandOutput, in_cmd=true) row={}: {:?}",
                    row,
                    &line[..line.floor_char_boundary(80)]
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
                    &line[..line.floor_char_boundary(80)]
                );
                return None;
            }
            DetectionScope::All => {
                crate::debug_trace!(
                    "PRETTIFIER",
                    "boundary::push_line (All) row={}, accumulated={}: {:?}",
                    row,
                    self.current_lines.len(),
                    &line[..line.floor_char_boundary(80)]
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
            &command[..command.floor_char_boundary(80)]
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
            crate::debug_trace!(
                "PRETTIFIER",
                "boundary::emit_block: no lines accumulated, returning None"
            );
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
            command.as_deref().map(|c| &c[..c.floor_char_boundary(40)]),
            lines.first().map(|l| &l[..l.floor_char_boundary(80)])
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
