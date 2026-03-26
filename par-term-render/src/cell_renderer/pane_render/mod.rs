// ARC-005 / ARC-009 TODO: This file is ~1000 lines (limit: 800). Remaining extraction
// candidates (require a more involved refactor since they mutate `self.bg_instances` in-place):
//
//   rle_merge.rs    — RLE background-color merge inner loop (currently inlined in
//                     build_pane_instance_buffers). Extract helper:
//                     `fn merge_rle_bg_spans(cells, ...) -> Vec<BackgroundInstance>`
//
//   powerline.rs    — Powerline fringe-extension logic (~80 lines). Extract helper:
//                     `fn extend_powerline_fringes(spans, cell_w, cell_h) -> Vec<BackgroundInstance>`
//
// Note: The glyph font-fallback loop previously duplicated here has been extracted to
// `CellRenderer::resolve_glyph_with_fallback()` in `atlas.rs` (ARC-004 / QA-003).
//
// IMPORTANT invariants to preserve (see MEMORY.md and CLAUDE.md):
//   • 3-phase draw ordering: bg instances → text instances → cursor overlays
//   • `fill_default_bg_cells` controls default-bg skip in bg-image mode
//   • `skip_solid_background` must NOT be used to gate default-bg rendering
//
// Tracking: Issues ARC-005 and ARC-009 in AUDIT.md.

use super::block_chars;
use super::instance_buffers::{
    compute_cursor_text_color, STIPPLE_OFF_PX, STIPPLE_ON_PX, UNDERLINE_HEIGHT_RATIO,
};
use super::{BackgroundInstance, Cell, CellRenderer, PaneViewport, TextInstance};
use anyhow::Result;
use par_term_config::{SeparatorMark, color_u8x4_rgb_to_f32, color_u8x4_rgb_to_f32_a};
mod cursor_overlays;
mod separators;

