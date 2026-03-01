//! Config propagation from the settings window to all terminal windows.
//!
//! The [`apply_config_to_windows`] method is the single entry point for applying
//! a freshly-edited `Config` to every open `WindowState`. It detects what changed
//! (via [`ConfigChanges::detect`]) and applies only the relevant updates to avoid
//! unnecessary re-renders.
//!
//! Extracted from `settings_actions.rs` (R-39) so that the lifecycle/routing code
//! and the per-window propagation code can be read and extended independently.

use std::sync::Arc;

use crate::app::config_updates::ConfigChanges;
use crate::config::{Config, resolve_shader_config};

use super::WindowManager;

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

            // Apply changes to renderer and collect any shader errors
            let (shader_result, cursor_result) = if let Some(renderer) = &mut window_state.renderer
            {
                // Update opacity
                renderer.update_opacity(config.window_opacity);

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

                // Update cursor color
                if changes.cursor_color {
                    renderer.update_cursor_color(config.cursor_color);
                }

                // Update cursor text color
                if changes.cursor_text_color {
                    renderer.update_cursor_text_color(config.cursor_text_color);
                }

                // Update cursor style and blink for all tabs
                if changes.cursor_style || changes.cursor_blink {
                    use crate::config::CursorStyle as ConfigCursorStyle;
                    use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

                    let term_style = if config.cursor_blink {
                        match config.cursor_style {
                            ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
                            ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
                            ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                        }
                    } else {
                        match config.cursor_style {
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
                    renderer.update_cursor_guide(
                        config.cursor_guide_enabled,
                        config.cursor_guide_color,
                    );
                    renderer.update_cursor_shadow(
                        config.cursor_shadow_enabled,
                        config.cursor_shadow_color,
                        config.cursor_shadow_offset,
                        config.cursor_shadow_blur,
                    );
                    renderer.update_cursor_boost(config.cursor_boost, config.cursor_boost_color);
                    renderer.update_unfocused_cursor_style(config.unfocused_cursor_style);
                    window_state.focus_state.needs_redraw = true;
                }

                // Apply command separator changes
                if changes.command_separator {
                    renderer.update_command_separator(
                        config.command_separator_enabled,
                        config.command_separator_thickness,
                        config.command_separator_opacity,
                        config.command_separator_exit_color,
                        config.command_separator_color,
                    );
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
                    renderer.set_background(
                        config.background_mode,
                        config.background_color,
                        expanded_path.as_deref(),
                        config.background_image_mode,
                        config.background_image_opacity,
                        config.background_image_enabled,
                    );
                    window_state.focus_state.needs_redraw = true;
                }

                // Apply per-pane background changes to existing panes
                if changes.pane_backgrounds {
                    // Pre-load all pane background textures into the renderer cache
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
                    renderer.mark_dirty();
                    window_state.focus_state.needs_redraw = true;
                }

                // Apply inline image settings changes
                if changes.image_scaling_mode {
                    renderer.update_image_scaling_mode(config.image_scaling_mode);
                    window_state.focus_state.needs_redraw = true;
                }
                if changes.image_preserve_aspect_ratio {
                    renderer.update_image_preserve_aspect_ratio(config.image_preserve_aspect_ratio);
                    window_state.focus_state.needs_redraw = true;
                }

                // Apply theme changes
                if changes.theme
                    && let Some(tab) = window_state.tab_manager.active_tab()
                {
                    match tab.terminal.try_write() {
                        Ok(mut term) => term.set_theme(config.load_theme()),
                        Err(_) => crate::debug::record_try_lock_failure("theme_change"),
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
                        config.unicode.unicode_version,
                        config.unicode.ambiguous_width,
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
                            term.set_normalization_form(config.unicode.normalization_form);
                        }
                    }
                }

                // Resolve per-shader settings (user override -> metadata defaults -> global)
                let shader_override = config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| config.shader_configs.get(name));
                // Get shader metadata from cache for full 3-tier resolution
                let metadata = config.custom_shader.as_ref().and_then(|name| {
                    window_state
                        .shader_state
                        .shader_metadata_cache
                        .get(name)
                        .cloned()
                });
                let resolved = resolve_shader_config(shader_override, metadata.as_ref(), config);

                // Apply shader changes - track if change was attempted and result
                let shader_result =
                    if changes.any_shader_change() || changes.shader_per_shader_config {
                        log::info!(
                            "SETTINGS: applying shader change: {:?} -> {:?}",
                            window_state.config.custom_shader,
                            config.custom_shader
                        );
                        Some(
                            renderer
                                .set_custom_shader_enabled(
                                    par_term_render::renderer::shaders::CustomShaderEnableParams {
                                        enabled: config.custom_shader_enabled,
                                        shader_path: config.custom_shader.as_deref(),
                                        window_opacity: config.window_opacity,
                                        animation_enabled: config.custom_shader_animation,
                                        animation_speed: resolved.animation_speed,
                                        full_content: resolved.full_content,
                                        brightness: resolved.brightness,
                                        channel_paths: &resolved.channel_paths(),
                                        cubemap_path: resolved.cubemap_path().map(|p| p.as_path()),
                                    },
                                )
                                .err(),
                        )
                    } else {
                        None // No change attempted
                    };

                // Apply use_background_as_channel0 setting
                if changes.any_shader_change()
                    || changes.shader_use_background_as_channel0
                    || changes.any_bg_change()
                    || changes.shader_per_shader_config
                {
                    renderer.update_background_as_channel0_with_mode(
                        resolved.use_background_as_channel0,
                        config.background_mode,
                        config.background_color,
                    );
                }

                // Apply cursor shader changes
                let cursor_result = if changes.any_cursor_shader_toggle() {
                    Some(
                        renderer
                            .set_cursor_shader_enabled(
                                config.cursor_shader_enabled,
                                config.cursor_shader.as_deref(),
                                config.window_opacity,
                                config.cursor_shader_animation,
                                config.cursor_shader_animation_speed,
                            )
                            .err(),
                    )
                } else {
                    None // No change attempted
                };

                (shader_result, cursor_result)
            } else {
                (None, None)
            };

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
                    window_state.pending_font_rebuild = true;
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
                            let _ =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                        }
                        tab.active_cache_mut().cells = None;
                    }
                }
                window_state.focus_state.needs_redraw = true;
            }

            // Queue font rebuild if needed
            if changes.font {
                window_state.pending_font_rebuild = true;
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
                    tab.scripting.trigger_security = term.sync_triggers(&config.triggers);
                }
            }

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
