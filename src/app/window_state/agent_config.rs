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

/// Config keys whose value names a shader file (SEC-003).
pub(super) const SHADER_PATH_KEYS: &[&str] = &["custom_shader", "cursor_shader"];

/// Whether an agent-supplied shader-path value needs the user's confirmation
/// before it can activate (SEC-003 residual): any non-null value that differs
/// from the currently active one would activate new or changed GPU code.
/// `null` disables the shader and re-sending the active value is a no-op, so
/// both stay confirmation-free.
fn shader_change_needs_confirmation(value: &serde_json::Value, current: &Option<String>) -> bool {
    match value.as_str() {
        Some(name) => current.as_deref() != Some(name),
        None => false,
    }
}

/// Split an agent config batch into what may apply immediately and what must
/// wait for the user's confirmation (SEC-003 residual).
///
/// A shader-path value that differs from the active one is validated first:
/// an invalid value is left in the batch so the apply path reports its own
/// per-key error (the dialog never offers a shader the app would refuse to
/// load), a valid one is returned as pending. Every pending key also yields
/// a notice in the error list so the agent is told the value was accepted
/// but not activated.
///
/// `validate` is [`Config::validate_agent_shader_path`], injected so tests can
/// exercise the split without touching `HOME`.
fn partition_agent_shader_updates(
    updates: &std::collections::HashMap<String, serde_json::Value>,
    current_background: &Option<String>,
    current_cursor: &Option<String>,
    validate: impl Fn(&str) -> Result<(), String>,
) -> (
    std::collections::HashMap<String, serde_json::Value>,
    Vec<super::shader_state::PendingShaderChange>,
    Vec<String>,
) {
    let mut apply_now = updates.clone();
    let mut pending = Vec::new();
    let mut notices = Vec::new();

    for key in SHADER_PATH_KEYS {
        let Some(value) = apply_now.get(*key) else {
            continue;
        };
        let current = if *key == "custom_shader" {
            current_background
        } else {
            current_cursor
        };
        if !shader_change_needs_confirmation(value, current) {
            continue;
        }
        let Some(name) = value.as_str().map(str::to_string) else {
            // Wrong JSON type: let the apply path report it.
            continue;
        };
        match validate(&name) {
            Ok(()) => {
                notices.push(format!(
                    "{key}: pending user confirmation (value activates only after the \
                         user approves it in par-term)"
                ));
                apply_now.remove(*key);
                pending.push(super::shader_state::PendingShaderChange {
                    key: (*key).to_string(),
                    value: name,
                });
            }
            Err(_) => {
                // Invalid value: leave the key in the batch untouched so the
                // apply path reports its own per-key error, unchanged from
                // the pre-gate behavior.
            }
        }
    }

    (apply_now, pending, notices)
}

