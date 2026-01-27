//! Config change detection for WindowState.
//!
//! This module provides the `ConfigChanges` struct for detecting what changed
//! between two configurations, eliminating the need for 21+ individual boolean variables.

use crate::config::Config;

/// Tracks which config fields changed between old and new config
/// This replaces 21+ individual boolean variables with a structured approach
#[derive(Default)]
pub(crate) struct ConfigChanges {
    // Theme
    pub theme: bool,

    // Background shader
    pub shader_animation: bool,
    pub shader_enabled: bool,
    pub shader_path: bool,
    pub shader_speed: bool,
    pub shader_full_content: bool,
    pub shader_text_opacity: bool,
    pub shader_brightness: bool,
    pub shader_textures: bool,
    pub shader_cubemap: bool,
    pub shader_per_shader_config: bool,

    // Cursor shader
    pub cursor_shader_config: bool,
    pub cursor_shader_path: bool,
    pub cursor_shader_enabled: bool,
    pub cursor_shader_animation: bool,
    pub cursor_shader_speed: bool,
    pub cursor_shader_hides_cursor: bool,
    pub cursor_shader_disable_in_alt_screen: bool,

    // Window
    pub window_title: bool,
    pub window_decorations: bool,
    pub max_fps: bool,

    // Cursor appearance
    pub cursor_style: bool,
    pub cursor_blink: bool,
    pub cursor_color: bool,

    // Background image
    pub bg_enabled: bool,
    pub bg_path: bool,
    pub bg_mode: bool,
    pub bg_opacity: bool,

    // Font/spacing (requires rebuild)
    pub font: bool,
    pub padding: bool,

    // Shader hot reload
    pub shader_hot_reload: bool,
    pub shader_hot_reload_delay: bool,
}

impl ConfigChanges {
    /// Compare two configs and detect what changed
    pub fn detect(old: &Config, new: &Config) -> Self {
        Self {
            theme: new.theme != old.theme,

            shader_animation: new.custom_shader_animation != old.custom_shader_animation,
            shader_enabled: new.custom_shader_enabled != old.custom_shader_enabled,
            shader_path: new.custom_shader != old.custom_shader,
            shader_speed: (new.custom_shader_animation_speed - old.custom_shader_animation_speed)
                .abs()
                > f32::EPSILON,
            shader_full_content: new.custom_shader_full_content != old.custom_shader_full_content,
            shader_text_opacity: (new.custom_shader_text_opacity - old.custom_shader_text_opacity)
                .abs()
                > f32::EPSILON,
            shader_brightness: (new.custom_shader_brightness - old.custom_shader_brightness).abs()
                > f32::EPSILON,
            shader_textures: new.custom_shader_channel0 != old.custom_shader_channel0
                || new.custom_shader_channel1 != old.custom_shader_channel1
                || new.custom_shader_channel2 != old.custom_shader_channel2
                || new.custom_shader_channel3 != old.custom_shader_channel3,
            shader_cubemap: new.custom_shader_cubemap != old.custom_shader_cubemap
                || new.custom_shader_cubemap_enabled != old.custom_shader_cubemap_enabled,
            shader_per_shader_config: {
                // Check if the per-shader config for the current shader changed
                let old_override = old.custom_shader.as_ref()
                    .and_then(|name| old.shader_configs.get(name));
                let new_override = new.custom_shader.as_ref()
                    .and_then(|name| new.shader_configs.get(name));
                old_override != new_override
            },

            cursor_shader_config: new.cursor_shader_color != old.cursor_shader_color
                || (new.cursor_shader_trail_duration - old.cursor_shader_trail_duration).abs()
                    > f32::EPSILON
                || (new.cursor_shader_glow_radius - old.cursor_shader_glow_radius).abs()
                    > f32::EPSILON
                || (new.cursor_shader_glow_intensity - old.cursor_shader_glow_intensity).abs()
                    > f32::EPSILON,
            cursor_shader_path: new.cursor_shader != old.cursor_shader,
            cursor_shader_enabled: new.cursor_shader_enabled != old.cursor_shader_enabled,
            cursor_shader_animation: new.cursor_shader_animation != old.cursor_shader_animation,
            cursor_shader_speed: (new.cursor_shader_animation_speed
                - old.cursor_shader_animation_speed)
                .abs()
                > f32::EPSILON,
            cursor_shader_hides_cursor: new.cursor_shader_hides_cursor
                != old.cursor_shader_hides_cursor,
            cursor_shader_disable_in_alt_screen: new.cursor_shader_disable_in_alt_screen
                != old.cursor_shader_disable_in_alt_screen,

            window_title: new.window_title != old.window_title,
            window_decorations: new.window_decorations != old.window_decorations,
            max_fps: new.max_fps != old.max_fps,

            cursor_style: new.cursor_style != old.cursor_style,
            cursor_blink: new.cursor_blink != old.cursor_blink,
            cursor_color: new.cursor_color != old.cursor_color,

            bg_enabled: new.background_image_enabled != old.background_image_enabled,
            bg_path: new.background_image != old.background_image,
            bg_mode: new.background_image_mode != old.background_image_mode,
            bg_opacity: (new.background_image_opacity - old.background_image_opacity).abs()
                > f32::EPSILON,

            font: new.font_family != old.font_family
                || new.font_family_bold != old.font_family_bold
                || new.font_family_italic != old.font_family_italic
                || new.font_family_bold_italic != old.font_family_bold_italic
                || (new.font_size - old.font_size).abs() > f32::EPSILON
                || (new.line_spacing - old.line_spacing).abs() > f32::EPSILON
                || (new.char_spacing - old.char_spacing).abs() > f32::EPSILON,
            padding: (new.window_padding - old.window_padding).abs() > f32::EPSILON,

            shader_hot_reload: new.shader_hot_reload != old.shader_hot_reload,
            shader_hot_reload_delay: new.shader_hot_reload_delay != old.shader_hot_reload_delay,
        }
    }

    /// Returns true if any shader-related setting changed
    pub fn any_shader_change(&self) -> bool {
        self.shader_animation
            || self.shader_enabled
            || self.shader_path
            || self.shader_speed
            || self.shader_full_content
            || self.shader_text_opacity
            || self.shader_brightness
            || self.shader_textures
            || self.shader_cubemap
            || self.shader_per_shader_config
    }

    /// Returns true if any cursor shader path/enabled/animation changed
    pub fn any_cursor_shader_toggle(&self) -> bool {
        self.cursor_shader_path
            || self.cursor_shader_enabled
            || self.cursor_shader_animation
            || self.cursor_shader_speed
            || self.cursor_shader_disable_in_alt_screen
    }

    /// Returns true if any background image setting changed
    pub fn any_bg_change(&self) -> bool {
        self.bg_enabled || self.bg_path || self.bg_mode || self.bg_opacity
    }

    /// Returns true if shader watcher needs to be reinitialized
    pub fn needs_watcher_reinit(&self) -> bool {
        self.shader_hot_reload
            || self.shader_hot_reload_delay
            || self.shader_path
            || self.cursor_shader_path
            || self.shader_enabled
            || self.cursor_shader_enabled
    }
}
