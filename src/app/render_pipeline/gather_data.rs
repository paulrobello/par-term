//! Terminal-state snapshot for one render frame.
//!
//! `gather_render_data` is the first substantive step of every render cycle.
//! It assembles a `FrameRenderData` by coordinating helpers from three
//! focused sub-modules:
//!
//! - `viewport`: `gather_viewport_sizing`, `resolve_cursor_shader_hide`
//! - `tab_snapshot`: `extract_tab_cells` / `TabCellsSnapshot`
//! - (this module): prettifier pipeline feed, URL detection, search highlights,
//!   scrollback marks, window title update, cursor blink
//!
//! # R-48: Residual Complexity Note
//!
//! After the Wave 3 extraction (`viewport.rs`, `tab_snapshot.rs`), this module
//! retains ~700 lines driven by shared local variables that prevent clean
//! sub-function extraction without significant restructuring:
//!
//! - `cells`, `current_generation`, `scroll_offset`, `scrollback_len` are
//!   computed early and consumed/mutated by every downstream phase.
//! - The prettifier pipeline phases borrow `tab.prettifier` mutably while
//!   other phases access the focused pane's cache via `tab.pane_manager` —
//!   distinct struct fields, so NLL allows simultaneous borrows (R-32).
//!
//! ## Proposed `ClaudeCodePrettifierBridge` struct (future extraction)
//!
//! The Claude Code-specific prettifier logic (heuristic session detection,
//! viewport hashing, action-bullet segmentation, segment preprocessing) is
//! the densest block in this file (~200 lines, line ~260–465). It could be
//! encapsulated in a dedicated struct that takes the shared variables as
//! constructor arguments, reducing the borrow-checker surface:
//!
//! ```ignore
//! /// Encapsulates per-frame Claude Code viewport → prettifier pipeline interaction.
//! struct ClaudeCodePrettifierBridge<'a> {
//!     pipeline: &'a mut PrettifierPipeline,
//!     cells: &'a [TermCell],
//!     visible_lines: usize,
//!     grid_cols: usize,
//!     scrollback_len: usize,
//!     scroll_offset: usize,
//!     cache: &'a mut RenderCache,
//! }
//!
//! impl<'a> ClaudeCodePrettifierBridge<'a> {
//!     fn detect_session(&mut self) -> bool { ... }
//!     fn compute_viewport_hash(&self) -> u64 { ... }
//!     fn segment_and_submit(&mut self) { ... }
//! }
//! ```
//!
//! Extraction is deferred as a follow-on to R-32 and is a prerequisite of
//! R-31 (gpu_submit stabilization).

use super::FrameRenderData;
use super::claude_code_bridge::ClaudeCodePrettifierBridge;
use super::tab_snapshot;
use crate::app::window_state::WindowState;

impl WindowState {
    /// Gather all data needed for this render frame.
    /// Returns None if rendering should be skipped (no renderer, no active tab, terminal locked, etc.)
    pub(super) fn gather_render_data(&mut self) -> Option<FrameRenderData> {
        let (renderer_size, visible_lines, grid_cols) = self.gather_viewport_sizing()?;

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
            cached_terminal_title,
            hovered_url,
        ) = match self.tab_manager.active_tab() {
            Some(t) => (
                t.terminal.clone(),
                t.active_scroll_state().offset,
                t.selection_mouse().selection,
                t.active_cache().cells.clone(),
                t.active_cache().generation,
                t.active_cache().scroll_offset,
                t.active_cache().cursor_pos,
                t.active_cache().selection,
                t.active_cache().scrollback_len,
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
        let snap = self.extract_tab_cells(tab_snapshot::TabCellsParams {
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            terminal: terminal.clone(),
        })?;

        let mut cells = snap.cells;
        let current_cursor_pos = snap.cursor_pos;
        let cursor_style = snap.cursor_style;
        let is_alt_screen = snap.is_alt_screen;
        let current_generation = snap.current_generation;

        // Sync prettifier alt-screen state, cell dims, and debounce.
        self.sync_prettifier_state(is_alt_screen);

        // Ensure cursor visibility flag for cell renderer reflects current config every frame
        // (so toggling "Hide default cursor" takes effect immediately even if no other changes).
        // Use the focused viewport helper to resolve hide-cursor state.
        let hide_cursor_for_shader = self.resolve_cursor_shader_hide(is_alt_screen);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_cursor_hidden_for_shader(hide_cursor_for_shader);
        }

        // Flush regenerated cells into the render cache (no-op on cache hit).
        self.flush_cell_cache(&cells, current_cursor_pos);

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title, shell_lifecycle_events) = self
            .collect_scrollback_state(
                &terminal,
                current_cursor_pos,
                cached_scrollback_len,
                &cached_terminal_title,
            );

