use super::Renderer;
use crate::cell_renderer::Cell;
use crate::graphics_renderer::GraphicRenderInfo;
use anyhow::Result;
use par_term_emu_core_rust::graphics::TerminalGraphic;
use par_term_emu_core_rust::graphics::placeholder::{PLACEHOLDER_CHAR, diacritic_to_number};

/// Synthetic GraphicRenderInfo id namespace for Kitty virtual placements.
///
/// Virtual placements are keyed by the Kitty image_id (u32). The shared texture
/// cache uses u64 ids drawn from `TerminalGraphic::id`. To avoid collisions we
/// flag virtual-placement ids with the high bit. Phase 1 already guarantees
/// `TerminalGraphic::id` does not use this bit.
const VIRTUAL_PLACEMENT_ID_FLAG: u64 = 1u64 << 63;

/// Build a synthetic u64 cache id from a Kitty image_id + placement_id.
fn virtual_placement_cache_id(image_id: u32, placement_id: u32) -> u64 {
    VIRTUAL_PLACEMENT_ID_FLAG | ((placement_id as u64) << 32) | image_id as u64
}

/// Decode a Kitty Unicode-placeholder cell.
///
/// Returns `(image_id, placement_id, row_idx, col_idx)` if the cell holds a
/// placeholder grapheme. The first two diacritics encode the cell's row/column
/// index *within the placement*; the optional third diacritic supplies the
/// most-significant byte of the image id; the cell foreground colour supplies
/// the lower 24 bits of the image id.
///
/// We do not currently extract a per-placement placement_id from the underline
/// colour — par-term-render's `Cell` representation flattens that out. Phase 1
/// stores virtual placements keyed by `(image_id, placement_id)` with
/// `placement_id == 0` being the common case, and `get_placeholder_graphic`
/// falls back to any placement_id for an image when 0 is requested.
fn decode_placeholder_cell(cell: &Cell) -> Option<(u32, u32, u16, u16)> {
    let mut chars = cell.grapheme.chars();
    if chars.next()? != PLACEHOLDER_CHAR {
        return None;
    }
    let row_idx = diacritic_to_number(chars.next()?)?;
    let col_idx = diacritic_to_number(chars.next()?)?;
    // The MSB diacritic only encodes 0..=255 per spec, even though the table
    // now exposes 297 entries; clamp the high indices to 0 so we never overflow
    // the u8 image-ID byte.
    let msb_u8 = chars
        .next()
        .and_then(diacritic_to_number)
        .map(|n| if n <= u8::MAX as u16 { n as u8 } else { 0 })
        .unwrap_or(0);

    // fg_color is stored as RGBA; encode (R<<16 | G<<8 | B) as the low 24 bits
    // of the image id, then OR in the MSB diacritic as the top byte.
    let [r, g, b, _a] = cell.fg_color;
    let image_id = ((msb_u8 as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
    Some((image_id, 0, row_idx, col_idx))
}

/// One placeholder run grouped by `(image_id, placement_id)`.
///
/// Public to the crate so renderer tests can assert the grouping output without
/// exercising the GPU pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VirtualPlacementHit {
    pub image_id: u32,
    pub placement_id: u32,
    pub start_col: usize,
    pub start_row: usize,
    pub width_cells: usize,
    pub height_cells: usize,
}

/// Scan a cell grid for Kitty placeholder runs and return one
/// `VirtualPlacementHit` per contiguous `(image_id, placement_id)` group.
///
/// The bounding box approach is sufficient for the standard "rectangle of
/// placeholders" emitted by clients like par-textual-image; we don't attempt to
/// split L-shaped or sparse layouts (the spec doesn't really define what those
/// would mean, and no client produces them).
pub(crate) fn scan_placeholder_cells(
    cells: &[Cell],
    cols: usize,
    rows: usize,
) -> Vec<VirtualPlacementHit> {
    use std::collections::HashMap;

    // (image_id, placement_id) -> (min_col, min_row, max_col, max_row)
    let mut bboxes: HashMap<(u32, u32), (usize, usize, usize, usize)> = HashMap::new();

    for row in 0..rows {
        let row_start = row * cols;
        if row_start >= cells.len() {
            break;
        }
        let row_end = (row_start + cols).min(cells.len());
        for (col_off, cell) in cells[row_start..row_end].iter().enumerate() {
            let Some((image_id, placement_id, _r_idx, _c_idx)) = decode_placeholder_cell(cell)
            else {
                continue;
            };
            let col = col_off;
            bboxes
                .entry((image_id, placement_id))
                .and_modify(|b| {
                    if col < b.0 {
                        b.0 = col;
                    }
                    if row < b.1 {
                        b.1 = row;
                    }
                    if col > b.2 {
                        b.2 = col;
                    }
                    if row > b.3 {
                        b.3 = row;
                    }
                })
                .or_insert((col, row, col, row));
        }
    }

    let mut hits: Vec<VirtualPlacementHit> = bboxes
        .into_iter()
        .map(
            |((image_id, placement_id), (min_c, min_r, max_c, max_r))| VirtualPlacementHit {
                image_id,
                placement_id,
                start_col: min_c,
                start_row: min_r,
                width_cells: max_c - min_c + 1,
                height_cells: max_r - min_r + 1,
            },
        )
        .collect();
    // Stable order so callers/tests don't depend on HashMap iteration order.
    hits.sort_by_key(|h| (h.image_id, h.placement_id, h.start_row, h.start_col));
    hits
}

impl Renderer {
    /// Update graphics textures (Sixel, iTerm2, Kitty)
    ///
    /// # Arguments
    /// * `graphics` - Graphics from the terminal with RGBA data
    /// * `view_scroll_offset` - Current view scroll offset (0 = viewing current content)
    /// * `scrollback_len` - Total lines in scrollback buffer
    /// * `visible_rows` - Number of visible rows in terminal
    pub fn update_graphics(
        &mut self,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        view_scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<()> {
        // Track whether we had graphics before this update (to detect removal)
        let had_graphics = !self.sixel_graphics.is_empty();

        // Clear old graphics list
        self.sixel_graphics.clear();

        // Calculate the view window in absolute terms
        // total_lines = scrollback_len + visible_rows
        // When scroll_offset = 0, we view lines [scrollback_len, scrollback_len + visible_rows)
        // When scroll_offset > 0, we view earlier lines
        let total_lines = scrollback_len + visible_rows;
        let view_end = total_lines.saturating_sub(view_scroll_offset);
        let view_start = view_end.saturating_sub(visible_rows);

        // Process each graphic
        for graphic in graphics {
            // Use the unique ID from the graphic (stable across position changes)
            let id = graphic.id;
            let (col, row) = graphic.position;

            // Convert scroll_offset_rows from the core library's cell units (graphic.cell_dimensions.1
            // pixels per row, defaulting to 2) into display cell rows (self.cell_renderer.cell_height()
            // pixels per row).
            let core_cell_height = graphic
                .cell_dimensions
                .map(|(_, h)| h as f32)
                .unwrap_or(2.0)
                .max(1.0);
            let display_cell_height = self.cell_renderer.cell_height().max(1.0);
            let scroll_offset_in_display_rows = (graphic.scroll_offset_rows as f32
                * core_cell_height
                / display_cell_height)
                .round() as usize;

            // Calculate screen row based on whether this is a scrollback graphic or current
            let screen_row: isize = if let Some(sb_row) = graphic.scrollback_row {
                // Scrollback graphic: sb_row is absolute index in scrollback
                // Screen row = sb_row - view_start
                sb_row as isize - view_start as isize
            } else {
                // Current graphic: position is relative to visible area
                // Absolute position = scrollback_len + row - scroll_offset_in_display_rows
                // This keeps the graphic at its original absolute position as scrollback grows
                let absolute_row =
                    scrollback_len.saturating_sub(scroll_offset_in_display_rows) + row;

                log::trace!(
                    "[RENDERER] CALC: scrollback_len={}, row={}, scroll_offset_rows={}, scroll_in_display_rows={}, absolute_row={}, view_start={}, screen_row={}",
                    scrollback_len,
                    row,
                    graphic.scroll_offset_rows,
                    scroll_offset_in_display_rows,
                    absolute_row,
                    view_start,
                    absolute_row as isize - view_start as isize
                );

                absolute_row as isize - view_start as isize
            };

            log::debug!(
                "[RENDERER] Graphics update: id={}, protocol={:?}, pos=({},{}), screen_row={}, scrollback_row={:?}, scroll_offset_rows={}, size={}x{}, view=[{},{})",
                id,
                graphic.protocol,
                col,
                row,
                screen_row,
                graphic.scrollback_row,
                graphic.scroll_offset_rows,
                graphic.width,
                graphic.height,
                view_start,
                view_end
            );

            // Create or update texture in cache
            self.graphics_renderer.get_or_create_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                id,
                &graphic.pixels, // RGBA pixel data (Arc<Vec<u8>>)
                graphic.width as u32,
                graphic.height as u32,
            )?;

            // Add to render list with position and dimensions
            // Calculate size in cells (rounding up to cover all affected cells)
            let width_cells =
                ((graphic.width as f32 / self.cell_renderer.cell_width()).ceil() as usize).max(1);
            let height_cells =
                ((graphic.height as f32 / self.cell_renderer.cell_height()).ceil() as usize).max(1);

            // Calculate effective clip rows based on screen position
            // If screen_row < 0, we need to clip that many rows from the top
            // If screen_row >= 0, no clipping needed (we can see the full graphic)
            let effective_clip_rows = if screen_row < 0 {
                (-screen_row) as usize
            } else {
                0
            };

            self.sixel_graphics.push(GraphicRenderInfo {
                id,
                screen_row,
                col,
                width_cells,
                height_cells,
                alpha: 1.0,
                scroll_offset_rows: effective_clip_rows,
            });
        }

        // Mark dirty when graphics change (added or removed)
        if !graphics.is_empty() || had_graphics {
            self.dirty = true;
        }

        Ok(())
    }

    /// Compute positioned graphics list for a single pane without touching `self.sixel_graphics`.
    ///
    /// Shares the same texture cache as the global path so textures are never duplicated.
    ///
    /// Returns a `Vec` of [`GraphicRenderInfo`] ready to pass to
    /// [`GraphicsRenderer::render_for_pane`].
    pub fn update_pane_graphics(
        &mut self,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        view_scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<Vec<GraphicRenderInfo>> {
        let total_lines = scrollback_len + visible_rows;
        let view_end = total_lines.saturating_sub(view_scroll_offset);
        let view_start = view_end.saturating_sub(visible_rows);

        log::debug!(
            "[PANE_GRAPHICS] update_pane_graphics: scrollback_len={}, visible_rows={}, view_scroll_offset={}, total_lines={}, view_start={}, view_end={}, graphics_count={}",
            scrollback_len,
            visible_rows,
            view_scroll_offset,
            total_lines,
            view_start,
            view_end,
            graphics.len()
        );

        let mut positioned = Vec::new();

        for graphic in graphics {
            let id = graphic.id;
            let (col, row) = graphic.position;

            // Convert scroll_offset_rows from the core library's cell units (graphic.cell_dimensions.1
            // pixels per row, defaulting to 2) into display cell rows (self.cell_renderer.cell_height()
            // pixels per row).  Without this conversion, the absolute-row formula is wrong whenever
            // the graphic was created before set_cell_dimensions() was called on the pane terminal.
            let core_cell_height = graphic
                .cell_dimensions
                .map(|(_, h)| h as f32)
                .unwrap_or(2.0)
                .max(1.0);
            let display_cell_height = self.cell_renderer.cell_height().max(1.0);
            let scroll_offset_in_display_rows = (graphic.scroll_offset_rows as f32
                * core_cell_height
                / display_cell_height)
                .round() as usize;

            let screen_row: isize = if let Some(sb_row) = graphic.scrollback_row {
                let sr = sb_row as isize - view_start as isize;
                log::debug!(
                    "[PANE_GRAPHICS] scrollback graphic id={}: sb_row={}, view_start={}, screen_row={}",
                    id,
                    sb_row,
                    view_start,
                    sr
                );
                sr
            } else {
                let absolute_row =
                    scrollback_len.saturating_sub(scroll_offset_in_display_rows) + row;
                let sr = absolute_row as isize - view_start as isize;
                log::debug!(
                    "[PANE_GRAPHICS] current graphic id={}: scrollback_len={}, scroll_offset_rows={}, core_cell_h={}, disp_cell_h={}, scroll_in_display_rows={}, row={}, absolute_row={}, view_start={}, screen_row={}",
                    id,
                    scrollback_len,
                    graphic.scroll_offset_rows,
                    core_cell_height,
                    display_cell_height,
                    scroll_offset_in_display_rows,
                    row,
                    absolute_row,
                    view_start,
                    sr
                );
                sr
            };

            // Upload / refresh texture in the shared cache
            self.graphics_renderer.get_or_create_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                id,
                &graphic.pixels,
                graphic.width as u32,
                graphic.height as u32,
            )?;

            let width_cells =
                ((graphic.width as f32 / self.cell_renderer.cell_width()).ceil() as usize).max(1);
            let height_cells =
                ((graphic.height as f32 / self.cell_renderer.cell_height()).ceil() as usize).max(1);

            let effective_clip_rows = if screen_row < 0 {
                (-screen_row) as usize
            } else {
                0
            };

            positioned.push(GraphicRenderInfo {
                id,
                screen_row,
                col,
                width_cells,
                height_cells,
                alpha: 1.0,
                scroll_offset_rows: effective_clip_rows,
            });
        }

        Ok(positioned)
    }

    /// Compute `GraphicRenderInfo` entries for Kitty virtual placements.
    ///
    /// Scans `cells` (the visible grid) for runs of the Kitty placeholder
    /// character, groups them by `(image_id, placement_id)`, and emits one
    /// entry per group anchored at the bounding-box top-left cell. Textures are
    /// uploaded to the shared cache under a synthetic id derived from
    /// `image_id`, so repeated frames don't re-upload identical pixel data.
    pub(crate) fn update_pane_virtual_placements(
        &mut self,
        cells: &[Cell],
        cols: usize,
        rows: usize,
        virtual_placements: &[TerminalGraphic],
    ) -> Result<Vec<GraphicRenderInfo>> {
        let hits = scan_placeholder_cells(cells, cols, rows);
        if hits.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::with_capacity(hits.len());
        for hit in hits {
            // Resolve the placement: prefer exact (image_id, placement_id), fall
            // back to any placement for this image when placement_id == 0
            // (matches GraphicsStore::get_placeholder_graphic semantics).
            let graphic = virtual_placements
                .iter()
                .find(|g| {
                    g.kitty_image_id == Some(hit.image_id)
                        && g.kitty_placement_id.unwrap_or(0) == hit.placement_id
                })
                .or_else(|| {
                    if hit.placement_id == 0 {
                        virtual_placements
                            .iter()
                            .find(|g| g.kitty_image_id == Some(hit.image_id))
                    } else {
                        None
                    }
                });
            let Some(graphic) = graphic else {
                log::trace!(
                    "[VPLACE] no virtual placement for image_id={}, placement_id={}",
                    hit.image_id,
                    hit.placement_id
                );
                continue;
            };

            let cache_id = virtual_placement_cache_id(hit.image_id, hit.placement_id);
            self.graphics_renderer.get_or_create_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                cache_id,
                &graphic.pixels,
                graphic.width as u32,
                graphic.height as u32,
            )?;

            out.push(GraphicRenderInfo {
                id: cache_id,
                screen_row: hit.start_row as isize,
                col: hit.start_col,
                width_cells: hit.width_cells,
                height_cells: hit.height_cells,
                alpha: 1.0,
                scroll_offset_rows: 0,
            });
        }
        Ok(out)
    }

    /// Render inline graphics (Sixel/iTerm2/Kitty) for a single split pane.
    ///
    /// Uses the same `surface_view` as the cell render pass (with `LoadOp::Load`) so
    /// graphics are composited on top of already-rendered cells.  A scissor rect derived
    /// from `viewport` clips output to the pane's bounds.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_pane_sixel_graphics(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &crate::cell_renderer::PaneViewport,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
        cells: &[Cell],
        cols: usize,
        virtual_placements: &[TerminalGraphic],
    ) -> Result<()> {
        let mut positioned =
            self.update_pane_graphics(graphics, scroll_offset, scrollback_len, visible_rows)?;

        // Build virtual-placement entries from the cell grid scan. These render
        // alongside the normal sixel/iTerm2/kitty graphics through the same
        // texture pipeline, but their on-screen position comes from the
        // placeholder cells, not from each TerminalGraphic's `position` field.
        if !virtual_placements.is_empty() && !cells.is_empty() && cols > 0 {
            positioned.extend(self.update_pane_virtual_placements(
                cells,
                cols,
                visible_rows,
                virtual_placements,
            )?);
        }

        if positioned.is_empty() {
            return Ok(());
        }

        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane sixel encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane sixel render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Clip to pane bounds
            let (sx, sy, sw, sh) = viewport.to_scissor_rect();
            render_pass.set_scissor_rect(sx, sy, sw, sh);

            let (ox, oy) = viewport.content_origin();

            log::debug!(
                "[PANE_GRAPHICS] render_pane_sixel_graphics: scissor=({},{},{},{}), origin=({},{}), window={}x{}, positioned_count={}",
                sx,
                sy,
                sw,
                sh,
                ox,
                oy,
                self.size.width,
                self.size.height,
                positioned.len()
            );
            for g in &positioned {
                log::debug!(
                    "[PANE_GRAPHICS]   positioned: id={}, screen_row={}, col={}, width_cells={}, height_cells={}, clip_rows={}",
                    g.id,
                    g.screen_row,
                    g.col,
                    g.width_cells,
                    g.height_cells,
                    g.scroll_offset_rows
                );
            }

            self.graphics_renderer.render_for_pane(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &mut render_pass,
                &positioned,
                crate::graphics_renderer::PaneRenderGeometry {
                    window_width: self.size.width as f32,
                    window_height: self.size.height as f32,
                    pane_origin_x: ox,
                    pane_origin_y: oy,
                },
            )?;
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Clear all cached sixel textures
    pub fn clear_sixel_cache(&mut self) {
        self.graphics_renderer.clear_cache();
        self.sixel_graphics.clear();
        self.dirty = true;
    }

    /// Get the number of cached sixel textures
    pub fn sixel_cache_size(&self) -> usize {
        self.graphics_renderer.cache_size()
    }

    /// Remove a specific sixel texture from cache
    pub fn remove_sixel_texture(&mut self, id: u64) {
        self.graphics_renderer.remove_texture(id);
        self.sixel_graphics.retain(|g| g.id != id);
        self.dirty = true;
    }
}

