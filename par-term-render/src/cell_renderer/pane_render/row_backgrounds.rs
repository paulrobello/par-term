//! Phase 1 of pane rendering: per-row cell background instance generation.
//!
//! Provides [`CellRenderer::emit_row_backgrounds`], which walks one grid row and appends
//! background quads to `self.bg_instances` starting at `bg_index`, returning the updated
//! index. Consecutive cells sharing a background color are merged into a single quad by
//! the run-length encoding (RLE) loop; that loop stays inlined here because it mutates
//! `self.bg_instances` in place and has no clean free-function boundary.
//!
//! IMPORTANT: `fill_default_bg_cells` — not `skip_solid_background` — gates whether
//! default-background cells emit a quad.

use super::super::{BackgroundInstance, Cell, CellRenderer};
use super::powerline;
use par_term_config::{color_u8x4_rgb_to_f32, color_u8x4_rgb_to_f32_a};

/// Parameters for [`CellRenderer::emit_row_backgrounds`].
pub(super) struct RowBackgroundParams<'a> {
    /// The cells of this row (already sliced out of the pane's cell buffer).
    pub row_cells: &'a [Cell],
    /// Zero-based row index within the pane.
    pub row: usize,
    /// Cursor position (col, row) within this pane, or None if no cursor.
    pub cursor_pos: Option<(usize, usize)>,
    /// Cursor blink opacity (0.0 = hidden, 1.0 = fully visible).
    pub cursor_opacity: f32,
    /// Pixel X of the content area origin (left edge after padding).
    pub content_x: f32,
    /// Pixel Y of the content area origin (top edge after padding).
    pub content_y: f32,
    /// Pane dimming factor applied on top of window opacity.
    pub opacity_multiplier: f32,
    /// When true, emit background quads for default-bg cells (background-image mode).
    pub fill_default_bg_cells: bool,
    /// True when the full-viewport solid fill was skipped (shader / background-image mode).
    pub skip_solid_background: bool,
}

/// Parameters for emitting a cursor cell background instance (QA-006 extraction).
struct CursorCellBgParams {
    col: usize,
    row: usize,
    content_x: f32,
    content_y: f32,
    bg_color: [f32; 4],
    cursor_opacity: f32,
    render_hollow_here: bool,
    opacity_multiplier: f32,
    bg_index: usize,
}

