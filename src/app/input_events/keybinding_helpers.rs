//! Helper keybinding actions and WindowState helpers: visual notifications,
//! shader toggles, and the clear actions.
//!
//! - `show_toast`, `show_pane_indices`: visual notification helpers
//! - `toggle_background_shader`, `toggle_cursor_shader`: shader toggle helpers
//! - `clear_scrollback`, `clear_screen`, `send_clear_screen_sequence`: the
//!   clear family (moved here when keybinding_actions crossed the 800-line
//!   gate)

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

    /// Show a toast with no auto-hide timer — it stays until replaced or
    /// cleared, the V11 error-surface shape for failures the user must
    /// see and act on rather than watch fade.
    pub(crate) fn show_persistent_toast(&mut self, message: impl Into<String>) {
        self.overlay_state.toast_message = Some(message.into());
        self.overlay_state.toast_hide_time = None;
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
                .load()
                .shader
                .custom_shader
                .as_ref()
                .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
            let shader_override =
                self.config
                    .load()
                    .shader
                    .custom_shader
                    .as_ref()
                    .and_then(|name| {
                        self.config
                            .load()
                            .shader_overrides
                            .shader_configs
                            .get(name)
                            .cloned()
                    });
            let mut resolved = resolve_shader_config(
                shader_override.as_ref(),
                metadata.as_ref(),
                &self.config.load(),
            );
            if self.config.load().shader.custom_shader_readability_mode {
                resolved.brightness = resolved.brightness.min(
                    self.config
                        .load()
                        .shader
                        .custom_shader_readability_brightness,
                );
            }

            let _ = renderer.set_custom_shader_enabled(
                par_term_render::renderer::shaders::CustomShaderEnableParams {
                    enabled: self.config.load().shader.custom_shader_enabled,
                    shader_path: self.config.load().shader.custom_shader.as_deref(),
                    window_opacity: self.config.load().window.window_opacity,
                    animation_enabled: self.config.load().shader.custom_shader_animation
                        && !self.config.load().shader.custom_shader_readability_mode,
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
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.custom_shader_enabled = !old.shader.custom_shader_enabled;
            std::sync::Arc::new(new)
        });
        self.refresh_background_shader_renderer();

        self.focus_state.needs_redraw = true;
        self.request_redraw();

        log::info!(
            "Background shader {}",
            if self.config.load().shader.custom_shader_enabled {
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
            .load()
            .shader
            .custom_shader
            .as_ref()
            .and_then(|current| shaders.iter().position(|shader| shader == current))
            .map(|index| (index + 1) % shaders.len())
            .unwrap_or(0);
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.custom_shader = Some(shaders[next_index].clone());
            std::sync::Arc::new(new)
        });
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.custom_shader_enabled = true;
            std::sync::Arc::new(new)
        });
        self.refresh_background_shader_renderer();
        self.show_toast(format!("Shader: {}", shaders[next_index]));
    }

    /// Pause/resume background shader animation.
    pub(crate) fn toggle_shader_animation(&mut self) {
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.custom_shader_animation = !old.shader.custom_shader_animation;
            std::sync::Arc::new(new)
        });
        if let Some(renderer) = &mut self.renderer {
            renderer.set_custom_shader_animation(self.config.load().shader.custom_shader_animation);
        }
        self.show_toast(if self.config.load().shader.custom_shader_animation {
            "Shader animation resumed"
        } else {
            "Shader animation paused"
        });
    }

    /// Toggle low-power/readability shader mode.
    pub(crate) fn toggle_shader_readability_mode(&mut self) {
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.custom_shader_readability_mode = !old.shader.custom_shader_readability_mode;
            std::sync::Arc::new(new)
        });
        self.refresh_background_shader_renderer();
        self.show_toast(
            if self.config.load().shader.custom_shader_readability_mode {
                "Shader readability mode on"
            } else {
                "Shader readability mode off"
            },
        );
    }

    /// Toggle the cursor shader on/off.
    pub(crate) fn toggle_cursor_shader(&mut self) {
        self.config.rcu(|old| {
            let mut new = (**old).clone();
            new.shader.cursor_shader_enabled = !old.shader.cursor_shader_enabled;
            std::sync::Arc::new(new)
        });

        if let Some(renderer) = &mut self.renderer {
            let _ = renderer.set_cursor_shader_enabled(
                self.config.load().shader.cursor_shader_enabled,
                self.config.load().shader.cursor_shader.as_deref(),
                self.config.load().window.window_opacity,
                self.config.load().shader.cursor_shader_animation,
                self.config.load().shader.cursor_shader_animation_speed,
            );
        }

        self.focus_state.needs_redraw = true;
        self.request_redraw();

        log::info!(
            "Cursor shader {}",
            if self.config.load().shader.cursor_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
}

// ── clear actions (moved from keybinding_actions to stay under the
// 800-line gate) ──────────────────────────────────────────────────────────

pub(crate) fn clear_scrollback(s: &mut WindowState) -> bool {
    let cleared = if let Some(tab) = s.tab_manager.active_tab_mut() {
        // The focused pane's terminal, so a split clears the pane the user is
        // looking at (parity with the dissolved utility-layer branch).
        let terminal = if let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            std::sync::Arc::clone(&focused_pane.terminal)
        } else {
            std::sync::Arc::clone(&tab.terminal)
        };
        // try_lock: intentional — keybinding action in sync event loop.
        // On miss: scrollback not cleared this invocation. User can retry.
        let did_clear = if let Ok(mut term) = terminal.try_write() {
            term.clear_scrollback();
            term.clear_scrollback_metadata();
            true
        } else {
            false
        };
        if did_clear {
            tab.active_cache_mut().scrollback_len = 0;
            tab.scripting.trigger_marks.clear();
            if let Some(pm) = tab.pane_manager_mut() {
                for pane in pm.all_panes_mut() {
                    if std::sync::Arc::ptr_eq(&pane.terminal, &terminal) {
                        pane.cache.invalidate_pane_cells();
                    }
                }
            }
        }
        did_clear
    } else {
        false
    };
    if cleared {
        s.set_scroll_target(0);
        log::info!("Cleared scrollback buffer (focused pane)");
    }
    true
}

