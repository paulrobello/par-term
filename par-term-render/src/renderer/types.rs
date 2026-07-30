// Render-input data types: the per-pane, divider, and title descriptions the
// frontend hands to `Renderer`, plus the viewport mapping for separator marks.
// These are plain data with no dependency on `Renderer` itself.

use crate::cell_renderer::{Cell, PaneViewport};
use par_term_config::SeparatorMark;

/// Compute which separator marks are visible in the current viewport.
///
/// Maps absolute scrollback line numbers to screen rows for the current view.
/// Returns only the marks whose absolute line index falls within the visible
/// window `[viewport_start, viewport_start + visible_lines)`, converting each
/// to a zero-based screen row. No deduplication or merging is performed; marks
/// are returned in the same order they appear in `marks`.
pub fn compute_visible_separator_marks(
    marks: &[par_term_config::ScrollbackMark],
    scrollback_len: usize,
    scroll_offset: usize,
    visible_lines: usize,
) -> Vec<SeparatorMark> {
    let mut out = Vec::new();
    fill_visible_separator_marks(
        &mut out,
        marks,
        scrollback_len,
        scroll_offset,
        visible_lines,
    );
    out
}

/// Fill `out` with the separator marks visible in the current viewport, reusing
/// the provided allocation to avoid a per-call heap allocation on the render hot path.
///
/// The buffer is cleared on entry. Semantics are identical to
/// [`compute_visible_separator_marks`].
pub(crate) fn fill_visible_separator_marks(
    out: &mut Vec<SeparatorMark>,
    marks: &[par_term_config::ScrollbackMark],
    scrollback_len: usize,
    scroll_offset: usize,
    visible_lines: usize,
) {
    out.clear();
    let viewport_start = scrollback_len.saturating_sub(scroll_offset);
    let viewport_end = viewport_start + visible_lines;
    for mark in marks {
        if mark.line >= viewport_start && mark.line < viewport_end {
            let screen_row = mark.line - viewport_start;
            out.push((screen_row, mark.exit_code, mark.color));
        }
    }
}

/// Information needed to render a single pane
pub struct PaneRenderInfo<'a> {
    /// Viewport bounds and state for this pane
    pub viewport: PaneViewport,
    /// Cells to render (should match viewport grid size)
    pub cells: &'a [Cell],
    /// Grid dimensions (cols, rows)
    pub grid_size: (usize, usize),
    /// Cursor position within this pane (col, row), or None if no cursor visible
    pub cursor_pos: Option<(usize, usize)>,
    /// Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    pub cursor_opacity: f32,
    /// Whether this pane has a scrollbar visible
    pub show_scrollbar: bool,
    /// Scrollback marks for this pane
    pub marks: Vec<par_term_config::ScrollbackMark>,
    /// Scrollback length for this pane (needed for separator mark mapping)
    pub scrollback_len: usize,
    /// Current scroll offset for this pane (needed for separator mark mapping)
    pub scroll_offset: usize,
    /// Per-pane background image override (None = use global background)
    pub background: Option<par_term_config::PaneBackground>,
    /// Inline graphics (Sixel/iTerm2/Kitty) to render for this pane
    pub graphics: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
    /// Kitty virtual placements (U=1) used as prototypes for Unicode placeholder
    /// rendering. The actual on-screen position is taken from the placeholder
    /// cells in `cells`, not from each graphic's `position` field.
    pub virtual_placements: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
}

/// Information needed to render a pane divider
#[derive(Clone, Copy, Debug)]
pub struct DividerRenderInfo {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Whether this divider is currently being hovered
    pub hovered: bool,
}

impl DividerRenderInfo {
    /// Create from a DividerRect
    pub fn from_rect(rect: &par_term_config::DividerRect, hovered: bool) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            hovered,
        }
    }
}

/// Information needed to render a pane title bar
#[derive(Clone, Debug)]
pub struct PaneTitleInfo {
    /// X position of the title bar in pixels
    pub x: f32,
    /// Y position of the title bar in pixels
    pub y: f32,
    /// Width of the title bar in pixels
    pub width: f32,
    /// Height of the title bar in pixels
    pub height: f32,
    /// Title text to display
    pub title: String,
    /// Whether this pane is focused
    pub focused: bool,
    /// Text color [R, G, B] as floats (0.0-1.0)
    pub text_color: [f32; 3],
    /// Background color [R, G, B] as floats (0.0-1.0)
    pub bg_color: [f32; 3],
}

/// Settings for rendering pane dividers and focus indicators
#[derive(Clone, Copy, Debug)]
pub struct PaneDividerSettings {
    /// Color for dividers [R, G, B] as floats (0.0-1.0)
    pub divider_color: [f32; 3],
    /// Color when hovering over dividers [R, G, B] as floats (0.0-1.0)
    pub hover_color: [f32; 3],
    /// Whether to show focus indicator around focused pane
    pub show_focus_indicator: bool,
    /// Color for focus indicator [R, G, B] as floats (0.0-1.0)
    pub focus_color: [f32; 3],
    /// Width of focus indicator border in pixels
    pub focus_width: f32,
    /// Style of dividers (solid, double, dashed, shadow)
    pub divider_style: par_term_config::DividerStyle,
}

impl Default for PaneDividerSettings {
    fn default() -> Self {
        Self {
            divider_color: [0.3, 0.3, 0.3],
            hover_color: [0.5, 0.6, 0.8],
            show_focus_indicator: true,
            focus_color: [0.4, 0.6, 1.0],
            focus_width: 1.0,
            divider_style: par_term_config::DividerStyle::default(),
        }
    }
}
