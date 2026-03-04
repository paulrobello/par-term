//! Focused helper methods for individual phases of `gather_render_data`.
//!
//! Each function here is a thin `impl WindowState` method that accepts the
//! shared local values that `gather_render_data` has already computed, reducing
//! the size of that function without requiring a `GatherDataContext` struct
//! (which the borrow checker fights when `tab.prettifier` and `tab.pane_manager`
//! are simultaneously borrowed).

use std::sync::Arc;
use std::time::Instant;

use par_term_config::ScrollbackMark;
use par_term_terminal::TerminalManager;
use winit::dpi::PhysicalSize;

use crate::app::window_state::WindowState;
use crate::cell_renderer::Cell;

impl WindowState {
    /// Collect scrollback length, terminal title, and drain shell lifecycle events
    /// from the active terminal.  Updates command history from scrollback marks and
    /// the core library.
    ///
    /// Returns `(scrollback_len, terminal_title, shell_lifecycle_events)`.
    /// Falls back to cached values when the terminal is locked.
    pub(super) fn collect_scrollback_state(
        &mut self,
        terminal: &Arc<tokio::sync::RwLock<TerminalManager>>,
        current_cursor_pos: Option<(usize, usize)>,
        cached_scrollback_len: usize,
        cached_terminal_title: &str,
    ) -> (usize, String, Vec<par_term_terminal::ShellLifecycleEvent>) {
        if let Ok(mut term) = terminal.try_write() {
            let cursor_row = current_cursor_pos.map(|(_, row)| row).unwrap_or(0);
            let sb_len = term.scrollback_len();
            term.update_scrollback_metadata(sb_len, cursor_row);

            let shell_events = term.drain_shell_lifecycle_events();

            // Feed newly completed commands into persistent history from two sources:
            // 1. Scrollback marks (populated via set_mark_command_at from grid text extraction)
            // 2. Core library command history (populated by the terminal emulator core)
            for mark in term.scrollback_marks() {
                if let Some(ref cmd) = mark.command
                    && !cmd.is_empty()
                    && self.overlay_ui.synced_commands.insert(cmd.clone())
                {
                    self.overlay_ui.command_history.add(
                        cmd.clone(),
                        mark.exit_code,
                        mark.duration_ms,
                    );
                }
            }
            for (cmd, exit_code, duration_ms) in term.core_command_history() {
                if !cmd.is_empty() && self.overlay_ui.synced_commands.insert(cmd.clone()) {
                    self.overlay_ui
                        .command_history
                        .add(cmd, exit_code, duration_ms);
                }
            }

            (sb_len, term.get_title(), shell_events)
        } else {
            (
                cached_scrollback_len,
                cached_terminal_title.to_string(),
                Vec::new(),
            )
        }
    }

