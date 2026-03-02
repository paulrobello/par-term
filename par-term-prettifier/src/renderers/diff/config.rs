//! Configuration types for the diff renderer.

/// Display style for diff output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiffStyle {
    /// Traditional unified diff (inline).
    Inline,
    /// Side-by-side removed/added columns.
    SideBySide,
    /// Auto-select based on terminal width.
    #[default]
    Auto,
}

/// Configuration for the diff renderer.
#[derive(Clone, Debug)]
pub struct DiffRendererConfig {
    /// Display style (Inline, SideBySide, or Auto).
    pub style: DiffStyle,
    /// Minimum terminal columns for side-by-side mode (default: 160).
    pub side_by_side_min_width: usize,
    /// Enable word-level highlighting within changed lines (default: true).
    pub word_diff: bool,
    /// Show line number gutter (default: true).
    pub show_line_numbers: bool,
}

impl Default for DiffRendererConfig {
    fn default() -> Self {
        Self {
            style: DiffStyle::Auto,
            side_by_side_min_width: 160,
            word_diff: true,
            show_line_numbers: true,
        }
    }
}

/// Tracks current line numbers while rendering.
pub(super) struct DiffLineState {
    pub old_line: usize,
    pub new_line: usize,
}
