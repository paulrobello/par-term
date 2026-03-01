//! Visual mode and selection methods for the copy mode state machine.

use super::types::{CopyModeState, VisualMode};
use crate::selection::{Selection, SelectionMode};

impl CopyModeState {
    // ========================================================================
    // Visual mode
    // ========================================================================

    /// Toggle character-wise visual mode
    pub fn toggle_visual_char(&mut self) {
        if self.visual_mode == VisualMode::Char {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Char;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Toggle line-wise visual mode
    pub fn toggle_visual_line(&mut self) {
        if self.visual_mode == VisualMode::Line {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Line;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Toggle block/rectangular visual mode
    pub fn toggle_visual_block(&mut self) {
        if self.visual_mode == VisualMode::Block {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Block;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Compute a `Selection` from the current visual mode state.
    ///
    /// The selection coordinates are in screen-relative terms
    /// (row = line - viewport_top) for rendering.
    /// `scroll_offset` is the current viewport scroll position.
    pub fn compute_selection(&self, scroll_offset: usize) -> Option<Selection> {
        if self.visual_mode == VisualMode::None {
            return None;
        }

        let (anchor_line, anchor_col) = self.selection_anchor?;

        // Convert absolute lines to viewport-relative rows
        // viewport_top = scrollback_len - scroll_offset (absolute line at top of screen)
        let viewport_top = self.scrollback_len.saturating_sub(scroll_offset);

        // Both anchor and cursor must produce valid viewport rows for rendering
        // We allow negative (above screen) and beyond-screen positions
        // by clamping to 0..rows-1 for rendering purposes
        let anchor_row = anchor_line.saturating_sub(viewport_top);
        let cursor_row = self.cursor_absolute_line.saturating_sub(viewport_top);

        let mode = match self.visual_mode {
            VisualMode::None => return None,
            VisualMode::Char => SelectionMode::Normal,
            VisualMode::Line => SelectionMode::Line,
            VisualMode::Block => SelectionMode::Rectangular,
        };

        let start = (anchor_col, anchor_row);
        let end = (self.cursor_col, cursor_row);

        Some(Selection::new(start, end, mode))
    }
}
