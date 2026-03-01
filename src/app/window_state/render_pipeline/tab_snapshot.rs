//! Per-tab terminal content snapshot for one render frame.
//!
//! `extract_tab_cells` is called by `gather_render_data` to lock the active
//! terminal, build the cell grid (or fall back to cache), resolve cursor
//! position and style, and detect alt-screen state.  All of this must happen
//! inside a single terminal try-lock window so no separate mutable borrow of
//! the tab is needed after the lock is released.

use crate::app::window_state::WindowState;
use crate::config::CursorStyle;
use crate::selection::SelectionMode;
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use std::sync::Arc;

/// Data returned by `extract_tab_cells`.
pub(super) struct TabCellsSnapshot {
    /// Rendered cell grid (with selection marks, cursor blink applied)
    pub(super) cells: Vec<crate::cell_renderer::Cell>,
    /// Cursor position on screen (col, row), None if hidden or scrolled away
    pub(super) cursor_pos: Option<(usize, usize)>,
    /// Cursor glyph style (after config overrides)
    pub(super) cursor_style: Option<TermCursorStyle>,
    /// Whether the alternate screen is currently active
    pub(super) is_alt_screen: bool,
    /// Terminal generation counter at the time cells were generated
    pub(super) current_generation: u64,
}

impl WindowState {
    /// Lock the active terminal and extract the cell grid for this frame.
    ///
    /// Uses dirty-generation tracking to avoid re-generating cells on every
    /// cursor-blink frame.  Falls back to the cached cell vector when the
    /// terminal write-lock is held by another thread (e.g., PTY reader during
    /// a large upload).  Returns `None` when no cached cells are available and
    /// the lock is unavailable.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn extract_tab_cells(
        &mut self,
        scroll_offset: usize,
        mouse_selection: Option<crate::selection::Selection>,
        cache_cells: Option<Arc<Vec<crate::cell_renderer::Cell>>>,
        cache_generation: u64,
        cache_scroll_offset: usize,
        cache_cursor_pos: Option<(usize, usize)>,
        cache_selection: Option<crate::selection::Selection>,
        terminal: Arc<tokio::sync::RwLock<par_term_terminal::TerminalManager>>,
    ) -> Option<TabCellsSnapshot> {
        if let Ok(term) = terminal.try_write() {
            // Get current generation to check if terminal content has changed
            let current_generation = term.update_generation();

            // Normalize selection if it exists and extract mode
            let (selection, rectangular) = if let Some(sel) = mouse_selection {
                (
                    Some(sel.normalized()),
                    sel.mode == SelectionMode::Rectangular,
                )
            } else {
                (None, false)
            };

            // Get cursor position and opacity (only show if we're at the bottom with no scroll
            // offset and the cursor is visible — TUI apps hide cursor via DECTCEM escape sequence).
            // If lock_cursor_visibility is enabled, ignore the terminal's visibility state.
            // In copy mode, use the copy mode cursor position instead.
            let cursor_visible = self.config.lock_cursor_visibility || term.is_cursor_visible();
            let current_cursor_pos = if self.copy_mode.active {
                self.copy_mode.screen_cursor_pos(scroll_offset)
            } else if scroll_offset == 0 && cursor_visible {
                Some(term.cursor_position())
            } else {
                None
            };

            let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_anim.cursor_opacity));

            // Get cursor style for geometric rendering.
            // In copy mode, always use SteadyBlock for clear visibility.
            // If lock_cursor_style is enabled, use the config's cursor style instead of the
            // terminal's. If lock_cursor_blink is enabled and cursor_blink is false, force steady.
            let cursor_style = if self.copy_mode.active && current_cursor_pos.is_some() {
                Some(TermCursorStyle::SteadyBlock)
            } else if current_cursor_pos.is_some() {
                if self.config.lock_cursor_style {
                    let style = if self.config.cursor_blink {
                        match self.config.cursor_style {
                            CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                            CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                            CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                        }
                    } else {
                        match self.config.cursor_style {
                            CursorStyle::Block => TermCursorStyle::SteadyBlock,
                            CursorStyle::Beam => TermCursorStyle::SteadyBar,
                            CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                        }
                    };
                    Some(style)
                } else {
                    let mut style = term.cursor_style();
                    // If blink is locked off, convert blinking styles to steady
                    if self.config.lock_cursor_blink && !self.config.cursor_blink {
                        style = match style {
                            TermCursorStyle::BlinkingBlock => TermCursorStyle::SteadyBlock,
                            TermCursorStyle::BlinkingBar => TermCursorStyle::SteadyBar,
                            TermCursorStyle::BlinkingUnderline => TermCursorStyle::SteadyUnderline,
                            other => other,
                        };
                    }
                    Some(style)
                }
            } else {
                None
            };

            log::trace!(
                "Cursor: pos={:?}, opacity={:.2}, style={:?}, scroll={}, visible={}",
                current_cursor_pos,
                self.cursor_anim.cursor_opacity,
                cursor_style,
                scroll_offset,
                term.is_cursor_visible()
            );

            // Check if we need to regenerate cells.
            // Only regenerate when content actually changes, not on every cursor blink.
            let needs_regeneration = cache_cells.is_none()
                || current_generation != cache_generation
                || scroll_offset != cache_scroll_offset
                || current_cursor_pos != cache_cursor_pos
                || mouse_selection != cache_selection;

            let cell_gen_start = std::time::Instant::now();
            let (cells, is_cache_hit) = if needs_regeneration {
                let fresh_cells =
                    term.get_cells_with_scrollback(scroll_offset, selection, rectangular, cursor);
                (fresh_cells, false)
            } else {
                (
                    cache_cells
                        .as_ref()
                        .expect(
                            "window_state: cache_cells must be Some when needs_regeneration is false",
                        )
                        .as_ref()
                        .clone(),
                    true,
                )
            };
            self.debug.cache_hit = is_cache_hit;
            self.debug.cell_gen_time = cell_gen_start.elapsed();

            let is_alt_screen = term.is_alt_screen_active();

            Some(TabCellsSnapshot {
                cells,
                cursor_pos: current_cursor_pos,
                cursor_style,
                is_alt_screen,
                current_generation,
            })
        } else if let Some(cached) = cache_cells {
            // Terminal locked (e.g., upload in progress) — use cached cells so the
            // rest of the render pipeline (including file transfer overlay) can proceed.
            // Unwrap the Arc: if this is the sole reference the Vec is moved for free,
            // otherwise a clone is made (rare — only if another Arc clone is live).
            let cached_vec = Arc::try_unwrap(cached).unwrap_or_else(|a| (*a).clone());
            Some(TabCellsSnapshot {
                cells: cached_vec,
                cursor_pos: cache_cursor_pos,
                cursor_style: None,
                is_alt_screen: false,
                current_generation: cache_generation,
            })
        } else {
            // Terminal locked and no cache available — skip this frame.
            None
        }
    }
}
