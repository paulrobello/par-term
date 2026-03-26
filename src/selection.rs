/// Selection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Normal character-based selection
    Normal,
    /// Rectangular/block selection
    Rectangular,
    /// Full line selection (triple-click)
    Line,
}

/// Selection state for text selection
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    /// Start position (col, row) in viewport-relative coordinates at `scroll_offset`
    pub start: (usize, usize),
    /// End position (col, row) in viewport-relative coordinates at `scroll_offset`
    pub end: (usize, usize),
    /// Selection mode
    pub mode: SelectionMode,
    /// Scroll offset at the time the selection was captured.
    ///
    /// Row coordinates are viewport-relative (0 = top of the visible screen) when
    /// `scroll_offset` was the active viewport offset.  The renderer adjusts the
    /// rows by `(self.scroll_offset as isize - current_scroll_offset as isize)` so
    /// the highlight tracks the content as the user scrolls.
    pub scroll_offset: usize,
}

impl Selection {
    /// Create a new selection.
    ///
    /// `scroll_offset` must be the current viewport scroll offset so the
    /// renderer can compensate when the user scrolls after the selection.
    pub fn new(
        start: (usize, usize),
        end: (usize, usize),
        mode: SelectionMode,
        scroll_offset: usize,
    ) -> Self {
        Self {
            start,
            end,
            mode,
            scroll_offset,
        }
    }

    /// Return a copy of this selection with rows adjusted to `current_scroll_offset`.
    ///
    /// Rows that shift above the top of the viewport become `usize::MAX` so that
    /// `is_cell_selected` never matches them.  Rows shifted below the viewport are
    /// left as-is (they exceed the row count and are also never matched).
    pub fn viewport_adjusted(&self, current_scroll_offset: usize) -> Self {
        let delta = current_scroll_offset as isize - self.scroll_offset as isize;
        let adjust = |row: usize| -> usize {
            let adjusted = row as isize + delta;
            if adjusted < 0 {
                usize::MAX
            } else {
                adjusted as usize
            }
        };
        Self {
            start: (self.start.0, adjust(self.start.1)),
            end: (self.end.0, adjust(self.end.1)),
            mode: self.mode,
            scroll_offset: current_scroll_offset,
        }
    }

    /// Get normalized selection (ensures start is before end)
    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        let (start_col, start_row) = self.start;
        let (end_col, end_row) = self.end;

        if start_row < end_row || (start_row == end_row && start_col <= end_col) {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_normalization() {
        // Forward selection
        let sel = Selection::new((0, 0), (10, 0), SelectionMode::Normal, 0);
        assert_eq!(sel.normalized(), ((0, 0), (10, 0)));

        // Backward selection (same line)
        let sel = Selection::new((10, 0), (0, 0), SelectionMode::Normal, 0);
        assert_eq!(sel.normalized(), ((0, 0), (10, 0)));

        // Forward selection (multi-line)
        let sel = Selection::new((10, 0), (5, 1), SelectionMode::Normal, 0);
        assert_eq!(sel.normalized(), ((10, 0), (5, 1)));

        // Backward selection (multi-line)
        let sel = Selection::new((5, 1), (10, 0), SelectionMode::Normal, 0);
        assert_eq!(sel.normalized(), ((10, 0), (5, 1)));
    }
}
