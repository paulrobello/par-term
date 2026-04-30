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

    /// Apply current background shader config to the live renderer.
    pub(crate) fn refresh_background_shader_renderer(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            let metadata = self
                .config
                .shader
                .custom_shader
                .as_ref()
                .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
            let shader_override = self
                .config
                .shader
                .custom_shader
                .as_ref()
                .and_then(|name| self.config.shader_configs.get(name).cloned());
            let mut resolved =
                resolve_shader_config(shader_override.as_ref(), metadata.as_ref(), &self.config);
            if self.config.shader.custom_shader_readability_mode {
                resolved.brightness = resolved
                    .brightness
                    .min(self.config.shader.custom_shader_readability_brightness);
            }

            let _ = renderer.set_custom_shader_enabled(
                par_term_render::renderer::shaders::CustomShaderEnableParams {
                    enabled: self.config.shader.custom_shader_enabled,
                    shader_path: self.config.shader.custom_shader.as_deref(),
                    window_opacity: self.config.window.window_opacity,
                    animation_enabled: self.config.shader.custom_shader_animation
                        && !self.config.shader.custom_shader_readability_mode,
                    animation_speed: resolved.animation_speed,
                    full_content: resolved.full_content,
                    brightness: resolved.brightness,
                    channel_paths: &resolved.channel_paths(),
                    cubemap_path: resolved.cubemap_path().map(|p| p.as_path()),
                    custom_uniforms: &resolved.custom_uniforms,
                    background_channel0_blend_mode: resolved.background_channel0_blend_mode,
                    auto_dim_under_text: resolved.auto_dim_under_text,
                    auto_dim_strength: resolved.auto_dim_strength,
                },
            );
        }
    }

    /// Toggle the background/custom shader on/off.
    pub(crate) fn toggle_background_shader(&mut self) {
        self.config.shader.custom_shader_enabled = !self.config.shader.custom_shader_enabled;
        self.refresh_background_shader_renderer();

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

    /// Cycle to the next available background shader.
    pub(crate) fn cycle_background_shader(&mut self) {
        let mut shaders = Vec::new();
        if let Ok(entries) = std::fs::read_dir(crate::config::Config::shaders_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("glsl")
                    && let Some(name) = path.file_name().and_then(|name| name.to_str())
                    && !name.starts_with("cursor_")
                {
                    shaders.push(name.to_string());
                }
            }
        }
        shaders.sort();
        if shaders.is_empty() {
            self.show_toast("No background shaders found");
            return;
        }

        let next_index = self
            .config
            .shader
            .custom_shader
            .as_ref()
            .and_then(|current| shaders.iter().position(|shader| shader == current))
            .map(|index| (index + 1) % shaders.len())
            .unwrap_or(0);
        self.config.shader.custom_shader = Some(shaders[next_index].clone());
        self.config.shader.custom_shader_enabled = true;
        self.refresh_background_shader_renderer();
        self.show_toast(format!("Shader: {}", shaders[next_index]));
    }

    /// Pause/resume background shader animation.
    pub(crate) fn toggle_shader_animation(&mut self) {
        self.config.shader.custom_shader_animation = !self.config.shader.custom_shader_animation;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_custom_shader_animation(self.config.shader.custom_shader_animation);
        }
        self.show_toast(if self.config.shader.custom_shader_animation {
            "Shader animation resumed"
        } else {
            "Shader animation paused"
        });
    }

    /// Toggle low-power/readability shader mode.
    pub(crate) fn toggle_shader_readability_mode(&mut self) {
        self.config.shader.custom_shader_readability_mode =
            !self.config.shader.custom_shader_readability_mode;
        self.refresh_background_shader_renderer();
        self.show_toast(if self.config.shader.custom_shader_readability_mode {
            "Shader readability mode on"
        } else {
            "Shader readability mode off"
        });
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