        // Capture prettifier block count before processing events/feed so we can
        // detect when new blocks are added and invalidate the cell cache.
        let prettifier_block_count_before = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.prettifier.as_ref())
            .map(|p| p.active_blocks().len())
            .unwrap_or(0);

        // Forward shell lifecycle events to the prettifier pipeline (outside terminal lock)
        if !shell_lifecycle_events.is_empty()
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
        {
            for event in &shell_lifecycle_events {
                match event {
                    par_term_terminal::ShellLifecycleEvent::CommandStarted {
                        command,
                        absolute_line,
                    } => {
                        // Access the focused pane's cache directly (R-32).
                        // `tab.prettifier` is borrowed as `pipeline` above; `tab.pane_manager`
                        // is a distinct struct field so NLL allows both borrows simultaneously.
                        if let Some(ref mut pm) = tab.pane_manager
                            && let Some(pane) = pm.focused_pane_mut()
                        {
                            pane.cache.prettifier_command_start_line = Some(*absolute_line);
                            pane.cache.prettifier_command_text = Some(command.clone());
                        }
                        pipeline.on_command_start(command);
                    }
                    par_term_terminal::ShellLifecycleEvent::CommandFinished { absolute_line } => {
                        // Extract the cached start line and command text from the focused pane.
                        // `tab.pane_manager` is a distinct field from `tab.prettifier` (borrowed
                        // as `pipeline` above) — NLL permits the simultaneous borrows.
                        let (start, cmd_text) = if let Some(ref mut pm) = tab.pane_manager
                            && let Some(pane) = pm.focused_pane_mut()
                        {
                            (
                                pane.cache.prettifier_command_start_line.take(),
                                pane.cache.prettifier_command_text.take(),
                            )
                        } else {
                            (None, None)
                        };
                        if let Some(start) = start {
                            // Read full command output from scrollback so the
                            // prettified block covers the entire output, not just
                            // the visible portion. This ensures scrolling through
                            // long output shows prettified content throughout.
                            let output_start = start + 1;
                            if let Ok(term) = terminal.try_write() {
                                let lines = term.lines_text_range(output_start, *absolute_line);
                                crate::debug_info!(
                                    "PRETTIFIER",
                                    "submit_command_output: {} lines (rows {}..{})",
                                    lines.len(),
                                    output_start,
                                    absolute_line
                                );
                                pipeline.submit_command_output(lines, cmd_text);
                            } else {
                                // Lock failed — fall back to boundary detector state
                                pipeline.on_command_end();
                            }
                        } else {
                            pipeline.on_command_end();
                        }
                    }
                }
            }
        }

        // Fire CommandComplete alert sound for any finished commands.
        if shell_lifecycle_events
            .iter()
            .any(|e| matches!(e, par_term_terminal::ShellLifecycleEvent::CommandFinished { .. }))
        {
            self.play_alert_sound(crate::config::AlertEvent::CommandComplete);
        }

        // Feed terminal output lines to the prettifier pipeline (gated on content changes).
        // Skip per-frame viewport feed for CommandOutput scope — it reads full output
        // from scrollback on CommandFinished instead.
        //
        // Note: We check the cache conditions first (before borrowing `pipeline`) to avoid
        // simultaneous borrow conflicts between `tab.prettifier` and the focused pane cache.
        // After R-32, per-pane cache is accessed via `tab.pane_manager` (a distinct field from
        // `tab.prettifier`), so NLL permits simultaneous borrows inside the pipeline block.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Check conditions that don't need pipeline borrow
            let needs_feed = tab.prettifier.as_ref().is_some_and(|p| p.is_enabled())
                && !is_alt_screen
                && (current_generation != tab.active_cache().prettifier_feed_generation
                    || scroll_offset != tab.active_cache().prettifier_feed_scroll_offset);

            if needs_feed
                && let Some(ref mut pipeline) = tab.prettifier
                && pipeline.detection_scope()
                    != crate::prettifier::boundary::DetectionScope::CommandOutput
            {
                // Update the focused pane's cache feed tracking fields.
                // `tab.prettifier` is borrowed above as `pipeline`; `tab.pane_manager`
                // is a distinct struct field — NLL allows the simultaneous borrows.
                if let Some(ref mut pm) = tab.pane_manager
                    && let Some(pane) = pm.focused_pane_mut()
                {
                    pane.cache.prettifier_feed_generation = current_generation;
                    pane.cache.prettifier_feed_scroll_offset = scroll_offset;
                }

                // Delegate Claude Code session detection, viewport hashing, and
                // segment submission to `ClaudeCodePrettifierBridge`.
                // `tab.prettifier` is borrowed above as `pipeline`; `tab.pane_manager`
                // is a distinct struct field — NLL permits the simultaneous borrows.
                let is_claude_session = {
                    let mut bridge = ClaudeCodePrettifierBridge {
                        pipeline,
                        pane_manager: &mut tab.pane_manager,
                        cells: &cells,
                        visible_lines,
                        grid_cols,
                        scrollback_len,
                        scroll_offset,
                    };
                    bridge.detect_session();
                    let active = bridge.pipeline.claude_code().is_active();
                    if active {
                        let viewport_hash = bridge.compute_viewport_hash();
                        let cached_hash = bridge.cached_viewport_hash();
                        let viewport_changed = viewport_hash != cached_hash;
                        if viewport_changed {
                            bridge.store_viewport_hash(viewport_hash);
                        }
                        bridge.segment_and_submit(viewport_changed);
                    }
                    active
                    // bridge is dropped here, releasing borrows on pipeline and pane_manager
                };

                if !is_claude_session {
                    // Non-Claude session: submit the entire visible content as a
                    // single block. This gives the detector full context (avoids
                    // splitting markdown at blank lines) and reduces block churn.
                    //
                    // Throttle: during streaming, content changes every frame (~16ms).
                    // Recompute a quick hash and skip if content hasn't changed.
                    // If content did change, only re-submit if enough time has elapsed
                    // (150ms) to avoid rendering 60 intermediate states per second.
                    pipeline.reset_boundary();

                    let mut lines: Vec<(String, usize)> = Vec::with_capacity(visible_lines);
                    for row_idx in 0..visible_lines {
                        let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;

                        let start = row_idx * grid_cols;
                        let end = (start + grid_cols).min(cells.len());
                        if start >= cells.len() {
                            break;
                        }

                        let line: String = cells[start..end]
                            .iter()
                            .map(|c| {
                                let g = c.grapheme.as_str();
                                if g.is_empty() || g == "\0" { " " } else { g }
                            })
                            .collect::<String>()
                            .trim_end()
                            .to_string();

                        lines.push((line, absolute_row));
                    }

                    if !lines.is_empty() {
                        // Quick content hash for dedup.
                        let content_hash = {
                            use std::hash::{Hash, Hasher};
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            for (line, row) in &lines {
                                line.hash(&mut hasher);
                                row.hash(&mut hasher);
                            }
                            hasher.finish()
                        };

                        // R-32: read/write the focused pane's cache via tab.pane_manager
                        // (distinct field from tab.prettifier/pipeline — NLL allows it).
                        let cached_last_hash = tab
                            .pane_manager
                            .as_ref()
                            .and_then(|pm| pm.focused_pane())
                            .map(|p| p.cache.prettifier_feed_last_hash)
                            .unwrap_or(0);

                        if content_hash == cached_last_hash {
                            // Identical content — skip entirely.
                            crate::debug_trace!(
                                "PRETTIFIER",
                                "per-frame feed (non-CC): content unchanged, skipping"
                            );
                        } else {
                            let elapsed = tab
                                .pane_manager
                                .as_ref()
                                .and_then(|pm| pm.focused_pane())
                                .map(|p| p.cache.prettifier_feed_last_time.elapsed())
                                .unwrap_or_default();
                            let throttle = std::time::Duration::from_millis(150);
                            let has_block = !pipeline.active_blocks().is_empty();

                            if has_block && elapsed < throttle {
                                // Actively streaming with an existing prettified block.
                                // Defer re-render to avoid per-frame churn.
                                crate::debug_trace!(
                                    "PRETTIFIER",
                                    "per-frame feed (non-CC): throttled ({:.0}ms < {}ms), deferring",
                                    elapsed.as_secs_f64() * 1000.0,
                                    throttle.as_millis()
                                );
                            } else {
                                crate::debug_log!(
                                    "PRETTIFIER",
                                    "per-frame feed (non-CC): submitting {} visible lines as single block, scrollback={}, scroll_offset={}",
                                    visible_lines,
                                    scrollback_len,
                                    scroll_offset
                                );
                                if let Some(ref mut pm) = tab.pane_manager
                                    && let Some(pane) = pm.focused_pane_mut()
                                {
                                    pane.cache.prettifier_feed_last_hash = content_hash;
                                    pane.cache.prettifier_feed_last_time =
                                        std::time::Instant::now();
                                }
                                pipeline.submit_command_output(lines, None);
                            }
                        }
                    }
                } // end if !is_claude_session
            } // end if let Some(tab) pipeline-borrow scope
        } // end if let Some(tab) = self.tab_manager.active_tab_mut()

        // If new prettified blocks were added during event processing or per-frame feed,
        // invalidate the cell cache so the next frame runs cell substitution.
        {
            let block_count_after = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.prettifier.as_ref())
                .map(|p| p.active_blocks().len())
                .unwrap_or(0);
            if block_count_after > prettifier_block_count_before {
                crate::debug_info!(
                    "PRETTIFIER",
                    "new blocks detected ({} -> {}), invalidating cell cache",
                    prettifier_block_count_before,
                    block_count_after
                );
                self.invalidate_tab_cache();
            }
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
            grid_cols,
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
            grid_cols,
            scrollback_marks,
            total_lines,
            debug_url_detect_time,
        })
    }
}
