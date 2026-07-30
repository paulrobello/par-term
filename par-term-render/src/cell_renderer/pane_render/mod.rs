// Pane rendering: builds and submits one pane's GPU instance buffers.
//
// Module layout:
//   render_to_view.rs    — `CellRenderer::render_pane_to_view()`: per-pane background image
//                          preparation, render pass setup, and 3-phase draw call emission.
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
// This file keeps only the orchestrator, `build_pane_instance_buffers()`: buffer reset,
// viewport fill quad, per-row phase dispatch, and the final instance counts / GPU upload.
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

impl CellRenderer {
    /// Build instance buffers for a pane's cells with viewport offset.
    ///
    /// Similar to `build_instance_buffers` but adjusts all positions to be relative to the
    /// viewport origin. Also appends cursor overlay instances (beam bar and hollow borders)
    /// after the cell background instances.
    ///
    /// Returns the index in `bg_instances` where cursor overlays begin (`cursor_overlay_start`).
    /// The caller uses this for 3-phase rendering: cell bgs, text, then cursor overlays on top.
    ///
    /// `skip_solid_background`: if true, skip the solid background fill for the viewport
    /// (use when a custom shader or background image was already rendered full-screen).
    fn build_pane_instance_buffers(&mut self, p: PaneInstanceBuildParams<'_>) -> Result<usize> {
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
        // Clear whatever a previous build left behind.
        //
        // Every emitter below writes a slot before advancing the index, and only
        // `[0..bg_index]` / `[0..text_index]` is uploaded and drawn, so the reset
        // only has to cover slots this builder has populated before — not the
        // whole full-window array. That distinction matters with split panes:
        // this runs once per pane per frame, so clearing `max_bg_instances` +
        // `max_text_instances` entries made the reset alone cost O(window) × N
        // panes for buffers of which each pane uses roughly 1/N.
        let bg_reset = self.buffers.pane_bg_high_water.min(self.bg_instances.len());
        for instance in &mut self.bg_instances[..bg_reset] {
            instance.size = [0.0, 0.0];
            instance.color = [0.0, 0.0, 0.0, 0.0];
        }

        // Add a background rectangle covering the entire pane viewport (unless skipped)
        // This ensures the pane has a proper background even when cells are skipped.
        // Skip when a custom shader or background image was already rendered full-screen.
        let bg_start_index = if !skip_solid_background && !self.bg_instances.is_empty() {
            let bg_color = self.background_color;
            let opacity = self.window_opacity * viewport.opacity;
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            self.bg_instances[0] = super::types::BackgroundInstance {
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
            1 // Start cell backgrounds at index 1
        } else {
            0 // Start cell backgrounds at index 0 (no viewport fill)
        };

        let text_reset = self
            .buffers
            .pane_text_high_water
            .min(self.text_instances.len());
        for instance in &mut self.text_instances[..text_reset] {
            instance.size = [0.0, 0.0];
        }

        // Start at bg_start_index (1 if viewport fill was added, 0 otherwise)
        let mut bg_index = bg_start_index;
        let mut text_index = 0;

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

        // Update actual instance counts for draw calls
        self.buffers.actual_bg_instances = bg_index;
        self.buffers.actual_text_instances = text_index;
        self.buffers.pane_bg_high_water = self.buffers.pane_bg_high_water.max(bg_index);
        self.buffers.pane_text_high_water = self.buffers.pane_text_high_water.max(text_index);

        // The offscreen screenshot builder (`build_instance_buffers`) keeps a
        // per-row cache and skips rows it believes are still valid. Those rows
        // live in the same `bg_instances` / `text_instances` arrays this builder
        // has just overwritten with pane-relative geometry, so anything it skips
        // would draw this pane's leftovers. Force it to rebuild in full; it only
        // runs when a screenshot is taken, so the cost is not on the frame path.
        self.dirty_rows.fill(true);

        // Upload only the used portion of instance buffers to GPU.
        // Each pane typically uses a fraction of the full-window buffer, so uploading
        // only [0..count] instead of the entire array significantly reduces per-pane
        // staging bandwidth — critical when rendering many split panes per frame.
        if bg_index > 0 {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                0,
                bytemuck::cast_slice(&self.bg_instances[..bg_index]),
            );
        }
        if text_index > 0 {
            self.queue.write_buffer(
                &self.buffers.text_instance_buffer,
                0,
                bytemuck::cast_slice(&self.text_instances[..text_index]),
            );
        }

        Ok(cursor_overlay_start)
    }
}
