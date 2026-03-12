//! Config propagation from the settings window to all terminal windows.
//!
//! The [`apply_config_to_windows`] method is the single entry point for applying
//! a freshly-edited `Config` to every open `WindowState`. It detects what changed
//! (via [`ConfigChanges::detect`]) and applies only the relevant updates to avoid
//! unnecessary re-renders.
//!
//! Renderer-specific settings application lives in `config_renderer_apply`
//! (extracted to keep this file under 500 lines).
//!
//! Extracted from `settings_actions.rs` (R-39) so that the lifecycle/routing code
//! and the per-window propagation code can be read and extended independently.

use std::sync::Arc;

use crate::app::window_state::config_updates::ConfigChanges;
use crate::config::Config;

use super::WindowManager;
use super::config_renderer_apply::apply_renderer_config;

impl WindowManager {
    /// Apply config changes from settings window to all terminal windows.
    pub fn apply_config_to_windows(&mut self, config: &Config) {
        // Apply log level change immediately
        crate::debug::set_log_level(config.log_level.to_level_filter());

        // Track shader errors for the standalone settings window
        // Option<Option<String>>: None = no change attempted, Some(None) = success, Some(Some(err)) = error
        let mut last_shader_result: Option<Option<String>> = None;
        let mut last_cursor_shader_result: Option<Option<String>> = None;
        let mut ai_agent_list_changed = false;

        for window_state in self.windows.values_mut() {
            // Detect what changed
            let changes = ConfigChanges::detect(&window_state.config, config);

            // Update the config
            window_state.config = config.clone();

            if changes.ai_inspector_custom_agents {
                window_state.refresh_available_agents();
                ai_agent_list_changed = true;
            }

            // Rebuild keybinding registry if keybindings changed
            if changes.keybindings {
                window_state.keybinding_registry =
                    crate::keybindings::KeybindingRegistry::from_config(&config.keybindings);
                log::info!(
                    "Keybinding registry rebuilt with {} bindings",
                    config.keybindings.len()
                );
            }

            // Sync AI Inspector chat font size to the panel
            if changes.ai_inspector_chat_font_size {
                window_state.overlay_ui.ai_inspector.chat_font_size =
                    config.ai_inspector.ai_inspector_chat_font_size;
            }

            // Sync AI Inspector auto-approve / YOLO mode to connected agent
            if changes.ai_inspector_auto_approve
                && let Some(agent) = &window_state.agent_state.agent
            {
                let agent = agent.clone();
                let auto_approve = config.ai_inspector.ai_inspector_auto_approve;
                let mode = if auto_approve {
                    "bypassPermissions"
                } else {
                    "default"
                }
                .to_string();
                window_state.runtime.spawn(async move {
                    let agent = agent.lock().await;
                    agent
                        .auto_approve
                        .store(auto_approve, std::sync::atomic::Ordering::Relaxed);
                    if let Err(e) = agent.set_mode(&mode).await {
                        log::error!("ACP: failed to set mode '{mode}': {e}");
                    }
                });
            }

            // Apply changes to renderer and collect any shader errors.
            // Delegated to `config_renderer_apply` to keep this file under 500 lines.
            let (shader_result, cursor_result) =
                apply_renderer_config(window_state, config, &changes);

            // Track shader errors for propagation to standalone settings window
            if let Some(result) = shader_result {
                last_shader_result = Some(result);
            }
            if let Some(result) = cursor_result {
                last_cursor_shader_result = Some(result);
            }

            // Apply font rendering changes that can update live
            if changes.font_rendering {
                if let Some(renderer) = &mut window_state.renderer {
                    let mut updated = false;
                    updated |= renderer.update_font_antialias(config.font_antialias);
                    updated |= renderer.update_font_hinting(config.font_hinting);
                    updated |= renderer.update_font_thin_strokes(config.font_thin_strokes);
                    updated |= renderer.update_minimum_contrast(config.minimum_contrast);
                    if updated {
                        window_state.focus_state.needs_redraw = true;
                    }
                } else {
                    window_state.render_loop.pending_font_rebuild = true;
                }
            }

            // Apply window-related changes
            if let Some(window) = &window_state.window {
                // Update window title (handles both title change and show_window_number toggle)
                if changes.window_title || changes.show_window_number {
                    let title = window_state.format_title(&window_state.config.window_title);
                    window.set_title(&title);
                }
                if changes.window_decorations {
                    window.set_decorations(config.window_decorations);
                }
                if changes.lock_window_size {
                    window.set_resizable(!config.lock_window_size);
                    log::info!("Window resizable set to: {}", !config.lock_window_size);
                }
                window.set_window_level(if config.window_always_on_top {
                    winit::window::WindowLevel::AlwaysOnTop
                } else {
                    winit::window::WindowLevel::Normal
                });

                // Apply blur changes (macOS only)
                #[cfg(target_os = "macos")]
                if changes.blur {
                    let blur_radius = if config.blur_enabled && config.window_opacity < 1.0 {
                        config.blur_radius
                    } else {
                        0 // Disable blur when not enabled or fully opaque
                    };
                    if let Err(e) = crate::macos_blur::set_window_blur(window, blur_radius) {
                        log::warn!("Failed to set window blur: {}", e);
                    }
                }

                window.request_redraw();
            }

            // Apply window padding changes live without full renderer rebuild
            if changes.padding
                && let Some(renderer) = &mut window_state.renderer
            {
                if let Some((new_cols, new_rows)) =
                    renderer.update_window_padding(config.window_padding)
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_write() {
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            if let Err(e) =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px)
                            {
                                crate::debug_error!(
                                    "TERMINAL",
                                    "resize_with_pixels failed (config_propagation): {e}"
                                );
                            }
                        }
                        tab.active_cache_mut().cells = None;
                    }
                }
                window_state.focus_state.needs_redraw = true;
            }

            // Queue font rebuild if needed
            if changes.font {
                window_state.render_loop.pending_font_rebuild = true;
            }

            // Reinitialize shader watcher if shader paths changed
            if changes.needs_watcher_reinit() {
                window_state.reinit_shader_watcher();
            }

            // Restart refresh tasks when max_fps or inactive_tab_fps changes
            if (changes.max_fps || changes.inactive_tab_fps)
                && let Some(window) = &window_state.window
            {
                for tab in window_state.tab_manager.tabs_mut() {
                    tab.stop_refresh_task();
                    tab.start_refresh_task(
                        Arc::clone(&window_state.runtime),
                        Arc::clone(window),
                        config.max_fps,
                        config.inactive_tab_fps,
                    );
                }
                log::info!("Restarted refresh tasks with max_fps={}", config.max_fps);
            }

            // Update badge state if badge settings changed
            if changes.badge {
                window_state.badge_state.update_config(config);
                window_state.badge_state.mark_dirty();
            }

            // Sync status bar monitor state after config changes
            window_state.status_bar_ui.sync_monitor_state(config);

            // Update pane divider settings on all tabs with pane managers
            let dpi_scale = window_state
                .renderer
                .as_ref()
                .map(|r| r.scale_factor())
                .unwrap_or(1.0);
            let divider_width = config.pane_divider_width.unwrap_or(2.0) * dpi_scale;
            for tab in window_state.tab_manager.tabs_mut() {
                if let Some(pm) = tab.pane_manager_mut() {
                    pm.set_divider_width(divider_width);
                    pm.set_divider_hit_width(config.pane_divider_hit_width * dpi_scale);
                }
            }

            // Resync triggers from config into core registry for all tabs
            for tab in window_state.tab_manager.tabs_mut() {
                if let Ok(term) = tab.terminal.try_write() {
                    tab.scripting.trigger_prompt_before_run = term.sync_triggers(&config.triggers);
                }
            }

            // Clear session-level "always allow" approvals when config is reloaded,
            // so users must re-approve after a config change.
            window_state.trigger_state.always_allow_trigger_ids.clear();

            // Rebuild prettifier pipelines for all tabs when config changes.
            if changes.prettifier_changed {
                for tab in window_state.tab_manager.tabs_mut() {
                    tab.prettifier = crate::prettifier::config_bridge::create_pipeline_from_config(
                        config,
                        config.cols,
                        None,
                    );
                }
            }

            // Invalidate cache
            if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                tab.active_cache_mut().cells = None;
            }
            window_state.focus_state.needs_redraw = true;
        }

        if ai_agent_list_changed
            && let Some(sw) = &mut self.settings_window
            && let Some(ws) = self.windows.values().next()
        {
            sw.settings_ui.available_agent_ids = ws
                .agent_state
                .available_agents
                .iter()
                .map(|a| (a.identity.clone(), a.name.clone()))
                .collect();
        }

        // Restart dynamic profile manager if sources changed
        let dynamic_sources_changed =
            self.config.dynamic_profile_sources != config.dynamic_profile_sources;

        // Also update the shared config
        self.config = config.clone();

        // Restart dynamic profile manager with new sources if they changed
        if dynamic_sources_changed {
            self.dynamic_profile_manager.stop();
            if !config.dynamic_profile_sources.is_empty() {
                self.dynamic_profile_manager
                    .start(&config.dynamic_profile_sources, &self.runtime);
            }
            log::info!(
                "Dynamic profile manager restarted with {} sources",
                config.dynamic_profile_sources.len()
            );
        }

        // Update standalone settings window with shader errors only when a change was attempted
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(result) = last_shader_result {
                settings_window.set_shader_error(result);
            }
            if let Some(result) = last_cursor_shader_result {
                settings_window.set_cursor_shader_error(result);
            }
        }
    }
}
