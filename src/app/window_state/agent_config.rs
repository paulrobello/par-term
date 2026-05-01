//! ACP agent config update application for WindowState.
//!
//! Contains:
//! - `check_config_reload`: reload config from disk when file changes detected
//! - `apply_agent_config_updates`: apply config changes from agent responses
//! - `apply_single_config_update`: dispatch a single config change
//! - Private helpers: `json_as_f32`

use crate::app::window_state::WindowState;
use crate::config::Config;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Module-private helpers
// ---------------------------------------------------------------------------

pub(super) fn json_as_f32(value: &serde_json::Value) -> Result<f32, String> {
    if let Some(f) = value.as_f64() {
        Ok(f as f32)
    } else if let Some(i) = value.as_i64() {
        Ok(i as f32)
    } else {
        Err("expected number".to_string())
    }
}

impl WindowState {
    /// Check for pending config file changes and apply them.
    ///
    /// Called periodically from the event loop. On config change:
    /// 1. Reloads config from disk
    /// 2. Applies shader-related config changes
    /// 3. Reinitializes shader watcher if shader paths changed
    pub(crate) fn check_config_reload(&mut self) {
        let Some(watcher) = &self.watcher_state.config_watcher else {
            return;
        };
        let Some(_event) = watcher.try_recv() else {
            return;
        };

        log::info!("CONFIG: config file changed, reloading...");

        match Config::load() {
            Ok(new_config) => {
                use crate::app::window_state::config_updates::ConfigChanges;

                let changes = ConfigChanges::detect(&self.config.load(), &new_config);

                // Replace the entire in-memory config so that any subsequent
                // config.save() writes the agent's changes, not stale values.
                self.config.store(Arc::new(new_config));

                log::info!(
                    "CONFIG: shader_changed={} cursor_changed={} shader={:?}",
                    changes.any_shader_change(),
                    changes.any_cursor_shader_toggle(),
                    self.config.load().shader.custom_shader
                );

                // Apply shader changes to the renderer
                if let Some(renderer) = &mut self.renderer {
                    if changes.any_shader_change() || changes.shader_per_shader_config {
                        log::info!("CONFIG: applying background shader change to renderer");
                        let cfg = self.config.load();
                        let shader_override = cfg
                            .shader
                            .custom_shader
                            .as_ref()
                            .and_then(|name| cfg.shader_configs.get(name));
                        let metadata =
                            self.config
                                .load()
                                .shader
                                .custom_shader
                                .as_ref()
                                .and_then(|name| {
                                    self.shader_state.shader_metadata_cache.get(name).cloned()
                                });
                        let resolved = crate::config::shader_config::resolve_shader_config(
                            shader_override,
                            metadata.as_ref(),
                            &self.config.load(),
                        );
                        match renderer.set_custom_shader_enabled(
                            par_term_render::renderer::shaders::CustomShaderEnableParams {
                                enabled: self.config.load().shader.custom_shader_enabled,
                                shader_path: self.config.load().shader.custom_shader.as_deref(),
                                window_opacity: self.config.load().window.window_opacity,
                                animation_enabled: self
                                    .config
                                    .load()
                                    .shader
                                    .custom_shader_animation,
                                animation_speed: resolved.animation_speed,
                                full_content: resolved.full_content,
                                brightness: resolved.brightness,
                                channel_paths: &resolved.channel_paths(),
                                cubemap_path: resolved.cubemap_path().map(|p| p.as_path()),
                                custom_uniforms: &resolved.custom_uniforms,
                                background_channel0_blend_mode: resolved
                                    .background_channel0_blend_mode,
                                auto_dim_under_text: resolved.auto_dim_under_text,
                                auto_dim_strength: resolved.auto_dim_strength,
                            },
                        ) {
                            Ok(()) => self.shader_state.background_shader_last_error = None,
                            Err(e) => {
                                let error = e.to_string();
                                self.shader_state.background_shader_last_error =
                                    Some(error.clone());
                                log::error!("Config reload: shader load failed: {error}");
                            }
                        }
                    }
                    if changes.any_cursor_shader_toggle() {
                        log::info!("CONFIG: applying cursor shader change to renderer");
                        match renderer.set_cursor_shader_enabled(
                            self.config.load().shader.cursor_shader_enabled,
                            self.config.load().shader.cursor_shader.as_deref(),
                            self.config.load().window.window_opacity,
                            self.config.load().shader.cursor_shader_animation,
                            self.config.load().shader.cursor_shader_animation_speed,
                        ) {
                            Ok(()) => self.shader_state.cursor_shader_last_error = None,
                            Err(e) => {
                                let error = e.to_string();
                                self.shader_state.cursor_shader_last_error = Some(error.clone());
                                log::error!("Config reload: cursor shader load failed: {error}");
                            }
                        }
                    }
                }

                // Reinit shader watcher if paths changed
                if changes.needs_watcher_reinit() {
                    self.reinit_shader_watcher();
                }

                self.focus_state.needs_redraw = true;
                debug_info!("CONFIG", "Config reloaded successfully");
            }
            Err(e) => {
                log::error!("Failed to reload config: {}", e);
            }
        }
    }

