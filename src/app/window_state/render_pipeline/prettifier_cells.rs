//! Prettifier cell substitution for the render pipeline.
//!
//! `apply_prettifier_cell_substitution` runs every frame when prettified blocks
//! are present.  It overlays styled rendered content onto the raw terminal cell
//! grid and collects inline RGBA graphics (Mermaid diagrams, etc.) for GPU
//! compositing in the same pass as Sixel/iTerm2/Kitty graphics.

use crate::tab::Tab;

/// Type alias for a prettifier graphic entry:
/// `(texture_id, rgba_data, pixel_width, pixel_height, screen_row, col)`
#[allow(clippy::type_complexity)]
pub(super) type PrettifierGraphic = (u64, std::sync::Arc<Vec<u8>>, u32, u32, isize, usize);

/// Apply prettifier cell substitution for the current frame.
///
/// Iterates over all viewport rows and, for each row that falls within a
/// rendered prettifier block, replaces the raw terminal cells with the block's
/// styled output lines.  Simultaneously collects any RGBA graphics produced by
/// the block (e.g. Mermaid diagrams) so they can be composited by the GPU pass.
///
/// # Arguments
/// * `tab` - active tab (holds prettifier pipeline and gutter manager)
/// * `cells` - mutable cell grid for the current frame
/// * `is_alt_screen` - if true, substitution is skipped (alt screen apps own the display)
/// * `visible_lines` - number of visible rows in the grid
/// * `scrollback_len` - total scrollback lines (used to map absolute rows)
/// * `grid_cols` - number of columns per row
///
/// # Returns
/// A `Vec` of prettifier graphics to upload to the GPU.
#[allow(clippy::type_complexity)]
pub(super) fn apply_prettifier_cell_substitution(
    tab: &Tab,
    cells: &mut [crate::cell_renderer::Cell],
    is_alt_screen: bool,
    visible_lines: usize,
    scrollback_len: usize,
    grid_cols: usize,
) -> Vec<PrettifierGraphic> {
    let mut prettifier_graphics: Vec<PrettifierGraphic> = Vec::new();

    if is_alt_screen {
        return prettifier_graphics;
    }

    let Some(ref pipeline) = tab.prettifier else {
        return prettifier_graphics;
    };

    if !pipeline.is_enabled() {
        return prettifier_graphics;
    }

    let scroll_off = tab.scroll_state.offset;
    let gutter_w = tab.gutter_manager.gutter_width;

    // Track which blocks we've already collected graphics from
    // to avoid duplicates when multiple viewport rows fall in
    // the same block.
    let mut collected_block_ids = std::collections::HashSet::new();

    for viewport_row in 0..visible_lines {
        let absolute_row = scrollback_len.saturating_sub(scroll_off) + viewport_row;
        if let Some(block) = pipeline.block_at_row(absolute_row) {
            if !block.has_rendered() {
                continue;
            }

            // Collect inline graphics from this block (once per block).
            if collected_block_ids.insert(block.block_id) {
                let block_start = block.content().start_row;
                for graphic in block.buffer.rendered_graphics() {
                    if !graphic.is_rgba
                        || graphic.data.is_empty()
                        || graphic.pixel_width == 0
                        || graphic.pixel_height == 0
                    {
                        continue;
                    }
                    // Compute screen row: block_start + graphic.row within block,
                    // then convert to viewport coordinates.
                    let abs_graphic_row = block_start + graphic.row;
                    let view_start = scrollback_len.saturating_sub(scroll_off);
                    let screen_row = abs_graphic_row as isize - view_start as isize;

                    // Use block_id + graphic row as a stable texture ID
                    // (offset to avoid colliding with terminal graphic IDs).
                    let texture_id =
                        0x8000_0000_0000_0000_u64 | (block.block_id << 16) | (graphic.row as u64);

                    crate::debug_info!(
                        "PRETTIFIER",
                        "uploading graphic: block={}, row={}, screen_row={}, {}x{} px, {} bytes RGBA",
                        block.block_id,
                        graphic.row,
                        screen_row,
                        graphic.pixel_width,
                        graphic.pixel_height,
                        graphic.data.len()
                    );

                    prettifier_graphics.push((
                        texture_id,
                        graphic.data.clone(),
                        graphic.pixel_width,
                        graphic.pixel_height,
                        screen_row,
                        graphic.col + gutter_w,
                    ));
                }
            }

            // Use display_lines_ref() to borrow rendered lines directly
            // without cloning the entire Vec on every frame.  Falls back
            // to an owned allocation only when source view mode is active
            // (rare — the block has already passed the has_rendered() guard).
            let owned_fallback;
            let display_lines: &[_] = if let Some(lines) = block.buffer.display_lines_ref() {
                lines
            } else {
                owned_fallback = block.buffer.display_lines();
                &owned_fallback
            };
            let block_start = block.content().start_row;
            let source_offset = absolute_row.saturating_sub(block_start);
            // Use the source→rendered line mapping when available so
            // that consumed source lines (e.g., code-fence closes) don't
            // cause index drift.  Fall back to direct indexing when no
            // mapping exists (unrendered blocks use source lines 1:1).
            let rendered_idx = block
                .buffer
                .rendered_line_for_source(source_offset)
                .unwrap_or(source_offset);
            if let Some(styled_line) = display_lines.get(rendered_idx) {
                crate::debug_trace!(
                    "PRETTIFIER",
                    "cell sub: vp_row={}, abs_row={}, block_id={}, src_off={}, rnd_idx={}, segs={}",
                    viewport_row,
                    absolute_row,
                    block.block_id,
                    source_offset,
                    rendered_idx,
                    styled_line.segments.len()
                );
                let cell_start = viewport_row * grid_cols;
                let cell_end = (cell_start + grid_cols).min(cells.len());
                if cell_start >= cells.len() {
                    break;
                }
                // Clear row
                for cell in &mut cells[cell_start..cell_end] {
                    *cell = par_term_config::Cell::default();
                }
                // Write styled segments (offset by gutter width to avoid clipping)
                let mut col = gutter_w;
                for segment in &styled_line.segments {
                    for ch in segment.text.chars() {
                        if col >= grid_cols {
                            break;
                        }
                        let idx = cell_start + col;
                        if idx < cells.len() {
                            cells[idx].grapheme = ch.to_string();
                            if let Some([r, g, b]) = segment.fg {
                                cells[idx].fg_color = [r, g, b, 0xFF];
                            }
                            if let Some([r, g, b]) = segment.bg {
                                cells[idx].bg_color = [r, g, b, 0xFF];
                            }
                            cells[idx].bold = segment.bold;
                            cells[idx].italic = segment.italic;
                            cells[idx].underline = segment.underline;
                            cells[idx].strikethrough = segment.strikethrough;
                        }
                        col += 1;
                    }
                }
            }
        }
    }

    prettifier_graphics
}
