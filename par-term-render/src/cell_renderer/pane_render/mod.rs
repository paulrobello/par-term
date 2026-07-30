// Pane rendering: builds each pane's slice of the shared GPU instance buffers.
//
// Module layout:
//   render_to_view.rs    — `prepare_pane()` / `draw_prepared_panes()`: per-pane background
//                          image preparation, then one encoder for the whole frame with a
//                          render pass and 3-phase draw call emission per pane.
//   row_backgrounds.rs   — Phase 1. `emit_row_backgrounds()` emits RLE-merged cell background
//                          quads for one row, plus the cursor-cell background helper.
//   text_render.rs       — Phase 2. `emit_row_text()` emits one row's glyph quads and its
//                          underline rectangles; both go through the text pipeline.
//   cursor_overlays.rs   — Phase 3. `emit_cursor_overlays()` appends guide, shadow,
//                          beam/underline bar and hollow-outline instances on top of text.
//   separators.rs        — `emit_separator_instances()` injects command separator lines.
//   block_char_render.rs — Geometric rendering of block/box-drawing characters via the text
//                          pipeline. `render_block_char_geometrically()` returns Some(new_idx)
//                          when rendered; caller continues the per-cell loop.
//   powerline.rs         — Powerline fringe-extension logic (pure fn, no self access).
//                          `extend_powerline_fringes()` adjusts bg-quad x0/x1 to eliminate
//                          anti-aliased dark fringes at separator boundaries.
//
// This file keeps only the orchestrator, `build_pane_instance_buffers()`: the pane's
// viewport fill quad, per-row phase dispatch, and the final instance ranges / GPU upload —
// plus `begin_pane_batch()`, which sizes the shared buffers for a whole frame of panes and
// resets the suballocation cursors they append at (ARC-004).
//
// Note: The glyph font-fallback loop was extracted to `CellRenderer::resolve_glyph_with_fallback()`
// in `atlas.rs` (ARC-004 / QA-003). The RLE bg-instance merge inner loop remains inlined,
// now inside `emit_row_backgrounds()` in `row_backgrounds.rs`, as it mutates self.bg_instances
// in place with no clean free-function boundary.
//
// IMPORTANT invariants to preserve (see MEMORY.md and CLAUDE.md):
//   • 3-phase draw ordering: bg instances → text instances → cursor overlays
//   • `fill_default_bg_cells` controls default-bg skip in bg-image mode
//   • `skip_solid_background` must NOT be used to gate default-bg rendering
//
// Tracking: Issues ARC-005 and ARC-009 in AUDIT.md.

use super::{Cell, CellRenderer, PaneViewport};
use anyhow::Result;
use par_term_config::SeparatorMark;
mod block_char_render;
mod cursor_overlays;
mod powerline;
mod render_to_view;
mod row_backgrounds;
mod separators;
mod text_render;

use cursor_overlays::CursorOverlayParams;
use row_backgrounds::RowBackgroundParams;
use text_render::RowTextParams;

/// Atlas texture size in pixels. Must match the value used at atlas creation time.
/// See `PREFERRED_ATLAS_SIZE` in `pipeline.rs` and `atlas_size` on `CellRendererAtlas`.
pub(crate) const ATLAS_SIZE: f32 = 2048.0;

/// Parameters for rendering a single pane to a surface texture view.
pub struct PaneRenderViewParams<'a> {
    pub viewport: &'a PaneViewport,
    pub cells: &'a [Cell],
    pub cols: usize,
    pub rows: usize,
    pub cursor_pos: Option<(usize, usize)>,
    pub cursor_opacity: f32,
    pub show_scrollbar: bool,
    pub clear_first: bool,
    pub skip_background_image: bool,
    /// When true, emit background quads for default-bg cells (fills gaps in background-image mode).
    /// Set to false in custom shader mode so the shader output shows through.
    pub fill_default_bg_cells: bool,
    pub separator_marks: &'a [SeparatorMark],
    pub pane_background: Option<&'a par_term_config::PaneBackground>,
}

/// Parameters for building GPU instance buffers for a pane.
pub(super) struct PaneInstanceBuildParams<'a> {
    pub viewport: &'a PaneViewport,
    pub cells: &'a [Cell],
    pub cols: usize,
    pub rows: usize,
    pub cursor_pos: Option<(usize, usize)>,
    pub cursor_opacity: f32,
    pub skip_solid_background: bool,
    pub fill_default_bg_cells: bool,
    pub separator_marks: &'a [SeparatorMark],
}

/// Absolute instance ranges one pane occupies in the shared instance buffers.
pub(super) struct PaneInstanceRanges {
    /// Phase 1 — viewport fill quad, cell backgrounds and separators.
    pub bg: std::ops::Range<usize>,
    /// Phase 3 — cursor overlays, which sit directly after `bg`.
    pub cursor_overlays: std::ops::Range<usize>,
    /// Phase 2 — glyph and underline quads.
    pub text: std::ops::Range<usize>,
}

