use super::{BackgroundInstance, CellRenderer, TextInstance};
use anyhow::Result;

/// Number of extra background instance slots reserved for cursor overlays
/// (beam/underline, guide line, shadow, boost glow, hollow outline sides).
/// Layout: [0] cursor overlay, [1] guide, [2] shadow, [3] boost glow, [4-7] hollow outline.
///
/// Re-exported publicly by `super::render`, which owns the buffer layout this
/// count belongs to; this module is private.
pub const CURSOR_OVERLAY_SLOTS: usize = 10;

/// Width of gutter indicator bars in terminal cell columns.
/// Each gutter indicator occupies this many cell-widths on the left side.
pub(crate) const GUTTER_WIDTH_CELLS: f32 = 2.0;

/// Underline thickness as a fraction of cell height.
/// Scaled at render time so underlines remain proportional across font sizes.
pub(crate) const UNDERLINE_HEIGHT_RATIO: f32 = 0.07;

/// Pixel tolerance for snapping glyphs to cell boundaries during rendering.
/// Glyphs within this many pixels of a cell edge are snapped to it.
pub(crate) const GLYPH_SNAP_THRESHOLD_PX: f32 = 3.0;

/// Sub-pixel extension applied when snapping glyphs to cell boundaries.
/// Prevents hairline gaps between adjacent block-drawing characters.
pub(crate) const GLYPH_SNAP_EXTENSION_PX: f32 = 0.5;

/// Floating-point epsilon for color component comparisons.
/// Used to detect when a cell's background matches the default terminal background.
pub(crate) const COLOR_COMPONENT_EPSILON: f32 = 0.001;

/// Brightness threshold for automatic cursor text-contrast selection.
/// Cursors brighter than this use dark text; darker cursors use light text.
pub(crate) const CURSOR_BRIGHTNESS_THRESHOLD: f32 = 0.5;

/// Maximum alpha for cursor boost glow effect (as a multiplier of boost intensity).
/// Keeps the glow subtle even at full boost strength.
pub(crate) const CURSOR_BOOST_MAX_ALPHA: f32 = 0.3;

/// Width of the hollow-cursor border in pixels.
/// Used for the four thin rectangles that form the hollow block cursor outline.
pub(crate) const HOLLOW_CURSOR_BORDER_PX: f32 = 1.0;

/// Stipple on-length in pixels for dashed link underlines.
pub(crate) const STIPPLE_ON_PX: f32 = 2.0;

/// Stipple off-length in pixels for dashed link underlines.
pub(crate) const STIPPLE_OFF_PX: f32 = 2.0;

/// Number of text instances pre-allocated per terminal cell.
/// 2× because wide (double-width) characters can emit two instances.
pub(crate) const TEXT_INSTANCES_PER_CELL: usize = 2;

/// Text instances budgeted per cell when sizing a *pane batch*.
///
/// Higher than [`TEXT_INSTANCES_PER_CELL`] because the pane emitters can exceed
/// two instances for a single cell: a stippled link underline emits one quad per
/// dash across the cell, and geometrically drawn block/box-drawing characters emit
/// several rectangles. Sizing exactly to 2× would let one such cell eat into the
/// next pane's region.
pub(crate) const PANE_TEXT_INSTANCES_PER_CELL: usize = 3;

/// Compute the text foreground color when a block cursor covers a cell.
///
/// If the cursor has an explicit `text_color` (RGB, 3 components), that is used
/// directly with `text_alpha` appended.  Otherwise, a simple luminance-based
/// auto-contrast rule is applied: cursors brighter than `CURSOR_BRIGHTNESS_THRESHOLD`
/// get dark text; darker cursors get light text.
///
/// `cursor_color` and `cursor_text_color` are 3-component RGB (no alpha) as stored in
/// `CursorState`. The returned value is a 4-component RGBA with `text_alpha` as alpha.
///
/// This is a free function rather than a method so it can be called from
/// both `text_instance_builder.rs` and `pane_render/text_render.rs` without
/// any borrowing conflicts.
pub(crate) fn compute_cursor_text_color(
    cursor_color: [f32; 3],
    cursor_text_color: Option<[f32; 3]>,
    text_alpha: f32,
) -> [f32; 4] {
    if let Some(cursor_text) = cursor_text_color {
        [cursor_text[0], cursor_text[1], cursor_text[2], text_alpha]
    } else {
        let cursor_brightness = (cursor_color[0] + cursor_color[1] + cursor_color[2]) / 3.0;
        if cursor_brightness > CURSOR_BRIGHTNESS_THRESHOLD {
            [0.0, 0.0, 0.0, text_alpha] // Dark text on bright cursor
        } else {
            [1.0, 1.0, 1.0, text_alpha] // Bright text on dark cursor
        }
    }
}

