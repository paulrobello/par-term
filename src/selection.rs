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
    /// Start position (col, row) in terminal coordinates
    pub start: (usize, usize),
    /// End position (col, row) in terminal coordinates
    pub end: (usize, usize),
    /// Selection mode
    pub mode: SelectionMode,
}

impl Selection {
    /// Create a new selection
    pub fn new(start: (usize, usize), end: (usize, usize), mode: SelectionMode) -> Self {
        Self { start, end, mode }
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
        let sel = Selection::new((0, 0), (10, 0), SelectionMode::Normal);
        assert_eq!(sel.normalized(), ((0, 0), (10, 0)));

        // Backward selection (same line)
        let sel = Selection::new((10, 0), (0, 0), SelectionMode::Normal);
        assert_eq!(sel.normalized(), ((0, 0), (10, 0)));

        // Forward selection (multi-line)
        let sel = Selection::new((10, 0), (5, 1), SelectionMode::Normal);
        assert_eq!(sel.normalized(), ((10, 0), (5, 1)));

        // Backward selection (multi-line)
        let sel = Selection::new((5, 1), (10, 0), SelectionMode::Normal);
        assert_eq!(sel.normalized(), ((10, 0), (5, 1)));
    }
}
