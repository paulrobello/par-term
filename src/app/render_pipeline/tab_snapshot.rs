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

/// Parameters for [`WindowState::extract_tab_cells`].
pub(super) struct TabCellsParams {
    pub scroll_offset: usize,
    pub mouse_selection: Option<crate::selection::Selection>,
    pub cache_cells: Option<Arc<Vec<crate::cell_renderer::Cell>>>,
    pub cache_generation: u64,
    pub cache_scroll_offset: usize,
    pub cache_cursor_pos: Option<(usize, usize)>,
    pub cache_selection: Option<crate::selection::Selection>,
    pub cache_grid_dims: (usize, usize),
    pub terminal: Arc<tokio::sync::RwLock<par_term_terminal::TerminalManager>>,
    /// Previous frame's alt-screen state (used as fallback when terminal is locked).
    pub was_alt_screen: bool,
}

/// Data returned by `extract_tab_cells`.
pub(super) struct TabCellsSnapshot {
    /// Rendered cell grid (with selection marks, cursor blink applied)
    pub(super) cells: Vec<crate::cell_renderer::Cell>,
    /// Actual terminal grid dimensions (cols, rows) at the time cells were generated.
    /// May differ from the renderer grid when split panes are active or a scrollbar
    /// inset reduces the column count.
    pub(super) grid_dims: (usize, usize),
    /// Cursor position on screen (col, row), None if hidden or scrolled away
    pub(super) cursor_pos: Option<(usize, usize)>,
    /// Cursor glyph style (after config overrides)
    pub(super) cursor_style: Option<TermCursorStyle>,
    /// Terminal cursor position for shader uniforms, even when the visible cursor is hidden.
    pub(super) shader_cursor_pos: Option<(usize, usize)>,
    /// Terminal cursor style for shader uniforms.
    pub(super) shader_cursor_style: Option<TermCursorStyle>,
    /// Whether the alternate screen is currently active
    pub(super) is_alt_screen: bool,
    /// Terminal generation counter at the time cells were generated
    pub(super) current_generation: u64,
}

fn snapshot_generation_for_cells(
    terminal_generation: u64,
    cache_generation: u64,
    used_stale_cache: bool,
) -> u64 {
    if used_stale_cache {
        cache_generation
    } else {
        terminal_generation
    }
}

fn cursor_is_on_live_view(scroll_offset: usize, is_alt_screen: bool) -> bool {
    is_alt_screen || scroll_offset == 0
}

