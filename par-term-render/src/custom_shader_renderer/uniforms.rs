//! Uniform buffer management for custom shader renderer.
//!
//! Provides creation of the GPU uniform buffer and the logic to build
//! `CustomShaderUniforms` values from the current renderer state, ready
//! to be written to the GPU each frame.

use wgpu::*;

use super::CustomShaderRenderer;
use super::types::CustomShaderUniforms;

impl CustomShaderRenderer {
    /// Create the GPU uniform buffer for shader parameters.
    pub(super) fn create_uniform_buffer(device: &Device) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Custom Shader Uniforms"),
            size: std::mem::size_of::<CustomShaderUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Build the uniform buffer data from current renderer state.
    pub(super) fn build_uniforms(
        &self,
        time: f32,
        time_delta: f32,
        apply_opacity: bool,
    ) -> CustomShaderUniforms {
        // Calculate iMouse uniform
        let height = self.texture_height as f32;
        let mouse_y_flipped = height - self.mouse_position[1];
        let click_y_flipped = height - self.mouse_click_position[1];

        let mouse = if self.mouse_button_down {
            [
                self.mouse_position[0],
                mouse_y_flipped,
                self.mouse_click_position[0],
                click_y_flipped,
            ]
        } else {
            [
                self.mouse_position[0],
                mouse_y_flipped,
                -self.mouse_click_position[0].abs(),
                -click_y_flipped.abs(),
            ]
        };

        // Calculate iDate uniform
        let date = Self::calculate_date();

        // Calculate cursor pixel positions
        let (curr_x, curr_y) =
            self.cursor_to_pixels(self.current_cursor_pos.0, self.current_cursor_pos.1);
        let (prev_x, prev_y) =
            self.cursor_to_pixels(self.previous_cursor_pos.0, self.previous_cursor_pos.1);

        // When rendering to intermediate texture (for further shader processing),
        // use 0.0 to signal "chain mode" to the shader. This tells the shader to:
        // - Use full background color for RGB (not premultiplied by opacity)
        // - Output terminal-only alpha (so next shader can detect transparent areas)
        // The final shader in the chain will apply actual window opacity.
        let effective_opacity = if apply_opacity {
            self.window_opacity
        } else {
            0.0 // Chain mode: shader detects this and preserves transparency info
        };

        // Resolution stays at full texture size for correct UV sampling
        // The viewport (set in render) limits where output appears
        CustomShaderUniforms {
            resolution: [self.texture_width as f32, self.texture_height as f32],
            time,
            time_delta,
            mouse,
            date,
            opacity: effective_opacity,
            // When keep_text_opaque is true, text stays at full opacity (1.0)
            // When false, text uses the same opacity as the window background
            text_opacity: if self.keep_text_opaque || !apply_opacity {
                1.0
            } else {
                self.window_opacity
            },
            full_content_mode: if self.full_content_mode { 1.0 } else { 0.0 },
            frame: self.frame_count as f32,
            frame_rate: self.current_frame_rate,
            resolution_z: 1.0,
            brightness: self.brightness,
            key_press_time: self.key_press_time,
            current_cursor: [
                curr_x,
                curr_y,
                self.cursor_width_for_style(self.current_cursor_style, self.scale_factor),
                self.cursor_height_for_style(self.current_cursor_style, self.scale_factor),
            ],
            previous_cursor: [
                prev_x,
                prev_y,
                self.cursor_width_for_style(self.previous_cursor_style, self.scale_factor),
                self.cursor_height_for_style(self.previous_cursor_style, self.scale_factor),
            ],
            current_cursor_color: [
                self.current_cursor_color[0],
                self.current_cursor_color[1],
                self.current_cursor_color[2],
                self.current_cursor_color[3] * self.current_cursor_opacity,
            ],
            previous_cursor_color: [
                self.previous_cursor_color[0],
                self.previous_cursor_color[1],
                self.previous_cursor_color[2],
                self.previous_cursor_color[3] * self.previous_cursor_opacity,
            ],
            cursor_change_time: self.cursor_change_time,
            cursor_trail_duration: self.cursor_trail_duration,
            cursor_glow_radius: self.cursor_glow_radius,
            cursor_glow_intensity: self.cursor_glow_intensity,
            cursor_shader_color: self.cursor_shader_color,
            channel0_resolution: self.effective_channel0_resolution(),
            channel1_resolution: self.channel_textures[1].resolution(),
            channel2_resolution: self.channel_textures[2].resolution(),
            channel3_resolution: self.channel_textures[3].resolution(),
            channel4_resolution: [
                self.texture_width as f32,
                self.texture_height as f32,
                1.0,
                0.0,
            ],
            cubemap_resolution: self.cubemap.resolution(),
            background_color: self.background_color,
            progress: self.progress_data,
        }
    }

    /// Calculate the iDate uniform value.
    ///
    /// Returns `[year, month (0-11), day (1-31), seconds_since_midnight]`.
    pub(super) fn calculate_date() -> [f32; 4] {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_sys = SystemTime::now();
        let since_epoch = now_sys.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = since_epoch.as_secs();

        let days_since_epoch = secs / 86400;
        let secs_today = (secs % 86400) as f32;

        let mut year = 1970i32;
        let mut remaining_days = days_since_epoch as i32;

        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_months: [i32; 12] = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 0i32;
        for (i, &days) in days_in_months.iter().enumerate() {
            if remaining_days < days {
                month = i as i32;
                break;
            }
            remaining_days -= days;
        }

        let day = remaining_days + 1;
        [year as f32, month as f32, day as f32, secs_today]
    }
}
