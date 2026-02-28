//! Renderer lifecycle and layout-sync operations for WindowState.

use crate::app::window_state::WindowState;
use anyhow::Result;
use std::sync::Arc;

impl WindowState {
    /// Rebuild the renderer after font-related changes and resize the terminal accordingly
    pub(crate) fn rebuild_renderer(&mut self) -> Result<()> {
        use crate::app::renderer_init::RendererInitParams;

        let window = if let Some(w) = &self.window {
            Arc::clone(w)
        } else {
            return Ok(()); // Nothing to rebuild yet
        };

        // Create renderer using DRY init params
        let theme = self.config.load_theme();
        // Get shader metadata from cache for full 3-tier resolution
        let metadata = self
            .config
            .custom_shader
            .as_ref()
            .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self.config.cursor_shader.as_ref().and_then(|name| {
            self.shader_state
                .cursor_shader_metadata_cache
                .get(name)
                .cloned()
        });
        let params = RendererInitParams::from_config(
            &self.config,
            &theme,
            metadata.as_ref(),
            cursor_metadata.as_ref(),
        );

        // Drop the old renderer BEFORE creating a new one.
        // wgpu only allows one surface per window, so the old surface must be
        // released before we can create a new one.
        self.renderer = None;

        let mut renderer = self
            .runtime
            .block_on(params.create_renderer(Arc::clone(&window)))?;

        let (cols, rows) = renderer.grid_size();
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (cols as f32 * cell_width) as usize;
        let height_px = (rows as f32 * cell_height) as usize;

        // Resize all tabs' terminals
        for tab in self.tab_manager.tabs_mut() {
            if let Ok(mut term) = tab.terminal.try_lock() {
                let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                term.set_theme(self.config.load_theme());
            }
            tab.cache.cells = None;
        }

        // Apply cursor shader configuration
        self.apply_cursor_shader_config(&mut renderer, &params);

        self.renderer = Some(renderer);
        self.focus_state.needs_redraw = true;

        // Re-apply AI Inspector panel inset to the new renderer.
        // The old renderer had the correct content_inset_right but the new one
        // starts with 0.0. Force last_inspector_width to 0 so sync detects the change.
        self.overlay_ui.last_inspector_width = 0.0;
        self.sync_ai_inspector_width();

        // Reset egui with preserved memory (window positions, collapse state)
        self.init_egui(&window, true);
        self.request_redraw();

        Ok(())
    }

    /// Force surface reconfiguration - useful when rendering becomes corrupted
    /// after moving between monitors or when automatic detection fails.
    /// Also clears glyph cache to ensure fonts render correctly.
    pub(crate) fn force_surface_reconfigure(&mut self) {
        log::info!("Force surface reconfigure triggered");

        if let Some(renderer) = &mut self.renderer {
            // Reconfigure the surface
            renderer.reconfigure_surface();

            // Clear glyph cache to force re-rasterization at correct DPI
            renderer.clear_glyph_cache();

            // Invalidate cached cells to force full re-render
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.cells = None;
            }
        }

        // On macOS, reconfigure the Metal layer
        #[cfg(target_os = "macos")]
        {
            if let Some(window) = &self.window
                && let Err(e) = crate::macos_metal::configure_metal_layer_for_performance(window)
            {
                log::warn!("Failed to reconfigure Metal layer: {}", e);
            }
        }

        // Request redraw
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    // ========================================================================
    // Tab Bar Offsets
    // ========================================================================

    /// Apply tab bar offsets based on the current position configuration.
    /// Sets content_offset_y (top), content_offset_x (left), and content_inset_bottom (bottom).
    /// Returns Some((cols, rows)) if any offset changed and caused a grid resize.
    pub(crate) fn apply_tab_bar_offsets(
        &self,
        renderer: &mut crate::renderer::Renderer,
        tab_bar_height: f32,
        tab_bar_width: f32,
    ) -> Option<(usize, usize)> {
        Self::apply_tab_bar_offsets_for_position(
            self.config.tab_bar_position,
            renderer,
            tab_bar_height,
            tab_bar_width,
        )
    }