/// Instance capacity one pane needs, given its grid size.
///
/// Background: viewport fill + at most one RLE-merged quad per cell + one
/// separator line per row + the cursor overlay slots.
pub(crate) fn pane_instance_capacity(cols: usize, rows: usize) -> (usize, usize) {
    let cells = cols * rows;
    (
        1 + cells + rows + super::CURSOR_OVERLAY_SLOTS,
        cells * super::instance_buffers::PANE_TEXT_INSTANCES_PER_CELL,
    )
}

impl CellRenderer {
    /// Start a new pane batch: reset the suballocation cursors and make sure the
    /// instance buffers can hold every pane.
    ///
    /// `required_bg` / `required_text` are the summed [`pane_instance_capacity`] of
    /// the panes about to be built. The single-grid (offscreen) layout still has to
    /// fit as well, since it writes the same buffers.
    pub(crate) fn begin_pane_batch(&mut self, required_bg: usize, required_text: usize) {
        let (grid_bg, grid_text) = self.single_grid_instance_capacity();
        let need_bg = required_bg.max(grid_bg);
        let need_text = required_text.max(grid_text);
        if need_bg > self.buffers.max_bg_instances || need_text > self.buffers.max_text_instances {
            log::info!(
                "Growing instance buffers for {} bg / {} text instances (was {} / {})",
                need_bg,
                need_text,
                self.buffers.max_bg_instances,
                self.buffers.max_text_instances,
            );
            self.allocate_instance_buffers(
                need_bg.max(self.buffers.max_bg_instances),
                need_text.max(self.buffers.max_text_instances),
            );
        }
        self.buffers.pane_bg_cursor = 0;
        self.buffers.pane_text_cursor = 0;
    }

