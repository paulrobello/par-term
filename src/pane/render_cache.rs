use crate::cell_renderer::Cell;
use crate::selection::Selection;
use std::sync::Arc;

/// State related to render caching and dirty tracking
pub struct RenderCache {
    pub(crate) cells: Option<Arc<Vec<Cell>>>, // Cached cells from last render (dirty tracking; Arc avoids double-clone on cache hit)
    pub(crate) generation: u64, // Last terminal generation number (for dirty tracking)
    pub(crate) scroll_offset: usize, // Last scroll offset (for cache invalidation)
    pub(crate) cursor_pos: Option<(usize, usize)>, // Last cursor position (for cache invalidation)
    pub(crate) selection: Option<Selection>, // Last selection state (for cache invalidation)
    pub(crate) grid_dims: (usize, usize), // Last known terminal grid dimensions (cols, rows)
    pub(crate) terminal_title: String, // Last known terminal title (for change detection)
    pub(crate) scrollback_len: usize, // Last known scrollback length
    pub(crate) pane_cells: Option<Arc<Vec<Cell>>>, // Cached cells for pane rendering (reuse across frames)
    pub(crate) pane_cells_generation: u64, // Generation of cached pane_cells (0 = stale/unset)
    pub(crate) pane_cells_scroll_offset: usize, // Scroll offset used when pane_cells was generated
    pub(crate) pane_cells_grid_dims: (usize, usize), // Grid dimensions used when pane_cells was generated
    pub(crate) pane_scrollback_len: usize,           // Cached scrollback_len for pane rendering
}

impl RenderCache {
    pub(crate) fn new() -> Self {
        Self {
            cells: None,
            generation: 0,
            scroll_offset: 0,
            cursor_pos: None,
            selection: None,
            grid_dims: (0, 0),
            terminal_title: String::new(),
            scrollback_len: 0,
            pane_cells: None,
            pane_cells_generation: 0,
            pane_cells_scroll_offset: 0,
            pane_cells_grid_dims: (0, 0),
            pane_scrollback_len: 0,
        }
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}