    /// Static helper to apply tab bar offsets (avoids borrowing self).
    pub(crate) fn apply_tab_bar_offsets_for_position(
        position: crate::config::TabBarPosition,
        renderer: &mut crate::renderer::Renderer,
        tab_bar_height: f32,
        tab_bar_width: f32,
    ) -> Option<(usize, usize)> {
        use crate::config::TabBarPosition;
        let (offset_y, offset_x, inset_bottom) = match position {
            TabBarPosition::Top => (tab_bar_height, 0.0, 0.0),
            TabBarPosition::Bottom => (0.0, 0.0, tab_bar_height),
            TabBarPosition::Left => (0.0, tab_bar_width, 0.0),
        };

        let mut result = None;
        if let Some(grid) = renderer.set_content_offset_y(offset_y) {
            result = Some(grid);
        }
        if let Some(grid) = renderer.set_content_offset_x(offset_x) {
            result = Some(grid);
        }
        if let Some(grid) = renderer.set_content_inset_bottom(inset_bottom) {
            result = Some(grid);
        }
        result
    }

    // ========================================================================
    // AI Inspector Panel Width Sync
    // ========================================================================

    /// Sync the AI Inspector panel consumed width with the renderer.
    ///
    /// When the panel opens, closes, or is resized by dragging, the terminal
    /// column count must be updated so text reflows to fit the available space.
    /// This method checks whether the consumed width has changed and, if so,
    /// updates the renderer's right content inset and resizes all terminals.
    pub(crate) fn sync_ai_inspector_width(&mut self) {
        let current_width = self.overlay_ui.ai_inspector.consumed_width();

        if let Some(renderer) = &mut self.renderer {
            // Always verify the renderer's content_inset_right matches the expected
            // physical value. This catches cases where content_inset_right was reset
            // (e.g., renderer rebuild, scale factor change) even when the logical
            // panel width hasn't changed.
            // The renderer's set_content_inset_right() has its own guard that only
            // triggers a resize when the physical value actually differs.
            if let Some((new_cols, new_rows)) = renderer.set_content_inset_right(current_width) {
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let width_px = (new_cols as f32 * cell_width) as usize;
                let height_px = (new_rows as f32 * cell_height) as usize;

                for tab in self.tab_manager.tabs_mut() {
                    if let Ok(mut term) = tab.terminal.try_lock() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                    }
                    tab.cache.cells = None;
                }

                crate::debug_info!(
                    "AI_INSPECTOR",
                    "Panel width synced to {:.0}px, resized terminals to {}x{}",
                    current_width,
                    new_cols,
                    new_rows
                );
                self.focus_state.needs_redraw = true;
            } else if (current_width - self.overlay_ui.last_inspector_width).abs() >= 1.0 {
                // Logical width changed but physical grid didn't resize
                // (could happen with very small changes below cell width threshold)
                self.focus_state.needs_redraw = true;
            }
        }

