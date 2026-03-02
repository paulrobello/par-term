//! Vi-style Copy Mode state machine.
//!
//! Copy Mode provides keyboard-driven text selection and navigation,
//! matching iTerm2's Copy Mode. When active, all keyboard input navigates
//! an independent cursor through the terminal buffer (including scrollback).
//!
//! ## Module layout
//!
//! - [`types`]: All type and struct definitions (`CopyModeState`, `VisualMode`, etc.)
//! - [`cursor`]: Cursor movement methods (basic motions, page motions, viewport helpers)
//! - [`motion`]: Word and line navigation helpers (`move_word_forward`, etc.)
//! - [`visual`]: Visual mode and selection methods (`toggle_visual_*`, `compute_selection`)
//! - [`search`]: Search state methods (`start_search`, `search_input`, etc.)

mod cursor;
mod motion;
mod search;
mod types;
mod visual;

// Re-export the public API so external callers are unaffected.
pub use crate::selection::SelectionMode;
pub use types::{CopyModeState, Mark, PendingOperator, SearchDirection, VisualMode};

impl CopyModeState {
    // ========================================================================
    // Marks
    // ========================================================================

    /// Set a named mark at the current cursor position
    pub fn set_mark(&mut self, name: char) {
        self.marks.insert(
            name,
            Mark {
                col: self.cursor_col,
                absolute_line: self.cursor_absolute_line,
            },
        );
    }

    /// Jump to a named mark, returning true if the mark exists
    pub fn goto_mark(&mut self, name: char) -> bool {
        if let Some(mark) = self.marks.get(&name) {
            self.cursor_col = mark.col;
            self.cursor_absolute_line = mark.absolute_line;
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Status
    // ========================================================================

    /// Get a status line description of the current mode
    pub fn status_text(&self) -> String {
        if self.is_searching {
            let dir = match self.search_direction {
                SearchDirection::Forward => '/',
                SearchDirection::Backward => '?',
            };
            format!("{}{}", dir, self.search_query)
        } else {
            let mode = match self.visual_mode {
                VisualMode::None => "COPY",
                VisualMode::Char => "VISUAL",
                VisualMode::Line => "VISUAL LINE",
                VisualMode::Block => "VISUAL BLOCK",
            };
            let pos = format!(
                "{}:{} (abs {})",
                self.cursor_absolute_line
                    .saturating_sub(self.scrollback_len),
                self.cursor_col,
                self.cursor_absolute_line,
            );
            format!("-- {} -- {}", mode, pos)
        }
    }
}

#[cfg(test)]
mod tests;
