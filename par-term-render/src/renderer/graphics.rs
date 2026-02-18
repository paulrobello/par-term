use super::Renderer;
use anyhow::Result;

impl Renderer {
    /// Update graphics textures (Sixel, iTerm2, Kitty)
    ///
    /// # Arguments
    /// * `graphics` - Graphics from the terminal with RGBA data
    /// * `view_scroll_offset` - Current view scroll offset (0 = viewing current content)
    /// * `scrollback_len` - Total lines in scrollback buffer
    /// * `visible_rows` - Number of visible rows in terminal
    #[allow(dead_code)]
    pub fn update_graphics(
        &mut self,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        view_scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<()> {
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

            // Calculate screen row based on whether this is a scrollback graphic or current
            let screen_row: isize = if let Some(sb_row) = graphic.scrollback_row {
                // Scrollback graphic: sb_row is absolute index in scrollback
                // Screen row = sb_row - view_start
                sb_row as isize - view_start as isize
            } else {
                // Current graphic: position is relative to visible area
                // Absolute position = scrollback_len + row - scroll_offset_rows
                // This keeps the graphic at its original absolute position as scrollback grows
                let absolute_row = scrollback_len.saturating_sub(graphic.scroll_offset_rows) + row;

                log::trace!(
                    "[RENDERER] CALC: scrollback_len={}, row={}, scroll_offset_rows={}, absolute_row={}, view_start={}, screen_row={}",
                    scrollback_len,
                    row,
                    graphic.scroll_offset_rows,
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

            self.sixel_graphics.push((
                id,
                screen_row, // row position (can be negative if scrolled off top)
                col,        // col position
                width_cells,
                height_cells,
                1.0,                 // Full opacity by default
                effective_clip_rows, // Rows to clip from top for partial rendering
            ));
        }

        if !graphics.is_empty() {
            self.dirty = true; // Mark dirty when graphics change
        }

        Ok(())
    }

    /// Render sixel graphics on top of terminal cells
    pub(crate) fn render_sixel_graphics(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
    ) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for sixel rendering
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("sixel encoder"),
                });

        // Create render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sixel render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render all sixel graphics
            self.graphics_renderer.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &mut render_pass,
                &self.sixel_graphics,
                self.size.width as f32,
                self.size.height as f32,
            )?;
        } // render_pass dropped here

        // Submit sixel commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Clear all cached sixel textures
    #[allow(dead_code)]
    pub fn clear_sixel_cache(&mut self) {
        self.graphics_renderer.clear_cache();
        self.sixel_graphics.clear();
        self.dirty = true;
    }

    /// Get the number of cached sixel textures
    #[allow(dead_code)]
    pub fn sixel_cache_size(&self) -> usize {
        self.graphics_renderer.cache_size()
    }

    /// Remove a specific sixel texture from cache
    #[allow(dead_code)]
    pub fn remove_sixel_texture(&mut self, id: u64) {
        self.graphics_renderer.remove_texture(id);
        self.sixel_graphics
            .retain(|(gid, _, _, _, _, _, _)| *gid != id);
        self.dirty = true;
    }
}
