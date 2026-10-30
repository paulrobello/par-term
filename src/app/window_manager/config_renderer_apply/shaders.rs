use crate::app::window_state::WindowState;
use crate::app::window_state::config_updates::ConfigChanges;
use crate::config::{Config, resolve_shader_config};

/// Resolve shader settings and apply background/cursor shader changes.
///
/// Returns `(shader_result, cursor_result)`; each is `None` when no shader
/// change was attempted.
pub(super) fn apply_shaders(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) -> (Option<Option<String>>, Option<Option<String>>) {
    // Resolve per-shader settings (user override -> metadata defaults -> global)
    let shader_override = config
        .shader
        .custom_shader
        .as_ref()
        .and_then(|name| config.shader_overrides.shader_configs.get(name));
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
            window_state.config.load().shader.custom_shader,
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
            config.image.background_mode,
            config.image.background_color,
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
