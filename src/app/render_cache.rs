use crate::cell_renderer::Cell;
use crate::selection::Selection;
use std::sync::Arc;
use std::time::Instant;

/// State related to render caching and dirty tracking
pub(crate) struct RenderCache {
    pub(crate) cells: Option<Arc<Vec<Cell>>>, // Cached cells from last render (dirty tracking; Arc avoids double-clone on cache hit)
    pub(crate) generation: u64, // Last terminal generation number (for dirty tracking)
    pub(crate) scroll_offset: usize, // Last scroll offset (for cache invalidation)
    pub(crate) cursor_pos: Option<(usize, usize)>, // Last cursor position (for cache invalidation)
    pub(crate) selection: Option<Selection>, // Last selection state (for cache invalidation)
    pub(crate) terminal_title: String, // Last known terminal title (for change detection)
    pub(crate) scrollback_len: usize, // Last known scrollback length
    pub(crate) prettifier_feed_generation: u64, // Last terminal generation fed to prettifier
    pub(crate) prettifier_feed_scroll_offset: usize, // Last scroll offset fed to prettifier
    pub(crate) prettifier_command_start_line: Option<usize>, // Absolute line from CommandStarted
    pub(crate) prettifier_command_text: Option<String>, // Command text for ContentBlock
    pub(crate) prettifier_cc_dump_count: u32, // How many CC viewport dumps we've done (cap at ~5)
    pub(crate) prettifier_cc_last_dump_rows: (usize, usize), // Last dumped (start, end) range
    pub(crate) prettifier_feed_last_time: Instant, // Throttle: last time we fed the non-CC pipeline
    pub(crate) prettifier_feed_last_hash: u64, // Throttle: content hash of last feed (skip if unchanged)
}

impl RenderCache {
    pub(crate) fn new() -> Self {
        Self {
            cells: None,
            generation: 0,
            scroll_offset: 0,
            cursor_pos: None,
            selection: None,
            terminal_title: String::new(),
            scrollback_len: 0,
            prettifier_feed_generation: 0,
            prettifier_feed_scroll_offset: usize::MAX, // Force first feed
            prettifier_command_start_line: None,
            prettifier_command_text: None,
            prettifier_cc_dump_count: 0,
            prettifier_cc_last_dump_rows: (0, 0),
            prettifier_feed_last_time: Instant::now(),
            prettifier_feed_last_hash: 0,
        }
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}
