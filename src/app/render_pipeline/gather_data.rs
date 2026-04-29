//! Terminal-state snapshot for one render frame.
//!
//! `gather_render_data` is the first substantive step of every render cycle.
//! It assembles a `FrameRenderData` by coordinating helpers from three
//! focused sub-modules:
//!
//! - `viewport`: `gather_viewport_sizing`, `resolve_cursor_shader_hide`
//! - `tab_snapshot`: `extract_tab_cells` / `TabCellsSnapshot`
//! - (this module): URL detection, search highlights, scrollback marks,
//!   window title update, cursor blink
//!
use super::FrameRenderData;
use super::tab_snapshot;
use crate::app::window_state::WindowState;
impl WindowState {
    /// Gather all data needed for this render frame.
    /// Returns None if rendering should be skipped (no renderer, no active tab, terminal locked, etc.)
    pub(super) fn gather_render_data(&mut self) -> Option<FrameRenderData> {
        let (renderer_size, visible_lines, _grid_cols) = self.gather_viewport_sizing()?;

        // Get active tab's terminal and immediate state snapshots (avoid long borrows)
        let (
            terminal,
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            cached_scrollback_len,
            cache_grid_dims,
            cached_terminal_title,
            hovered_url,
        ) = match self.tab_manager.active_tab() {
            Some(t) => (
                // Use the focused pane's terminal for cache invalidation.
                // In single-pane mode this is the same Arc as tab.terminal.
                // In split-pane mode, using the primary pane's terminal means changes
                // to a secondary focused pane never trigger a cache miss, so URL
                // detection never re-runs and stale underlines persist after content
                // changes or terminal clears in the focused pane.
                t.pane_manager
                    .as_ref()
                    .and_then(|pm| pm.focused_pane())
                    .map(|p| p.terminal.clone())
                    .unwrap_or_else(|| t.terminal.clone()),
                t.active_scroll_state().offset,
                t.selection_mouse().selection,
                t.active_cache().cells.clone(),
                t.active_cache().generation,
                t.active_cache().scroll_offset,
                t.active_cache().cursor_pos,
                t.active_cache().selection,
                t.active_cache().scrollback_len,
                t.active_cache().grid_dims,
                t.active_cache().terminal_title.clone(),
                t.active_mouse().hovered_url.clone(),
            ),
            None => return None,
        };

        // Check if shell has exited
        let _is_running = if let Ok(term) = terminal.try_write() {
            term.is_running()
        } else {
            true // Assume running if locked
        };

        // Extract terminal cells using the focused tab_snapshot helper.
        let was_alt_screen = self
            .tab_manager
            .active_tab()
            .map(|t| t.was_alt_screen)
            .unwrap_or(false);
        let snap = self.extract_tab_cells(tab_snapshot::TabCellsParams {
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            cache_grid_dims,
            terminal: terminal.clone(),
            was_alt_screen,
        })?;

        let mut cells = snap.cells;
        let current_cursor_pos = snap.cursor_pos;
        let cursor_style = snap.cursor_style;
        let is_alt_screen = snap.is_alt_screen;
        let current_generation = snap.current_generation;
        let cell_grid_dims = snap.grid_dims;

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.was_alt_screen = is_alt_screen;
        }