/// Parse an agent-supplied `custom_shader` / `cursor_shader` value (SEC-003).
///
/// `null` disables the shader and re-sending the currently configured value is
/// a no-op, so a user-chosen path outside the shaders directory survives. Any
/// other value must name an existing file inside the shaders directory.
fn agent_shader_value(
    value: &serde_json::Value,
    current: &Option<String>,
) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let name = value.as_str().ok_or("expected string or null")?;
    if current.as_deref() != Some(name) {
        Config::validate_agent_shader_path(name).map_err(|e| e.to_string())?;
    }
    Ok(Some(name.to_string()))
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

                // An outside edit to the bindings must reach the registry the
                // key handler matches against, the same rebuild
                // `apply_config_to_windows` does; the menu follows the config
                // copy (UX.md MN3), so the two cannot disagree.
                if changes.keybindings {
                    self.keybinding_registry =
                        par_term_keybindings::KeybindingRegistry::from_config(
                            &new_config.keybindings,
                        );
                    log::info!("CONFIG: keybinding registry rebuilt from the reloaded config");
                }

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
                            .and_then(|name| cfg.shader_overrides.shader_configs.get(name));
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
                self.render_loop.config_reloaded_from_disk = true;
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
    ///
    /// Every agent entry point (ACP `config/update` RPC, the MCP
    /// `.config-update.json` IPC file, and the inline-tool-markup fallback)
    /// funnels through here, so this is also the single confirmation gate
    /// (SEC-003 residual): shader-path values that differ from the active
    /// ones are parked for the user's first-use confirmation dialog instead
    /// of being applied. Re-sending the active value and `null` stay
    /// confirmation-free.
    pub(super) fn apply_agent_config_updates(
        &mut self,
        updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.apply_agent_config_updates_inner(updates, false)
    }

    /// Body of [`Self::apply_agent_config_updates`].
    ///
    /// `request_confirmation=false` (every agent entry point) parks shader
    /// values that differ from the active ones for the confirmation dialog;
    /// `true` is only for the approval drain, so an approved value cannot
    /// re-queue itself.
    fn apply_agent_config_updates_inner(
        &mut self,
        updates: &std::collections::HashMap<String, serde_json::Value>,
        request_confirmation: bool,
    ) -> Result<(), String> {
        let old_config = (**self.config.load()).clone();
        let (updates, pending, mut errors) = if request_confirmation {
            (updates.clone(), Vec::new(), Vec::new())
        } else {
            partition_agent_shader_updates(
                updates,
                &old_config.shader.custom_shader,
                &old_config.shader.cursor_shader,
                |name| {
                    Config::validate_agent_shader_path(name)
                        .map_err(|e| e.to_string())
                        .map(|_| ())
                },
            )
        };
        if !pending.is_empty() {
            log::info!("SEC-003: agent shader change(s) parked for user confirmation: {pending:?}");
            let note = pending
                .iter()
                .map(|c| format!("{} = \"{}\"", c.key, c.value))
                .collect::<Vec<_>>()
                .join(", ");
            self.overlay_ui
                .ai_inspector
                .chat
                .add_system_message(format!(
                    "Agent shader change queued for your confirmation: {note}. \
                 Approve or ignore it in the dialog; re-sending the value after \
                 approval applies immediately."
                ));
        }
        for change in pending {
            self.shader_state.queue_shader_change(change);
        }
        self.focus_state.needs_redraw = true;

        for (key, value) in &updates {
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
                    .and_then(|name| cfg.shader_overrides.shader_configs.get(name));
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
                let v = agent_shader_value(value, &self.config.load().shader.custom_shader)?;
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
                let v = agent_shader_value(value, &self.config.load().shader.cursor_shader)?;
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

    /// Drain the shader-confirmation dialog's decision: on approval, activate
    /// the parked value through the apply path with the gate bypassed; on
    /// decline, drop it. Called from `about_to_wait`, like the agent-command
    /// approval drain, where `&mut self` is free of the egui borrow.
    pub(crate) fn run_approved_shader_changes(&mut self) {
        let Some((change, approved)) = self.shader_state.take_shader_decision() else {
            return;
        };
        if !approved {
            log::info!(
                "SEC-003: user declined agent {} = \"{}\"; value not activated",
                change.key,
                change.value
            );
            return;
        }
        log::info!(
            "SEC-003: user approved agent {} = \"{}\"; activating",
            change.key,
            change.value
        );
        let mut updates = std::collections::HashMap::new();
        updates.insert(change.key.clone(), serde_json::Value::String(change.value));
        if let Err(e) = self.apply_agent_config_updates_inner(&updates, true) {
            log::error!("SEC-003: approved shader activation failed: {e}");
        } else {
            // The other agent apply callers set this for the cross-window
            // sync in `app_handler_impl`; the approval drain is its own
            // caller, so set it here.
            self.render_loop.config_changed_by_agent = true;
        }
        self.focus_state.needs_redraw = true;
    }
}

#[cfg(test)]
#[path = "agent_config_tests.rs"]
mod tests;
