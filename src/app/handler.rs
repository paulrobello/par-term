use crate::app::AppState;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

impl AppState {
    pub(crate) fn check_notifications(&mut self) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            // Check for OSC 9/777 notifications
            if term.has_notifications() {
                let notifications = term.take_notifications();
                for notif in notifications {
                    self.deliver_notification(&notif.title, &notif.message);
                }
            }
        }
    }

    pub(crate) fn check_bell(&mut self) {
        // Skip if all bell notifications are disabled
        if self.config.notification_bell_sound == 0
            && !self.config.notification_bell_visual
            && !self.config.notification_bell_desktop
        {
            return;
        }

        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let current_bell_count = term.bell_count();

            if current_bell_count > self.bell.last_count {
                // Bell event(s) occurred
                let bell_events = current_bell_count - self.bell.last_count;
                log::info!("🔔 Bell event detected ({} bell(s))", bell_events);
                log::info!(
                    "  Config: sound={}, visual={}, desktop={}",
                    self.config.notification_bell_sound,
                    self.config.notification_bell_visual,
                    self.config.notification_bell_desktop
                );

                // Play audio bell if enabled (volume > 0)
                if self.config.notification_bell_sound > 0 {
                    if let Some(audio_bell) = &self.bell.audio {
                        log::info!(
                            "  Playing audio bell at {}% volume",
                            self.config.notification_bell_sound
                        );
                        audio_bell.play(self.config.notification_bell_sound);
                    } else {
                        log::warn!("  Audio bell requested but not initialized");
                    }
                } else {
                    log::debug!("  Audio bell disabled (volume=0)");
                }

                // Trigger visual bell flash if enabled
                if self.config.notification_bell_visual {
                    log::info!("  Triggering visual bell flash");
                    self.bell.visual_flash = Some(std::time::Instant::now());
                    // Request immediate redraw to show flash
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    log::debug!("  Visual bell disabled");
                }

                // Send desktop notification if enabled
                if self.config.notification_bell_desktop {
                    log::info!("  Sending desktop notification");
                    let message = if bell_events == 1 {
                        "Terminal bell".to_string()
                    } else {
                        format!("Terminal bell ({} events)", bell_events)
                    };
                    self.deliver_notification("Terminal", &message);
                } else {
                    log::debug!("  Desktop notification disabled");
                }

                self.bell.last_count = current_bell_count;
            }
        }
    }

    #[allow(dead_code)]
    fn take_screenshot(&self) {
        log::info!("Taking screenshot...");

        if let Some(terminal) = &self.terminal {
            // Generate timestamp-based filename
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let format = &self.config.screenshot_format;
            let filename = format!("par-term_screenshot_{}.{}", timestamp, format);

            // Create screenshots directory in user's home dir
            if let Some(home_dir) = dirs::home_dir() {
                let screenshot_dir = home_dir.join("par-term-screenshots");
                if !screenshot_dir.exists()
                    && let Err(e) = std::fs::create_dir_all(&screenshot_dir)
                {
                    log::error!("Failed to create screenshot directory: {}", e);
                    self.deliver_notification(
                        "Screenshot Error",
                        &format!("Failed to create directory: {}", e),
                    );
                    return;
                }

                let path = screenshot_dir.join(&filename);
                let path_str = path.to_string_lossy().to_string();

                // Take screenshot (include scrollback for better context)
                let terminal_clone = Arc::clone(terminal);
                let format_clone = format.clone();

                // Use async to avoid blocking the UI
                let result = std::thread::spawn(move || {
                    if let Ok(term) = terminal_clone.try_lock() {
                        // Include 0 scrollback lines (just visible content)
                        term.screenshot_to_file(&path, &format_clone, 0)
                    } else {
                        Err(anyhow::anyhow!("Failed to lock terminal"))
                    }
                })
                .join();

                match result {
                    Ok(Ok(())) => {
                        log::info!("Screenshot saved to: {}", path_str);
                        self.deliver_notification(
                            "Screenshot Saved",
                            &format!("Saved to: {}", path_str),
                        );
                    }
                    Ok(Err(e)) => {
                        log::error!("Failed to save screenshot: {}", e);
                        self.deliver_notification(
                            "Screenshot Error",
                            &format!("Failed to save: {}", e),
                        );
                    }
                    Err(e) => {
                        log::error!("Screenshot thread panicked: {:?}", e);
                        self.deliver_notification("Screenshot Error", "Screenshot thread failed");
                    }
                }
            } else {
                log::error!("Failed to get home directory");
                self.deliver_notification("Screenshot Error", "Failed to get home directory");
            }
        } else {
            log::warn!("No terminal available for screenshot");
            self.deliver_notification("Screenshot Error", "No terminal available");
        }
    }

    // TODO: Recording APIs not yet available in par-term-emu-core-rust
    // Uncomment when the core library supports recording again
    #[allow(dead_code)]
    fn toggle_recording(&mut self) {
        self.deliver_notification(
            "Recording Not Available",
            "Recording APIs are not yet implemented in the core library",
        );
    }

    fn deliver_notification(&self, title: &str, message: &str) {
        // Always log notifications
        if !title.is_empty() {
            log::info!("=== Notification: {} ===", title);
            log::info!("{}", message);
            log::info!("===========================");
        } else {
            log::info!("=== Notification ===");
            log::info!("{}", message);
            log::info!("===================");
        }

        // Send desktop notification if enabled
        #[cfg(not(target_os = "macos"))]
        {
            use notify_rust::Notification;
            let notification_title = if !title.is_empty() {
                title
            } else {
                "Terminal Notification"
            };

            if let Err(e) = Notification::new()
                .summary(notification_title)
                .body(message)
                .timeout(notify_rust::Timeout::Milliseconds(3000))
                .show()
            {
                log::warn!("Failed to send desktop notification: {}", e);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS notifications via osascript
            let notification_title = if !title.is_empty() {
                title
            } else {
                "Terminal Notification"
            };

            // Escape quotes in title and message for AppleScript
            let escaped_title = notification_title.replace('"', "\\\"");
            let escaped_message = message.replace('"', "\\\"");

            // Use osascript to display notification
            let script = format!(
                r#"display notification "{}" with title "{}""#,
                escaped_message, escaped_title
            );

            if let Err(e) = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
            {
                log::warn!("Failed to send macOS desktop notification: {}", e);
            }
        }
    }

    /// Update window title with shell integration info (cwd and exit code)
    /// Only updates if not scrolled and not hovering over URL
    pub(crate) fn update_window_title_with_shell_integration(&self) {
        // Skip if scrolled (scrollback indicator takes priority)
        if self.scroll_state.offset != 0 {
            return;
        }

        // Skip if hovering over URL (URL tooltip takes priority)
        if self.mouse.hovered_url.is_some() {
            return;
        }

        // Skip if window not available
        let window = if let Some(w) = &self.window {
            w
        } else {
            return;
        };

        // Skip if terminal not available
        let terminal = if let Some(t) = &self.terminal {
            t
        } else {
            return;
        };

        // Try to get shell integration info
        if let Ok(term) = terminal.try_lock() {
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
}

impl ApplicationHandler for AppState {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut window_attrs = Window::default_attributes()
                .with_title(&self.config.window_title)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.config.window_width,
                    self.config.window_height,
                ))
                .with_decorations(self.config.window_decorations);

            // Load and set the application icon
            let icon_bytes = include_bytes!("../../assets/icon.png");
            if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
                let rgba = icon_image.to_rgba8();
                let (width, height) = rgba.dimensions();
                if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), width, height) {
                    window_attrs = window_attrs.with_window_icon(Some(icon));
                    log::info!("Window icon set ({}x{})", width, height);
                } else {
                    log::warn!("Failed to create window icon from RGBA data");
                }
            } else {
                log::warn!("Failed to load embedded icon image");
            }

            // Set window always-on-top if requested
            if self.config.window_always_on_top {
                window_attrs =
                    window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
                log::info!("Window always-on-top enabled");
            }

            // Always enable window transparency support for runtime opacity changes
            // Even if starting at opacity 1.0, we need this for real-time updates
            window_attrs = window_attrs.with_transparent(true);
            log::info!(
                "Window transparency enabled (opacity: {})",
                self.config.window_opacity
            );

            match event_loop.create_window(window_attrs) {
                Ok(window) => {
                    // Initialize async components using the shared runtime
                    let runtime = Arc::clone(&self.runtime);
                    if let Err(e) = runtime.block_on(self.initialize_async(window)) {
                        log::error!("Failed to initialize: {}", e);
                        event_loop.exit();
                    }
                }
                Err(e) => {
                    log::error!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
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
                Key::Named(NamedKey::Space) => {
                    log::debug!("🔔 SPACE EVENT: state={:?}", key_event.state);
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
        let any_ui_visible =
            self.settings_ui.visible || self.help_ui.visible || self.clipboard_history_ui.visible;
        if egui_consumed
            && any_ui_visible
            && !matches!(
                event,
                WindowEvent::CloseRequested | WindowEvent::RedrawRequested
            )
        {
            if let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            {
                match &key_event.logical_key {
                    Key::Named(NamedKey::Space) => {
                        log::debug!("egui consumed Space while UI panel is visible")
                    }
                    Key::Named(_) => {
                        log::debug!("egui consumed named key while UI panel is visible")
                    }
                    _ => {}
                }
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested, cleaning up and exiting");
                // Set shutdown flag to stop redraw loop
                self.is_shutting_down = true;
                // Abort the refresh task to prevent lockup on shutdown
                if let Some(task) = self.refresh_task.take() {
                    task.abort();
                    log::info!("Refresh task aborted");
                }
                event_loop.exit();
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

                    // Resize terminal with pixel dimensions for TIOCGWINSZ support
                    if let Some(terminal) = &self.terminal
                        && let Ok(mut term) = terminal.try_lock()
                    {
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
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

                    // Resize terminal with pixel dimensions for TIOCGWINSZ support
                    // This allows applications like kitty icat to query pixel dimensions
                    // Note: The core library (v0.11.0+) implements scrollback reflow when
                    // width changes - wrapped lines are unwrapped/re-wrapped as needed.
                    if let Some(terminal) = &self.terminal
                        && let Ok(mut term) = terminal.try_lock()
                    {
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        self.cache.scrollback_len = term.scrollback_len();

                        // Update scrollbar internal state
                        let total_lines = rows + self.cache.scrollback_len;
                        renderer.update_scrollbar(self.scroll_state.offset, rows, total_lines);
                    }

                    // Invalidate cell cache to force regeneration
                    self.cache.cells = None;
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

            WindowEvent::RedrawRequested => {
                // Skip rendering if shutting down
                if self.is_shutting_down {
                    return;
                }

                // Check if shell has exited and close window if configured
                if self.config.exit_on_shell_exit
                    && let Some(terminal) = &self.terminal
                    && let Ok(term) = terminal.try_lock()
                    && !term.is_running()
                {
                    log::info!("Shell has exited, closing terminal");
                    // Set shutdown flag to stop redraw loop
                    self.is_shutting_down = true;
                    // Abort the refresh task to prevent lockup on shutdown
                    if let Some(task) = self.refresh_task.take() {
                        task.abort();
                        log::info!("Refresh task aborted");
                    }
                    event_loop.exit();
                    return;
                }

                self.render();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Skip all processing if shutting down
        if self.is_shutting_down {
            return;
        }

        // Check for and deliver notifications (OSC 9/777)
        self.check_notifications();

        // Check for bell events and play audio/visual feedback
        self.check_bell();

        // Update window title with shell integration info (CWD, exit code)
        self.update_window_title_with_shell_integration();

        // --- POWER SAVING & SMART REDRAW LOGIC ---
        // We use ControlFlow::WaitUntil to sleep until the next expected event.
        // This drastically reduces CPU/GPU usage compared to continuous polling (ControlFlow::Poll).
        // The loop calculates the earliest time any component needs to update.

        let now = std::time::Instant::now();
        let mut next_wake = now + std::time::Duration::from_secs(1); // Default sleep for 1s of inactivity

        // 1. Cursor Blinking
        // Wake up exactly when the cursor needs to toggle visibility or fade.
        if self.config.cursor_blink {
            if self.cursor_blink_timer.is_none() {
                let blink_interval =
                    std::time::Duration::from_millis(self.config.cursor_blink_interval);
                self.cursor_blink_timer = Some(now + blink_interval);
            }

            if let Some(next_blink) = self.cursor_blink_timer {
                if now >= next_blink {
                    // Time to toggle: trigger redraw and schedule next phase
                    self.needs_redraw = true;
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
        // If a scroll interpolation or terminal animation is active, target ~60 FPS (16.6ms).
        if self.scroll_state.animation_start.is_some() {
            self.needs_redraw = true;
            let next_frame = now + std::time::Duration::from_millis(16);
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 3. Visual Bell Feedback
        // Maintain high frame rate during the visual flash fade-out.
        if self.bell.visual_flash.is_some() {
            self.needs_redraw = true;
            let next_frame = now + std::time::Duration::from_millis(16);
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 4. Interactive UI Elements
        // Ensure high responsiveness during mouse dragging (text selection or scrollbar).
        if (self.mouse.is_selecting || self.mouse.selection.is_some() || self.scroll_state.dragging)
            && self.mouse.button_pressed
        {
            self.needs_redraw = true;
        }

        // 5. Custom Background Shaders
        // If a custom shader is animated, we must render continuously at high FPS.
        if let Some(renderer) = &self.renderer
            && renderer.needs_continuous_render()
        {
            self.needs_redraw = true;
            let next_frame = now + std::time::Duration::from_millis(16);
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
