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

        // Mark dirty when graphics change (added or removed)
        if !graphics.is_empty() || had_graphics {
            self.dirty = true;
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

    /// Compute positioned graphics list for a single pane without touching `self.sixel_graphics`.
    ///
    /// Shares the same texture cache as the global path so textures are never duplicated.
    ///
    /// Returns a `Vec` of `(id, screen_row, col, width_cells, height_cells, alpha,
    /// effective_clip_rows)` tuples ready to pass to
    /// [`GraphicsRenderer::render_for_pane`].
    #[allow(clippy::type_complexity)]
    pub fn update_pane_graphics(
        &mut self,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        view_scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<Vec<(u64, isize, usize, usize, usize, f32, usize)>> {
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

            positioned.push((
                id,
                screen_row,
                col,
                width_cells,
                height_cells,
                1.0,
                effective_clip_rows,
            ));
        }

        Ok(positioned)
    }

    /// Render inline graphics (Sixel/iTerm2/Kitty) for a single split pane.
    ///
    /// Uses the same `surface_view` as the cell render pass (with `LoadOp::Load`) so
    /// graphics are composited on top of already-rendered cells.  A scissor rect derived
    /// from `viewport` clips output to the pane's bounds.
    pub(crate) fn render_pane_sixel_graphics(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &crate::cell_renderer::PaneViewport,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<()> {
        let positioned =
            self.update_pane_graphics(graphics, scroll_offset, scrollback_len, visible_rows)?;

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
            for (id, screen_row, col, wc, hc, _, clip) in &positioned {
                log::debug!(
                    "[PANE_GRAPHICS]   positioned: id={}, screen_row={}, col={}, width_cells={}, height_cells={}, clip_rows={}",
                    id,
                    screen_row,
                    col,
                    wc,
                    hc,
                    clip
                );
            }

            self.graphics_renderer.render_for_pane(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &mut render_pass,
                &positioned,
                self.size.width as f32,
                self.size.height as f32,
                ox,
                oy,
            )?;
        }

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