        // Persist panel width to config when the user finishes resizing.
        if !self.overlay_ui.ai_inspector.is_resizing()
            && (current_width - self.overlay_ui.last_inspector_width).abs() >= 1.0
            && current_width > 0.0
            && self.overlay_ui.ai_inspector.open
        {
            self.config.ai_inspector_width = self.overlay_ui.ai_inspector.width;
            // Save to disk so the width is remembered across sessions.
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save AI inspector width: {}", e);
            }
        }

        self.overlay_ui.last_inspector_width = current_width;
    }

    // ========================================================================
    // Status Bar Inset Sync
    // ========================================================================

    /// Sync the status bar bottom inset with the renderer so that the terminal
    /// grid does not extend behind the status bar.
    ///
    /// Must be called before cells are gathered each frame so the grid size
    /// is correct. Only triggers a terminal resize when the height changes
    /// (e.g., status bar toggled on/off or height changed in settings).
    pub(crate) fn sync_status_bar_inset(&mut self) {
        let is_tmux = self.is_tmux_connected();
        let tmux_bar = crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux);
        let custom_bar = self.status_bar_ui.height(&self.config, self.is_fullscreen);
        let total = tmux_bar + custom_bar;

        if let Some(renderer) = &mut self.renderer
            && let Some((new_cols, new_rows)) = renderer.set_egui_bottom_inset(total)
        {
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (new_cols as f32 * cell_width) as usize;
            let height_px = (new_rows as f32 * cell_height) as usize;

            for tab in self.tab_manager.tabs_mut() {
                if let Ok(mut term) = tab.terminal.try_lock() {
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                }
                tab.cache.cells = None;
            }
        }
    }

    // ========================================================================
    // Cursor Blink
    // ========================================================================

    /// Update cursor blink state based on configured interval and DECSCUSR style
    ///
    /// The cursor blink state is determined by:
    /// 1. If lock_cursor_style is enabled: use config.cursor_blink
    /// 2. If lock_cursor_blink is enabled and cursor_blink is false: force no blink
    /// 3. Otherwise: terminal's cursor style (set via DECSCUSR escape sequence)
    /// 4. Fallback: user's config setting (cursor_blink)
    ///
    /// DECSCUSR values: odd = blinking, even = steady
    /// - 0/1: Blinking block (default)
    /// - 2: Steady block
    /// - 3: Blinking underline
    /// - 4: Steady underline
    /// - 5: Blinking bar
    /// - 6: Steady bar
    pub(crate) fn update_cursor_blink(&mut self) {
        // If cursor style is locked, use the config's blink setting directly
        if self.config.lock_cursor_style {
            if !self.config.cursor_blink {
                self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
                return;
            }
        } else if self.config.lock_cursor_blink && !self.config.cursor_blink {
            // If blink is locked off, don't blink regardless of terminal style
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
            return;
        }

        // Get cursor style from terminal to check if DECSCUSR specified blinking
        let cursor_should_blink = if self.config.lock_cursor_style {
            // Style is locked, use config's blink setting
            self.config.cursor_blink
        } else if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
        {
            use par_term_emu_core_rust::cursor::CursorStyle;
            let style = term.cursor_style();
            // DECSCUSR: odd values (1,3,5) = blinking, even values (2,4,6) = steady
            matches!(
                style,
                CursorStyle::BlinkingBlock
                    | CursorStyle::BlinkingUnderline
                    | CursorStyle::BlinkingBar
            )
        } else {
            // Fallback to config setting if terminal lock unavailable
            self.config.cursor_blink
        };

        if !cursor_should_blink {
            // Smoothly fade to full visibility if blinking disabled (by DECSCUSR or config)
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
            return;
        }

        let now = std::time::Instant::now();

        // If key was pressed recently (within 500ms), smoothly fade in cursor and reset blink timer
        if let Some(last_key) = self.cursor_anim.last_key_press
            && now.duration_since(last_key).as_millis() < 500
        {
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
            self.cursor_anim.last_cursor_blink = Some(now);
            return;
        }

        // Smooth cursor blink animation using sine wave for natural fade
        let blink_interval = std::time::Duration::from_millis(self.config.cursor_blink_interval);

        if let Some(last_blink) = self.cursor_anim.last_cursor_blink {
            let elapsed = now.duration_since(last_blink);
            let progress = (elapsed.as_millis() as f32) / (blink_interval.as_millis() as f32);

            // Use cosine wave for smooth fade in/out (starts at 1.0, fades to 0.0, back to 1.0)
            self.cursor_anim.cursor_opacity = ((progress * std::f32::consts::PI).cos())
                .abs()
                .clamp(0.0, 1.0);

            // Reset timer after full cycle (2x interval for full on+off)
            if elapsed >= blink_interval * 2 {
                self.cursor_anim.last_cursor_blink = Some(now);
            }
        } else {
            // First time, start the blink timer with cursor fully visible
            self.cursor_anim.cursor_opacity = 1.0;
            self.cursor_anim.last_cursor_blink = Some(now);
        }
    }
}