    /// Apply config updates from the ACP agent.
    ///
    /// Updates the in-memory config, applies changes to the renderer, and
    /// saves to disk. Returns `Ok(())` on success or an error string.
    pub(super) fn apply_agent_config_updates(
        &mut self,
        updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        let old_config = (**self.config.load()).clone();

        for (key, value) in updates {
            if let Err(e) = self.apply_single_config_update(key, value) {
                errors.push(format!("{key}: {e}"));
            }
        }

        if !errors.is_empty() {
            return Err(errors.join("; "));
        }

        // Detect changes and apply to renderer
        use crate::app::window_state::config_updates::ConfigChanges;
        let changes = ConfigChanges::detect(&old_config, &self.config.load());

        log::info!(
            "ACP config/update: shader_change={} cursor_change={} old_shader={:?} new_shader={:?}",
            changes.any_shader_change(),
            changes.any_cursor_shader_toggle(),
            old_config.shader.custom_shader,
            self.config.load().shader.custom_shader
        );

        if let Some(renderer) = &mut self.renderer {
            if changes.any_shader_change() || changes.shader_per_shader_config {
                log::info!("ACP config/update: applying background shader change to renderer");
                let cfg = self.config.load();
                let shader_override = cfg
                    .shader
                    .custom_shader
                    .as_ref()
                    .and_then(|name| cfg.shader_configs.get(name));
                let metadata =
                    cfg.shader.custom_shader.as_ref().and_then(|name| {
                        self.shader_state.shader_metadata_cache.get(name).cloned()
                    });
                let resolved = crate::config::shader_config::resolve_shader_config(
                    shader_override,
                    metadata.as_ref(),
                    &self.config.load(),
                );
                match renderer.set_custom_shader_enabled(
                    par_term_render::renderer::shaders::CustomShaderEnableParams {
                        enabled: self.config.load().shader.custom_shader_enabled,
                        shader_path: self.config.load().shader.custom_shader.as_deref(),
                        window_opacity: self.config.load().window.window_opacity,
                        animation_enabled: self.config.load().shader.custom_shader_animation,
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
                ) {
                    Ok(()) => self.shader_state.background_shader_last_error = None,
                    Err(e) => {
                        let error = e.to_string();
                        self.shader_state.background_shader_last_error = Some(error.clone());
                        log::error!("ACP config/update: shader load failed: {error}");
                    }
                }
            }
            if changes.any_cursor_shader_toggle() {
                log::info!("ACP config/update: applying cursor shader change to renderer");
                match renderer.set_cursor_shader_enabled(
                    self.config.load().shader.cursor_shader_enabled,
                    self.config.load().shader.cursor_shader.as_deref(),
                    self.config.load().window.window_opacity,
                    self.config.load().shader.cursor_shader_animation,
                    self.config.load().shader.cursor_shader_animation_speed,
                ) {
                    Ok(()) => self.shader_state.cursor_shader_last_error = None,
                    Err(e) => {
                        let error = e.to_string();
                        self.shader_state.cursor_shader_last_error = Some(error.clone());
                        log::error!("ACP config/update: cursor shader load failed: {error}");
                    }
                }
            }
        }

        if changes.needs_watcher_reinit() {
            self.reinit_shader_watcher();
        }

        // Save to disk
        if let Err(e) = self.save_config_debounced() {
            return Err(format!("Failed to save config: {e}"));
        }

        Ok(())
    }

    /// Apply a single config key/value update to the in-memory config.
    fn apply_single_config_update(
        &mut self,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        match key {
            // -- Background shader --
            "custom_shader" => {
                let v = if value.is_null() {
                    None
                } else {
                    Some(value.as_str().ok_or("expected string or null")?.to_string())
                };
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader = v.clone();
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_enabled" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_enabled = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_animation" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_animation = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_animation_speed" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_animation_speed = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_brightness" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_brightness = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_text_opacity" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_text_opacity = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "custom_shader_full_content" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.custom_shader_full_content = v;
                    Arc::new(new)
                });
                Ok(())
            }

            // -- Cursor shader --
            "cursor_shader" => {
                let v = if value.is_null() {
                    None
                } else {
                    Some(value.as_str().ok_or("expected string or null")?.to_string())
                };
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader = v.clone();
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_enabled" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_enabled = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_animation" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_animation = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_animation_speed" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_animation_speed = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_glow_radius" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_glow_radius = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_glow_intensity" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_glow_intensity = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_trail_duration" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_trail_duration = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "cursor_shader_hides_cursor" => {
                let v = value.as_bool().ok_or("expected boolean")?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.shader.cursor_shader_hides_cursor = v;
                    Arc::new(new)
                });
                Ok(())
            }

            // -- Window --
            "window_opacity" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.window.window_opacity = v;
                    Arc::new(new)
                });
                Ok(())
            }
            "font_size" => {
                let v = json_as_f32(value)?;
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.font_size = v;
                    Arc::new(new)
                });
                Ok(())
            }

            _ => Err(format!("unknown or read-only config key: {key}")),
        }
    }
}
