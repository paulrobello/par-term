// Surface resize and DPI scale-factor changes. Both recompute the grid and fan
// the new geometry out to the graphics and shader renderers, so they are kept
// together: `handle_scale_factor_change` finishes by delegating to `resize`.

use winit::dpi::PhysicalSize;

use super::Renderer;

impl Renderer {
    /// Resize the renderer and recalculate grid dimensions based on padding/font metrics
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) -> (usize, usize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.dirty = true; // Mark dirty on resize
            let result = self.cell_renderer.resize(new_size.width, new_size.height);

            // Update graphics renderer cell dimensions
            self.graphics_renderer.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                self.cell_renderer.window_padding(),
            );

            // Update custom shader renderer dimensions
            if let Some(ref mut custom_shader) = self.custom_shader_renderer {
                custom_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                custom_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            // Update cursor shader renderer dimensions
            if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
                cursor_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                cursor_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            return result;
        }

        self.cell_renderer.grid_size()
    }

    /// Update scale factor and resize so the PTY grid matches the new DPI.
    pub fn handle_scale_factor_change(
        &mut self,
        scale_factor: f64,
        new_size: PhysicalSize<u32>,
    ) -> (usize, usize) {
        let old_scale = self.cell_renderer.scale_factor;
        self.cell_renderer.update_scale_factor(scale_factor);
        let new_scale = self.cell_renderer.scale_factor;

        // Rescale physical pixel values when DPI changes
        if old_scale > 0.0 && (old_scale - new_scale).abs() > f32::EPSILON {
            // Rescale content_offset_y
            let logical_offset_y = self.cell_renderer.content_offset_y() / old_scale;
            let new_physical_offset_y = logical_offset_y * new_scale;
            self.cell_renderer
                .set_content_offset_y(new_physical_offset_y);
            self.graphics_renderer
                .set_content_offset_y(new_physical_offset_y);
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_content_offset_y(new_physical_offset_y);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_content_offset_y(new_physical_offset_y);
            }

            // Rescale content_offset_x
            let logical_offset_x = self.cell_renderer.content_offset_x() / old_scale;
            let new_physical_offset_x = logical_offset_x * new_scale;
            self.cell_renderer
                .set_content_offset_x(new_physical_offset_x);
            self.graphics_renderer
                .set_content_offset_x(new_physical_offset_x);
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_content_offset_x(new_physical_offset_x);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_content_offset_x(new_physical_offset_x);
            }

            // Rescale content_inset_bottom
            let logical_inset_bottom = self.cell_renderer.content_inset_bottom() / old_scale;
            let new_physical_inset_bottom = logical_inset_bottom * new_scale;
            self.cell_renderer
                .set_content_inset_bottom(new_physical_inset_bottom);

            // Rescale egui_bottom_inset (status bar)
            if self.cell_renderer.grid.egui_bottom_inset > 0.0 {
                let logical_egui_bottom = self.cell_renderer.grid.egui_bottom_inset / old_scale;
                self.cell_renderer.grid.egui_bottom_inset = logical_egui_bottom * new_scale;
            }

            // Rescale content_inset_right (AI Inspector panel).
            // The shader renderers keep their own copy so they can exclude the
            // panel area from effects; `set_content_inset_right` fans the value
            // out to them and this path has to do the same, or the shaders keep
            // masking the pre-DPI-change panel width.
            if self.cell_renderer.grid.content_inset_right > 0.0 {
                let logical_inset_right = self.cell_renderer.grid.content_inset_right / old_scale;
                let new_physical_inset_right = logical_inset_right * new_scale;
                self.cell_renderer.grid.content_inset_right = new_physical_inset_right;
                if let Some(ref mut cs) = self.custom_shader_renderer {
                    cs.set_content_inset_right(new_physical_inset_right);
                }
                if let Some(ref mut cs) = self.cursor_shader_renderer {
                    cs.set_content_inset_right(new_physical_inset_right);
                }
            }

            // Rescale egui_right_inset
            if self.cell_renderer.grid.egui_right_inset > 0.0 {
                let logical_egui_right = self.cell_renderer.grid.egui_right_inset / old_scale;
                self.cell_renderer.grid.egui_right_inset = logical_egui_right * new_scale;
            }

            // Rescale window_padding
            let logical_padding = self.cell_renderer.window_padding() / old_scale;
            let new_physical_padding = logical_padding * new_scale;
            self.cell_renderer
                .update_window_padding(new_physical_padding);

            // Rescale scrollbar width
            let logical_scrollbar = self.cell_renderer.scrollbar.width() / old_scale;
            let new_physical_scrollbar = logical_scrollbar * new_scale;
            self.cell_renderer.scrollbar.update_appearance(
                new_physical_scrollbar,
                self.cell_renderer.scrollbar.thumb_color(),
                self.cell_renderer.scrollbar.track_color(),
            );

            // Sync new scale factor to shader renderers for cursor sizing
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_scale_factor(new_scale);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_scale_factor(new_scale);
            }

            // Every inset rescaled above feeds scrollbar geometry, and so does the
            // new scrollbar width. `update_scrollbar` skips the GPU upload when its
            // cached tuple is unchanged, and none of those values are in the tuple,
            // so without this the scrollbar stays at its pre-DPI-change size and
            // position — the same failure the inset setters call out.
            self.last_scrollbar_state = (usize::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        }

        self.resize(new_size)
    }
}
