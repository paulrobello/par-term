use super::{BackgroundInstance, Cell, CellRenderer, RowCacheEntry, TextInstance, pipeline};

/// Terminal grid dimensions, cell sizes, padding, and content offsets.
pub(crate) struct GridLayout {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cell_width: f32,
    pub(crate) cell_height: f32,
    pub(crate) window_padding: f32,
    /// Vertical offset for terminal content (e.g., tab bar at top).
    /// Content is rendered starting at y = window_padding + content_offset_y.
    pub(crate) content_offset_y: f32,
    /// Horizontal offset for terminal content (e.g., tab bar on left).
    /// Content is rendered starting at x = window_padding + content_offset_x.
    pub(crate) content_offset_x: f32,
    /// Bottom inset for terminal content (e.g., tab bar at bottom).
    /// Reduces available height without shifting content vertically.
    pub(crate) content_inset_bottom: f32,
    /// Right inset for terminal content (e.g., AI Inspector panel).
    /// Reduces available width without shifting content horizontally.
    pub(crate) content_inset_right: f32,
    /// Additional bottom inset from egui panels (status bar, tmux bar).
    /// This is added to content_inset_bottom for scrollbar bounds only,
    /// since egui panels already claim space before wgpu rendering.
    pub(crate) egui_bottom_inset: f32,
    /// Additional right inset from egui panels (AI Inspector).
    /// This is added to content_inset_right for scrollbar bounds only,
    /// since egui panels already claim space before wgpu rendering.
    pub(crate) egui_right_inset: f32,
}

