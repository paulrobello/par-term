//! Application event handler
//!
//! This module implements the winit `ApplicationHandler` trait for `WindowManager`,
//! routing window events to the appropriate `WindowState` and handling menu events.

use crate::app::window_manager::WindowManager;
use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

impl WindowState {
    /// Update window title with shell integration info (cwd and exit code)
    /// Only updates if not scrolled and not hovering over URL
    pub(crate) fn update_window_title_with_shell_integration(&self) {
        // Get active tab state
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Skip if scrolled (scrollback indicator takes priority)
        if tab.scroll_state.offset != 0 {
            return;
        }

        // Skip if hovering over URL (URL tooltip takes priority)
        if tab.mouse.hovered_url.is_some() {
            return;
        }

        // Skip if window not available
        let window = if let Some(w) = &self.window {
            w
        } else {
            return;
        };

        // Try to get shell integration info
        if let Ok(term) = tab.terminal.try_lock() {
            let mut title_parts = vec![self.config.window_title.clone()];

            // Add current working directory if available
            if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated_cwd = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                title_parts.push(format!("({})", abbreviated_cwd));
            }

            // Add exit code indicator if last command failed
            if let Some(exit_code) = term.shell_integration_exit_code()
                && exit_code != 0
            {
                title_parts.push(format!("[Exit: {}]", exit_code));
            }

            // Add recording indicator
            if self.is_recording {
                title_parts.push("[RECORDING]".to_string());
            }

            // Build and set title
            let title = title_parts.join(" ");
            window.set_title(&title);
        }
    }

    /// Handle window events for this window state
    pub(crate) fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        use winit::keyboard::{Key, NamedKey};

        // Debug: Log ALL keyboard events at entry to diagnose Space issue
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
        {
            match &key_event.logical_key {
                Key::Character(s) => {
                    log::trace!(
                        "window_event: Character '{}', state={:?}",
                        s,
                        key_event.state
                    );
                }
                Key::Named(named) => {
                    log::trace!(
                        "window_event: Named key {:?}, state={:?}",
                        named,
                        key_event.state
                    );
                }
                other => {
                    log::trace!(
                        "window_event: Other key {:?}, state={:?}",
                        other,
                        key_event.state
                    );
                }
            }
        }

        // Let egui handle the event (needed for proper rendering state)
        let egui_consumed =
            if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
                let event_response = egui_state.on_window_event(window, &event);
                event_response.consumed
            } else {
                false
            };

        // Debug: Log when egui consumes events but we ignore it
        if egui_consumed
            && !self.settings_ui.visible
            && let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            && let Key::Named(NamedKey::Space) = &key_event.logical_key
        {
            log::debug!("egui tried to consume Space (UI closed, ignoring)");
        }

        // Only honor egui's consumption if an egui UI panel is actually visible
        // This prevents egui from stealing Tab/Space when UI is closed
        let any_ui_visible = self.settings_ui.visible
            || self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.settings_ui.is_shader_editor_visible()
            || self.settings_ui.is_cursor_shader_editor_visible();

        // When shader editor is visible, block keyboard events from terminal
        // even if egui didn't consume them (egui might not have focus)
        if any_ui_visible
            && let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            // Always block keyboard input when UI is visible (except system keys)
            && !matches!(
                key_event.logical_key,
                Key::Named(NamedKey::F1)
                    | Key::Named(NamedKey::F2)
                    | Key::Named(NamedKey::F3)
                    | Key::Named(NamedKey::F11)
                    | Key::Named(NamedKey::Escape)
            )
        {
            return false;
        }

        if egui_consumed
            && any_ui_visible
            && !matches!(
                event,
                WindowEvent::CloseRequested | WindowEvent::RedrawRequested
            )
        {
            return false; // Event consumed by egui, don't close window
        }

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested for window");
                // Set shutdown flag to stop redraw loop
                self.is_shutting_down = true;
                // Abort refresh tasks for all tabs
                for tab in self.tab_manager.tabs_mut() {
                    if let Some(task) = tab.refresh_task.take() {
                        task.abort();
                    }
                }
                log::info!("Refresh tasks aborted");
                return true; // Signal to close this window
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    log::info!(
                        "Scale factor changed to {} (display change detected)",
                        scale_factor
                    );

                    let size = window.inner_size();
                    let (cols, rows) = renderer.handle_scale_factor_change(scale_factor, size);

                    // Reconfigure surface after scale factor change
                    // This is important when dragging between displays with different DPIs
                    renderer.reconfigure_surface();

                    // Calculate pixel dimensions
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;

                    // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
                    for tab in self.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        }
                    }

                    // Reconfigure macOS Metal layer after display change
                    #[cfg(target_os = "macos")]
                    {
                        if let Err(e) =
                            crate::macos_metal::configure_metal_layer_for_performance(window)
                        {
                            log::warn!(
                                "Failed to reconfigure Metal layer after display change: {}",
                                e
                            );
                        }
                    }

                    // Request redraw to apply changes
                    window.request_redraw();
                }
            }

            // Handle window moved - surface may become invalid when moving between monitors
            WindowEvent::Moved(_) => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    log::debug!(
                        "Window moved - reconfiguring surface for potential display change"
                    );

                    // Reconfigure surface to handle potential display changes
                    // This catches cases where displays have same DPI but different surface properties
                    renderer.reconfigure_surface();

                    // On macOS, reconfigure the Metal layer for the potentially new display
                    #[cfg(target_os = "macos")]
                    {
                        if let Err(e) =
                            crate::macos_metal::configure_metal_layer_for_performance(window)
                        {
                            log::warn!(
                                "Failed to reconfigure Metal layer after window move: {}",
                                e
                            );
                        }
                    }

                    // Request redraw to ensure proper rendering on new display
                    window.request_redraw();
                }
            }

            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = &mut self.renderer {
                    let (cols, rows) = renderer.resize(physical_size);

                    // Calculate text area pixel dimensions
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;

                    // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
                    // This allows applications like kitty icat to query pixel dimensions
                    // Note: The core library (v0.11.0+) implements scrollback reflow when
                    // width changes - wrapped lines are unwrapped/re-wrapped as needed.
                    for tab in self.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                            tab.cache.scrollback_len = term.scrollback_len();
                        }
                        // Invalidate cell cache to force regeneration
                        tab.cache.cells = None;
                    }

                    // Update scrollbar for active tab
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let total_lines = rows + tab.cache.scrollback_len;
                        renderer.update_scrollbar(tab.scroll_state.offset, rows, total_lines);
                    }

                    // Update resize overlay state
                    self.resize_dimensions =
                        Some((physical_size.width, physical_size.height, cols, rows));
                    self.resize_overlay_visible = true;
                    // Hide overlay 1 second after resize stops
                    self.resize_overlay_hide_time =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

                    // Update settings UI with current terminal dimensions
                    self.settings_ui.update_current_size(cols, rows);
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event, event_loop);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.input_handler.update_modifiers(modifiers);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // Skip if egui UI is handling mouse
                if !self.is_egui_using_pointer() {
                    self.handle_mouse_wheel(delta);
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                // Skip if egui UI is handling mouse
                if !self.is_egui_using_pointer() {
                    self.handle_mouse_button(button, state);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Skip if egui UI is handling mouse
                if !self.is_egui_using_pointer() {
                    self.handle_mouse_move((position.x, position.y));
                }
            }

            WindowEvent::Focused(focused) => {
                self.handle_focus_change(focused);
            }

            WindowEvent::RedrawRequested => {
                // Skip rendering if shutting down
                if self.is_shutting_down {
                    return false;
                }

                // Check if active tab's shell has exited and close window/tab if configured
                if self.config.exit_on_shell_exit {
                    // First check if shell exited (gather info without mutable borrows)
                    let (shell_exited, active_tab_id, tab_count) = {
                        let exited = self.tab_manager.active_tab().is_some_and(|tab| {
                            tab.terminal
                                .try_lock()
                                .ok()
                                .is_some_and(|term| !term.is_running())
                        });
                        let tab_id = self.tab_manager.active_tab_id();
                        let count = self.tab_manager.tab_count();
                        (exited, tab_id, count)
                    };

                    if shell_exited {
                        log::info!("Shell in active tab has exited");
                        if tab_count <= 1 {
                            // Last tab - close window
                            log::info!("Last tab, closing window");
                            self.is_shutting_down = true;
                            for tab in self.tab_manager.tabs_mut() {
                                tab.stop_refresh_task();
                            }
                            return true;
                        } else if let Some(tab_id) = active_tab_id {
                            // Close just this tab
                            let _ = self.tab_manager.close_tab(tab_id);
                        }
                    }
                }

                self.render();
            }

            _ => {}
        }

        false // Don't close window
    }

    /// Handle window focus change for power saving
    pub(crate) fn handle_focus_change(&mut self, focused: bool) {
        if self.is_focused == focused {
            return; // No change
        }

        self.is_focused = focused;
        log::info!(
            "Window focus changed: {}",
            if focused { "focused" } else { "blurred" }
        );

        // Handle shader animation pause/resume
        if self.config.pause_shaders_on_blur
            && let Some(renderer) = &mut self.renderer
        {
            if focused {
                // Only resume if user has animation enabled in config
                renderer.resume_shader_animations(
                    self.config.custom_shader_animation,
                    self.config.cursor_shader_animation,
                );
            } else {
                renderer.pause_shader_animations();
            }
        }

        // Handle refresh rate adjustment for all tabs
        if self.config.pause_refresh_on_blur
            && let Some(window) = &self.window
        {
            let fps = if focused {
                self.config.max_fps
            } else {
                self.config.unfocused_fps
            };
            for tab in self.tab_manager.tabs_mut() {
                tab.stop_refresh_task();
                tab.start_refresh_task(Arc::clone(&self.runtime), Arc::clone(window), fps);
            }
            log::info!(
                "Adjusted refresh rate to {} FPS ({})",
                fps,
                if focused { "focused" } else { "unfocused" }
            );
        }

        // Request a redraw when focus changes
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Process per-window updates in about_to_wait
    pub(crate) fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Skip all processing if shutting down
        if self.is_shutting_down {
            return;
        }

        // Check for and deliver notifications (OSC 9/777)
        self.check_notifications();

        // Check for bell events and play audio/visual feedback
        self.check_bell();

        // Check for shader hot reload events
        if self.check_shader_reload() {
            log::debug!("Shader hot reload triggered redraw");
        }

        // Update window title with shell integration info (CWD, exit code)
        self.update_window_title_with_shell_integration();

        // --- POWER SAVING & SMART REDRAW LOGIC ---
        // We use ControlFlow::WaitUntil to sleep until the next expected event.
        // This drastically reduces CPU/GPU usage compared to continuous polling (ControlFlow::Poll).
        // The loop calculates the earliest time any component needs to update.

        let now = std::time::Instant::now();
        let mut next_wake = now + std::time::Duration::from_secs(1); // Default sleep for 1s of inactivity

        // Calculate frame interval based on focus state for power saving
        // When pause_refresh_on_blur is enabled and window is unfocused, use slower refresh rate
        let frame_interval_ms = if self.config.pause_refresh_on_blur && !self.is_focused {
            // Use unfocused FPS (e.g., 10 FPS = 100ms interval)
            1000 / self.config.unfocused_fps.max(1)
        } else {
            // Use normal animation rate based on max_fps
            1000 / self.config.max_fps.max(1)
        };
        let frame_interval = std::time::Duration::from_millis(frame_interval_ms as u64);

        // Check if enough time has passed since last render for FPS throttling
        let time_since_last_render = self
            .last_render_time
            .map(|t| now.duration_since(t))
            .unwrap_or(frame_interval); // If no last render, allow immediate render
        let can_render = time_since_last_render >= frame_interval;

        // 1. Cursor Blinking
        // Wake up exactly when the cursor needs to toggle visibility or fade.
        // Skip cursor blinking when unfocused with pause_refresh_on_blur to save power.
        if self.config.cursor_blink && (self.is_focused || !self.config.pause_refresh_on_blur) {
            if self.cursor_blink_timer.is_none() {
                let blink_interval =
                    std::time::Duration::from_millis(self.config.cursor_blink_interval);
                self.cursor_blink_timer = Some(now + blink_interval);
            }

            if let Some(next_blink) = self.cursor_blink_timer {
                if now >= next_blink {
                    // Time to toggle: trigger redraw (if throttle allows) and schedule next phase
                    if can_render {
                        self.needs_redraw = true;
                    }
                    let blink_interval =
                        std::time::Duration::from_millis(self.config.cursor_blink_interval);
                    self.cursor_blink_timer = Some(now + blink_interval);
                } else if next_blink < next_wake {
                    // Schedule wake-up for the next toggle
                    next_wake = next_blink;
                }
            }
        }

        // 2. Smooth Scrolling & Animations
        // If a scroll interpolation or terminal animation is active, use calculated frame interval.
        if let Some(tab) = self.tab_manager.active_tab() {
            if tab.scroll_state.animation_start.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 3. Visual Bell Feedback
            // Maintain frame rate during the visual flash fade-out.
            if tab.bell.visual_flash.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 4. Interactive UI Elements
            // Ensure high responsiveness during mouse dragging (text selection or scrollbar).
            // Always allow these for responsiveness, even if throttled.
            if (tab.mouse.is_selecting
                || tab.mouse.selection.is_some()
                || tab.scroll_state.dragging)
                && tab.mouse.button_pressed
            {
                self.needs_redraw = true;
            }
        }

        // 5. Resize Overlay
        // Check if the resize overlay should be hidden (timer expired).
        if self.resize_overlay_visible
            && let Some(hide_time) = self.resize_overlay_hide_time
        {
            if now >= hide_time {
                // Hide the overlay
                self.resize_overlay_visible = false;
                self.resize_overlay_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Overlay still visible - request redraw and schedule wake
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 6. Custom Background Shaders
        // If a custom shader is animated, render at the calculated frame interval.
        // When unfocused with pause_refresh_on_blur, this uses the slower unfocused_fps rate.
        if let Some(renderer) = &self.renderer
            && renderer.needs_continuous_render()
        {
            if can_render {
                self.needs_redraw = true;
            }
            // Schedule next frame at the appropriate interval
            let next_frame = self
                .last_render_time
                .map(|t| t + frame_interval)
                .unwrap_or(now);
            // Ensure we don't schedule in the past
            let next_frame = if next_frame <= now {
                now + frame_interval
            } else {
                next_frame
            };
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // --- TRIGGER REDRAW ---
        // Request a redraw if any of the logic above determined an update is due.
        if self.needs_redraw
            && let Some(window) = &self.window
        {
            window.request_redraw();
            self.needs_redraw = false;
        }

        // Set the calculated sleep interval
        event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
    }
}

impl ApplicationHandler for WindowManager {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the first window on app resume (or if all windows were closed on some platforms)
        if self.windows.is_empty() {
            self.create_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // Check if this event is for the settings window
        if self.is_settings_window(window_id) {
            if let Some(action) = self.handle_settings_window_event(event) {
                use crate::settings_window::SettingsWindowAction;
                match action {
                    SettingsWindowAction::Close => {
                        // Already handled in handle_settings_window_event
                    }
                    SettingsWindowAction::ApplyConfig(config) => {
                        // Apply live config changes to all terminal windows
                        self.apply_config_to_windows(&config);
                    }
                    SettingsWindowAction::SaveConfig(config) => {
                        // Save config to disk and apply to all windows
                        if let Err(e) = config.save() {
                            log::error!("Failed to save config: {}", e);
                        } else {
                            log::info!("Configuration saved successfully");
                        }
                        self.apply_config_to_windows(&config);
                        // Update settings window with saved config
                        if let Some(settings_window) = &mut self.settings_window {
                            settings_window.update_config(config);
                        }
                    }
                    SettingsWindowAction::ApplyShader(shader_result) => {
                        let _ = self.apply_shader_from_editor(&shader_result.source);
                    }
                    SettingsWindowAction::ApplyCursorShader(cursor_shader_result) => {
                        let _ = self.apply_cursor_shader_from_editor(&cursor_shader_result.source);
                    }
                    SettingsWindowAction::None => {}
                }
            }
            return;
        }

        // Route event to the appropriate terminal window
        let should_close = if let Some(window_state) = self.windows.get_mut(&window_id) {
            window_state.handle_window_event(event_loop, event)
        } else {
            false
        };

        // Close window if requested
        if should_close {
            self.close_window(window_id);
        }

        // Exit if no windows remain
        if self.should_exit {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Process menu events
        // Find focused window (for now, use the first window if any)
        let focused_window = self.windows.keys().next().copied();
        self.process_menu_events(event_loop, focused_window);

        // Check if any window requested opening the settings window
        // Also collect shader reload results for propagation to standalone settings window
        let mut open_settings = false;
        let mut background_shader_result: Option<Option<String>> = None;
        let mut cursor_shader_result: Option<Option<String>> = None;

        for window_state in self.windows.values_mut() {
            if window_state.open_settings_window_requested {
                window_state.open_settings_window_requested = false;
                open_settings = true;
            }
            window_state.about_to_wait(event_loop);

            // Collect shader reload results and clear them from window_state
            if let Some(result) = window_state.background_shader_reload_result.take() {
                background_shader_result = Some(result);
            }
            if let Some(result) = window_state.cursor_shader_reload_result.take() {
                cursor_shader_result = Some(result);
            }
        }

        // Open settings window if requested
        if open_settings {
            self.open_settings_window(event_loop);
        }

        // Propagate shader reload results to standalone settings window
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(result) = background_shader_result {
                match result {
                    Some(err) => settings_window.set_shader_error(Some(err)),
                    None => settings_window.clear_shader_error(),
                }
            }
            if let Some(result) = cursor_shader_result {
                match result {
                    Some(err) => settings_window.set_cursor_shader_error(Some(err)),
                    None => settings_window.clear_cursor_shader_error(),
                }
            }
        }

        // Request redraw for settings window if it needs continuous updates
        self.request_settings_redraw();

        // Exit if no windows remain
        if self.should_exit {
            event_loop.exit();
        }
    }
}