impl WindowState {
    /// Lock the active terminal and extract the cell grid for this frame.
    ///
    /// Uses dirty-generation tracking to avoid re-generating cells on every
    /// cursor-blink frame.  Falls back to the cached cell vector when the
    /// terminal write-lock is held by another thread (e.g., PTY reader during
    /// a large upload).  Returns `None` when no cached cells are available and
    /// the lock is unavailable.
    pub(super) fn extract_tab_cells(&mut self, p: TabCellsParams) -> Option<TabCellsSnapshot> {
        let TabCellsParams {
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            cache_grid_dims,
            terminal,
            was_alt_screen,
        } = p;
        if let Ok(term) = terminal.try_write() {
            // Get current generation to check if terminal content has changed
            let current_generation = term.update_generation();

            // Normalize selection if it exists and extract mode.
            // Selection rows are viewport-relative at `sel.scroll_offset`.  Adjust
            // them to the current `scroll_offset` so the highlight tracks the content
            // when the user scrolls after making a selection.
            let (selection, rectangular) = if let Some(sel) = mouse_selection {
                let adjusted = sel.viewport_adjusted(scroll_offset);
                (
                    Some(adjusted.normalized()),
                    sel.mode == SelectionMode::Rectangular,
                )
            } else {
                (None, false)
            };

            let is_alt_screen = term.is_alt_screen_active();

            // Get cursor position and opacity (only show the geometric cursor if we're at the
            // bottom with no scroll offset and the cursor is visible — TUI apps may hide cursor
            // via DECTCEM). Alternate-screen buffers are always the live view, so stale
            // scrollback offsets must not suppress cursor state there.
            // If lock_cursor_visibility is enabled, ignore the terminal's visibility state.
            // In copy mode, use the copy mode cursor position instead.
            let cursor_visible =
                self.config.cursor.lock_cursor_visibility || term.is_cursor_visible();
            let live_view = cursor_is_on_live_view(scroll_offset, is_alt_screen);
            let terminal_cursor_pos =
                (!self.copy_mode.active && live_view).then(|| term.cursor_position());
            let current_cursor_pos = if self.copy_mode.active {
                self.copy_mode.screen_cursor_pos(scroll_offset)
            } else if cursor_visible {
                terminal_cursor_pos
            } else {
                None
            };
            let shader_cursor_pos = if self.copy_mode.active {
                current_cursor_pos
            } else {
                terminal_cursor_pos
            };

            // Get cursor style for geometric rendering.
            // In copy mode, always use SteadyBlock for clear visibility.
            // If lock_cursor_style is enabled, use the config's cursor style instead of the
            // terminal's. If lock_cursor_blink is enabled and cursor_blink is false, force steady.
            let cursor_style = if self.copy_mode.active && current_cursor_pos.is_some() {
                Some(TermCursorStyle::SteadyBlock)
            } else if current_cursor_pos.is_some() {
                if self.config.cursor.lock_cursor_style {
                    let style = if self.config.cursor.cursor_blink {
                        match self.config.cursor.cursor_style {
                            CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                            CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                            CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                        }
                    } else {
                        match self.config.cursor.cursor_style {
                            CursorStyle::Block => TermCursorStyle::SteadyBlock,
                            CursorStyle::Beam => TermCursorStyle::SteadyBar,
                            CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                        }
                    };
                    Some(style)
                } else {
                    let mut style = term.cursor_style();
                    // If blink is locked off, convert blinking styles to steady
                    if self.config.cursor.lock_cursor_blink && !self.config.cursor.cursor_blink {
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

            let shader_cursor_style = shader_cursor_pos.map(|_| term.cursor_style());

            log::trace!(
                "Cursor: pos={:?}, shader_pos={:?}, opacity={:.2}, style={:?}, scroll={}, alt_screen={}, visible={}",
                current_cursor_pos,
                shader_cursor_pos,
                self.cursor_anim.cursor_opacity,
                cursor_style,
                scroll_offset,
                is_alt_screen,
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
            let mut used_stale_cache = false;
            let (cells, is_cache_hit) = if needs_regeneration {
                // Use try_get_cells_with_scrollback to avoid blocking on the internal
                // pty_session / terminal mutexes when the PTY reader is processing
                // output.  Falls back to the tab-level cell cache on contention.
                if let Some(fresh_cells) =
                    term.try_get_cells_with_scrollback(scroll_offset, selection, rectangular)
                {
                    (fresh_cells, false)
                } else if let Some(ref cached) = cache_cells {
                    // Internal lock contention — use cached cells, but do not advance
                    // the snapshot generation. Downstream consumers must not treat the
                    // terminal's new generation as processed using stale cell content.
                    used_stale_cache = true;
                    (cached.as_ref().clone(), true)
                } else {
                    // No cache available — fall back to blocking lock for first frame.
                    let fresh_cells =
                        term.get_cells_with_scrollback(scroll_offset, selection, rectangular, None);
                    (fresh_cells, false)
                }
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
            let snapshot_generation = snapshot_generation_for_cells(
                current_generation,
                cache_generation,
                used_stale_cache,
            );
            if used_stale_cache {
                crate::debug_trace!(
                    "RENDER",
                    "cell snapshot used stale cache for terminal generation {}; keeping snapshot generation at {}",
                    current_generation,
                    snapshot_generation
                );
            }

            self.debug.cache_hit = is_cache_hit;
            self.debug.cell_gen_time = cell_gen_start.elapsed();

            let grid_dims = term.dimensions();

            Some(TabCellsSnapshot {
                cells,
                grid_dims,
                cursor_pos: current_cursor_pos,
                cursor_style,
                shader_cursor_pos,
                shader_cursor_style,
                is_alt_screen,
                current_generation: snapshot_generation,
            })
        } else if let Some(cached) = cache_cells {
            // Terminal locked (e.g., upload in progress) — use cached cells so the
            // rest of the render pipeline (including file transfer overlay) can proceed.
            // Unwrap the Arc: if this is the sole reference the Vec is moved for free,
            // otherwise a clone is made (rare — only if another Arc clone is live).
            let cached_vec = Arc::try_unwrap(cached).unwrap_or_else(|a| (*a).clone());
            Some(TabCellsSnapshot {
                cells: cached_vec,
                grid_dims: cache_grid_dims,
                cursor_pos: cache_cursor_pos,
                cursor_style: None,
                shader_cursor_pos: cache_cursor_pos,
                shader_cursor_style: None,
                is_alt_screen: was_alt_screen,
                current_generation: cache_generation,
            })
        } else {
            // Terminal locked and no cache available — skip this frame.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cursor_is_on_live_view, snapshot_generation_for_cells};

    #[test]
    fn stale_cached_cells_do_not_advance_snapshot_generation() {
        assert_eq!(snapshot_generation_for_cells(42, 41, true), 41);
    }

    #[test]
    fn fresh_cells_use_terminal_generation() {
        assert_eq!(snapshot_generation_for_cells(42, 41, false), 42);
    }

    #[test]
    fn primary_screen_cursor_is_not_live_when_scrolled_back() {
        assert!(!cursor_is_on_live_view(1, false));
    }

    #[test]
    fn alt_screen_cursor_is_live_even_with_stale_scroll_offset() {
        assert!(cursor_is_on_live_view(1, true));
    }
}
