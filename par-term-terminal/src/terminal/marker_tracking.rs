//! Shell lifecycle marker state machine for `TerminalManager`.
//!
//! Tracks OSC 133 shell integration markers (PromptStart, CommandStart,
//! CommandExecuted, CommandFinished) and accumulates shell lifecycle events
//! for the prettifier pipeline.

use super::ShellLifecycleEvent;
use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;

/// Encapsulates the mutable shell-integration marker state that
/// `TerminalManager` carries between frames.
///
/// Extracted from `TerminalManager` so that the update logic in
/// `scrollback.rs` has a focused type to operate on rather than reaching into
/// several disjoint fields.
pub(crate) struct MarkerTracker {
    /// Previous shell integration marker, used to detect transitions.
    pub last_shell_marker: Option<ShellIntegrationMarker>,
    /// Absolute line and column at CommandStart (B marker) for extracting
    /// command text. `(absolute_line, col)` where
    /// `absolute_line = scrollback_len + cursor_row` at the time the B marker
    /// was seen. Using absolute line (rather than grid row) ensures we can
    /// still find the command text even after it scrolls into scrollback.
    pub command_start_pos: Option<(usize, usize)>,
    /// Command text captured from the terminal, waiting to be applied to a
    /// mark. Stored as `(absolute_line, text)` so we can target the correct
    /// mark.
    pub captured_command_text: Option<(usize, String)>,
    /// Shell lifecycle events queued for the prettifier pipeline.
    pub shell_lifecycle_events: Vec<ShellLifecycleEvent>,
}

impl MarkerTracker {
    /// Create a new, empty `MarkerTracker`.
    pub fn new() -> Self {
        Self {
            last_shell_marker: None,
            command_start_pos: None,
            captured_command_text: None,
            shell_lifecycle_events: Vec::new(),
        }
    }

    /// Reset all marker state — call when the scrollback is cleared.
    pub fn reset(&mut self) {
        self.last_shell_marker = None;
        self.command_start_pos = None;
        self.captured_command_text = None;
    }

    /// Drain queued shell lifecycle events for the prettifier pipeline.
    pub fn drain_events(&mut self) -> Vec<ShellLifecycleEvent> {
        std::mem::take(&mut self.shell_lifecycle_events)
    }

    /// Process a single shell integration event update.
    ///
    /// Updates `last_shell_marker`, `command_start_pos`, and
    /// `captured_command_text` based on the incoming marker.
    ///
    /// Returns the captured command text if it was just finalised (i.e.
    /// the marker transitioned away from `CommandStart` and there was text
    /// to capture). The caller is responsible for providing that text by
    /// calling [`extract_text_fn`].
    pub fn process_marker_transition(
        &mut self,
        marker: Option<ShellIntegrationMarker>,
        abs_line: usize,
        cursor_col: usize,
        extract_text_fn: impl FnOnce(usize, usize) -> String,
    ) {
        let prev_marker = self.last_shell_marker;
        if marker == prev_marker {
            return;
        }

        match marker {
            Some(ShellIntegrationMarker::CommandStart) => {
                self.command_start_pos = Some((abs_line, cursor_col));
            }
            _ => {
                if let Some((start_abs_line, start_col)) = self.command_start_pos.take() {
                    let text = extract_text_fn(start_abs_line, start_col);
                    if !text.is_empty() {
                        self.captured_command_text = Some((start_abs_line, text));
                    }
                }
            }
        }

        self.last_shell_marker = marker;
    }

    /// Push a `CommandStarted` lifecycle event.
    pub fn push_command_started(&mut self, command: String, absolute_line: usize) {
        self.shell_lifecycle_events
            .push(ShellLifecycleEvent::CommandStarted {
                command,
                absolute_line,
            });
    }

    /// Push a `CommandFinished` lifecycle event.
    pub fn push_command_finished(&mut self, absolute_line: usize) {
        self.shell_lifecycle_events
            .push(ShellLifecycleEvent::CommandFinished { absolute_line });
    }

    /// Take the captured command text, if any.
    pub fn take_captured_command_text(&mut self) -> Option<(usize, String)> {
        self.captured_command_text.take()
    }
}
