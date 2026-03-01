//! Tests for the boundary detector.

use super::detector::{BoundaryConfig, BoundaryDetector, DetectionScope};

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