    /// Build instance buffers for a pane's cells with viewport offset.
    ///
    /// Similar to `build_instance_buffers` but adjusts all positions to be relative to the
    /// viewport origin. Also appends cursor overlay instances (beam bar and hollow borders)
    /// after the cell background instances.
    ///
    /// ARC-004: the pane's instances are appended at the batch cursors rather than
    /// at index 0, so every pane of the frame stays resident in the shared buffers
    /// and the frame can be drawn from one command encoder. The returned ranges are
    /// absolute indices into those buffers.
    ///
    /// No reset pass is needed: every emitter writes a slot before advancing the
    /// index, and only the returned ranges are uploaded and drawn, so no slot is
    /// ever drawn without having been written by this build.
    ///
    /// `skip_solid_background`: if true, skip the solid background fill for the viewport
    /// (use when a custom shader or background image was already rendered full-screen).
    fn build_pane_instance_buffers(
        &mut self,
        p: PaneInstanceBuildParams<'_>,
    ) -> Result<PaneInstanceRanges> {
        let PaneInstanceBuildParams {
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            skip_solid_background,
            fill_default_bg_cells,
            separator_marks,
        } = p;

        let bg_base = self.buffers.pane_bg_cursor;
        let text_base = self.buffers.pane_text_cursor;

        // Add a background rectangle covering the entire pane viewport (unless skipped)
        // This ensures the pane has a proper background even when cells are skipped.
        // Skip when a custom shader or background image was already rendered full-screen.
        let bg_start_index = if !skip_solid_background && bg_base < self.bg_instances.len() {
            let bg_color = self.background_color;
            let opacity = self.window_opacity * viewport.opacity;
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            self.bg_instances[bg_base] = super::types::BackgroundInstance {
                position: [
                    viewport.x / width_f * 2.0 - 1.0,
                    1.0 - (viewport.y / height_f * 2.0),
                ],
                size: [
                    viewport.width / width_f * 2.0,
                    viewport.height / height_f * 2.0,
                ],
                color: [
                    bg_color[0] * opacity,
                    bg_color[1] * opacity,
                    bg_color[2] * opacity,
                    opacity,
                ],
            };
            bg_base + 1 // Start cell backgrounds after the viewport fill
        } else {
            bg_base // Start cell backgrounds at the base (no viewport fill)
        };

        // Start after the viewport fill if one was added
        let mut bg_index = bg_start_index;
        let mut text_index = text_base;

        // Content offset - positions are relative to content area (with padding applied)
        let (content_x, content_y) = viewport.content_origin();
        let opacity_multiplier = viewport.opacity;

        for row in 0..rows {
            let row_start = row * cols;
            let row_end = (row + 1) * cols;
            if row_start >= cells.len() {
                break;
            }
            let row_cells = &cells[row_start..row_end.min(cells.len())];

            // Phase 1: cell background quads (RLE-merged) — see row_backgrounds.rs
            bg_index = self.emit_row_backgrounds(
                RowBackgroundParams {
                    row_cells,
                    row,
                    cursor_pos,
                    cursor_opacity,
                    content_x,
                    content_y,
                    opacity_multiplier,
                    fill_default_bg_cells,
                    skip_solid_background,
                },
                bg_index,
            );

            // Phase 2: glyphs and underlines — see text_render.rs
            text_index = self.emit_row_text(
                RowTextParams {
                    row_cells,
                    cells,
                    row_start,
                    cols,
                    row,
                    content_x,
                    content_y,
                    cursor_pos,
                    cursor_opacity,
                    opacity_multiplier,
                },
                text_index,
            );
        }

        // Inject command separator line instances — see separators.rs
        bg_index = self.emit_separator_instances(
            separator_marks,
            cols,
            rows,
            content_x,
            content_y,
            opacity_multiplier,
            bg_index,
        );

        // --- Cursor overlays (beam/underline bar + hollow borders) ---
        // These are rendered in Phase 3 (on top of text) via the 3-phase draw in render_pane_to_view.
        // Record where cursor overlays start — everything after this index is an overlay.
        let cursor_overlay_start = bg_index;

        if let Some((cursor_col, cursor_row)) = cursor_pos {
            let cursor_x0 = content_x + cursor_col as f32 * self.grid.cell_width;
            let cursor_x1 = cursor_x0 + self.grid.cell_width;
            let cursor_y0 = (content_y + cursor_row as f32 * self.grid.cell_height).round();
            let cursor_y1 = (content_y + (cursor_row + 1) as f32 * self.grid.cell_height).round();

            // Emit guide, shadow, beam/underline bar, hollow outline — see cursor_overlays.rs
            bg_index = self.emit_cursor_overlays(
                CursorOverlayParams {
                    cursor_x0,
                    cursor_x1,
                    cursor_y0,
                    cursor_y1,
                    cols,
                    content_x,
                    cursor_opacity,
                },
                bg_index,
            );
        }

        // Advance the batch cursors past this pane's region.
        self.buffers.pane_bg_cursor = bg_index;
        self.buffers.pane_text_cursor = text_index;

        // The emitters' bounds guards drop instances silently once a buffer is
        // full. Under suballocation that no longer just truncates the tail — a
        // full buffer starves every later pane — so say so loudly. Reaching the
        // cap exactly is reported too: it is indistinguishable from a drop here.
        //
        // Per-pane budgets assume zero RLE merging while the cursors advance by
        // actual usage, so a pane exceeding its own budget is harmless as long as
        // the others underuse theirs. This fires only when the totals do not fit,
        // which is the case that actually loses content. Logged, never asserted: a
        // link-heavy row can plausibly exhaust the text budget, and panicking a
        // debug build over a hover is worse than the missing glyphs it reports.
        if (bg_index >= self.buffers.max_bg_instances
            || text_index >= self.buffers.max_text_instances)
            && !self.buffers.overflow_reported
        {
            self.buffers.overflow_reported = true;
            log::error!(
                "Instance buffers exhausted while building a pane batch: bg {bg_index}/{} text {text_index}/{}. Later panes in this frame will be missing content.",
                self.buffers.max_bg_instances,
                self.buffers.max_text_instances,
            );
        }

        // The offscreen screenshot builder (`build_instance_buffers`) keeps a
        // per-row cache and skips rows it believes are still valid. Those rows
        // live in the same `bg_instances` / `text_instances` arrays this builder
        // has just overwritten with pane-relative geometry — pane 0 starts at
        // index 0, so the overlap is guaranteed — and anything it skips would
        // draw a pane's leftovers. Force it to rebuild in full; it only runs when
        // a screenshot is taken, so the cost is not on the frame path.
        self.dirty_rows.fill(true);

        // Upload only this pane's slice of the instance buffers. Panes occupy
        // disjoint ranges, so each write touches only the bytes it owns.
        if bg_index > bg_base {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                (bg_base * std::mem::size_of::<super::types::BackgroundInstance>()) as u64,
                bytemuck::cast_slice(&self.bg_instances[bg_base..bg_index]),
            );
        }
        if text_index > text_base {
            self.queue.write_buffer(
                &self.buffers.text_instance_buffer,
                (text_base * std::mem::size_of::<super::types::TextInstance>()) as u64,
                bytemuck::cast_slice(&self.text_instances[text_base..text_index]),
            );
        }

        Ok(PaneInstanceRanges {
            bg: bg_base..cursor_overlay_start,
            cursor_overlays: cursor_overlay_start..bg_index,
            text: text_base..text_index,
        })
    }
}