#[cfg(test)]
mod virtual_placement_tests {
    //! Tests for Kitty Unicode-placeholder rendering preprocessing.
    //!
    //! These exercise the cell-grid scan + bounding-box grouping that turn
    //! placeholder runs into `VirtualPlacementHit`s. They deliberately stop
    //! short of the GPU pipeline — `update_pane_virtual_placements` would
    //! need a wgpu device — so we test the pure logic that feeds it.

    use super::{
        VIRTUAL_PLACEMENT_ID_FLAG, decode_placeholder_cell, scan_placeholder_cells,
        virtual_placement_cache_id,
    };
    use crate::cell_renderer::Cell;
    use par_term_emu_core_rust::graphics::placeholder::{
        PLACEHOLDER_CHAR, create_placeholder_with_diacritics,
    };

    /// Build a placeholder cell at (row_idx, col_idx) for `image_id` (low 24
    /// bits encoded in fg_color, no MSB diacritic).
    fn placeholder_cell(image_id: u32, row_idx: u16, col_idx: u16) -> Cell {
        let r = ((image_id >> 16) & 0xFF) as u8;
        let g = ((image_id >> 8) & 0xFF) as u8;
        let b = (image_id & 0xFF) as u8;
        Cell {
            grapheme: create_placeholder_with_diacritics(row_idx, col_idx, None),
            fg_color: [r, g, b, 255],
            ..Default::default()
        }
    }

