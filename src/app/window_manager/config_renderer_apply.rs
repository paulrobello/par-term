//! Per-window renderer config application for `apply_config_to_windows`.
//!
//! Extracted from `config_propagation` to keep that file under 500 lines.
//!
//! `apply_renderer_config` applies all renderer-related settings from a freshly-edited
//! `Config` to a single `WindowState` that already has its renderer initialised.
//! Returns the shader-error results for propagation to the standalone settings window.

use std::sync::Arc;

use crate::app::window_state::WindowState;
use crate::app::window_state::config_updates::ConfigChanges;
use crate::config::{Config, resolve_shader_config};
use par_term_terminal::conversion::{
    to_core_ambiguous_width, to_core_normalization_form, to_core_unicode_version,
};

/// Apply all renderer-related config changes to a single window state.
///
/// Returns `(shader_result, cursor_shader_result)` where each is:
/// - `None`       — no change was attempted (no shader-related change detected)
/// - `Some(None)` — change applied successfully
/// - `Some(Some(err_msg))` — change attempted but failed with this error
pub(super) fn apply_renderer_config(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) -> (Option<Option<String>>, Option<Option<String>>) {
    let renderer = match &mut window_state.renderer {
        Some(r) => r,
        None => return (None, None),
    };

    // Update opacity
    renderer.update_opacity(config.window.window_opacity);

    // Update transparency mode if changed
    if changes.transparency_mode {
        renderer.set_transparency_affects_only_default_background(
            config.transparency_affects_only_default_background,
        );
        window_state.focus_state.needs_redraw = true;
    }

    // Update text opacity mode if changed
    if changes.keep_text_opaque {
        renderer.set_keep_text_opaque(config.keep_text_opaque);
        window_state.focus_state.needs_redraw = true;
    }

    if changes.link_underline_style {
        renderer.set_link_underline_style(config.link_underline_style);
        window_state.focus_state.needs_redraw = true;
    }

    // Update vsync mode if changed
    if changes.vsync_mode {
        let (actual_mode, _changed) = renderer.update_vsync_mode(config.vsync_mode);
        // If the actual mode differs, update config
        if actual_mode != config.vsync_mode {
            window_state.config.vsync_mode = actual_mode;
            log::warn!(
                "Vsync mode {:?} is not supported. Using {:?} instead.",
                config.vsync_mode,
                actual_mode
            );
        }
    }

    // Update scrollbar appearance
    renderer.update_scrollbar_appearance(
        config.scrollbar_width,
        config.scrollbar_thumb_color,
        config.scrollbar_track_color,
    );
    window_state.focus_state.needs_redraw = true;

    // Update cursor color
    if changes.cursor_color {
        renderer.update_cursor_color(config.cursor.cursor_color);
    }

    // Update cursor text color
    if changes.cursor_text_color {
        renderer.update_cursor_text_color(config.cursor.cursor_text_color);
    }

    // Update cursor style and blink for all tabs
    if changes.cursor_style || changes.cursor_blink {
        use crate::config::CursorStyle as ConfigCursorStyle;
        use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

        let term_style = if config.cursor.cursor_blink {
            match config.cursor.cursor_style {
                ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
                ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
                ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
            }
        } else {
            match config.cursor.cursor_style {
                ConfigCursorStyle::Block => TermCursorStyle::SteadyBlock,
                ConfigCursorStyle::Beam => TermCursorStyle::SteadyBar,
                ConfigCursorStyle::Underline => TermCursorStyle::SteadyUnderline,
            }
        };

        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_cursor_style(term_style);
            }
            tab.active_cache_mut().cells = None; // Invalidate cache to redraw cursor
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply cursor enhancement changes
    if changes.cursor_enhancements {
        // Re-borrow renderer (can't hold it across the tabs_mut loop above)
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_cursor_guide(
                config.cursor.cursor_guide_enabled,
                config.cursor.cursor_guide_color,
            );
            renderer.update_cursor_shadow(
                config.cursor.cursor_shadow_enabled,
                config.cursor.cursor_shadow_color,
                config.cursor.cursor_shadow_offset,
                config.cursor.cursor_shadow_blur,
            );
            renderer
                .update_cursor_boost(config.cursor.cursor_boost, config.cursor.cursor_boost_color);
            renderer.update_unfocused_cursor_style(config.cursor.unfocused_cursor_style);
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply command separator changes
    if changes.command_separator {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_command_separator(
                config.command_separator_enabled,
                config.command_separator_thickness,
                config.command_separator_opacity,
                config.command_separator_exit_color,
                config.command_separator_color,
            );
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply background changes (mode, color, or image)
    if changes.any_bg_change() {
        // Expand tilde in path
        let expanded_path = config.background_image.as_ref().map(|p| {
            if let Some(rest) = p.strip_prefix("~/")
                && let Some(home) = dirs::home_dir()
            {
                return home.join(rest).to_string_lossy().to_string();
            }
            p.clone()
        });
        if let Some(renderer) = &mut window_state.renderer {
            renderer.set_background(
                config.background_mode,
                config.background_color,
                expanded_path.as_deref(),
                config.background_image_mode,
                config.background_image_opacity,
                config.background_image_enabled,
            );
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply per-pane background changes to existing panes
    if changes.pane_backgrounds {
        // Pre-load all pane background textures into the renderer cache
        if let Some(renderer) = &mut window_state.renderer {
            for pb_config in &config.pane_backgrounds {
                if let Err(e) = renderer.load_pane_background(&pb_config.image) {
                    log::error!(
                        "Failed to load pane {} background '{}': {}",
                        pb_config.index,
                        pb_config.image,
                        e
                    );
                }
            }
        }

        for tab in window_state.tab_manager.tabs_mut() {
            if let Some(pm) = tab.pane_manager_mut() {
                let panes = pm.all_panes_mut();
                for (index, pane) in panes.into_iter().enumerate() {
                    if let Some((image_path, mode, opacity, darken)) =
                        config.get_pane_background(index)
                    {
                        let bg = crate::pane::PaneBackground {
                            image_path: Some(image_path),
                            mode,
                            opacity,
                            darken,
                        };
                        pane.set_background(bg);
                    } else {
                        // Clear pane background if no longer configured
                        pane.set_background(crate::pane::PaneBackground::new());
                    }
                }
            }
        }
        if let Some(renderer) = &mut window_state.renderer {
            renderer.mark_dirty();
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply inline image settings changes
    if changes.image_scaling_mode {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_image_scaling_mode(config.image_scaling_mode);
        }
        window_state.focus_state.needs_redraw = true;
    }
    if changes.image_preserve_aspect_ratio {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_image_preserve_aspect_ratio(config.image_preserve_aspect_ratio);
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply theme changes to all tabs and all pane terminals
    if changes.theme {
        let theme = config.load_theme();
        for tab in window_state.tab_manager.tabs_mut() {
            // Set theme on tab's primary terminal
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_theme(theme.clone());
            }
            // Set theme on additional split pane terminals (primary pane shares
            // tab.terminal's Arc, so skip it to avoid double-locking)
            let tab_terminal = Arc::clone(&tab.terminal);
            if let Some(pm) = tab.pane_manager_mut() {
                for pane in pm.all_panes() {
                    if !Arc::ptr_eq(&pane.terminal, &tab_terminal)
                        && let Ok(mut term) = pane.terminal.try_write()
                    {
                        term.set_theme(theme.clone());
                    }
                }
            }
        }
    }

    // Update ENQ answerback string across all tabs when changed
    if changes.answerback_string {
        let answerback = if config.answerback_string.is_empty() {
            None
        } else {
            Some(config.answerback_string.clone())
        };
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_write() {
                term.set_answerback_string(answerback.clone());
            }
        }
    }

    // Apply Unicode width settings
    if changes.unicode_width {
        let width_config = par_term_emu_core_rust::WidthConfig::new(
            to_core_unicode_version(config.unicode.unicode_version),
            to_core_ambiguous_width(config.unicode.ambiguous_width),
        );
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_write() {
                term.set_width_config(width_config);
            }
        }
    }

    // Apply Unicode normalization form
    if changes.normalization_form {
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_write() {
                term.set_normalization_form(to_core_normalization_form(
                    config.unicode.normalization_form,
                ));
            }
        }
    }

    // Resolve per-shader settings (user override -> metadata defaults -> global)
    let shader_override = config
        .shader
        .custom_shader
        .as_ref()
        .and_then(|name| config.shader_configs.get(name));
    // Get shader metadata from cache for full 3-tier resolution
    let metadata = config.shader.custom_shader.as_ref().and_then(|name| {
        window_state
            .shader_state
            .shader_metadata_cache
            .get(name)
            .cloned()
    });
    let mut resolved = resolve_shader_config(shader_override, metadata.as_ref(), config);
    if config.shader.custom_shader_readability_mode {
        resolved.brightness = resolved
            .brightness
            .min(config.shader.custom_shader_readability_brightness);
    }

    // Apply shader changes - track if change was attempted and result
    let shader_result = if changes.any_shader_change() || changes.shader_per_shader_config {
        log::info!(
            "SETTINGS: applying shader change: {:?} -> {:?}",
            window_state.config.shader.custom_shader,
            config.shader.custom_shader
        );
        window_state.renderer.as_mut().map(|r| {
            r.set_custom_shader_enabled(
                par_term_render::renderer::shaders::CustomShaderEnableParams {
                    enabled: config.shader.custom_shader_enabled,
                    shader_path: config.shader.custom_shader.as_deref(),
                    window_opacity: config.window.window_opacity,
                    animation_enabled: config.shader.custom_shader_animation
                        && !config.shader.custom_shader_readability_mode,
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
            )
            .err()
        })
    } else {
        None // No change attempted
    };

    // Apply use_background_as_channel0 setting
    if (changes.any_shader_change()
        || changes.shader_use_background_as_channel0
        || changes.any_bg_change()
        || changes.shader_per_shader_config)
        && let Some(renderer) = &mut window_state.renderer
    {
        renderer.update_background_as_channel0_with_mode(
            resolved.use_background_as_channel0,
            config.background_mode,
            config.background_color,
        );
    }

    // Apply cursor shader changes
    let cursor_result = if changes.any_cursor_shader_toggle() {
        window_state.renderer.as_mut().map(|r| {
            r.set_cursor_shader_enabled(
                config.shader.cursor_shader_enabled,
                config.shader.cursor_shader.as_deref(),
                config.window.window_opacity,
                config.shader.cursor_shader_animation,
                config.shader.cursor_shader_animation_speed,
            )
            .err()
        })
    } else {
        None // No change attempted
    };

    if let Some(result) = &shader_result {
        window_state.shader_state.background_shader_last_error = result.clone();
    }
    if let Some(result) = &cursor_result {
        window_state.shader_state.cursor_shader_last_error = result.clone();
    }

    (shader_result, cursor_result)
}