impl CellRenderer {
    pub fn cell_width(&self) -> f32 {
        self.grid.cell_width
    }
    pub fn cell_height(&self) -> f32 {
        self.grid.cell_height
    }
    pub fn window_padding(&self) -> f32 {
        self.grid.window_padding
    }
    pub fn content_offset_y(&self) -> f32 {
        self.grid.content_offset_y
    }
    /// Set the vertical content offset (e.g., tab bar height at top).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, offset: f32) -> Option<(usize, usize)> {
        if (self.grid.content_offset_y - offset).abs() > f32::EPSILON {
            self.grid.content_offset_y = offset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_offset_x(&self) -> f32 {
        self.grid.content_offset_x
    }
    /// Set the horizontal content offset (e.g., tab bar on left).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_x(&mut self, offset: f32) -> Option<(usize, usize)> {
        if (self.grid.content_offset_x - offset).abs() > f32::EPSILON {
            self.grid.content_offset_x = offset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_inset_bottom(&self) -> f32 {
        self.grid.content_inset_bottom
    }
    /// Set the bottom content inset (e.g., tab bar at bottom).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_bottom(&mut self, inset: f32) -> Option<(usize, usize)> {
        if (self.grid.content_inset_bottom - inset).abs() > f32::EPSILON {
            self.grid.content_inset_bottom = inset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_inset_right(&self) -> f32 {
        self.grid.content_inset_right
    }
    /// Set the right content inset (e.g., AI Inspector panel).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_right(&mut self, inset: f32) -> Option<(usize, usize)> {
        if (self.grid.content_inset_right - inset).abs() > f32::EPSILON {
            log::info!(
                "[SCROLLBAR] set_content_inset_right: {:.1} -> {:.1} (physical px)",
                self.grid.content_inset_right,
                inset
            );
            self.grid.content_inset_right = inset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn grid_size(&self) -> (usize, usize) {
        (self.grid.cols, self.grid.rows)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> (usize, usize) {
        if width == 0 || height == 0 {
            return (self.grid.cols, self.grid.rows);
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        let available_width = (width as f32
            - self.grid.window_padding * 2.0
            - self.grid.content_offset_x
            - self.grid.content_inset_right
            - self.scrollbar.width())
        .max(0.0);
        let available_height = (height as f32
            - self.grid.window_padding * 2.0
            - self.grid.content_offset_y
            - self.grid.content_inset_bottom
            - self.grid.egui_bottom_inset)
            .max(0.0);
        let new_cols = (available_width / self.grid.cell_width).max(1.0) as usize;
        let new_rows = (available_height / self.grid.cell_height).max(1.0) as usize;

        if new_cols != self.grid.cols || new_rows != self.grid.rows {
            self.grid.cols = new_cols;
            self.grid.rows = new_rows;
            self.cells = vec![Cell::default(); self.grid.cols * self.grid.rows];
            self.dirty_rows = vec![true; self.grid.rows];
            self.row_cache = (0..self.grid.rows).map(|_| None::<RowCacheEntry>).collect();
            self.recreate_instance_buffers();
        }

        self.update_bg_image_uniforms(None);
        (self.grid.cols, self.grid.rows)
    }

    pub(crate) fn recreate_instance_buffers(&mut self) {
        self.buffers.max_bg_instances =
            self.grid.cols * self.grid.rows + 10 + self.grid.rows + self.grid.rows; // Extra slots for cursor overlays + separator lines + gutter indicators
        self.buffers.max_text_instances = self.grid.cols * self.grid.rows * 2;
        let (bg_buf, text_buf) = pipeline::create_instance_buffers(
            &self.device,
            self.buffers.max_bg_instances,
            self.buffers.max_text_instances,
        );
        self.buffers.bg_instance_buffer = bg_buf;
        self.buffers.text_instance_buffer = text_buf;
        // Reset actual counts - will be updated when instance buffers are built
        self.buffers.actual_bg_instances = 0;
        self.buffers.actual_text_instances = 0;

        self.bg_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            self.buffers.max_bg_instances
        ];
        self.text_instances = vec![
            TextInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                tex_offset: [0.0, 0.0],
                tex_size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
                is_colored: 0,
            };
            self.buffers.max_text_instances
        ];

        // Resize scratch buffers to match new grid; keep existing allocations if large enough
        self.scratch_row_bg.reserve(
            self.grid
                .cols
                .saturating_sub(self.scratch_row_bg.capacity()),
        );
        self.scratch_row_text
            .reserve((self.grid.cols * 2).saturating_sub(self.scratch_row_text.capacity()));
    }

    /// Update scale factor and recalculate all font metrics and cell dimensions.
    /// This is called when the window is dragged between displays with different DPIs.
    pub fn update_scale_factor(&mut self, scale_factor: f64) {
        let new_scale = scale_factor as f32;

        // Skip if scale factor hasn't changed
        if (self.scale_factor - new_scale).abs() < f32::EPSILON {
            return;
        }

        log::info!(
            "Recalculating font metrics for scale factor change: {} -> {}",
            self.scale_factor,
            new_scale
        );

        self.scale_factor = new_scale;

        // Recalculate font_size_pixels based on new scale factor
        let platform_dpi = if cfg!(target_os = "macos") {
            crate::cell_renderer::MACOS_PLATFORM_DPI
        } else {
            crate::cell_renderer::DEFAULT_PLATFORM_DPI
        };
        let base_font_pixels =
            self.font.base_font_size * platform_dpi / crate::cell_renderer::FONT_REFERENCE_DPI;
        self.font.font_size_pixels = (base_font_pixels * new_scale).max(1.0);

        // Re-extract font metrics at new scale
        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = self.font_manager.get_font(0).expect(
                "Primary font at index 0 must exist in FontManager when updating scale factor",
            );
            let metrics = primary_font.metrics(&[]);
            let scale = self.font.font_size_pixels / metrics.units_per_em as f32;
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;
            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        self.font.font_ascent = font_ascent;
        self.font.font_descent = font_descent;
        self.font.font_leading = font_leading;
        self.font.char_advance = char_advance;

        // Recalculate cell dimensions
        let natural_line_height = font_ascent + font_descent + font_leading;
        self.grid.cell_height = (natural_line_height * self.font.line_spacing).max(1.0);
        self.grid.cell_width = (char_advance * self.font.char_spacing).max(1.0);

        log::info!(
            "New cell dimensions: {}x{} (font_size_pixels: {})",
            self.grid.cell_width,
            self.grid.cell_height,
            self.font.font_size_pixels
        );

        // Clear glyph cache - glyphs need to be re-rasterized at new DPI
        self.clear_glyph_cache();

        // Mark all rows as dirty to force re-rendering
        self.dirty_rows.fill(true);
    }

    pub fn update_window_padding(&mut self, padding: f32) -> Option<(usize, usize)> {
        if (self.grid.window_padding - padding).abs() > f32::EPSILON {
            self.grid.window_padding = padding;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
}