        // Ensure cursor visibility flag for cell renderer reflects current config every frame
        // (so toggling "Hide default cursor" takes effect immediately even if no other changes).
        // Use the focused viewport helper to resolve hide-cursor state.
        let hide_cursor_for_shader = self.resolve_cursor_shader_hide(is_alt_screen);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_cursor_hidden_for_shader(hide_cursor_for_shader);
        }

        // Flush regenerated cells into the render cache (no-op on cache hit).
        self.flush_cell_cache(&cells, current_cursor_pos, cell_grid_dims);

        // Pre-populate the focused pane's cell cache so that gather_pane_render_data
        // uses the SAME cells that URL detection saw.  Only on cache-miss frames
        // (fresh cells generated) — on cache-hit frames the cells are unchanged, so
        // the clone is unnecessary and its cost (10K+ String clones for each Cell's
        // grapheme) degrades FPS in long-running tmux sessions with many panes.
        if !self.debug.cache_hit
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pm) = tab.pane_manager
            && let Some(pane) = pm.focused_pane_mut()
        {
            pane.cache.pane_cells = Some(std::sync::Arc::new(cells.clone()));
            pane.cache.pane_cells_generation = current_generation;
            pane.cache.pane_cells_scroll_offset = scroll_offset;
            pane.cache.pane_cells_grid_dims = cell_grid_dims;
        }

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title, shell_lifecycle_events) = self
            .collect_scrollback_state(
                &terminal,
                current_cursor_pos,
                cached_scrollback_len,
                &cached_terminal_title,
            );

        // Fire CommandComplete alert sound for any finished commands.
        if shell_lifecycle_events.iter().any(|e| {
            matches!(
                e,
                par_term_terminal::ShellLifecycleEvent::CommandFinished { .. }
            )
        }) {
            self.play_alert_sound(crate::config::AlertEvent::CommandComplete);
        }

        // Update cache scrollback and clamp scroll state.
        //
        // In pane mode the focused pane's own terminal holds the scrollback, not
        // `tab.terminal`.  Clamping here with `tab.terminal.scrollback_len()` would
        // incorrectly cap (or zero-out) the scroll offset every frame.  The correct
        // clamp happens later in the pane render path once we know the focused pane's
        // actual scrollback length.
        let has_multiple_panes = self
            .tab_manager
            .active_tab()
            .map(|t| t.has_multiple_panes())
            .unwrap_or(false);
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // In multi-pane mode, tab.terminal may differ from the focused pane's
            // terminal (e.g. after a split, the new pane has its own terminal).
            // Writing tab.terminal's scrollback_len into the focused pane's cache
            // would incorrectly show the original pane's scrollbar on the new pane.
            // The correct per-pane scrollback_len is written later from
            // gather_pane_render_data in gpu_submit.rs.
            if !has_multiple_panes {
                tab.active_cache_mut().scrollback_len = scrollback_len;
                let sb_len = tab.active_cache().scrollback_len;
                tab.active_scroll_state_mut().clamp_to_scrollback(sb_len);
            }
        }

        // Keep copy mode dimensions in sync with terminal
        if self.copy_mode.active
            && let Ok(term) = terminal.try_write()
        {
            let (cols, rows) = term.dimensions();
            self.copy_mode.update_dimensions(cols, rows, scrollback_len);
        }

        let (scrollback_marks, marks_override_scrollbar) = self.collect_scrollback_marks(&terminal);

        // Keep scrollbar visible when mark indicators exist AND there is scrollback
        // to navigate. Without scrollback there is nothing to scroll to, and showing
        // a scrollbar (with marks from the current prompt line) would be misleading
        // and visually indistinguishable from marks that belong to a different tab.
        // In multi-pane mode, `scrollback_len` comes from tab.terminal which may
        // differ from the focused pane's terminal; skip this override and let the
        // per-pane scrollbar logic (should_show_scrollbar) handle it.
        if marks_override_scrollbar && scrollback_len > 0 && !has_multiple_panes {
            show_scrollbar = true;
        }

        // Update window title if terminal has set one via OSC sequences.
        self.update_window_title_if_changed(&terminal_title, &cached_terminal_title, &hovered_url);

        // Total lines = visible lines + actual scrollback content
        let total_lines = visible_lines + scrollback_len;

        // Detect URLs, apply underlines, and apply search highlights.
        let debug_url_detect_time = self.apply_url_and_search_highlights(
            &mut cells,
            &renderer_size,
            cell_grid_dims,
            scroll_offset,
            scrollback_len,
            visible_lines,
        );

        // Update cursor blink state
        self.update_cursor_blink();

        Some(FrameRenderData {
            cells,
            cursor_pos: current_cursor_pos,
            cursor_style,
            is_alt_screen,
            scrollback_len,
            show_scrollbar,
            visible_lines,
            scrollback_marks,
            total_lines,
            debug_url_detect_time,
        })
    }
}