    /// Collect scrollback marks from the terminal and append trigger-generated marks.
    ///
    /// Returns `(scrollback_marks, override_show_scrollbar)`.  When marks are
    /// present, `override_show_scrollbar` is `true`, which forces the scrollbar
    /// visible regardless of the configured threshold.
    pub(super) fn collect_scrollback_marks(
        &self,
        terminal: &Arc<tokio::sync::RwLock<TerminalManager>>,
    ) -> (Vec<ScrollbackMark>, bool) {
        let need_marks =
            self.config.scrollbar_command_marks || self.config.command_separator_enabled;
        let mut scrollback_marks: Vec<ScrollbackMark> = if need_marks {
            if let Ok(term) = terminal.try_write() {
                term.scrollback_marks()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Append trigger-generated marks
        self.with_active_tab(|tab| {
            scrollback_marks.extend(tab.scripting.trigger_marks.iter().cloned())
        });

        let override_show = !scrollback_marks.is_empty();
        (scrollback_marks, override_show)
    }

    /// Update the OS window title when the terminal has set one via OSC sequences.
    ///
    /// Only fires when `allow_title_change` is configured, no URL tooltip is
    /// being shown, and the terminal-provided title has changed since last frame.
    pub(super) fn update_window_title_if_changed(
        &mut self,
        terminal_title: &str,
        cached_terminal_title: &str,
        hovered_url: &Option<String>,
    ) {
        if self.config.allow_title_change
            && hovered_url.is_none()
            && terminal_title != cached_terminal_title
        {
            let owned = terminal_title.to_string();
            self.with_active_tab_mut(|tab| tab.active_cache_mut().terminal_title = owned.clone());
            if let Some(window) = &self.window {
                if terminal_title.is_empty() {
                    window.set_title(&self.format_title(&self.config.window_title));
                } else {
                    window.set_title(&self.format_title(terminal_title));
                }
            }
        }
    }

    /// Apply URL detection and search highlighting to the cell buffer.
    ///
    /// Re-detects URLs only on cache misses; search highlights are applied every
    /// frame since cells may be regenerated even on cache hits.
    ///
    /// Returns the elapsed time spent on URL detection (zero on cache hit).
    pub(super) fn apply_url_and_search_highlights(
        &mut self,
        cells: &mut [Cell],
        renderer_size: &PhysicalSize<u32>,
        grid_cols: usize,
        scroll_offset: usize,
        scrollback_len: usize,
        visible_lines: usize,
    ) -> std::time::Duration {
        let url_detect_start = Instant::now();
        let debug_url_detect_time = if !self.debug.cache_hit {
            self.detect_urls();
            url_detect_start.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        self.apply_url_underlines(cells, renderer_size);

        if self.overlay_ui.search_ui.visible {
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_write()
            {
                let lines_iter =
                    crate::app::window_state::search_highlight::get_all_searchable_lines(
                        &term,
                        visible_lines,
                    );
                self.overlay_ui.search_ui.update_search(lines_iter);
            }

            let scroll_offset_now = self
                .tab_manager
                .active_tab()
                .map(|t| t.active_scroll_state().offset)
                .unwrap_or(scroll_offset);

            self.apply_search_highlights(
                cells,
                grid_cols,
                scroll_offset_now,
                scrollback_len,
                visible_lines,
            );

            // Force GPU cell update when search is visible: highlights are applied to cells
            // every frame, but renderer.update_cells() is skipped on cache hits, causing
            // the highlighted cells to never reach the GPU.
            self.debug.cache_hit = false;
            // Also mark renderer dirty to ensure a full render pass runs (not just the egui
            // fast-path), so the updated cell buffer is actually drawn to screen.
            if let Some(renderer) = &mut self.renderer {
                renderer.mark_dirty();
            }
        }

        debug_url_detect_time
    }

    /// Sync the prettifier pipeline state for the active tab.
    ///
    /// Handles alt-screen transitions, keeps cell dimensions up-to-date, and
    /// triggers the debounce check.  Called once per frame before the main
    /// prettifier feed.
    pub(super) fn sync_prettifier_state(&mut self, is_alt_screen: bool) {
        let prettifier_cell_dims = self
            .renderer
            .as_ref()
            .map(|r| (r.cell_width(), r.cell_height()));

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            if is_alt_screen != tab.was_alt_screen {
                if let Some(ref mut pipeline) = tab.prettifier {
                    pipeline.on_alt_screen_change(is_alt_screen);
                }
                tab.was_alt_screen = is_alt_screen;
            }

            if let Some(ref mut pipeline) = tab.prettifier {
                if let Some((cw, ch)) = prettifier_cell_dims {
                    pipeline.update_cell_dims(cw, ch);
                }
                pipeline.check_debounce();
            }
        }
    }

    /// Flush the regenerated cell snapshot into the active tab's render cache.
    ///
    /// Skipped when `cache_hit` is set (no new content) to avoid redundant
    /// `Arc` allocations every frame.
    pub(super) fn flush_cell_cache(
        &mut self,
        cells: &[Cell],
        current_cursor_pos: Option<(usize, usize)>,
    ) {
        if self.debug.cache_hit {
            return;
        }
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let new_gen = if let Ok(term) = tab.terminal.try_write() {
                Some(term.update_generation())
            } else {
                None
            };
            if let Some(new_gen) = new_gen {
                let current_scroll_offset = tab.active_scroll_state().offset;
                let current_selection = tab.selection_mouse().selection;
                tab.active_cache_mut().cells = Some(Arc::new(cells.to_vec()));
                tab.active_cache_mut().generation = new_gen;
                tab.active_cache_mut().scroll_offset = current_scroll_offset;
                tab.active_cache_mut().cursor_pos = current_cursor_pos;
                tab.active_cache_mut().selection = current_selection;
            }
        }
    }
}
