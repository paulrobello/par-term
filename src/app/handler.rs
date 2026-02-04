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

            // Add window number if configured
            if self.config.show_window_number {
                title_parts.push(format!("[{}]", self.window_index));
            }

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
        let (egui_consumed, egui_needs_repaint) =
            if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
                let event_response = egui_state.on_window_event(window, &event);
                // Request redraw if egui needs it (e.g., text input in modals)
                if event_response.repaint {
                    window.request_redraw();
                }
                (event_response.consumed, event_response.repaint)
            } else {
                (false, false)
            };
        let _ = egui_needs_repaint; // Used above, silence unused warning

        // Debug: Log when egui consumes events but we ignore it
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        let any_ui_visible = self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.shader_install_ui.visible
            || self.integrations_ui.visible
            || self.profile_modal_ui.visible;
        if egui_consumed
            && !any_ui_visible
            && let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            && let Key::Named(NamedKey::Space) = &key_event.logical_key
        {
            log::debug!("egui tried to consume Space (UI closed, ignoring)");
        }

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

                // Save last working directory for "previous session" mode
                if self.config.startup_directory_mode
                    == crate::config::StartupDirectoryMode::Previous
                    && let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                    && let Some(cwd) = term.shell_integration_cwd()
                {
                    log::info!("Saving last working directory: {}", cwd);
                    if let Err(e) = self.config.save_last_working_directory(&cwd) {
                        log::warn!("Failed to save last working directory: {}", e);
                    }
                }

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
                        let marks = if let Ok(term) = tab.terminal.try_lock() {
                            term.scrollback_marks()
                        } else {
                            Vec::new()
                        };
                        renderer.update_scrollbar(
                            tab.scroll_state.offset,
                            rows,
                            total_lines,
                            &marks,
                        );
                    }

                    // Update resize overlay state
                    self.resize_dimensions =
                        Some((physical_size.width, physical_size.height, cols, rows));
                    self.resize_overlay_visible = true;
                    // Hide overlay 1 second after resize stops
                    self.resize_overlay_hide_time =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

                    // Notify tmux of the new size if gateway mode is active
                    self.notify_tmux_of_resize();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event, event_loop);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.input_handler.update_modifiers(modifiers);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // Skip terminal handling if egui UI is visible or using the pointer
                // Note: any_ui_visible check is needed because is_egui_using_pointer()
                // returns false before egui is initialized (e.g., at startup when
                // shader_install_ui is shown before first render)
                if !any_ui_visible && !self.is_egui_using_pointer() {
                    self.handle_mouse_wheel(delta);
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                use winit::event::ElementState;
                // Track UI mouse consumption to prevent release events bleeding through
                // when UI closes during a click (e.g., drawer toggle)
                let ui_wants_pointer = any_ui_visible || self.is_egui_using_pointer();

                if state == ElementState::Pressed {
                    if ui_wants_pointer {
                        self.ui_consumed_mouse_press = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        self.ui_consumed_mouse_press = false;
                        self.handle_mouse_button(button, state);
                    }
                } else {
                    // Release: block if we consumed the press OR if UI wants pointer
                    if self.ui_consumed_mouse_press || ui_wants_pointer {
                        self.ui_consumed_mouse_press = false;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        self.handle_mouse_button(button, state);
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Skip terminal handling if egui UI is visible or using the pointer
                if any_ui_visible || self.is_egui_using_pointer() {
                    // Request redraw so egui can update hover states
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
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

                // Check for exited panes in all tabs and close them
                if self.config.exit_on_shell_exit {
                    // Track tabs that need terminal resize and tabs that should close
                    let mut tabs_needing_resize: Vec<crate::tab::TabId> = Vec::new();

                    // Collect tabs that need to be closed (all panes exited)
                    let tabs_to_close: Vec<crate::tab::TabId> = self
                        .tab_manager
                        .tabs_mut()
                        .iter_mut()
                        .filter_map(|tab| {
                            // Skip tmux tabs - they don't have local shells
                            // tmux pane content comes from tmux, not local PTY
                            if tab.tmux_gateway_active || tab.tmux_pane_id.is_some() {
                                return None;
                            }

                            // Check for exited panes in this tab
                            if tab.pane_manager.is_some() {
                                let (closed_panes, tab_should_close) = tab.close_exited_panes();
                                if !closed_panes.is_empty() {
                                    log::info!(
                                        "Tab {}: closed {} exited pane(s)",
                                        tab.id,
                                        closed_panes.len()
                                    );
                                    // Mark for terminal resize if tab still has panes
                                    if !tab_should_close {
                                        tabs_needing_resize.push(tab.id);
                                    }
                                }
                                if tab_should_close {
                                    return Some(tab.id);
                                }
                            }
                            None
                        })
                        .collect();

                    // Resize terminals for tabs that had panes closed but still have remaining panes
                    if !tabs_needing_resize.is_empty()
                        && let Some(renderer) = &self.renderer
                    {
                        let cell_width = renderer.cell_width();
                        let cell_height = renderer.cell_height();
                        let padding = self.config.window_padding;
                        for tab_id in tabs_needing_resize {
                            if let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                                && let Some(pm) = tab.pane_manager_mut()
                            {
                                pm.resize_all_terminals_with_padding(
                                    cell_width,
                                    cell_height,
                                    padding,
                                );
                                crate::debug_info!(
                                    "PANE_RESIZE",
                                    "Resized terminals after pane closure in tab {}",
                                    tab_id
                                );
                            }
                        }
                    }

                    // Close tabs that have no panes left
                    for tab_id in &tabs_to_close {
                        log::info!("Closing tab {} - all panes exited", tab_id);
                        if self.tab_manager.tab_count() <= 1 {
                            // Last tab - close window
                            log::info!("Last tab, closing window");
                            self.is_shutting_down = true;
                            for tab in self.tab_manager.tabs_mut() {
                                tab.stop_refresh_task();
                            }
                            return true;
                        } else {
                            let _ = self.tab_manager.close_tab(*tab_id);
                        }
                    }

                    // Also check legacy single-pane tabs (no pane_manager)
                    let (shell_exited, active_tab_id, tab_count, tab_title, exit_notified) = {
                        if let Some(tab) = self.tab_manager.active_tab() {
                            // Only check legacy terminal if no pane_manager
                            let exited = tab.pane_manager.is_none()
                                && tab
                                    .terminal
                                    .try_lock()
                                    .ok()
                                    .is_some_and(|term| !term.is_running());
                            (
                                exited,
                                Some(tab.id),
                                self.tab_manager.tab_count(),
                                tab.title.clone(),
                                tab.exit_notified,
                            )
                        } else {
                            (false, None, 0, String::new(), false)
                        }
                    };

                    if shell_exited {
                        log::info!("Shell in active tab has exited");

                        // Send session exit notification BEFORE closing tab/window
                        if self.config.notification_session_ended && !exit_notified {
                            // Mark as notified to prevent duplicates
                            if let Some(tab) = self.tab_manager.active_tab_mut() {
                                tab.exit_notified = true;
                            }
                            let title = format!("Session Ended: {}", tab_title);
                            let message = "The shell process has exited".to_string();
                            log::info!("Session exit notification: {} has exited", tab_title);
                            self.deliver_notification(&title, &message);
                        }

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

            WindowEvent::DroppedFile(path) => {
                self.handle_dropped_file(path);
            }

            WindowEvent::CursorEntered { .. } => {
                // Focus follows mouse: auto-focus window when cursor enters
                if self.config.focus_follows_mouse
                    && let Some(window) = &self.window
                {
                    window.focus_window();
                }
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

        // Update renderer focus state for unfocused cursor styling
        if let Some(renderer) = &mut self.renderer {
            renderer.set_focused(focused);
        }

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

        // Re-assert tmux client size when window gains focus
        // This ensures par-term's size is respected even after other clients resize tmux
        if focused {
            self.notify_tmux_of_resize();
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

        // Check for activity/idle notifications
        self.check_activity_idle_notifications();

        // Check for session exit notifications
        self.check_session_exit_notifications();

        // Check for shader hot reload events
        if self.check_shader_reload() {
            log::debug!("Shader hot reload triggered redraw");
        }

        // Check for tmux control mode notifications
        if self.check_tmux_notifications() {
            self.needs_redraw = true;
        }

        // Update window title with shell integration info (CWD, exit code)
        self.update_window_title_with_shell_integration();

        // Check for automatic profile switching based on hostname detection (OSC 7)
        if self.check_auto_profile_switch() {
            self.needs_redraw = true;
        }

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

        // 5b. Toast Notification
        // Check if the toast notification should be hidden (timer expired).
        if self.toast_message.is_some()
            && let Some(hide_time) = self.toast_hide_time
        {
            if now >= hide_time {
                // Hide the toast
                self.toast_message = None;
                self.toast_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Toast still visible - request redraw and schedule wake
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

        // 7. Shader Install Dialog
        // Force continuous redraws when shader install dialog is visible (for spinner animation)
        // and when installation is in progress (to check for completion)
        if self.shader_install_ui.visible {
            self.needs_redraw = true;
            // Schedule frequent redraws for smooth spinner animation
            let next_frame = now + std::time::Duration::from_millis(16); // ~60fps
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 8. Anti-idle Keep-alive
        // Periodically send keep-alive codes to prevent SSH/connection timeouts.
        if let Some(next_anti_idle) = self.handle_anti_idle(now)
            && next_anti_idle < next_wake
        {
            next_wake = next_anti_idle;
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
                    SettingsWindowAction::TestNotification => {
                        // Send a test notification to verify permissions
                        self.send_test_notification();
                    }
                    SettingsWindowAction::OpenProfileManager => {
                        // Open the profile modal in the focused terminal window
                        if let Some(window_id) = self.get_focused_window_id()
                            && let Some(window_state) = self.windows.get_mut(&window_id)
                        {
                            window_state
                                .profile_modal_ui
                                .open(&window_state.profile_manager);
                            window_state.needs_redraw = true;
                            if let Some(window) = &window_state.window {
                                window.request_redraw();
                            }
                        }
                    }
                    SettingsWindowAction::None => {}
                }
            }
            return;
        }

        // Check if this is a resize event (before the event is consumed)
        let is_resize = matches!(event, WindowEvent::Resized(_));

        // Route event to the appropriate terminal window
        let (should_close, shader_states, grid_size) =
            if let Some(window_state) = self.windows.get_mut(&window_id) {
                let close = window_state.handle_window_event(event_loop, event);
                // Capture shader states to sync to settings window
                let states = (
                    window_state.config.custom_shader_enabled,
                    window_state.config.cursor_shader_enabled,
                );
                // Capture grid size if this was a resize
                let size = if is_resize {
                    window_state.renderer.as_ref().map(|r| r.grid_size())
                } else {
                    None
                };
                (close, Some(states), size)
            } else {
                (false, None, None)
            };

        // Sync shader states to settings window to prevent it from overwriting keybinding toggles
        if let (Some(settings_window), Some((custom_enabled, cursor_enabled))) =
            (&mut self.settings_window, shader_states)
        {
            settings_window.sync_shader_states(custom_enabled, cursor_enabled);
        }

        // Update settings window with new terminal dimensions after resize
        if let (Some(settings_window), Some((cols, rows))) = (&mut self.settings_window, grid_size)
        {
            settings_window.settings_ui.update_current_size(cols, rows);
        }

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
        // Check CLI timing-based options (exit-after, screenshot, command)
        self.check_cli_timers();

        // Check for updates (respects configured frequency)
        self.check_for_updates();

        // Process menu events
        // Find the actually focused window (the one with is_focused == true)
        let focused_window = self.get_focused_window_id();
        self.process_menu_events(event_loop, focused_window);

        // Check if any window requested opening the settings window
        // Also collect shader reload results for propagation to standalone settings window
        let mut open_settings = false;
        let mut background_shader_result: Option<Option<String>> = None;
        let mut cursor_shader_result: Option<Option<String>> = None;
        let mut profiles_to_update: Option<Vec<crate::profile::Profile>> = None;

        for window_state in self.windows.values_mut() {
            if window_state.open_settings_window_requested {
                window_state.open_settings_window_requested = false;
                open_settings = true;
            }

            // Check if profiles menu needs updating (from profile modal save)
            if window_state.profiles_menu_needs_update {
                window_state.profiles_menu_needs_update = false;
                // Get a copy of the profiles for menu update
                profiles_to_update = Some(window_state.profile_manager.to_vec());
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

        // Update profiles menu if profiles changed
        if let Some(profiles) = profiles_to_update
            && let Some(menu) = &mut self.menu
        {
            let profile_refs: Vec<&crate::profile::Profile> = profiles.iter().collect();
            menu.update_profiles(&profile_refs);
        }

        // Open settings window if requested (F12 or Cmd+,)
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