use cursor_overlays::CursorOverlayParams;

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
    /// Render a single pane's content within a viewport to an existing surface texture
    ///
    /// This method renders cells to a specific region of the render target,
    /// using a GPU scissor rect to clip to the pane bounds.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `viewport` - The pane's viewport (position, size, focus state, opacity)
    /// * `cells` - The cells to render (should match viewport grid size)
    /// * `cols` - Number of columns in the cell grid
    /// * `rows` - Number of rows in the cell grid
    /// * `cursor_pos` - Cursor position (col, row) within this pane, or None if no cursor
    /// * `cursor_opacity` - Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    /// * `show_scrollbar` - Whether to render the scrollbar for this pane
    /// * `clear_first` - If true, clears the viewport region before rendering
    /// * `skip_background_image` - If true, skip rendering the background image. Use this
    ///   when the background image has already been rendered full-screen (for split panes).
    pub fn render_pane_to_view(
        &mut self,
        surface_view: &wgpu::TextureView,
        p: PaneRenderViewParams<'_>,
    ) -> Result<()> {
        let PaneRenderViewParams {
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            show_scrollbar,
            clear_first,
            skip_background_image,
            fill_default_bg_cells,
            separator_marks,
            pane_background,
        } = p;
        // Build instance buffers for this pane's cells.
        // Returns cursor_overlay_start: the bg_instance index where cursor overlays begin.
        // Used for 3-phase rendering (bgs → text → cursor overlays).
        let cursor_overlay_start = self.build_pane_instance_buffers(PaneInstanceBuildParams {
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            skip_solid_background: skip_background_image,
            fill_default_bg_cells,
            separator_marks,
        })?;

        // Pre-update per-pane background uniform buffer and bind group if needed (must happen
        // before the render pass). Buffers are allocated once and reused across frames.
        // Per-pane backgrounds are explicit user overrides and always prepared, even when a
        // custom shader or global background would normally be skipped.
        let has_pane_bg = if let Some(pane_bg) = pane_background
            && let Some(ref path) = pane_bg.image_path
            && self.bg_state.pane_bg_cache.contains_key(path.as_str())
        {
            self.prepare_pane_bg_bind_group(
                path.as_str(),
                super::background::PaneBgBindGroupParams {
                    pane_x: viewport.x,
                    pane_y: viewport.y,
                    pane_width: viewport.width,
                    pane_height: viewport.height,
                    mode: pane_bg.mode,
                    opacity: pane_bg.opacity,
                    darken: pane_bg.darken,
                },
            );
            true
        } else {
            false
        };

        // Retrieve cached path for use in the render pass (must be done before borrow in pass).
        let pane_bg_path: Option<String> = if has_pane_bg {
            pane_background
                .and_then(|pb| pb.image_path.as_ref())
                .map(|p| p.to_string())
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pane render encoder"),
            });

        // Determine load operation and clear color
        let load_op = if clear_first {
            let clear_color = if self.bg_state.bg_is_solid_color {
                wgpu::Color {
                    r: self.bg_state.solid_bg_color[0] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    g: self.bg_state.solid_bg_color[1] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    b: self.bg_state.solid_bg_color[2] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    a: self.window_opacity as f64 * viewport.opacity as f64,
                }
            } else {
                wgpu::Color {
                    r: self.background_color[0] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    g: self.background_color[1] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    b: self.background_color[2] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    a: self.window_opacity as f64 * viewport.opacity as f64,
                }
            };
            wgpu::LoadOp::Clear(clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set scissor rect to clip rendering to pane bounds
            let (sx, sy, sw, sh) = viewport.to_scissor_rect();
            render_pass.set_scissor_rect(sx, sy, sw, sh);

            // Render per-pane background image within scissor rect.
            // Per-pane backgrounds are explicit user overrides and always render,
            // even when a custom shader or global background is active.
            if let Some(ref path) = pane_bg_path
                && let Some(cached) = self.bg_state.pane_bg_uniform_cache.get(path.as_str())
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, &cached.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            self.emit_three_phase_draw_calls(
                &mut render_pass,
                cursor_overlay_start as u32,
                self.buffers.actual_bg_instances as u32,
            );

            // Render scrollbar if requested (uses its own scissor rect internally)
            if show_scrollbar {
                // Reset scissor to full surface for scrollbar
                render_pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
                self.scrollbar.render(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

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
        // Clear previous instance buffers
        for instance in &mut self.bg_instances {
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

        for instance in &mut self.text_instances {
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
                let bg_alpha =
                    if self.transparency_affects_only_default_background && !is_default_bg {
                        1.0
                    } else {
                        self.window_opacity
                    };
                let pane_alpha = bg_alpha * opacity_multiplier;
                let mut bg_color = color_u8x4_rgb_to_f32_a(cell.bg_color, pane_alpha);

                // TODO(QA-002/QA-008): extract into `render_cursor_cell()` helper.
                // Signature would be:
                //   fn render_cursor_cell(&mut self, col: usize, row: usize,
                //       content_x: f32, content_y: f32, bg_color: [f32; 4],
                //       cursor_opacity: f32, render_hollow_here: bool,
                //       bg_index: &mut usize)
                // Handle cursor at this position
                if has_cursor {
                    use par_term_emu_core_rust::cursor::CursorStyle;
                    match self.cursor.style {
                        CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                            if !render_hollow_here {
                                // Solid block cursor: blend cursor color into background
                                for (bg, &cursor) in
                                    bg_color.iter_mut().take(3).zip(&self.cursor.color)
                                {
                                    *bg = *bg * (1.0 - cursor_opacity) + cursor * cursor_opacity;
                                }
                                bg_color[3] = bg_color[3].max(cursor_opacity * opacity_multiplier);
                            }
                            // If hollow: keep original background color (outline added as overlay)
                        }
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
                    let next_cursor_at_cell = cursor_pos
                        .is_some_and(|(cx, cy)| cx == col && cy == row)
                        && !self.cursor.hidden_for_shader;
                    let next_hollow = next_cursor_at_cell
                        && !self.is_focused
                        && self.cursor.unfocused_style
                            == par_term_config::UnfocusedCursorStyle::Hollow;
                    let next_has_cursor =
                        (next_cursor_at_cell && cursor_opacity > 0.0) || next_hollow;
                    let next_is_half_block = {
                        let mut chars = next_cell.grapheme.chars();
                        matches!(chars.next(), Some('\u{2580}' | '\u{2584}'))
                            && chars.next().is_none()
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
                let x1 =
                    (content_x + (start_col + run_length) as f32 * self.grid.cell_width).round();
                let y0 = (content_y + row as f32 * self.grid.cell_height).round();
                let y1 = (content_y + (row + 1) as f32 * self.grid.cell_height).round();

                // Extend the colored bg quad 1 px under adjacent powerline separator glyphs
                // to eliminate the dark fringe at their anti-aliased edges.
                //
                // Powerline separators with default bg rely on the viewport fill (no BG quad
                // in normal mode). Their anti-aliased corner/edge pixels blend:
                //   fg * alpha + dark_fill * (1 - alpha)  →  visible dark fringe
                // Extending the adjacent colored quad by 1 px underneath changes the blend to:
                //   fg * alpha + colored * (1 - alpha)  →  seamless transition
                // The 1 px is small enough to be hidden under the glyph itself.
                let is_default_bg_cell = |bg: [u8; 4]| -> bool {
                    let f = color_u8x4_rgb_to_f32(bg);
                    (f[0] - self.background_color[0]).abs() < 0.001
                        && (f[1] - self.background_color[1]).abs() < 0.001
                        && (f[2] - self.background_color[2]).abs() < 0.001
                };
                // Extend right if the next cell is any powerline separator with default bg.
                // Covers anti-aliased left edges and transparent left corners of left-pointing seps.
                let x1 = if col < row_cells.len()
                    && matches!(
                        row_cells[col].grapheme.as_str(),
                        "\u{E0B0}"
                            | "\u{E0B1}"
                            | "\u{E0B2}"
                            | "\u{E0B3}"
                            | "\u{E0B4}"
                            | "\u{E0B5}"
                            | "\u{E0B6}"
                            | "\u{E0B7}"
                    )
                    && is_default_bg_cell(row_cells[col].bg_color)
                {
                    x1 + 1.0
                } else {
                    x1
                };
                // Extend left if the previous cell is any powerline separator with default bg.
                // Covers anti-aliased right edges and transparent right corners of right-pointing seps.
                let x0 = if start_col > 0
                    && matches!(
                        row_cells[start_col - 1].grapheme.as_str(),
                        "\u{E0B0}"
                            | "\u{E0B1}"
                            | "\u{E0B2}"
                            | "\u{E0B3}"
                            | "\u{E0B4}"
                            | "\u{E0B5}"
                            | "\u{E0B6}"
                            | "\u{E0B7}"
                    )
                    && is_default_bg_cell(row_cells[start_col - 1].bg_color)
                {
                    x0 - 1.0
                } else {
                    x0
                };

                // In background-image mode (skip_solid_background=true), right-pointing
                // separator cells (E0B0/E0B1/E0B4/E0B5) are rendered in the RLE path and
                // their BG quad is drawn AFTER the adjacent colored run's quad. This causes
                // them to overwrite the 1px EXT-RIGHT extension from the colored run.
                //
                // Fix: when this cell IS a right-pointing separator with a colored left
                // neighbor, trim our own BG quad x0 by 1px so the colored extension stays
                // visible under the separator's left edge.
                let x0 = if skip_solid_background
                    && is_default_bg
                    && matches!(
                        row_cells[start_col].grapheme.as_str(),
                        "\u{E0B0}" | "\u{E0B1}" | "\u{E0B4}" | "\u{E0B5}"
                    )
                    && start_col > 0
                    && !is_default_bg_cell(row_cells[start_col - 1].bg_color)
                {
                    x0 + 1.0
                } else {
                    x0
                };

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

            // Text rendering
            let natural_line_height =
                self.font.font_ascent + self.font.font_descent + self.font.font_leading;
            let vertical_padding = (self.grid.cell_height - natural_line_height).max(0.0) / 2.0;
            let baseline_y = content_y
                + (row as f32 * self.grid.cell_height)
                + vertical_padding
                + self.font.font_ascent;

            // Compute text alpha - force opaque if keep_text_opaque is enabled
            let text_alpha = if self.keep_text_opaque {
                opacity_multiplier // Only apply pane dimming, not window transparency
            } else {
                self.window_opacity * opacity_multiplier
            };

            // Check if this row has the cursor and it's a visible block cursor
            // (for cursor text color override in split-pane rendering)
            let cursor_is_block_on_this_row = {
                use par_term_emu_core_rust::cursor::CursorStyle;
                cursor_pos.is_some_and(|(_, cy)| cy == row)
                    && cursor_opacity > 0.0
                    && !self.cursor.hidden_for_shader
                    && matches!(
                        self.cursor.style,
                        CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
                    )
                    && (self.is_focused
                        || self.cursor.unfocused_style
                            == par_term_config::UnfocusedCursorStyle::Same)
            };

            for (col_idx, cell) in row_cells.iter().enumerate() {
                if cell.wide_char_spacer || cell.grapheme == " " {
                    continue;
                }

                // Avoid Vec<char> allocation: use iterator-based char access.
                let Some(ch) = cell.grapheme.chars().next() else {
                    continue;
                };
                let second_char = cell.grapheme.chars().nth(1);
                // grapheme_len is 1, 2, or "more than 2" — we stop counting at 3.
                let grapheme_len = match second_char {
                    None => 1usize,
                    Some(_) => {
                        if cell.grapheme.chars().nth(2).is_none() { 2 } else { 3 }
                    }
                };

                // Determine text color - apply cursor_text_color (or auto-contrast) when the
                // block cursor is on this cell, otherwise use the cell's foreground color.
                let render_fg_color: [f32; 4] = if cursor_is_block_on_this_row
                    && cursor_pos.is_some_and(|(cx, _)| cx == col_idx)
                {
                    compute_cursor_text_color(
                        self.cursor.color,
                        self.cursor.text_color,
                        text_alpha,
                    )
                } else {
                    color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha)
                };

                // TODO(QA-002/QA-008): extract into `render_block_char()` helper.
                // The block char path returns early via `continue` — the helper would
                // return `bool` (true = rendered, caller should continue) and write
                // directly into `self.text_instances[text_index]`.
                // Check for block characters that should be rendered geometrically
                let char_type = block_chars::classify_char(ch);
                if grapheme_len == 1 && block_chars::should_render_geometrically(char_type) {
                    let char_w = if cell.wide_char {
                        self.grid.cell_width * 2.0
                    } else {
                        self.grid.cell_width
                    };
                    let x0 = (content_x + col_idx as f32 * self.grid.cell_width).round();
                    let y0 = (content_y + row as f32 * self.grid.cell_height).round();
                    let y1 = (content_y + (row + 1) as f32 * self.grid.cell_height).round();
                    let snapped_cell_height = y1 - y0;

                    // Try box drawing geometry first
                    let aspect_ratio = snapped_cell_height / char_w;
                    if let Some(box_geo) = block_chars::get_box_drawing_geometry(ch, aspect_ratio) {
                        for segment in &box_geo.segments {
                            let rect = segment
                                .to_pixel_rect(x0, y0, char_w, snapped_cell_height)
                                .snap_to_pixels();

                            // Extension for seamless lines
                            let extension = 1.0;
                            let ext_x = if segment.x <= 0.01 { extension } else { 0.0 };
                            let ext_y = if segment.y <= 0.01 { extension } else { 0.0 };
                            let ext_w = if segment.x + segment.width >= 0.99 {
                                extension
                            } else {
                                0.0
                            };
                            let ext_h = if segment.y + segment.height >= 0.99 {
                                extension
                            } else {
                                0.0
                            };

                            let final_x = rect.x - ext_x;
                            let final_y = rect.y - ext_y;
                            let final_w = rect.width + ext_x + ext_w;
                            let final_h = rect.height + ext_y + ext_h;

                            if text_index < self.buffers.max_text_instances {
                                self.text_instances[text_index] = TextInstance {
                                    position: [
                                        final_x / self.config.width as f32 * 2.0 - 1.0,
                                        1.0 - (final_y / self.config.height as f32 * 2.0),
                                    ],
                                    size: [
                                        final_w / self.config.width as f32 * 2.0,
                                        final_h / self.config.height as f32 * 2.0,
                                    ],
                                    tex_offset: [
                                        self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
                                        self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
                                    ],
                                    tex_size: [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE],
                                    color: render_fg_color,
                                    is_colored: 0,
                                };
                                text_index += 1;
                            }
                        }
                        continue;
                    }

                    // Half-block characters (▄/▀): render BOTH halves through the
                    // text pipeline to eliminate cross-pipeline coordinate seams.
                    // Use snapped cell edges (no extensions) for seamless tiling.
                    if ch == '\u{2584}' || ch == '\u{2580}' {
                        let x1 = (content_x + (col_idx + 1) as f32 * self.grid.cell_width).round();
                        let cell_w = x1 - x0;
                        let y_mid = y0 + self.grid.cell_height / 2.0;

                        let bg_half_color = color_u8x4_rgb_to_f32_a(cell.bg_color, text_alpha);
                        let (top_color, bottom_color) = if ch == '\u{2584}' {
                            (bg_half_color, render_fg_color) // ▄: top=bg, bottom=fg
                        } else {
                            (render_fg_color, bg_half_color) // ▀: top=fg, bottom=bg
                        };

                        let tex_offset = [
                            self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
                            self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
                        ];
                        let tex_size = [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE];

                        // Top half: [y0, y_mid)
                        if text_index < self.buffers.max_text_instances {
                            self.text_instances[text_index] = TextInstance {
                                position: [
                                    x0 / self.config.width as f32 * 2.0 - 1.0,
                                    1.0 - (y0 / self.config.height as f32 * 2.0),
                                ],
                                size: [
                                    cell_w / self.config.width as f32 * 2.0,
                                    (y_mid - y0) / self.config.height as f32 * 2.0,
                                ],
                                tex_offset,
                                tex_size,
                                color: top_color,
                                is_colored: 0,
                            };
                            text_index += 1;
                        }

                        // Bottom half: [y_mid, y1)
                        if text_index < self.buffers.max_text_instances {
                            self.text_instances[text_index] = TextInstance {
                                position: [
                                    x0 / self.config.width as f32 * 2.0 - 1.0,
                                    1.0 - (y_mid / self.config.height as f32 * 2.0),
                                ],
                                size: [
                                    cell_w / self.config.width as f32 * 2.0,
                                    (y1 - y_mid) / self.config.height as f32 * 2.0,
                                ],
                                tex_offset,
                                tex_size,
                                color: bottom_color,
                                is_colored: 0,
                            };
                            text_index += 1;
                        }
                        continue;
                    }

                    // Try block element geometry
                    if let Some(geo_block) = block_chars::get_geometric_block(ch) {
                        let rect = geo_block.to_pixel_rect(x0, y0, char_w, self.grid.cell_height);

                        // Add small extension to prevent gaps (1 pixel overlap).
                        let extension = 1.0;
                        let ext_x = if geo_block.x == 0.0 { extension } else { 0.0 };
                        let ext_y = if geo_block.y == 0.0 { extension } else { 0.0 };
                        let ext_w = if geo_block.x + geo_block.width >= 1.0 {
                            extension
                        } else {
                            0.0
                        };
                        let ext_h = if geo_block.y + geo_block.height >= 1.0 {
                            extension
                        } else {
                            0.0
                        };

                        let final_x = rect.x - ext_x;
                        let final_y = rect.y - ext_y;
                        let final_w = rect.width + ext_x + ext_w;
                        let final_h = rect.height + ext_y + ext_h;

                        if text_index < self.buffers.max_text_instances {
                            self.text_instances[text_index] = TextInstance {
                                position: [
                                    final_x / self.config.width as f32 * 2.0 - 1.0,
                                    1.0 - (final_y / self.config.height as f32 * 2.0),
                                ],
                                size: [
                                    final_w / self.config.width as f32 * 2.0,
                                    final_h / self.config.height as f32 * 2.0,
                                ],
                                tex_offset: [
                                    self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
                                    self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
                                ],
                                tex_size: [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE],
                                color: render_fg_color,
                                is_colored: 0,
                            };
                            text_index += 1;
                        }
                        continue;
                    }
                }

                // Check if this character should be rendered as a monochrome symbol.
                // Also handle symbol + VS16 (U+FE0F): strip VS16, render monochrome.
                let (force_monochrome, base_char) = if grapheme_len == 1 {
                    (super::atlas::should_render_as_symbol(ch), ch)
                } else if grapheme_len == 2
                    && second_char == Some('\u{FE0F}')
                    && super::atlas::should_render_as_symbol(ch)
                {
                    // Symbol + VS16: strip VS16 and render base char as monochrome
                    (true, ch)
                } else {
                    (false, ch)
                };

                // Resolve a renderable glyph via the shared font-fallback helper (ARC-004 / QA-003).
                // This replaces the duplicated excluded_fonts/get_or_rasterize_glyph loop
                // that previously existed in both pane_render/mod.rs and text_instance_builder.rs.
                let resolved_info = self.resolve_glyph_with_fallback(
                    base_char,
                    &cell.grapheme,
                    cell.bold,
                    cell.italic,
                    force_monochrome,
                );

                if let Some(info) = resolved_info {
                    let char_w = if cell.wide_char {
                        self.grid.cell_width * 2.0
                    } else {
                        self.grid.cell_width
                    };
                    let x0 = content_x + col_idx as f32 * self.grid.cell_width;
                    let y0 = content_y + row as f32 * self.grid.cell_height;
                    let x1 = x0 + char_w;
                    let y1 = y0 + self.grid.cell_height;

                    let cell_w = x1 - x0;
                    let cell_h = y1 - y0;
                    let scale_x = cell_w / char_w;
                    let scale_y = cell_h / self.grid.cell_height;

                    let baseline_offset =
                        baseline_y - (content_y + row as f32 * self.grid.cell_height);
                    let glyph_left = x0 + (info.bearing_x * scale_x).round();
                    let baseline_in_cell = (baseline_offset * scale_y).round();
                    let glyph_top = y0 + baseline_in_cell - info.bearing_y;

                    let render_w = info.width as f32 * scale_x;
                    let render_h = info.height as f32 * scale_y;

                    let (final_left, final_top, final_w, final_h) =
                        if grapheme_len == 1 && block_chars::should_snap_to_boundaries(char_type) {
                            block_chars::snap_glyph_to_cell(block_chars::SnapGlyphParams {
                                glyph_left,
                                glyph_top,
                                render_w,
                                render_h,
                                cell_x0: x0,
                                cell_y0: y0,
                                cell_x1: x1,
                                cell_y1: y1,
                                snap_threshold: 3.0,
                                extension: 0.5,
                            })
                        } else {
                            (glyph_left, glyph_top, render_w, render_h)
                        };

                    if text_index < self.buffers.max_text_instances {
                        self.text_instances[text_index] = TextInstance {
                            position: [
                                final_left / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - (final_top / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                final_w / self.config.width as f32 * 2.0,
                                final_h / self.config.height as f32 * 2.0,
                            ],
                            tex_offset: [info.x as f32 / ATLAS_SIZE, info.y as f32 / ATLAS_SIZE],
                            tex_size: [
                                info.width as f32 / ATLAS_SIZE,
                                info.height as f32 / ATLAS_SIZE,
                            ],
                            color: render_fg_color,
                            is_colored: if info.is_colored { 1 } else { 0 },
                        };
                        text_index += 1;
                    }
                }
            }

            // Underlines: emit a thin rectangle at the bottom of each underlined cell.
            // Mirrors the logic in text_instance_builder.rs but uses pane-local coordinates.
            {
                let underline_thickness = (self.grid.cell_height * UNDERLINE_HEIGHT_RATIO)
                    .max(1.0)
                    .round();
                let tex_offset = [
                    self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
                    self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
                ];
                let tex_size = [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE];
                let y0 = content_y + (row + 1) as f32 * self.grid.cell_height - underline_thickness;
                let ndc_y = 1.0 - (y0 / self.config.height as f32 * 2.0);
                let ndc_h = underline_thickness / self.config.height as f32 * 2.0;
                let is_stipple =
                    self.link_underline_style == par_term_config::LinkUnderlineStyle::Stipple;
                let stipple_period = STIPPLE_ON_PX + STIPPLE_OFF_PX;

                for col_idx in 0..cols {
                    if row_start + col_idx >= cells.len() {
                        break;
                    }
                    let cell = &cells[row_start + col_idx];
                    if !cell.underline {
                        continue;
                    }
                    let fg = color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha);
                    let cell_x0 = content_x + col_idx as f32 * self.grid.cell_width;

                    if is_stipple {
                        let mut px = 0.0;
                        while px < self.grid.cell_width
                            && text_index < self.buffers.max_text_instances
                        {
                            let seg_w = STIPPLE_ON_PX.min(self.grid.cell_width - px);
                            let x = cell_x0 + px;
                            self.text_instances[text_index] = TextInstance {
                                position: [x / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                                size: [seg_w / self.config.width as f32 * 2.0, ndc_h],
                                tex_offset,
                                tex_size,
                                color: fg,
                                is_colored: 0,
                            };
                            text_index += 1;
                            px += stipple_period;
                        }
                    } else if text_index < self.buffers.max_text_instances {
                        self.text_instances[text_index] = TextInstance {
                            position: [cell_x0 / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                            size: [self.grid.cell_width / self.config.width as f32 * 2.0, ndc_h],
                            tex_offset,
                            tex_size,
                            color: fg,
                            is_colored: 0,
                        };
                        text_index += 1;
                    }
                }
            }
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

        // Upload instance buffers to GPU
        self.queue.write_buffer(
            &self.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&self.bg_instances),
        );
        self.queue.write_buffer(
            &self.buffers.text_instance_buffer,
            0,
            bytemuck::cast_slice(&self.text_instances),
        );

        Ok(cursor_overlay_start)
    }
}
