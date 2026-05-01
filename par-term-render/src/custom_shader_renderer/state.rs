//! Public state mutation methods for [`CustomShaderRenderer`].
//!
//! Collects all setter/getter methods that update renderer state without
//! touching the GPU pipeline. These are called from the render loop and
//! settings UI to configure shader behavior at runtime.

use std::collections::BTreeMap;
use std::time::Instant;

use super::CustomShaderRenderer;
use super::cubemap::CubemapTexture;
use super::textures::ChannelTexture;
use anyhow::Result;
use wgpu::*;

impl CustomShaderRenderer {
    // ---- Animation ----

    /// Check if animation is enabled
    pub fn animation_enabled(&self) -> bool {
        self.animation_enabled
    }

    /// Set animation enabled state
    pub fn set_animation_enabled(&mut self, enabled: bool) {
        let now = Instant::now();
        self.start_time = super::animation_start_after_enabled_update(
            self.animation_enabled,
            enabled,
            self.start_time,
            now,
        );
        self.animation_enabled = enabled;
    }

    /// Update animation speed multiplier
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.animation_speed = speed.max(0.0);
    }

    // ---- Window / display ----

    /// Update window opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.window_opacity = opacity.clamp(0.0, 1.0);
    }

    /// Update shader brightness multiplier
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.05, 1.0);
    }

    /// Update automatic dimming beneath terminal text/content.
    pub fn set_auto_dim_under_text(&mut self, enabled: bool, strength: f32) {
        self.auto_dim_under_text = enabled;
        self.auto_dim_strength = strength.clamp(0.0, 1.0);
    }

    /// Update full content mode
    pub fn set_full_content_mode(&mut self, enabled: bool) {
        self.full_content_mode = enabled;
    }

    /// Check if full content mode is enabled
    pub fn full_content_mode(&self) -> bool {
        self.full_content_mode
    }

    /// Set whether text should always be rendered at full opacity
    /// When true, overrides text_opacity to 1.0
    pub fn set_keep_text_opaque(&mut self, keep_opaque: bool) {
        self.keep_text_opaque = keep_opaque;
    }

    // ---- Mouse ----

    /// Update mouse position in pixel coordinates
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = [x, y];
    }

    /// Update mouse button state and click position
    pub fn set_mouse_button(&mut self, pressed: bool, x: f32, y: f32) {
        self.mouse_button_down = pressed;
        if pressed {
            self.mouse_click_position = [x, y];
        }
    }

    // ---- Key press ----

    /// Update key press time for shader effects
    ///
    /// Call this when a key is pressed to enable key-press-based shader effects
    /// like screen pulses or typing animations.
    pub fn update_key_press(&mut self) {
        self.key_press_time = if self.animation_enabled {
            self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
        } else {
            0.0
        };
        log::trace!("Key pressed at shader time={:.3}", self.key_press_time);
    }

    // ---- Channel textures ----

    /// Update a channel texture at runtime
    pub fn update_channel_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        channel: u8,
        path: Option<&std::path::Path>,
    ) -> Result<()> {
        if !(1..=4).contains(&channel) {
            anyhow::bail!("Invalid channel index: {} (must be 1-4)", channel);
        }

        let index = (channel - 1) as usize;

        let new_texture = match path {
            Some(p) => ChannelTexture::from_file(device, queue, p)?,
            None => ChannelTexture::placeholder(device, queue),
        };

        self.channel_textures[index] = new_texture;
        // Use recreate_bind_group to properly handle use_background_as_channel0 logic
        self.recreate_bind_group(device);

        log::info!(
            "Updated iChannel{} texture: {}",
            channel,
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| "placeholder".to_string())
        );

        Ok(())
    }

    // ---- Cubemap ----

    /// Update the cubemap texture at runtime
    pub fn update_cubemap(
        &mut self,
        device: &Device,
        queue: &Queue,
        path: Option<&std::path::Path>,
    ) -> Result<()> {
        let new_cubemap = match path {
            Some(p) => CubemapTexture::from_prefix(device, queue, p)?,
            None => CubemapTexture::placeholder(device, queue),
        };

        self.cubemap = new_cubemap;

        // Use recreate_bind_group to properly handle use_background_as_channel0 logic
        self.recreate_bind_group(device);

        log::info!(
            "Updated cubemap texture: {}",
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| "placeholder".to_string())
        );

        Ok(())
    }

    // ---- Background as iChannel0 ----

    /// Set whether to use the background image as iChannel0.
    ///
    /// When enabled and a background texture is set, the background image will be
    /// used as iChannel0 instead of the configured channel0 texture file.
    ///
    /// Note: This only updates the flag. Use `update_use_background_as_channel0`
    /// if you also need to recreate the bind group.
    pub fn set_use_background_as_channel0(&mut self, use_background: bool) {
        if self.use_background_as_channel0 != use_background {
            self.use_background_as_channel0 = use_background;
            log::info!("use_background_as_channel0 set to {}", use_background);
        }
    }

    /// Check if using background image as iChannel0.
    pub fn use_background_as_channel0(&self) -> bool {
        self.use_background_as_channel0
    }

    /// Set the background texture to use as iChannel0 when enabled.
    ///
    /// Call this whenever the background image changes to update the shader's
    /// channel0 binding. The device parameter is needed to recreate the bind group.
    ///
    /// When use_background_as_channel0 is enabled, the background texture takes
    /// priority over any configured channel0 texture.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `texture` - The background texture (view, sampler, dimensions), or None to clear
    pub fn set_background_texture(&mut self, device: &Device, texture: Option<ChannelTexture>) {
        self.background_channel_texture = texture;

        // Recreate bind group if we're using background as channel0
        // The background texture takes priority over configured channel0 when enabled
        if self.use_background_as_channel0 {
            self.recreate_bind_group(device);
        }
    }

    /// Set the solid background color for shader compositing.
    ///
    /// When set (alpha > 0), the shader uses this color as background instead of shader output.
    /// This allows solid background colors to show through properly with window transparency.
    ///
    /// # Arguments
    /// * `color` - RGB color values [R, G, B] (0.0-1.0, NOT premultiplied)
    /// * `active` - Whether solid color mode is active (sets alpha to 1.0 or 0.0)
    pub fn set_background_color(&mut self, color: [f32; 3], active: bool) {
        self.background_color = [color[0], color[1], color[2], if active { 1.0 } else { 0.0 }];
    }

    /// Update the use_background_as_channel0 setting and recreate bind group if needed.
    ///
    /// Call this when the setting changes in the UI or config.
    pub fn update_use_background_as_channel0(&mut self, device: &Device, use_background: bool) {
        if self.use_background_as_channel0 != use_background {
            self.use_background_as_channel0 = use_background;
            self.recreate_bind_group(device);
            log::info!("use_background_as_channel0 toggled to {}", use_background);
        }
    }

    /// Update the background channel blend-mode hint exposed to shaders.
    pub fn set_background_channel0_blend_mode(
        &mut self,
        mode: par_term_config::ShaderBackgroundBlendMode,
    ) {
        self.background_channel0_blend_mode = mode;
    }

    // ---- Progress / command / scroll / pane state ----

    /// Update progress bar state for shader effects.
    ///
    /// # Arguments
    /// * `state` - Progress state (0=hidden, 1=normal, 2=error, 3=indeterminate, 4=warning)
    /// * `percent` - Progress percentage as 0.0-1.0
    /// * `is_active` - 1.0 if any progress bar is active, 0.0 otherwise
    /// * `active_count` - Total count of active bars (simple + named)
    pub fn update_progress(&mut self, state: f32, percent: f32, is_active: f32, active_count: f32) {
        self.progress_data = [state, percent, is_active, active_count];
    }

    /// Update command lifecycle state for shader effects.
    ///
    /// `state`: 0=unknown, 1=running, 2=success, 3=failure.
    /// `exit_code`: last exit code, or 0 when unknown/running.
    /// `running`: 1 when a command is currently running, otherwise 0.
    pub fn update_command_status(&mut self, state: f32, exit_code: f32, running: f32) {
        let state = state.clamp(0.0, 3.0);
        let exit_code = if exit_code.is_finite() {
            exit_code
        } else {
            0.0
        };
        let running = if running > 0.5 { 1.0 } else { 0.0 };
        let changed = (self.command_data[0] - state).abs() > f32::EPSILON
            || (self.command_data[1] - exit_code).abs() > f32::EPSILON
            || (self.command_data[3] - running).abs() > f32::EPSILON;

        if changed {
            let event_time = if self.animation_enabled {
                self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
            } else {
                0.0
            };
            self.command_data = [state, exit_code, event_time, running];
        }
    }

    /// Update focused pane bounds in bottom-left-origin pixels.
    pub fn update_focused_pane(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.focused_pane = [x.max(0.0), y.max(0.0), width.max(0.0), height.max(0.0)];
    }

    /// Update scrollback context for shader effects.
    pub fn update_scrollback(&mut self, offset: f32, visible_lines: f32, scrollback_lines: f32) {
        let offset = offset.max(0.0);
        let scrollback_lines = scrollback_lines.max(0.0);
        let normalized = if scrollback_lines > 0.0 {
            (offset / scrollback_lines).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.scroll_data = [offset, visible_lines.max(0.0), scrollback_lines, normalized];
    }

    // ---- Content insets ----

    /// Set the right content inset (e.g., AI Inspector panel).
    ///
    /// When non-zero, the shader will render to a viewport that excludes
    /// the right inset area, ensuring effects don't appear under the panel.
    pub fn set_content_inset_right(&mut self, inset: f32) {
        self.content_inset_right = inset;
    }

    // ---- Custom controls ----

    /// Update custom shader uniform values keyed by control name.
    pub fn set_custom_uniform_values(
        &mut self,
        values: BTreeMap<String, par_term_config::ShaderUniformValue>,
    ) {
        self.custom_uniform_values = values;
    }
}
