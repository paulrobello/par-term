//! Focused helper methods for individual phases of `gather_render_data`.
//!
//! Each function here is a thin `impl WindowState` method that accepts the
//! shared local values that `gather_render_data` has already computed, reducing
//! the size of that function without requiring a `GatherDataContext` struct
//! while keeping the borrow checker surface small.

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
            //
            // Only run when scrollback has grown since last sync — both
            // scrollback_marks() and core_command_history() clone the FULL
            // internal Vec on each call, which is O(n) where n grows with
            // session time. Skipping when scrollback is unchanged avoids
            // these allocations entirely on idle frames, preventing the
            // gradual FPS degradation seen in long tmux sessions.
            if sb_len > cached_scrollback_len {
                let marks = term.scrollback_marks();
                let prev_mark_count = self.overlay_ui.synced_mark_count;
                if marks.len() > prev_mark_count {
                    for mark in marks.iter().skip(prev_mark_count) {
                        if let Some(ref cmd) = mark.command
                            && !cmd.is_empty()
                        {
                            if self.overlay_ui.synced_commands.insert(cmd.clone()) {
                                self.overlay_ui.command_history.add(
                                    cmd.clone(),
                                    mark.exit_code,
                                    mark.duration_ms,
                                );
                            } else if mark.exit_code.is_some() {
                                self.overlay_ui.command_history.update_exit_code_if_unknown(
                                    cmd,
                                    mark.exit_code,
                                    mark.duration_ms,
                                );
                            }
                        }
                    }
                    self.overlay_ui.synced_mark_count = marks.len();
                }
                let history = term.core_command_history();
                let prev_history_count = self.overlay_ui.synced_core_history_count;
                if history.len() > prev_history_count {
                    for (cmd, exit_code, duration_ms) in history.iter().skip(prev_history_count) {
                        if !cmd.is_empty() {
                            if self.overlay_ui.synced_commands.insert(cmd.clone()) {
                                self.overlay_ui.command_history.add(
                                    cmd.clone(),
                                    *exit_code,
                                    *duration_ms,
                                );
                            } else if exit_code.is_some() {
                                self.overlay_ui.command_history.update_exit_code_if_unknown(
                                    cmd,
                                    *exit_code,
                                    *duration_ms,
                                );
                            }
                        }
                    }
                    self.overlay_ui.synced_core_history_count = history.len();
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
        let need_marks = self.config.load().scrollbar_command_marks
            || self.config.load().command_separator_enabled;
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
        if self.config.load().allow_title_change
            && hovered_url.is_none()
            && terminal_title != cached_terminal_title
        {
            let owned = terminal_title.to_string();
            self.with_active_tab_mut(|tab| tab.active_cache_mut().terminal_title = owned.clone());
            if let Some(window) = &self.window {
                if terminal_title.is_empty() {
                    window.set_title(&self.format_title(&self.config.load().window_title));
                } else {
                    window.set_title(&self.format_title(terminal_title));
                }
            }
        }
    }

    /// Run URL detection and search index updates for the current frame.
    ///
    /// Re-detects URLs only on cache misses; search match positions are refreshed
    /// every frame since cells may be regenerated even on cache hits.
    ///
    /// Cell modifications (underlines, highlight colors) are NOT applied here —
    /// they are applied to pane cells in `gpu_submit.rs` after
    /// `gather_pane_render_data()`, which is the only path visible to the renderer.
    ///
    /// Returns the elapsed time spent on URL detection (zero on cache hit).
    pub(super) fn apply_url_and_search_highlights(
        &mut self,
        cells: &mut [Cell],
        _renderer_size: &PhysicalSize<u32>,
        cell_grid_dims: (usize, usize),
        scroll_offset: usize,
        _scrollback_len: usize,
        visible_lines: usize,
    ) -> std::time::Duration {
        let url_detect_start = Instant::now();
        let debug_url_detect_time = if !self.debug.cache_hit {
            // Use the terminal's actual grid dimensions (from TabCellsSnapshot)
            // rather than the renderer's grid_cols.  In split-pane mode or when
            // the scrollbar is visible, the pane terminal has different dimensions
            // than the renderer grid.  Using renderer dims would mis-align row
            // boundaries in the cell array and produce wrong URL positions.
            let (actual_cols, actual_rows) = cell_grid_dims;
            self.detect_urls(crate::app::window_state::url_hover::UrlDetectData {
                cells,
                cols: if actual_cols > 0 { actual_cols } else { 1 },
                rows: if actual_rows > 0 {
                    actual_rows
                } else {
                    visible_lines
                },
                scroll_offset,
            });
            url_detect_start.elapsed()
        } else {
            std::time::Duration::ZERO
        };

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

            // Force GPU cell update when search is visible: highlights are applied to
            // pane cells every frame, but renderer.update_cells() is skipped on cache
            // hits, causing the highlighted cells to never reach the GPU.
            self.debug.cache_hit = false;
            // Also mark renderer dirty to ensure a full render pass runs (not just the
            // egui fast-path), so the updated cell buffer is actually drawn to screen.
            if let Some(renderer) = &mut self.renderer {
                renderer.mark_dirty();
            }
        }

        debug_url_detect_time
    }

    /// Flush the regenerated cell snapshot into the active tab's render cache.
    ///
    /// Skipped when `cache_hit` is set (no new content) to avoid redundant
    /// `Arc` allocations every frame.
    pub(super) fn flush_cell_cache(
        &mut self,
        cells: &[Cell],
        current_cursor_pos: Option<(usize, usize)>,
        grid_dims: (usize, usize),
    ) {
        if self.debug.cache_hit {
            return;
        }
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Use the focused pane's terminal to store the generation, matching the
            // terminal used for cache invalidation in gather_render_data. If we stored
            // the primary pane's generation but checked the focused pane's generation
            // next frame, a mismatch would force a cache miss every frame in split mode.
            let focused_terminal = tab
                .pane_manager
                .as_ref()
                .and_then(|pm| pm.focused_pane())
                .map(|p| p.terminal.clone())
                .unwrap_or_else(|| tab.terminal.clone());
            let new_gen = if let Ok(term) = focused_terminal.try_write() {
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
                tab.active_cache_mut().grid_dims = grid_dims;
            }
        }
    }
}