/// Send the Ctrl+L clear-screen byte (0x0C) to the focused pane. In a mux
/// tab the focused pane is a daemon mirror, so the byte routes to the
/// daemon — `tab.terminal` there is the hidden login shell the clear must
/// never reach.
pub(crate) fn clear_screen(s: &mut WindowState) -> bool {
    s.send_clear_screen_sequence();
    true
}

impl WindowState {
    /// Send the Ctrl+L clear-screen byte (0x0C) to the focused pane.
    ///
    /// A method (not inline in the handler) so the tmux pane-write path and
    /// tests can drive it without fabricating a winit `KeyEvent` (which has
    /// a private field and no public constructor).
    pub(crate) fn send_clear_screen_sequence(&self) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        let clear_sequence = vec![0x0C]; // Ctrl+L character
        if self.route_mux_tab_write(tab, &clear_sequence) {
            return;
        }
        // Use the focused pane's terminal so Ctrl+L clears the correct
        // pane in split-pane mode, falling back to the tab's root terminal.
        let terminal_clone = if let Some(ref pm) = tab.pane_manager {
            if let Some(focused_pane) = pm.focused_pane() {
                std::sync::Arc::clone(&focused_pane.terminal)
            } else {
                std::sync::Arc::clone(&tab.terminal)
            }
        } else {
            std::sync::Arc::clone(&tab.terminal)
        };
        self.runtime.spawn(async move {
            // try_lock: intentional — spawned async task uses try-lock to avoid
            // blocking the tokio worker. On miss: the Ctrl+L clear is silently
            // dropped. User can press the shortcut again.
            if let Ok(term) = terminal_clone.try_read() {
                if let Err(e) = term.write(&clear_sequence) {
                    crate::debug_error!("INPUT", "PTY write failed (clear screen): {e}");
                } else {
                    log::debug!("Sent clear screen sequence (Ctrl+L)");
                }
            }
        });
    }
}
