//! Window creation, destruction, and monitor positioning.
//!
//! This module handles the core lifecycle of terminal windows: creating new
//! windows with proper configuration, applying monitor-based positioning, and
//! closing windows cleanly.
//!
//! Session save/restore and arranged-window creation live in the sibling
//! `window_session` module.
//!
//! CLI timer handling (check_cli_timers, send_command_to_shell, take_screenshot)
//! lives in the sibling `cli_timer` module.

use std::sync::Arc;
use std::time::Instant;

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::menu::MenuManager;

use super::WindowManager;
use super::update_checker::update_available_version;

impl WindowManager {
    /// Create a new window with a fresh terminal session
    pub fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        use crate::config::WindowType;
        use crate::font_metrics::window_size_from_config;
        use winit::window::Window;

        // Reload config from disk to pick up any changes made by other windows
        if let Ok(fresh_config) = Config::load() {
            self.config = fresh_config;
        }

        // Calculate window size from cols/rows BEFORE window creation.
        let (width, height) = window_size_from_config(&self.config, 1.0).unwrap_or((800, 600));

        // Build window title, optionally including window number
        let window_number = self.windows.len() + 1;
        let title = if self.config.show_window_number {
            format!("{} [{}]", self.config.window_title, window_number)
        } else {
            self.config.window_title.clone()
        };

        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_decorations(self.config.window_decorations);

