use super::{BackgroundInstance, CellRenderer, RowCacheEntry, TextInstance};
use anyhow::Result;
use par_term_fonts::text_shaper::ShapingOptions;

/// Number of extra background instance slots reserved for cursor overlays
/// (beam/underline, guide line, shadow, boost glow, hollow outline sides).
/// Layout: [0] cursor overlay, [1] guide, [2] shadow, [3] boost glow, [4-7] hollow outline.
pub(crate) const CURSOR_OVERLAY_SLOTS: usize = 10;

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
pub(crate) const HOLLOW_CURSOR_BORDER_PX: f32 = 2.0;

/// Stipple on-length in pixels for dashed link underlines.
pub(crate) const STIPPLE_ON_PX: f32 = 2.0;

/// Stipple off-length in pixels for dashed link underlines.
pub(crate) const STIPPLE_OFF_PX: f32 = 2.0;

/// Number of text instances pre-allocated per terminal cell.
/// 2× because wide (double-width) characters can emit two instances.
pub(crate) const TEXT_INSTANCES_PER_CELL: usize = 2;

impl CellRenderer {
    /// Orchestrate a full instance-buffer update for the current frame.
    ///
    /// For each dirty row the per-row background and text instance builders are called
    /// (see `instance_builders.rs`) and the results are written to the GPU buffers
    /// incrementally. After processing all rows, cursor overlay, separator, and gutter
    /// instances are built and uploaded in a single write per region.
    pub(crate) fn build_instance_buffers(&mut self) -> Result<()> {
        let _shaping_options = ShapingOptions {
            enable_ligatures: self.font.enable_ligatures,
            enable_kerning: self.font.enable_kerning,
            ..Default::default()
        };

        for row in 0..self.grid.rows {
            if self.dirty_rows[row] || self.row_cache[row].is_none() {
                let start = row * self.grid.cols;
                let end = (row + 1) * self.grid.cols;

                // Clone the slice data we need — required because build_row_bg_instances
                // and build_row_text_instances borrow self mutably while row_cells is a
                // shared slice into self.cells.
                let row_cells: Vec<_> = self.cells[start..end].to_vec();

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

                self.row_cache[row] = Some(RowCacheEntry {});
                self.dirty_rows[row] = false;
            }
        }

        // --- Cursor overlay instances ---
        // Write cursor-related overlays to extra slots at the end of bg_instances.
        // Slot layout: [0] cursor overlay (beam/underline), [1] guide, [2] shadow,
        //              [3] boost glow, [4-7] hollow outline.
        let base_overlay_index = self.grid.cols * self.grid.rows;
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
        let separator_base = self.grid.cols * self.grid.rows + CURSOR_OVERLAY_SLOTS;
        let separator_instances = self.build_separator_instances();

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
        let gutter_base = separator_base + self.grid.rows;
        let gutter_instances = self.build_gutter_instances();

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
        // Layout: [0..cols*rows] cells + [cols*rows..+CURSOR_OVERLAY_SLOTS] overlays
        //         + [+CURSOR_OVERLAY_SLOTS..+rows] separators + [..+rows] gutters
        self.buffers.actual_bg_instances = self.grid.cols * self.grid.rows
            + CURSOR_OVERLAY_SLOTS
            + self.grid.rows
            + self.grid.rows;
        self.buffers.actual_text_instances =
            self.grid.cols * self.grid.rows * TEXT_INSTANCES_PER_CELL;

        Ok(())
    }
}