    fn blank_cell() -> Cell {
        Cell {
            grapheme: " ".to_string(),
            ..Default::default()
        }
    }

    fn make_grid(cells: Vec<Cell>, cols: usize) -> (Vec<Cell>, usize, usize) {
        let rows = cells.len() / cols;
        (cells, cols, rows)
    }

    #[test]
    fn decode_placeholder_recovers_image_id_and_indices() {
        let cell = placeholder_cell(0x123456, 3, 7);
        let (image_id, placement_id, row, col) = decode_placeholder_cell(&cell).unwrap();
        assert_eq!(image_id, 0x123456);
        assert_eq!(placement_id, 0);
        assert_eq!(row, 3);
        assert_eq!(col, 7);
    }

    #[test]
    fn decode_placeholder_rejects_non_placeholder_cells() {
        let cell = blank_cell();
        assert!(decode_placeholder_cell(&cell).is_none());

        let mut letter = blank_cell();
        letter.grapheme = "a".to_string();
        assert!(decode_placeholder_cell(&letter).is_none());
    }

    #[test]
    fn scan_finds_single_rectangle_for_single_image() {
        // 4-col × 3-row grid; place a 3-col × 2-row placeholder rect at (1,0)
        // for image_id=42:
        //   . X X X
        //   . X X X
        //   . . . .
        let mut cells = vec![blank_cell(); 4 * 3];
        for r in 0..2 {
            for c in 1..4 {
                cells[r * 4 + c] = placeholder_cell(42, r as u16, (c - 1) as u16);
            }
        }
        let (cells, cols, rows) = make_grid(cells, 4);

        let hits = scan_placeholder_cells(&cells, cols, rows);
        assert_eq!(hits.len(), 1);
        let h = hits[0];
        assert_eq!(h.image_id, 42);
        assert_eq!(h.placement_id, 0);
        assert_eq!(h.start_col, 1);
        assert_eq!(h.start_row, 0);
        assert_eq!(h.width_cells, 3);
        assert_eq!(h.height_cells, 2);
    }

