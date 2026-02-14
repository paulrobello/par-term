//! Config change detection for WindowState.
//!
//! This module provides the `ConfigChanges` struct for detecting what changed
//! between two configurations, eliminating the need for 21+ individual boolean variables.

use crate::config::Config;

/// Tracks which config fields changed between old and new config
/// This replaces 21+ individual boolean variables with a structured approach
#[derive(Default)]
#[allow(dead_code)] // Fields reserved for future use
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
    pub shader_use_background_as_channel0: bool,

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
    pub window_type: bool,
    pub target_monitor: bool,
    pub lock_window_size: bool,
    pub show_window_number: bool,
    pub max_fps: bool,
    pub vsync_mode: bool,

    // Cursor appearance
    pub cursor_style: bool,
    pub cursor_blink: bool,
    pub cursor_color: bool,
    pub cursor_text_color: bool,

    // Cursor enhancements
    pub cursor_enhancements: bool,

    // Terminal identification
    pub answerback_string: bool,

    // Unicode width settings
    pub unicode_width: bool,

    // Unicode normalization form
    pub normalization_form: bool,

    // Anti-idle keep-alive
    pub anti_idle_enabled: bool,
    pub anti_idle_seconds: bool,
    pub anti_idle_code: bool,

    // Background (mode, image, and solid color)
    pub bg_mode: bool,
    pub bg_color: bool,
    pub bg_image_enabled: bool,
    pub bg_image_path: bool,
    pub bg_image_mode: bool,
    pub bg_image_opacity: bool,

    // Inline image settings
    pub image_scaling_mode: bool,
    pub image_preserve_aspect_ratio: bool,

    // Font/spacing (requires rebuild)
    pub font: bool,
    // Font rendering options that can be applied live without full rebuild
    pub font_rendering: bool,
    pub padding: bool,

    // Shader hot reload
    pub shader_hot_reload: bool,
    pub shader_hot_reload_delay: bool,

    // Transparency mode
    pub transparency_mode: bool,
    pub keep_text_opaque: bool,

    // Blur settings (macOS only)
    pub blur: bool,

    // Keybindings
    pub keybindings: bool,

    // Badge
    pub badge: bool,

    // Command separator lines
    pub command_separator: bool,

    // Dynamic profile sources
    pub dynamic_profile_sources: bool,
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
            shader_use_background_as_channel0: new.custom_shader_use_background_as_channel0
                != old.custom_shader_use_background_as_channel0,
            shader_per_shader_config: {
                // Check if the per-shader config for the current shader changed
                let old_override = old
                    .custom_shader
                    .as_ref()
                    .and_then(|name| old.shader_configs.get(name));
                let new_override = new
                    .custom_shader
                    .as_ref()
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
            window_type: new.window_type != old.window_type,
            target_monitor: new.target_monitor != old.target_monitor,
            lock_window_size: new.lock_window_size != old.lock_window_size,
            show_window_number: new.show_window_number != old.show_window_number,
            max_fps: new.max_fps != old.max_fps,
            vsync_mode: new.vsync_mode != old.vsync_mode,

            cursor_style: new.cursor_style != old.cursor_style,
            cursor_blink: new.cursor_blink != old.cursor_blink,
            cursor_color: new.cursor_color != old.cursor_color,
            cursor_text_color: new.cursor_text_color != old.cursor_text_color,

            cursor_enhancements: new.cursor_guide_enabled != old.cursor_guide_enabled
                || new.cursor_guide_color != old.cursor_guide_color
                || new.cursor_shadow_enabled != old.cursor_shadow_enabled
                || new.cursor_shadow_color != old.cursor_shadow_color
                || new.cursor_shadow_offset != old.cursor_shadow_offset
                || (new.cursor_shadow_blur - old.cursor_shadow_blur).abs() > f32::EPSILON
                || (new.cursor_boost - old.cursor_boost).abs() > f32::EPSILON
                || new.cursor_boost_color != old.cursor_boost_color
                || new.unfocused_cursor_style != old.unfocused_cursor_style,

            answerback_string: new.answerback_string != old.answerback_string,

            unicode_width: new.unicode_version != old.unicode_version
                || new.ambiguous_width != old.ambiguous_width,

            normalization_form: new.normalization_form != old.normalization_form,

            anti_idle_enabled: new.anti_idle_enabled != old.anti_idle_enabled,
            anti_idle_seconds: new.anti_idle_seconds != old.anti_idle_seconds,
            anti_idle_code: new.anti_idle_code != old.anti_idle_code,

            bg_mode: new.background_mode != old.background_mode,
            bg_color: new.background_color != old.background_color,
            bg_image_enabled: new.background_image_enabled != old.background_image_enabled,
            bg_image_path: new.background_image != old.background_image,
            bg_image_mode: new.background_image_mode != old.background_image_mode,
            bg_image_opacity: (new.background_image_opacity - old.background_image_opacity).abs()
                > f32::EPSILON,

            image_scaling_mode: new.image_scaling_mode != old.image_scaling_mode,
            image_preserve_aspect_ratio: new.image_preserve_aspect_ratio
                != old.image_preserve_aspect_ratio,

            font: new.font_family != old.font_family
                || new.font_family_bold != old.font_family_bold
                || new.font_family_italic != old.font_family_italic
                || new.font_family_bold_italic != old.font_family_bold_italic
                || (new.font_size - old.font_size).abs() > f32::EPSILON
                || (new.line_spacing - old.line_spacing).abs() > f32::EPSILON
                || (new.char_spacing - old.char_spacing).abs() > f32::EPSILON,
            font_rendering: new.font_antialias != old.font_antialias
                || new.font_hinting != old.font_hinting
                || new.font_thin_strokes != old.font_thin_strokes
                || (new.minimum_contrast - old.minimum_contrast).abs() > f32::EPSILON,
            padding: (new.window_padding - old.window_padding).abs() > f32::EPSILON,

            shader_hot_reload: new.shader_hot_reload != old.shader_hot_reload,
            shader_hot_reload_delay: new.shader_hot_reload_delay != old.shader_hot_reload_delay,

            transparency_mode: new.transparency_affects_only_default_background
                != old.transparency_affects_only_default_background,
            keep_text_opaque: new.keep_text_opaque != old.keep_text_opaque,

            blur: new.blur_enabled != old.blur_enabled || new.blur_radius != old.blur_radius,

            keybindings: new.keybindings != old.keybindings,

            badge: new.badge_enabled != old.badge_enabled
                || new.badge_format != old.badge_format
                || new.badge_color != old.badge_color
                || (new.badge_color_alpha - old.badge_color_alpha).abs() > f32::EPSILON
                || new.badge_font != old.badge_font
                || new.badge_font_bold != old.badge_font_bold
                || (new.badge_top_margin - old.badge_top_margin).abs() > f32::EPSILON
                || (new.badge_right_margin - old.badge_right_margin).abs() > f32::EPSILON
                || (new.badge_max_width - old.badge_max_width).abs() > f32::EPSILON
                || (new.badge_max_height - old.badge_max_height).abs() > f32::EPSILON,

            command_separator: new.command_separator_enabled != old.command_separator_enabled
                || (new.command_separator_thickness - old.command_separator_thickness).abs()
                    > f32::EPSILON
                || (new.command_separator_opacity - old.command_separator_opacity).abs()
                    > f32::EPSILON
                || new.command_separator_exit_color != old.command_separator_exit_color
                || new.command_separator_color != old.command_separator_color,

            dynamic_profile_sources: new.dynamic_profile_sources != old.dynamic_profile_sources,
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
            || self.shader_use_background_as_channel0
    }

    /// Returns true if any cursor shader path/enabled/animation changed
    pub fn any_cursor_shader_toggle(&self) -> bool {
        self.cursor_shader_path
            || self.cursor_shader_enabled
            || self.cursor_shader_animation
            || self.cursor_shader_speed
            || self.cursor_shader_disable_in_alt_screen
    }

    /// Returns true if any background setting changed (mode, color, or image)
    pub fn any_bg_change(&self) -> bool {
        self.bg_mode
            || self.bg_color
            || self.bg_image_enabled
            || self.bg_image_path
            || self.bg_image_mode
            || self.bg_image_opacity
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