impl CellRenderer {
    /// Orchestrate a full instance-buffer update for the current frame.
    ///
    /// **No caller inside this workspace reaches this method.** Its only callers
    /// are `render_to_texture` and `render_to_view`, which are `pub` API of this
    /// published crate but are no longer invoked by par-term itself: QA-011 moved
    /// the offscreen screenshot path onto `Renderer::composite_panes`, the same
    /// code the live frame runs, and everything drawn to the window has always
    /// gone through `build_pane_instance_buffers` in `pane_render/mod.rs`.
    ///
    /// Kept because removing it would be a breaking change to the crate's public
    /// surface. Treat it — and the single-grid `CellRenderer::cells` state it
    /// reads — as an external-consumer path, not one par-term exercises.
    ///
    /// That also settles what `CellRenderer::update_cells` is now for. In
    /// split-pane mode the buffer it receives is the *focused pane's*, whose row
    /// stride is the pane's column count, and it stores it at the full-window
    /// stride — so `cells` holds a re-wrapped pane, and this builder could not
    /// reproduce a split from it even with correct ranges. Fixing that stride
    /// would only serve this unreachable path, so it was left alone. What does
    /// still matter is `update_cells`' *return value*, which becomes
    /// `Renderer::dirty` and gates whether a frame renders at all; it therefore
    /// has to compare every incoming cell, including the trailing partial row
    /// that the old `end <= new_cells.len()` guard silently dropped.
    ///
    /// QA-011: `render_to_view` used to skip this rebuild, which left it drawing the
    /// pane path's leftovers through the single-grid draw ranges. Both branches now
    /// rebuild, so the buffer contents always match the layout they are drawn with.
    /// The two paths still share the same buffers — the pane builder marks every row
    /// dirty precisely so this one rebuilds in full rather than trusting its cache.
    ///
    /// For each dirty row the per-row background and text instance builders are called
    /// (see `instance_builders.rs`) and the results are written to the GPU buffers
    /// incrementally. After processing all rows, cursor overlay, separator, and gutter
    /// instances are built and uploaded in a single write per region.
    pub(crate) fn build_instance_buffers(&mut self) -> Result<()> {
        for row in 0..self.grid.rows {
            if self.dirty_rows[row] || self.row_cache[row].is_none() {
                let start = row * self.grid.cols;
                let end = (row + 1) * self.grid.cols;

                // Copy the row's cells into the scratch buffer.  We can't pass a slice
                // of `self.cells` directly because `build_row_*` methods take `&mut self`,
                // creating a conflicting mutable borrow.  Taking `scratch_row_cells` out
                // via `std::mem::take` releases that field's borrow so we can re-borrow
                // the rest of `self` mutably while holding the row data.
                let mut row_cells = std::mem::take(&mut self.scratch_row_cells);
                row_cells.clear();
                row_cells.extend_from_slice(&self.cells[start..end]);

                self.scratch_row_bg.clear();
                self.scratch_row_text.clear();

                // --- Background instances (RLE-merged) ---
                self.build_row_bg_instances(row, &row_cells);

                // --- Text + underline instances ---
                self.build_row_text_instances(row, &row_cells, start);

                // Update CPU-side buffers
                let bg_start = row * self.grid.cols;
                self.bg_instances[bg_start..bg_start + self.grid.cols]
                    .copy_from_slice(&self.scratch_row_bg);

                let text_start = row * self.grid.cols * 2;
                // Clear row text segment first
                for i in 0..(self.grid.cols * 2) {
                    self.text_instances[text_start + i].size = [0.0, 0.0];
                }
                // Copy new text instances
                let text_count = self.scratch_row_text.len().min(self.grid.cols * 2);
                self.text_instances[text_start..text_start + text_count]
                    .copy_from_slice(&self.scratch_row_text[..text_count]);

                // Update GPU-side buffers incrementally
                self.queue.write_buffer(
                    &self.buffers.bg_instance_buffer,
                    (bg_start * std::mem::size_of::<BackgroundInstance>()) as u64,
                    bytemuck::cast_slice(&self.scratch_row_bg),
                );
                self.queue.write_buffer(
                    &self.buffers.text_instance_buffer,
                    (text_start * std::mem::size_of::<TextInstance>()) as u64,
                    bytemuck::cast_slice(
                        &self.text_instances[text_start..text_start + self.grid.cols * 2],
                    ),
                );

                self.row_cache[row] = Some(true);
                self.dirty_rows[row] = false;

                // Restore the scratch buffer so its capacity is retained for the next row.
                self.scratch_row_cells = row_cells;
            }
        }

        // --- Cursor overlay instances ---
        // Write cursor-related overlays to extra slots at the end of bg_instances.
        // Slot layout: [0] cursor overlay (beam/underline), [1] guide, [2] shadow,
        //              [3] boost glow, [4-7] hollow outline.
        //
        // Every region offset below comes from `SingleGridLayout` — the same
        // description the draw ranges are derived from, so the bytes written here
        // and the instances drawn there cannot disagree.
        let layout = super::render::SingleGridLayout::new(self.grid.cols, self.grid.rows);
        let base_overlay_index = layout.cursor_overlays.start;
        let overlay_instances = self.build_cursor_overlay_instances();

        for (i, instance) in overlay_instances.iter().enumerate() {
            self.bg_instances[base_overlay_index + i] = *instance;
        }
        self.queue.write_buffer(
            &self.buffers.bg_instance_buffer,
            (base_overlay_index * std::mem::size_of::<BackgroundInstance>()) as u64,
            bytemuck::cast_slice(&overlay_instances),
        );

        // --- Separator line instances ---
        // Write command separator line instances after cursor overlay slots.
        let separator_base = layout.separators.start;
        let mut separator_instances = self.build_separator_instances();

        // The draw range below covers `rows` separator slots regardless of how many
        // separators exist, so the unused tail has to be blanked or it draws
        // whatever was last written there. The pane builder used to zero these as a
        // side effect of its own reset pass; it no longer has one (ARC-004).
        separator_instances.resize(self.grid.rows, BackgroundInstance::BLANK);

        for (i, instance) in separator_instances.iter().enumerate() {
            if separator_base + i < self.buffers.max_bg_instances {
                self.bg_instances[separator_base + i] = *instance;
            }
        }
        let separator_byte_offset = separator_base * std::mem::size_of::<BackgroundInstance>();
        let separator_byte_count =
            separator_instances.len() * std::mem::size_of::<BackgroundInstance>();
        if separator_byte_offset + separator_byte_count
            <= self.buffers.max_bg_instances * std::mem::size_of::<BackgroundInstance>()
        {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                separator_byte_offset as u64,
                bytemuck::cast_slice(&separator_instances),
            );
        }

        // --- Gutter indicator instances ---
        // Write gutter indicator background instances after separator slots.
        let gutter_base = layout.gutters.start;
        let mut gutter_instances = self.build_gutter_instances();
        // Blank the unused tail — see the separator slots above.
        gutter_instances.resize(self.grid.rows, BackgroundInstance::BLANK);

        for (i, instance) in gutter_instances.iter().enumerate() {
            if gutter_base + i < self.buffers.max_bg_instances {
                self.bg_instances[gutter_base + i] = *instance;
            }
        }
        let gutter_byte_offset = gutter_base * std::mem::size_of::<BackgroundInstance>();
        let gutter_byte_count = gutter_instances.len() * std::mem::size_of::<BackgroundInstance>();
        if gutter_byte_offset + gutter_byte_count
            <= self.buffers.max_bg_instances * std::mem::size_of::<BackgroundInstance>()
        {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                gutter_byte_offset as u64,
                bytemuck::cast_slice(&gutter_instances),
            );
        }

        // Update actual instance counts for draw calls.
        self.buffers.actual_bg_instances = layout.bg_instances();
        self.buffers.actual_text_instances =
            self.grid.cols * self.grid.rows * TEXT_INSTANCES_PER_CELL;

        Ok(())
    }
}
