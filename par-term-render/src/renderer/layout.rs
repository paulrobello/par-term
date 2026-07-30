use winit::dpi::PhysicalSize;

use super::Renderer;

// Layout and sizing accessors — simple getters/setters for grid geometry, padding,
// and content offsets. Kept as the sibling of resize.rs since all of these deal
// with the spatial layout of the renderer.
impl Renderer {
    /// Get the current size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Get the current grid dimensions (columns, rows)
    pub fn grid_size(&self) -> (usize, usize) {
        self.cell_renderer.grid_size()
    }

    /// Get cell width in pixels
    pub fn cell_width(&self) -> f32 {
        self.cell_renderer.cell_width()
    }

    /// Get cell height in pixels
    pub fn cell_height(&self) -> f32 {
        self.cell_renderer.cell_height()
    }

    /// Get window padding in physical pixels (scaled by DPI)
    pub fn window_padding(&self) -> f32 {
        self.cell_renderer.window_padding()
    }

    /// Get the scrollbar width in physical pixels
    pub fn scrollbar_width(&self) -> f32 {
        self.cell_renderer.scrollbar.width()
    }

    /// Returns total non-terminal pixel overhead as (horizontal_px, vertical_px).
    /// See `CellRenderer::chrome_overhead` for details.
    pub fn chrome_overhead(&self) -> (f32, f32) {
        self.cell_renderer.chrome_overhead()
    }

    /// Get the vertical content offset in physical pixels (e.g., tab bar height scaled by DPI)
    pub fn content_offset_y(&self) -> f32 {
        self.cell_renderer.content_offset_y()
    }

    /// Get the display scale factor (e.g., 2.0 on Retina displays)
    pub fn scale_factor(&self) -> f32 {
        self.cell_renderer.scale_factor
    }

    /// Set the vertical content offset (e.g., tab bar height) in logical pixels.
    /// The offset is scaled by the display scale factor to physical pixels internally,
    /// since the cell renderer works in physical pixel coordinates while egui (tab bar)
    /// uses logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        // Scale from logical pixels (egui/config) to physical pixels (wgpu surface)
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_y(physical_offset);
        // Always update graphics renderer offset, even if grid size didn't change
        self.graphics_renderer.set_content_offset_y(physical_offset);
        // Update custom shader renderer content offset
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_y(physical_offset);
        }
        // Update cursor shader renderer content offset
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_y(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the horizontal content offset in physical pixels
    pub fn content_offset_x(&self) -> f32 {
        self.cell_renderer.content_offset_x()
    }

    /// Set the horizontal content offset (e.g., tab bar on left) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_x(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_x(physical_offset);
        self.graphics_renderer.set_content_offset_x(physical_offset);
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_x(physical_offset);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_x(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the bottom content inset in physical pixels
    pub fn content_inset_bottom(&self) -> f32 {
        self.cell_renderer.content_inset_bottom()
    }

    /// Get the right content inset in physical pixels
    pub fn content_inset_right(&self) -> f32 {
        self.cell_renderer.content_inset_right()
    }

    /// Set the bottom content inset (e.g., tab bar at bottom) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_bottom(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_bottom(physical_inset);
        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache — the track height depends on
            // the bottom inset, so the scrollbar must be repositioned.
            self.last_scrollbar_state = (usize::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }
        result
    }

    /// Set the right content inset (e.g., AI Inspector panel) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_right(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_right(physical_inset);

        // Also update custom shader renderer to exclude panel area from effects
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_inset_right(physical_inset);
        }
        // Also update cursor shader renderer
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_inset_right(physical_inset);
        }

        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache so the next update_scrollbar()
            // repositions the scrollbar at the new right inset. Without this,
            // the cache guard sees the same (scroll_offset, visible_lines,
            // total_lines) tuple and skips the GPU upload, leaving the
            // scrollbar stuck at the old position.
            self.last_scrollbar_state = (usize::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }
        result
    }

    /// Set the additional bottom inset from egui panels (status bar, tmux bar).
    ///
    /// This inset reduces the terminal grid height so content does not render
    /// behind the status bar. Also affects scrollbar bounds.
    /// Returns `Some((cols, rows))` if the grid was resized.
    pub fn set_egui_bottom_inset(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        if (self.cell_renderer.grid.egui_bottom_inset - physical_inset).abs() > f32::EPSILON {
            self.cell_renderer.grid.egui_bottom_inset = physical_inset;
            let (w, h) = (
                self.cell_renderer.config.width,
                self.cell_renderer.config.height,
            );
            return Some(self.cell_renderer.resize(w, h));
        }
        None
    }

    /// Set the additional right inset from egui panels (AI Inspector).
    ///
    /// This inset is added to `content_inset_right` for scrollbar bounds only.
    /// egui panels already claim space before wgpu rendering, so this doesn't
    /// affect the terminal grid sizing.
    pub fn set_egui_right_inset(&mut self, logical_inset: f32) {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        self.cell_renderer.grid.egui_right_inset = physical_inset;
    }
}
