//! Shader toggle and visual notification helpers for WindowState keybindings.
//!
//! - `show_toast`, `show_pane_indices`: visual notification helpers
//! - `toggle_background_shader`, `toggle_cursor_shader`: shader toggle helpers

use crate::app::window_state::WindowState;
use crate::config::resolve_shader_config;

impl WindowState {
    /// Show a toast notification with the given message.
    ///
    /// The toast will be displayed for 2 seconds and then automatically hidden.
    pub(crate) fn show_toast(&mut self, message: impl Into<String>) {
        self.overlay_state.toast_message = Some(message.into());
        self.overlay_state.toast_hide_time =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Show pane index overlays for a specified duration.
    pub(crate) fn show_pane_indices(&mut self, duration: std::time::Duration) {
        self.overlay_state.pane_identify_hide_time = Some(std::time::Instant::now() + duration);
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Toggle the background/custom shader on/off.
    pub(crate) fn toggle_background_shader(&mut self) {
        self.config.shader.custom_shader_enabled = !self.config.shader.custom_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            // Get shader metadata from cache for resolution
            let metadata = self
                .config
                .shader
                .custom_shader
                .as_ref()
                .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());

            // Get per-shader overrides
            let shader_override = self
                .config
                .shader
                .custom_shader
                .as_ref()
                .and_then(|name| self.config.shader_configs.get(name).cloned());

            // Resolve config with 3-tier system
            let resolved =
                resolve_shader_config(shader_override.as_ref(), metadata.as_ref(), &self.config);

            let _ = renderer.set_custom_shader_enabled(
                par_term_render::renderer::shaders::CustomShaderEnableParams {
                    enabled: self.config.shader.custom_shader_enabled,
                    shader_path: self.config.shader.custom_shader.as_deref(),
                    window_opacity: self.config.window.window_opacity,
                    animation_enabled: self.config.shader.custom_shader_animation,
                    animation_speed: resolved.animation_speed,
                    full_content: resolved.full_content,
                    brightness: resolved.brightness,
                    channel_paths: &resolved.channel_paths(),
                    cubemap_path: resolved.cubemap_path().map(|p| p.as_path()),
                },
            );
        }

        self.focus_state.needs_redraw = true;
        self.request_redraw();

        log::info!(
            "Background shader {}",
            if self.config.shader.custom_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    /// Toggle the cursor shader on/off.
    pub(crate) fn toggle_cursor_shader(&mut self) {
        self.config.shader.cursor_shader_enabled = !self.config.shader.cursor_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            let _ = renderer.set_cursor_shader_enabled(
                self.config.shader.cursor_shader_enabled,
                self.config.shader.cursor_shader.as_deref(),
                self.config.window.window_opacity,
                self.config.shader.cursor_shader_animation,
                self.config.shader.cursor_shader_animation_speed,
            );
        }

        self.focus_state.needs_redraw = true;
        self.request_redraw();

        log::info!(
            "Cursor shader {}",
            if self.config.shader.cursor_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
}
