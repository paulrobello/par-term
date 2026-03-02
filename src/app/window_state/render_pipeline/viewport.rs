//! Viewport and sizing helpers for the render pipeline.
//!
//! This module provides helpers that operate on renderer/viewport state
//! rather than on terminal content:
//! - `resolve_cursor_shader_hide`: resolves whether the cursor shader should
//!   hide the default cursor, consulting per-shader overrides, metadata
//!   defaults, and global config.
//! - `gather_viewport_sizing`: extracts the renderer's grid size and physical
//!   pixel dimensions needed at the start of every gather pass.

use crate::app::window_state::WindowState;
use winit::dpi::PhysicalSize;

impl WindowState {
    /// Extract renderer grid dimensions and physical size.
    ///
    /// Returns `None` when no renderer is attached (rendering must be skipped).
    /// Returns `(renderer_size, visible_lines, grid_cols)` otherwise.
    pub(super) fn gather_viewport_sizing(&self) -> Option<(PhysicalSize<u32>, usize, usize)> {
        let renderer = self.renderer.as_ref()?;
        let (cols, rows) = renderer.grid_size();
        Some((renderer.size(), rows, cols))
    }

    /// Resolve whether the cursor shader should hide the default terminal cursor.
    ///
    /// Precedence: per-shader config override → shader metadata defaults → global config.
    /// Returns `true` when the cursor should be hidden for the current shader.
    ///
    /// Requires `&mut self` because `cursor_shader_metadata_cache` is a lazy-loading
    /// cache whose `.get()` method may read from disk on first access.
    pub(super) fn resolve_cursor_shader_hide(&mut self, is_alt_screen: bool) -> bool {
        // Clone the shader name to an owned String so that the shared borrow on
        // `self.config` is released before the mutable borrow on
        // `self.shader_state.shader.cursor_shader_metadata_cache` in the closures below.
        let cursor_shader_name: Option<String> = self.config.shader.cursor_shader.clone();

        // hides_cursor: per-shader config override -> metadata defaults -> global config
        let hides_cursor_from_config = cursor_shader_name
            .as_deref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|cfg| cfg.hides_cursor);

        let resolved_hides_cursor = hides_cursor_from_config
            .or_else(|| {
                cursor_shader_name
                    .as_deref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.hides_cursor)
            })
            .unwrap_or(self.config.shader.cursor_shader_hides_cursor);

        // disable_in_alt_screen: per-shader override -> metadata defaults -> global config
        let disable_in_alt_screen_from_config = cursor_shader_name
            .as_deref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|cfg| cfg.disable_in_alt_screen);

        let resolved_disable_in_alt_screen = disable_in_alt_screen_from_config
            .or_else(|| {
                cursor_shader_name
                    .as_deref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.disable_in_alt_screen)
            })
            .unwrap_or(self.config.shader.cursor_shader_disable_in_alt_screen);

        self.config.shader.cursor_shader_enabled
            && resolved_hides_cursor
            && !(resolved_disable_in_alt_screen && is_alt_screen)
    }
}
