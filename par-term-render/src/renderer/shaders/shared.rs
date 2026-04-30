//! Shader methods that coordinate across both background and cursor shaders.
//!
//! These operations apply uniformly to whichever renderers are currently active:
//! mouse input forwarding, key press timing, cursor state, progress bar state,
//! cursor shader config updates, and animation pause/resume.

use super::super::Renderer;

impl Renderer {
    /// Update mouse position for custom shader (iMouse uniform)
    pub fn set_shader_mouse_position(&mut self, x: f32, y: f32) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_mouse_position(x, y);
        }
    }

    /// Update mouse button state for custom shader (iMouse uniform)
    pub fn set_shader_mouse_button(&mut self, pressed: bool, x: f32, y: f32) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_mouse_button(pressed, x, y);
        }
    }

    /// Record a key-press timestamp on both shader renderers (iTime-derived uniforms)
    pub fn update_key_press_time(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_key_press();
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_key_press();
        }
    }

    /// Update cursor state for both shader renderers (Ghostty-compatible cursor uniforms).
    ///
    /// Enables cursor trail effects and other cursor-based animations in both
    /// background and cursor shaders.
    pub fn update_shader_cursor(
        &mut self,
        col: usize,
        row: usize,
        opacity: f32,
        color: [f32; 4],
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cursor(col, row, opacity, color, style);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cursor(col, row, opacity, color, style);
        }
    }

    /// Clear cursor uniforms for shaders when the terminal cursor is hidden or unavailable.
    pub fn clear_shader_cursor(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.clear_cursor();
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.clear_cursor();
        }
    }

    /// Update progress bar state for both shader renderers (iProgress uniform).
    ///
    /// # Arguments
    /// * `state` - Progress state (0=hidden, 1=normal, 2=error, 3=indeterminate, 4=warning)
    /// * `percent` - Progress percentage as 0.0–1.0
    /// * `is_active` - 1.0 if any progress bar is active, 0.0 otherwise
    /// * `active_count` - Total count of active bars (simple + named)
    pub fn update_shader_progress(
        &mut self,
        state: f32,
        percent: f32,
        is_active: f32,
        active_count: f32,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_progress(state, percent, is_active, active_count);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_progress(state, percent, is_active, active_count);
        }
    }

    /// Update command lifecycle state for shader effects (iCommand uniform).
    pub fn update_shader_command_status(&mut self, state: f32, exit_code: f32, running: f32) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_command_status(state, exit_code, running);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_command_status(state, exit_code, running);
        }
    }

    /// Update focused pane bounds for shader effects (iFocusedPane uniform).
    pub fn update_shader_focused_pane(
        &mut self,
        focused_viewport: Option<&crate::cell_renderer::PaneViewport>,
    ) {
        let pane = focused_viewport
            .map(|viewport| {
                [
                    viewport.x,
                    self.size.height as f32 - (viewport.y + viewport.height),
                    viewport.width,
                    viewport.height,
                ]
            })
            .unwrap_or([0.0, 0.0, self.size.width as f32, self.size.height as f32]);

        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_focused_pane(pane[0], pane[1], pane[2], pane[3]);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_focused_pane(pane[0], pane[1], pane[2], pane[3]);
        }
    }

    /// Update scrollback context for shader effects (iScroll uniform).
    pub fn update_shader_scrollback(
        &mut self,
        offset: f32,
        visible_lines: f32,
        scrollback_lines: f32,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_scrollback(offset, visible_lines, scrollback_lines);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_scrollback(offset, visible_lines, scrollback_lines);
        }
    }

    /// Update cursor shader configuration on both renderer instances.
    ///
    /// Glow radius is in logical pixels and will be scaled to physical pixels internally.
    pub fn update_cursor_shader_config(
        &mut self,
        color: [u8; 3],
        trail_duration: f32,
        glow_radius: f32,
        glow_intensity: f32,
    ) {
        let physical_glow_radius = glow_radius * self.cell_renderer.scale_factor;
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cursor_shader_config(
                color,
                trail_duration,
                physical_glow_radius,
                glow_intensity,
            );
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cursor_shader_config(
                color,
                trail_duration,
                physical_glow_radius,
                glow_intensity,
            );
        }
    }

    /// Pause shader animations on all active renderers (e.g., when window loses focus).
    pub fn pause_shader_animations(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(false);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(false);
        }
        log::info!("[SHADER] Shader animations paused");
    }

    /// Resume shader animations on all active renderers (e.g., when window regains focus).
    ///
    /// Only resumes if the user's config has animation enabled for that shader.
    pub fn resume_shader_animations(
        &mut self,
        custom_shader_animation: bool,
        cursor_shader_animation: bool,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(custom_shader_animation);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(cursor_shader_animation);
        }
        self.dirty = true;
        log::info!(
            "[SHADER] Shader animations resumed (custom: {}, cursor: {})",
            custom_shader_animation,
            cursor_shader_animation
        );
    }
}