impl CellRenderer {
    /// Append this row's cell background instances to `self.bg_instances`.
    ///
    /// Returns the updated `bg_index` after the row's quads have been appended.
    pub(super) fn emit_row_backgrounds(
        &mut self,
        p: RowBackgroundParams<'_>,
        mut bg_index: usize,
    ) -> usize {
        let RowBackgroundParams {
            row_cells,
            row,
            cursor_pos,
            cursor_opacity,
            content_x,
            content_y,
            opacity_multiplier,
            fill_default_bg_cells,
            skip_solid_background,
        } = p;

        // Background - use RLE to merge consecutive cells with same color
        let mut col = 0;
        while col < row_cells.len() {
            let cell = &row_cells[col];
            let bg_f = color_u8x4_rgb_to_f32(cell.bg_color);
            let is_default_bg = (bg_f[0] - self.background_color[0]).abs() < 0.001
                && (bg_f[1] - self.background_color[1]).abs() < 0.001
                && (bg_f[2] - self.background_color[2]).abs() < 0.001;

            // Check for cursor at this position (position check only, no opacity gate)
            let cursor_at_cell = cursor_pos.is_some_and(|(cx, cy)| cx == col && cy == row)
                && !self.cursor.hidden_for_shader;
            // Hollow cursor (unfocused + Hollow style) must show regardless of blink opacity
            let render_hollow_here = cursor_at_cell
                && !self.is_focused
                && self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Hollow;
            let has_cursor = (cursor_at_cell && cursor_opacity > 0.0) || render_hollow_here;

            // Skip cells with half-block characters (▄/▀).
            // These are rendered entirely through the text pipeline to avoid
            // cross-pipeline coordinate seams that cause visible banding.
            let is_half_block = {
                let mut chars = cell.grapheme.chars();
                matches!(chars.next(), Some('\u{2580}' | '\u{2584}')) && chars.next().is_none()
            };

            // Skip default-bg cells only when NOT in background-image/shader mode.
            // When skip_solid_background is true (background image or custom shader active),
            // no viewport fill is drawn, so default-bg cells between colored segments would
            // show the background image through — causing visible gaps/lines in the tmux
            // status bar. In that mode we render them with the theme background color instead.
            // Skip default-bg cells unless fill_default_bg_cells is set (background-image mode).
            // In normal mode: viewport fill quad covers them — no individual quad needed.
            // In shader mode: shader output must show through — do not paint over it.
            // In bg-image mode: fill_default_bg_cells=true — render with theme bg color to
            // close gaps that would otherwise show the background image unexpectedly.
            if is_half_block || (is_default_bg && !has_cursor && !fill_default_bg_cells) {
                col += 1;
                continue;
            }

            // Calculate background color with alpha and pane opacity
            let bg_alpha = if self.transparency_affects_only_default_background && !is_default_bg {
                1.0
            } else {
                self.window_opacity
            };
            let pane_alpha = bg_alpha * opacity_multiplier;
            let bg_color = color_u8x4_rgb_to_f32_a(cell.bg_color, pane_alpha);

            // Handle cursor at this position (QA-006: extracted to helper for readability)
            if has_cursor {
                let emitted = self.emit_cursor_cell_bg(CursorCellBgParams {
                    col,
                    row,
                    content_x,
                    content_y,
                    bg_color,
                    cursor_opacity,
                    render_hollow_here,
                    opacity_multiplier,
                    bg_index,
                });
                if emitted {
                    bg_index += 1;
                }
                col += 1;
                continue;
            }

            // RLE: Find run of consecutive cells with same background color
            let start_col = col;
            let run_color = cell.bg_color;
            col += 1;
            while col < row_cells.len() {
                let next_cell = &row_cells[col];
                let next_cursor_at_cell = cursor_pos.is_some_and(|(cx, cy)| cx == col && cy == row)
                    && !self.cursor.hidden_for_shader;
                let next_hollow = next_cursor_at_cell
                    && !self.is_focused
                    && self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Hollow;
                let next_has_cursor = (next_cursor_at_cell && cursor_opacity > 0.0) || next_hollow;
                let next_is_half_block = {
                    let mut chars = next_cell.grapheme.chars();
                    matches!(chars.next(), Some('\u{2580}' | '\u{2584}')) && chars.next().is_none()
                };
                if next_cell.bg_color != run_color || next_has_cursor || next_is_half_block {
                    break;
                }
                col += 1;
            }
            let run_length = col - start_col;

            // Create single quad spanning entire run.
            // Snap all edges to pixel boundaries to match the text pipeline and
            // eliminate sub-pixel gaps between adjacent differently-colored cell runs.
            let x0 = (content_x + start_col as f32 * self.grid.cell_width).round();
            let x1 = (content_x + (start_col + run_length) as f32 * self.grid.cell_width).round();
            let y0 = (content_y + row as f32 * self.grid.cell_height).round();
            let y1 = (content_y + (row + 1) as f32 * self.grid.cell_height).round();

            // Extend the colored bg quad 1 px under adjacent powerline separator glyphs
            // to eliminate the dark fringe at their anti-aliased edges.
            // See powerline.rs for full rationale.
            let (x0, x1) = powerline::extend_powerline_fringes(powerline::PowerlineFringeParams {
                row_cells,
                start_col,
                col,
                x0,
                x1,
                skip_solid_background,
                is_default_bg,
                background_color: self.background_color,
            });

            if bg_index < self.buffers.max_bg_instances {
                self.bg_instances[bg_index] = BackgroundInstance {
                    position: [
                        x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (y0 / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        (x1 - x0) / self.config.width as f32 * 2.0,
                        (y1 - y0) / self.config.height as f32 * 2.0,
                    ],
                    color: bg_color,
                };
                bg_index += 1;
            }
        }

        bg_index
    }

    /// Emit a background instance for a cell containing the cursor.
    ///
    /// QA-006: Extracted from the RLE background loop in `build_pane_instance_buffers`.
    /// Handles block-cursor color blending and pixel-snapped background quad placement.
    /// Returns `true` if an instance was emitted (caller increments bg_index).
    fn emit_cursor_cell_bg(&mut self, p: CursorCellBgParams) -> bool {
        let CursorCellBgParams {
            col,
            row,
            content_x,
            content_y,
            mut bg_color,
            cursor_opacity,
            render_hollow_here,
            opacity_multiplier,
            bg_index,
        } = p;

        use par_term_emu_core_rust::cursor::CursorStyle;
        match self.cursor.style {
            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock if !render_hollow_here => {
                // Solid block cursor: blend cursor color into background
                for (bg, &cursor) in bg_color.iter_mut().take(3).zip(&self.cursor.color) {
                    *bg = *bg * (1.0 - cursor_opacity) + cursor * cursor_opacity;
                }
                bg_color[3] = bg_color[3].max(cursor_opacity * opacity_multiplier);
            }
            // If hollow: keep original background color (outline added as overlay)
            _ => {}
        }

        // Cursor cell can't be merged
        // Snap to pixel boundaries to match text pipeline alignment
        let x0 = (content_x + col as f32 * self.grid.cell_width).round();
        let x1 = (content_x + (col + 1) as f32 * self.grid.cell_width).round();
        let y0 = (content_y + row as f32 * self.grid.cell_height).round();
        let y1 = (content_y + (row + 1) as f32 * self.grid.cell_height).round();

        if bg_index < self.buffers.max_bg_instances {
            self.bg_instances[bg_index] = BackgroundInstance {
                position: [
                    x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    (x1 - x0) / self.config.width as f32 * 2.0,
                    (y1 - y0) / self.config.height as f32 * 2.0,
                ],
                color: bg_color,
            };
            true
        } else {
            false
        }
    }
}