        // Lock window size if requested (prevent resize)
        if self.config.lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
            log::info!("Window size locked (resizing disabled)");
        }

        // Start in fullscreen if window_type is Fullscreen
        if self.config.window_type == WindowType::Fullscreen {
            window_attrs =
                window_attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            log::info!("Window starting in fullscreen mode");
        }

        // Load and set the application icon
        let icon_bytes = include_bytes!("../../../assets/icon.png");
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
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            log::info!("Window always-on-top enabled");
        }

        // Always enable window transparency support for runtime opacity changes
        window_attrs = window_attrs.with_transparent(true);
        log::info!(
            "Window transparency enabled (opacity: {})",
            self.config.window_opacity
        );

        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_id = window.id();

                // Initialize menu BEFORE the blocking GPU init so that macOS
                // menu accelerators (Cmd+, for Settings, Cmd+Q for Quit) are
                // registered immediately. Without this, there is a multi-second
                // window during GPU setup where winit's default menu is active
                // and unhandled key combos can cause the app to exit via
                // [NSApp terminate:].
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            if let Err(e) = menu.init_global() {
                                log::warn!("Failed to initialize global menu: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                }

                let mut window_state =
                    WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
                // Set window index for title formatting (window_number calculated earlier)
                window_state.window_index = window_number;

                // Initialize async components using the shared runtime
                // (GPU setup — this blocks for 2-3 seconds)
                let runtime = Arc::clone(&self.runtime);
                if let Err(e) = runtime.block_on(window_state.initialize_async(window, None)) {
                    log::error!("Failed to initialize window: {}", e);
                    return;
                }

                // Attach menu to the window (platform-specific: per-window on Windows/Linux)
                if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    log::warn!("Failed to initialize menu for window: {}", e);
                }

                // Apply target monitor and edge positioning after window creation
                if let Some(win) = &window_state.window {
                    self.apply_window_positioning(win, event_loop);
                }

                // Handle tmux auto-attach on first window only
                if self.windows.is_empty()
                    && window_state.config.tmux_enabled
                    && window_state.config.tmux_auto_attach
                {
                    let session_name = window_state.config.tmux_auto_attach_session.clone();

                    // Use gateway mode: writes tmux commands to existing PTY
                    if let Some(ref name) = session_name {
                        if !name.is_empty() {
                            log::info!(
                                "tmux auto-attach: attempting to attach to session '{}' via gateway",
                                name
                            );
                            match window_state.attach_tmux_gateway(name) {
                                Ok(()) => {
                                    log::info!(
                                        "tmux auto-attach: gateway initiated for session '{}'",
                                        name
                                    );
                                }
                                Err(e) => {
                                    log::warn!(
                                        "tmux auto-attach: failed to attach to '{}': {} - continuing without tmux",
                                        name,
                                        e
                                    );
                                    // Continue without tmux - don't fail startup
                                }
                            }
                        } else {
                            // Empty string means create new session
                            log::info!(
                                "tmux auto-attach: no session specified, creating new session via gateway"
                            );
                            if let Err(e) = window_state.initiate_tmux_gateway(None) {
                                log::warn!(
                                    "tmux auto-attach: failed to create new session: {} - continuing without tmux",
                                    e
                                );
                            }
                        }
                    } else {
                        // None means create new session
                        log::info!(
                            "tmux auto-attach: no session specified, creating new session via gateway"
                        );
                        if let Err(e) = window_state.initiate_tmux_gateway(None) {
                            log::warn!(
                                "tmux auto-attach: failed to create new session: {} - continuing without tmux",
                                e
                            );
                        }
                    }
                }

                self.windows.insert(window_id, window_state);
                self.pending_window_count += 1;

                // Sync existing update state to new window's status bar and dialog
                let update_version = self
                    .last_update_result
                    .as_ref()
                    .and_then(update_available_version);
                let update_result_clone = self.last_update_result.clone();
                let install_type = self.detect_installation_type();
                if let Some(ws) = self.windows.get_mut(&window_id) {
                    ws.status_bar_ui.update_available_version = update_version;
                    ws.update_state.last_result = update_result_clone;
                    ws.update_state.installation_type = install_type;
                }

                // Set start time on first window creation (for CLI timers)
                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }

                log::info!(
                    "Created new window {:?} (total: {})",
                    window_id,
                    self.windows.len()
                );
            }
            Err(e) => {
                log::error!("Failed to create window: {}", e);
            }
        }
    }

    /// Apply window positioning based on config (target monitor and edge anchoring)
    pub(super) fn apply_window_positioning(
        &self,
        window: &std::sync::Arc<winit::window::Window>,
        event_loop: &ActiveEventLoop,
    ) {
        use crate::config::WindowType;

        // Get list of available monitors
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        if monitors.is_empty() {
            log::warn!("No monitors available for window positioning");
            return;
        }

        // Select target monitor (default to primary/first)
        let monitor = if let Some(index) = self.config.target_monitor {
            monitors
                .get(index)
                .cloned()
                .or_else(|| monitors.first().cloned())
        } else {
            event_loop
                .primary_monitor()
                .or_else(|| monitors.first().cloned())
        };

        let Some(monitor) = monitor else {
            log::warn!("Could not determine target monitor");
            return;
        };

        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let window_size = window.outer_size();

        // Apply edge positioning if configured
        match self.config.window_type {
            WindowType::EdgeTop => {
                // Position at top of screen, spanning full width
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    monitor_pos.x,
                    monitor_pos.y,
                ));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    monitor_size.width,
                    window_size.height,
                ));
                log::info!("Window positioned at top edge of monitor");
            }
            WindowType::EdgeBottom => {
                // Position at bottom of screen, spanning full width
                let y = monitor_pos.y + monitor_size.height as i32 - window_size.height as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(monitor_pos.x, y));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    monitor_size.width,
                    window_size.height,
                ));
                log::info!("Window positioned at bottom edge of monitor");
            }
            WindowType::EdgeLeft => {
                // Position at left of screen, spanning full height
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    monitor_pos.x,
                    monitor_pos.y,
                ));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    window_size.width,
                    monitor_size.height,
                ));
                log::info!("Window positioned at left edge of monitor");
            }
            WindowType::EdgeRight => {
                // Position at right of screen, spanning full height
                let x = monitor_pos.x + monitor_size.width as i32 - window_size.width as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, monitor_pos.y));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    window_size.width,
                    monitor_size.height,
                ));
                log::info!("Window positioned at right edge of monitor");
            }
            WindowType::Normal | WindowType::Fullscreen => {
                // For normal/fullscreen, just position on target monitor if specified
                if self.config.target_monitor.is_some() {
                    // Center window on target monitor
                    let x =
                        monitor_pos.x + (monitor_size.width as i32 - window_size.width as i32) / 2;
                    let y = monitor_pos.y
                        + (monitor_size.height as i32 - window_size.height as i32) / 2;
                    window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                    log::info!(
                        "Window centered on monitor {} at ({}, {})",
                        self.config.target_monitor.unwrap_or(0),
                        x,
                        y
                    );
                }
            }
        }

        // Move window to target macOS Space if configured (macOS only, no-op on other platforms)
        if let Some(space) = self.config.target_space
            && let Err(e) = crate::macos_space::move_window_to_space(window, space)
        {
            log::warn!("Failed to move window to Space {}: {}", space, e);
        }
    }

    /// Close a specific window
    pub fn close_window(&mut self, window_id: WindowId) {
        // Save session state before removing the last window (while data is still available).
        if self.config.restore_session
            && self.windows.len() == 1
            && self.windows.contains_key(&window_id)
        {
            self.save_session_state_background();
        }

        if let Some(window_state) = self.windows.remove(&window_id) {
            log::info!(
                "Closing window {:?} (remaining: {})",
                window_id,
                self.windows.len()
            );
            // Hide window immediately for instant visual feedback
            if let Some(ref window) = window_state.window {
                window.set_visible(false);
            }
            // WindowState's Drop impl handles cleanup
            drop(window_state);
        }

        // Exit app when last window closes
        if self.windows.is_empty() {
            log::info!("Last window closed, exiting application");
            // Close settings window FIRST before marking exit
            if self.settings_window.is_some() {
                log::info!("Closing settings window before exit");
                self.close_settings_window();
            }
            self.should_exit = true;
        }
    }
}
