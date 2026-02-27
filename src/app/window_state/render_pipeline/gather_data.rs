//! Terminal-state snapshot for one render frame.
//!
//! `gather_render_data` is the first substantive step of every render cycle.
//! It locks the active terminal (or falls back to cached cells), builds the
//! cell grid, runs the prettifier pipeline, detects URLs, applies search
//! highlights, and returns a `FrameRenderData` snapshot that the rest of the
//! render pipeline consumes.

use super::super::{preprocess_claude_code_segment, reconstruct_markdown_from_cells};
use super::FrameRenderData;
use crate::app::window_state::WindowState;
use crate::config::CursorStyle;
use crate::selection::SelectionMode;
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use std::sync::Arc;

impl WindowState {
    /// Gather all data needed for this render frame.
    /// Returns None if rendering should be skipped (no renderer, no active tab, terminal locked, etc.)
    pub(super) fn gather_render_data(&mut self) -> Option<FrameRenderData> {
        let (renderer_size, visible_lines, grid_cols) = if let Some(renderer) = &self.renderer {
            let (cols, rows) = renderer.grid_size();
            (renderer.size(), rows, cols)
        } else {
            return None;
        };

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
                t.scroll_state.offset,
                t.mouse.selection,
                t.cache.cells.clone(),
                t.cache.generation,
                t.cache.scroll_offset,
                t.cache.cursor_pos,
                t.cache.selection,
                t.cache.scrollback_len,
                t.cache.terminal_title.clone(),
                t.mouse.hovered_url.clone(),
            ),
            None => return None,
        };

        // Check if shell has exited
        let _is_running = if let Ok(term) = terminal.try_lock() {
            term.is_running()
        } else {
            true // Assume running if locked
        };

        // Get scroll offset and selection from active tab

        // Get terminal cells for rendering (with dirty tracking optimization)
        // Also capture alt screen state to disable cursor shader for TUI apps
        let (mut cells, current_cursor_pos, cursor_style, is_alt_screen, current_generation) =
            if let Ok(term) = terminal.try_lock() {
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

                // Get cursor position and opacity (only show if we're at the bottom with no scroll offset
                // and the cursor is visible - TUI apps hide cursor via DECTCEM escape sequence)
                // If lock_cursor_visibility is enabled, ignore the terminal's visibility state
                // In copy mode, use the copy mode cursor position instead
                let cursor_visible = self.config.lock_cursor_visibility || term.is_cursor_visible();
                let current_cursor_pos = if self.copy_mode.active {
                    self.copy_mode.screen_cursor_pos(scroll_offset)
                } else if scroll_offset == 0 && cursor_visible {
                    Some(term.cursor_position())
                } else {
                    None
                };

                let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_anim.cursor_opacity));

                // Get cursor style for geometric rendering
                // In copy mode, always use SteadyBlock for clear visibility
                // If lock_cursor_style is enabled, use the config's cursor style instead of terminal's
                // If lock_cursor_blink is enabled and cursor_blink is false, force steady cursor
                let cursor_style = if self.copy_mode.active && current_cursor_pos.is_some() {
                    Some(TermCursorStyle::SteadyBlock)
                } else if current_cursor_pos.is_some() {
                    if self.config.lock_cursor_style {
                        // Convert config cursor style to terminal cursor style
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
                                TermCursorStyle::BlinkingUnderline => {
                                    TermCursorStyle::SteadyUnderline
                                }
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

                // Check if we need to regenerate cells
                // Only regenerate when content actually changes, not on every cursor blink
                let needs_regeneration = cache_cells.is_none()
                || current_generation != cache_generation
                || scroll_offset != cache_scroll_offset
                || current_cursor_pos != cache_cursor_pos // Regenerate if cursor position changed
                || mouse_selection != cache_selection; // Regenerate if selection changed (including clearing)

                let cell_gen_start = std::time::Instant::now();
                let (cells, is_cache_hit) = if needs_regeneration {
                    // Generate fresh cells
                    let fresh_cells = term.get_cells_with_scrollback(
                        scroll_offset,
                        selection,
                        rectangular,
                        cursor,
                    );

                    (fresh_cells, false)
                } else {
                    // Cache hit: clone the Vec through the Arc (one allocation instead of two).
                    // apply_url_underlines needs a mutable Vec, so we still need an owned copy,
                    // but the Arc clone that extracted cache_cells from tab.cache was free.
                    (cache_cells.as_ref().expect("window_state: cache_cells must be Some when needs_regeneration is false").as_ref().clone(), true)
                };
                self.debug.cache_hit = is_cache_hit;
                self.debug.cell_gen_time = cell_gen_start.elapsed();

                // Check if alt screen is active (TUI apps like vim, htop)
                let is_alt_screen = term.is_alt_screen_active();

                (
                    cells,
                    current_cursor_pos,
                    cursor_style,
                    is_alt_screen,
                    current_generation,
                )
            } else if let Some(cached) = cache_cells {
                // Terminal locked (e.g., upload in progress), use cached cells so the
                // rest of the render pipeline (including file transfer overlay) can proceed.
                // Unwrap the Arc: if this is the sole reference the Vec is moved for free,
                // otherwise a clone is made (rare — only if another Arc clone is live).
                let cached_vec = Arc::try_unwrap(cached).unwrap_or_else(|a| (*a).clone());
                (cached_vec, cache_cursor_pos, None, false, cache_generation)
            } else {
                return None; // Terminal locked and no cache available, skip this frame
            };

        // --- Prettifier pipeline update ---
        // Feed terminal output changes to the prettifier, check debounce, and handle
        // alt-screen transitions. This runs outside the terminal lock.
        // Capture cell dims from the renderer before borrowing the tab mutably.
        let prettifier_cell_dims = self
            .renderer
            .as_ref()
            .map(|r| (r.cell_width(), r.cell_height()));
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Detect alt-screen transitions
            if is_alt_screen != tab.was_alt_screen {
                if let Some(ref mut pipeline) = tab.prettifier {
                    pipeline.on_alt_screen_change(is_alt_screen);
                }
                tab.was_alt_screen = is_alt_screen;
            }

            // Always check debounce (cheap: just a timestamp comparison)
            if let Some(ref mut pipeline) = tab.prettifier {
                // Keep prettifier cell dims in sync with the GPU renderer so
                // that inline graphics (e.g., Mermaid diagrams) are sized
                // correctly instead of using the fallback estimate.
                if let Some((cw, ch)) = prettifier_cell_dims {
                    pipeline.update_cell_dims(cw, ch);
                }
                pipeline.check_debounce();
            }
        }

        // Ensure cursor visibility flag for cell renderer reflects current config every frame
        // (so toggling "Hide default cursor" takes effect immediately even if no other changes)
        // Resolve hides_cursor: per-shader override -> metadata defaults -> global config
        let resolved_hides_cursor = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.hides_cursor)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.hides_cursor)
            })
            .unwrap_or(self.config.cursor_shader_hides_cursor);
        // Resolve disable_in_alt_screen: per-shader override -> metadata defaults -> global config
        let resolved_disable_in_alt_screen = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.disable_in_alt_screen)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.disable_in_alt_screen)
            })
            .unwrap_or(self.config.cursor_shader_disable_in_alt_screen);
        let hide_cursor_for_shader = self.config.cursor_shader_enabled
            && resolved_hides_cursor
            && !(resolved_disable_in_alt_screen && is_alt_screen);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_cursor_hidden_for_shader(hide_cursor_for_shader);
        }

        // Update cache with regenerated cells (if needed)
        // Need to re-borrow as mutable after the terminal lock is released
        if !self.debug.cache_hit
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Ok(term) = tab.terminal.try_lock()
        {
            let current_generation = term.update_generation();
            tab.cache.cells = Some(Arc::new(cells.clone()));
            tab.cache.generation = current_generation;
            tab.cache.scroll_offset = tab.scroll_state.offset;
            tab.cache.cursor_pos = current_cursor_pos;
            tab.cache.selection = tab.mouse.selection;
        }

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title, shell_lifecycle_events) =
            if let Ok(mut term) = terminal.try_lock() {
                // Use cursor row 0 when cursor not visible (e.g., alt screen)
                let cursor_row = current_cursor_pos.map(|(_, row)| row).unwrap_or(0);
                let sb_len = term.scrollback_len();
                term.update_scrollback_metadata(sb_len, cursor_row);

                // Drain shell lifecycle events for the prettifier pipeline
                let shell_events = term.drain_shell_lifecycle_events();

                // Feed newly completed commands into persistent history from two sources:
                // 1. Scrollback marks (populated via set_mark_command_at from grid text extraction)
                // 2. Core library command history (populated by the terminal emulator core)
                // Both sources are checked because command text may come from either path
                // depending on shell integration quality. The synced_commands set prevents
                // duplicate adds across frames and sources.
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
                    cached_terminal_title.clone(),
                    Vec::new(),
                )
            };

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
                        tab.cache.prettifier_command_start_line = Some(*absolute_line);
                        tab.cache.prettifier_command_text = Some(command.clone());
                        pipeline.on_command_start(command);
                    }
                    par_term_terminal::ShellLifecycleEvent::CommandFinished { absolute_line } => {
                        if let Some(start) = tab.cache.prettifier_command_start_line.take() {
                            let cmd_text = tab.cache.prettifier_command_text.take();
                            // Read full command output from scrollback so the
                            // prettified block covers the entire output, not just
                            // the visible portion. This ensures scrolling through
                            // long output shows prettified content throughout.
                            let output_start = start + 1;
                            if let Ok(term) = terminal.try_lock() {
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

        // Feed terminal output lines to the prettifier pipeline (gated on content changes).
        // Skip per-frame viewport feed for CommandOutput scope — it reads full output
        // from scrollback on CommandFinished instead.
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
            && pipeline.is_enabled()
            && !is_alt_screen
            && pipeline.detection_scope()
                != crate::prettifier::boundary::DetectionScope::CommandOutput
            && (current_generation != tab.cache.prettifier_feed_generation
                || scroll_offset != tab.cache.prettifier_feed_scroll_offset)
        {
            tab.cache.prettifier_feed_generation = current_generation;
            tab.cache.prettifier_feed_scroll_offset = scroll_offset;

            // Heuristic Claude Code session detection from visible output.
            // One-time: scan for signature patterns if not yet detected.
            if !pipeline.claude_code().is_active() {
                'detect: for row_idx in 0..visible_lines {
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }
                    let row_text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();
                    // Look for Claude Code signature patterns in output.
                    if row_text.contains("Claude Code")
                        || row_text.contains("claude.ai/code")
                        || row_text.contains("Tips for getting the best")
                        || (row_text.contains("Model:")
                            && (row_text.contains("Opus")
                                || row_text.contains("Sonnet")
                                || row_text.contains("Haiku")))
                    {
                        crate::debug_info!(
                            "PRETTIFIER",
                            "Claude Code session detected from output heuristic"
                        );
                        pipeline.mark_claude_code_active();
                        break 'detect;
                    }
                }
            }

            let is_claude_session = pipeline.claude_code().is_active();

            if is_claude_session {
                // Clear blocks when visible content changes. Claude Code
                // rewrites the screen in-place (e.g., permission prompts,
                // progress updates) without growing scrollback, so we hash
                // a sample of visible rows to detect viewport-level changes.
                let viewport_hash = {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    // Sample every 4th row for speed; enough to catch redraws.
                    for row_idx in (0..visible_lines).step_by(4) {
                        let start = row_idx * grid_cols;
                        let end = (start + grid_cols).min(cells.len());
                        if start >= cells.len() {
                            break;
                        }
                        for c in &cells[start..end] {
                            c.grapheme.as_str().hash(&mut hasher);
                        }
                    }
                    scrollback_len.hash(&mut hasher);
                    scroll_offset.hash(&mut hasher);
                    hasher.finish()
                };
                let viewport_changed = viewport_hash != tab.cache.prettifier_feed_last_hash;
                if viewport_changed {
                    tab.cache.prettifier_feed_last_hash = viewport_hash;
                    if !pipeline.active_blocks().is_empty() {
                        pipeline.clear_blocks();
                        crate::debug_log!("PRETTIFIER", "CC viewport changed, cleared all blocks");
                    }
                }

                // Claude Code session: segment the viewport by action bullets
                // (⏺) and collapse markers. Each segment is submitted independently
                // so detection sees focused content blocks rather than the entire
                // viewport (which mixes UI chrome with markdown and causes false
                // positives). Deduplication in handle_block prevents duplicates.
                pipeline.reset_boundary();

                crate::debug_log!(
                    "PRETTIFIER",
                    "per-frame feed (CC): scanning {} visible lines, viewport_changed={}, scrollback={}, scroll_offset={}",
                    visible_lines,
                    viewport_changed,
                    scrollback_len,
                    scroll_offset
                );

                // Collect all rows with raw + reconstructed text.
                let mut rows: Vec<(String, String, usize)> = Vec::new(); // (raw, recon, abs_row)

                for row_idx in 0..visible_lines {
                    let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }

                    let row_text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();

                    let line = reconstruct_markdown_from_cells(&cells[start..end]);
                    rows.push((row_text, line, absolute_row));
                }

                // Split into segments at action bullets (⏺) and collapse markers.
                // Each segment is the content between two boundaries.
                let mut segments: Vec<Vec<(String, usize)>> = Vec::new();
                let mut current: Vec<(String, usize)> = Vec::new();

                for (raw, recon, abs_row) in &rows {
                    let trimmed = raw.trim();
                    // Collapse markers — boundary, include the line in the
                    // preceding segment so row alignment is preserved (skipping
                    // it would cause the overlay to render wrong content at this row).
                    if raw.contains("(ctrl+o to expand)") {
                        current.push((recon.clone(), *abs_row));
                        segments.push(std::mem::take(&mut current));
                        continue;
                    }
                    // Action bullets (⏺) start a new segment
                    if trimmed.starts_with('⏺') || trimmed.starts_with("● ") {
                        if !current.is_empty() {
                            segments.push(std::mem::take(&mut current));
                        }
                        // Include this line in the new segment
                        current.push((recon.clone(), *abs_row));
                        continue;
                    }
                    // Horizontal rules (─────) are boundaries
                    if trimmed.len() > 10 && trimmed.chars().all(|c| c == '─' || c == '━') {
                        if !current.is_empty() {
                            segments.push(std::mem::take(&mut current));
                        }
                        continue;
                    }
                    current.push((recon.clone(), *abs_row));
                }
                if !current.is_empty() {
                    segments.push(current);
                }

                crate::debug_log!(
                    "PRETTIFIER",
                    "CC segmentation: {} total rows -> {} segments",
                    rows.len(),
                    segments.len()
                );

                // Submit each segment that has enough content for detection.
                // Short segments (tool call one-liners) are skipped.
                // The pipeline's handle_block() deduplicates by content hash,
                // so resubmitting the same segment on successive frames is cheap.
                let min_segment_lines = 5;
                let mut submitted = 0usize;
                let mut skipped_short = 0usize;
                let mut skipped_empty = 0usize;
                for mut segment in segments {
                    let non_empty = segment.iter().filter(|(l, _)| !l.trim().is_empty()).count();
                    if non_empty < min_segment_lines {
                        skipped_short += 1;
                        continue;
                    }

                    let pre_len = segment.len();
                    preprocess_claude_code_segment(&mut segment);
                    if segment.is_empty() {
                        skipped_empty += 1;
                        continue;
                    }

                    crate::debug_log!(
                        "PRETTIFIER",
                        "CC segment: {} lines (was {} before preprocess), rows={}..{}, first={:?}",
                        segment.len(),
                        pre_len,
                        segment.first().map(|(_, r)| *r).unwrap_or(0),
                        segment.last().map(|(_, r)| *r + 1).unwrap_or(0),
                        segment
                            .first()
                            .map(|(l, _)| &l[..l.floor_char_boundary(60)])
                    );

                    submitted += 1;
                    pipeline.submit_command_output(
                        std::mem::take(&mut segment),
                        Some("claude".to_string()),
                    );
                }

                crate::debug_log!(
                    "PRETTIFIER",
                    "CC segmentation complete: submitted={}, skipped_short={}, skipped_empty={}",
                    submitted,
                    skipped_short,
                    skipped_empty
                );
            } else {
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

                    if content_hash == tab.cache.prettifier_feed_last_hash {
                        // Identical content — skip entirely.
                        crate::debug_trace!(
                            "PRETTIFIER",
                            "per-frame feed (non-CC): content unchanged, skipping"
                        );
                    } else {
                        let elapsed = tab.cache.prettifier_feed_last_time.elapsed();
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
                            tab.cache.prettifier_feed_last_hash = content_hash;
                            tab.cache.prettifier_feed_last_time = std::time::Instant::now();
                            pipeline.submit_command_output(lines, None);
                        }
                    }
                }
            }
        }

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
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
            }
        }

        // Update cache scrollback and clamp scroll state.
        //
        // In pane mode the focused pane's own terminal holds the scrollback, not
        // `tab.terminal`.  Clamping here with `tab.terminal.scrollback_len()` would
        // incorrectly cap (or zero-out) the scroll offset every frame.  The correct
        // clamp happens later in the pane render path once we know the focused pane's
        // actual scrollback length.
        let is_pane_mode = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count() > 0)
            .unwrap_or(false);
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.scrollback_len = scrollback_len;
            if !is_pane_mode {
                tab.scroll_state
                    .clamp_to_scrollback(tab.cache.scrollback_len);
            }
        }

        // Keep copy mode dimensions in sync with terminal
        if self.copy_mode.active
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, rows) = term.dimensions();
            self.copy_mode.update_dimensions(cols, rows, scrollback_len);
        }

        let need_marks =
            self.config.scrollbar_command_marks || self.config.command_separator_enabled;
        let mut scrollback_marks = if need_marks {
            if let Ok(term) = terminal.try_lock() {
                term.scrollback_marks()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Append trigger-generated marks
        if let Some(tab) = self.tab_manager.active_tab() {
            scrollback_marks.extend(tab.trigger_marks.iter().cloned());
        }

        // Keep scrollbar visible when mark indicators exist (even if no scrollback).
        if !scrollback_marks.is_empty() {
            show_scrollbar = true;
        }

        // Update window title if terminal has set one via OSC sequences
        // Only if allow_title_change is enabled and we're not showing a URL tooltip
        if self.config.allow_title_change
            && hovered_url.is_none()
            && terminal_title != cached_terminal_title
        {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.terminal_title = terminal_title.clone();
            }
            if let Some(window) = &self.window {
                if terminal_title.is_empty() {
                    // Restore configured title when terminal clears title
                    window.set_title(&self.format_title(&self.config.window_title));
                } else {
                    // Use terminal-set title with window number
                    window.set_title(&self.format_title(&terminal_title));
                }
            }
        }

        // Total lines = visible lines + actual scrollback content
        let total_lines = visible_lines + scrollback_len;

        // Detect URLs in visible area (only when content changed)
        // This optimization saves ~0.26ms per frame on cache hits
        let url_detect_start = std::time::Instant::now();
        let debug_url_detect_time = if !self.debug.cache_hit {
            // Content changed - re-detect URLs
            self.detect_urls();
            url_detect_start.elapsed()
        } else {
            // Content unchanged - use cached URL detection
            std::time::Duration::ZERO
        };

        // Apply URL underlining to cells (always apply, since cells might be regenerated)
        let url_underline_start = std::time::Instant::now();
        self.apply_url_underlines(&mut cells, &renderer_size);
        let _debug_url_underline_time = url_underline_start.elapsed();

        // Update search and apply search highlighting
        if self.overlay_ui.search_ui.visible {
            // Get all searchable lines from cells (ensures consistent wide character handling)
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_lock()
            {
                let lines_iter =
                    crate::app::search_highlight::get_all_searchable_lines(&term, visible_lines);
                self.overlay_ui.search_ui.update_search(lines_iter);
            }

            // Apply search highlighting to visible cells
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            // Use actual terminal grid columns from renderer
            self.apply_search_highlights(
                &mut cells,
                grid_cols,
                scroll_offset,
                scrollback_len,
                visible_lines,
            );
        }

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