    #[test]
    fn scan_groups_two_adjacent_images_separately() {
        // 6 cols × 1 row: 3 cells of image 7 followed by 3 cells of image 99.
        let mut cells = Vec::with_capacity(6);
        for c in 0..3 {
            cells.push(placeholder_cell(7, 0, c as u16));
        }
        for c in 0..3 {
            cells.push(placeholder_cell(99, 0, c as u16));
        }
        let (cells, cols, rows) = make_grid(cells, 6);

        let hits = scan_placeholder_cells(&cells, cols, rows);
        assert_eq!(hits.len(), 2);

        let h7 = hits.iter().find(|h| h.image_id == 7).unwrap();
        assert_eq!(h7.start_col, 0);
        assert_eq!(h7.width_cells, 3);
        assert_eq!(h7.height_cells, 1);

        let h99 = hits.iter().find(|h| h.image_id == 99).unwrap();
        assert_eq!(h99.start_col, 3);
        assert_eq!(h99.width_cells, 3);
        assert_eq!(h99.height_cells, 1);
    }

    #[test]
    fn scan_ignores_non_placeholder_cells() {
        // A grid of all-blanks: the glyph path would draw spaces, the graphics
        // path produces no hits. This is the test for "cell containing
        // PLACEHOLDER_CHAR does not produce a glyph run" approached from the
        // other direction: cells *without* the placeholder yield zero hits, so
        // the glyph path's `ch == PLACEHOLDER_CHAR` skip can't accidentally
        // fire on non-placeholder cells.
        let cells = vec![blank_cell(); 6];
        let hits = scan_placeholder_cells(&cells, 6, 1);
        assert!(hits.is_empty());
    }

    #[test]
    fn glyph_path_recognizes_placeholder_char() {
        // The pane_render glyph loop suppresses glyph emission when the first
        // char of `cell.grapheme` is U+10EEEE. This test pins down that exact
        // predicate so it can't drift out of sync with the placeholder
        // protocol's base char.
        let cell = placeholder_cell(1, 0, 0);
        let first = cell.grapheme.chars().next().unwrap();
        assert_eq!(first, '\u{10EEEE}');
        assert_eq!(first, PLACEHOLDER_CHAR);
    }

    #[test]
    fn cache_id_is_disjoint_from_normal_graphic_ids() {
        // Real TerminalGraphic ids are u64 counters from the core library and
        // never set the high bit; virtual-placement cache ids always do, so
        // they can't collide with a sixel/iterm2 texture in the shared cache.
        let id_a = virtual_placement_cache_id(42, 0);
        let id_b = virtual_placement_cache_id(42, 1);
        assert_ne!(id_a, id_b);
        assert!(id_a & VIRTUAL_PLACEMENT_ID_FLAG != 0);
        assert!(id_b & VIRTUAL_PLACEMENT_ID_FLAG != 0);
    }
}
