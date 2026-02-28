//! Window creation, destruction, positioning, and session management.
//!
//! This module handles the lifecycle of terminal windows: creating new windows
//! with proper configuration, applying monitor-based positioning, restoring
//! saved sessions, and closing windows cleanly.

use std::path::PathBuf;
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
    /// Check and handle timing-based CLI options (exit-after, screenshot, command)
    pub fn check_cli_timers(&mut self) {
        let Some(start_time) = self.start_time else {
            return;
        };

        let elapsed = start_time.elapsed().as_secs_f64();

        // Send command after 1 second delay
        if !self.command_sent
            && elapsed >= 1.0
            && let Some(cmd) = self.runtime_options.command_to_send.clone()
        {
            self.send_command_to_shell(&cmd);
            self.command_sent = true;
        }

        // Take screenshot if requested (after exit_after - 1 second, or after 2 seconds if no exit_after)
        if !self.screenshot_taken && self.runtime_options.screenshot.is_some() {
            let screenshot_time = self
                .runtime_options
                .exit_after
                .map(|t| t - 1.0)
                .unwrap_or(2.0);
            if elapsed >= screenshot_time {
                self.take_screenshot();
                self.screenshot_taken = true;
            }
        }

        // Exit after specified time
        if let Some(exit_after) = self.runtime_options.exit_after
            && elapsed >= exit_after
        {
            log::info!("Exit-after timer expired ({:.1}s), exiting", exit_after);
            self.should_exit = true;
        }
    }

    /// Send a command to the shell
    pub(super) fn send_command_to_shell(&mut self, cmd: &str) {
        // Send to the first window's active tab
        if let Some(window_state) = self.windows.values_mut().next()
            && let Some(tab) = window_state.tab_manager.active_tab_mut()
        {
            // Send the command followed by Enter
            let cmd_with_enter = format!("{}\n", cmd);
            if let Ok(term) = tab.terminal.try_lock() {
                if let Err(e) = term.write(cmd_with_enter.as_bytes()) {
                    log::error!("Failed to send command to shell: {}", e);
                } else {
                    log::info!("Sent command to shell: {}", cmd);
                }
            }
        }
    }

    /// Take a screenshot
    pub(super) fn take_screenshot(&mut self) {
        log::info!("Taking screenshot...");

        // Determine output path
        let output_path = match &self.runtime_options.screenshot {
            Some(path) if !path.as_os_str().is_empty() => {
                log::info!("Screenshot path specified: {:?}", path);
                path.clone()
            }
            _ => {
                // Generate timestamped filename
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let path = PathBuf::from(format!("par-term-{}.png", timestamp));
                log::info!("Using auto-generated screenshot path: {:?}", path);
                path
            }
        };

        // Get the first window and take screenshot
        if let Some(window_state) = self.windows.values_mut().next() {
            if let Some(renderer) = &mut window_state.renderer {
                log::info!("Capturing screenshot from renderer...");
                match renderer.take_screenshot() {
                    Ok(image_data) => {
                        log::info!(
                            "Screenshot captured: {}x{} pixels",
                            image_data.width(),
                            image_data.height()
                        );
                        // Save the image
                        if let Err(e) = image_data.save(&output_path) {
                            log::error!("Failed to save screenshot to {:?}: {}", output_path, e);
                        } else {
                            log::info!("Screenshot saved to {:?}", output_path);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to take screenshot: {}", e);
                    }
                }
            } else {
                log::warn!("No renderer available for screenshot");
            }
        } else {
            log::warn!("No window available for screenshot");
        }
    }

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
                let mut window_state =
                    WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
                // Set window index for title formatting (window_number calculated earlier)
                window_state.window_index = window_number;

                // Initialize async components using the shared runtime
                let runtime = Arc::clone(&self.runtime);
                if let Err(e) = runtime.block_on(window_state.initialize_async(window, None)) {
                    log::error!("Failed to initialize window: {}", e);
                    return;
                }

                // Initialize menu for the first window (macOS global menu) or per-window (Windows/Linux)
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            // Attach menu to window (platform-specific)
                            if let Some(win) = &window_state.window
                                && let Err(e) = menu.init_for_window(win)
                            {
                                log::warn!("Failed to initialize menu for window: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                } else if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    // For additional windows on Windows/Linux, attach menu
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

    /// Save session state on a background thread to avoid blocking the main thread.
    /// Captures state synchronously (fast, in-memory) then spawns disk I/O.
    pub(super) fn save_session_state_background(&self) {
        let state = crate::session::capture::capture_session(&self.windows);
        let _ = std::thread::Builder::new()
            .name("session-save".into())
            .spawn(move || {
                if let Err(e) = crate::session::storage::save_session(&state) {
                    log::error!("Failed to save session state: {}", e);
                }
            });
    }

    /// Restore windows from the last saved session.
    ///
    /// Returns true if session was successfully restored, false otherwise.
    pub fn restore_session(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let session = match crate::session::storage::load_session() {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::info!("No saved session found, creating default window");
                return false;
            }
            Err(e) => {
                log::warn!(
                    "Failed to load session state: {}, creating default window",
                    e
                );
                return false;
            }
        };

        if session.windows.is_empty() {
            log::info!("Saved session has no windows, creating default window");
            return false;
        }

        log::info!(
            "Restoring session ({} windows) saved at {}",
            session.windows.len(),
            session.saved_at
        );

        for session_window in &session.windows {
            // Validate CWDs for tabs
            let tab_cwds: Vec<Option<String>> = session_window
                .tabs
                .iter()
                .map(|tab| crate::session::restore::validate_cwd(&tab.cwd))
                .collect();

            let created_window_id = self.create_window_with_overrides(
                event_loop,
                session_window.position,
                session_window.size,
                &tab_cwds,
                session_window.active_tab_index,
            );

            // Restore pane layouts, user titles, custom colors, and icons
            if let Some(window_id) = created_window_id
                && let Some(window_state) = self.windows.get_mut(&window_id)
            {
                let tabs = window_state.tab_manager.tabs_mut();
                for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                    if let Some(ref layout) = session_tab.pane_layout
                        && let Some(tab) = tabs.get_mut(tab_idx)
                    {
                        tab.restore_pane_layout(layout, &self.config, Arc::clone(&self.runtime));
                    }
                }

                // Restore user titles, custom colors, and icons
                for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = session_tab.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = session_tab.custom_color {
                            tab.set_custom_color(color);
                        }
                        if let Some(ref icon) = session_tab.custom_icon {
                            tab.custom_icon = Some(icon.clone());
                        }
                    }
                }
            }
        }

        // Clear the saved session file after successful restore
        if let Err(e) = crate::session::storage::clear_session() {
            log::warn!("Failed to clear session file after restore: {}", e);
        }

        // If no windows were created (shouldn't happen), fall back
        if self.windows.is_empty() {
            log::warn!("Session restore created no windows, creating default");
            return false;
        }

        true
    }

    /// Create a new window with specific position and size overrides.
    ///
    /// Unlike `create_window()`, this skips `apply_window_positioning()` and
    /// places the window at the exact specified position and size.
    /// Additional tabs (beyond the first) are created with the given CWDs.
    pub fn create_window_with_overrides(
        &mut self,
        event_loop: &ActiveEventLoop,
        position: (i32, i32),
        size: (u32, u32),
        tab_cwds: &[Option<String>],
        active_tab_index: usize,
    ) -> Option<WindowId> {
        use winit::window::Window;

        // Reload config from disk to pick up any changes
        if let Ok(fresh_config) = Config::load() {
            self.config = fresh_config;
        }

        // Build window title
        let window_number = self.windows.len() + 1;
        let title = if self.config.show_window_number {
            format!("{} [{}]", self.config.window_title, window_number)
        } else {
            self.config.window_title.clone()
        };

        // position and size are in logical pixels (scale-factor-independent).
        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(size.0 as f64, size.1 as f64))
            .with_position(winit::dpi::LogicalPosition::new(
                position.0 as f64,
                position.1 as f64,
            ))
            .with_decorations(self.config.window_decorations);

        if self.config.lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
        }

        // Load and set the application icon
        let icon_bytes = include_bytes!("../../../assets/icon.png");
        if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
            let rgba = icon_image.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        }

        if self.config.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }

        window_attrs = window_attrs.with_transparent(true);

        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_id = window.id();
                let mut window_state =
                    WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
                window_state.window_index = window_number;

                // Extract the first tab's CWD to pass during initialization
                let first_tab_cwd = tab_cwds.first().and_then(|c| c.clone());

                let runtime = Arc::clone(&self.runtime);
                if let Err(e) =
                    runtime.block_on(window_state.initialize_async(window, first_tab_cwd))
                {
                    log::error!("Failed to initialize arranged window: {}", e);
                    return None;
                }

                // Initialize menu for first window or attach to additional
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            if let Some(win) = &window_state.window
                                && let Err(e) = menu.init_for_window(win)
                            {
                                log::warn!("Failed to initialize menu: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                } else if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    log::warn!("Failed to initialize menu for window: {}", e);
                }

                // Set the position explicitly (in case the WM overrode it).
                if let Some(win) = &window_state.window {
                    win.set_outer_position(winit::dpi::LogicalPosition::new(
                        position.0 as f64,
                        position.1 as f64,
                    ));
                }

                // Create remaining tabs (first tab was already created with CWD)
                let grid_size = window_state.renderer.as_ref().map(|r| r.grid_size());
                for cwd in tab_cwds.iter().skip(1) {
                    if let Err(e) = window_state.tab_manager.new_tab_with_cwd(
                        &self.config,
                        Arc::clone(&self.runtime),
                        cwd.clone(),
                        grid_size,
                    ) {
                        log::warn!("Failed to create tab in arranged window: {}", e);
                    }
                }

                // Switch to the saved active tab (switch_to_index is 1-based)
                window_state
                    .tab_manager
                    .switch_to_index(active_tab_index + 1);

                // Start refresh tasks for all tabs
                if let Some(win) = &window_state.window {
                    for tab in window_state.tab_manager.tabs_mut() {
                        tab.start_refresh_task(
                            Arc::clone(&self.runtime),
                            Arc::clone(win),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );
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

                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }

                log::info!(
                    "Created arranged window {:?} at ({}, {}) size {}x{} with {} tabs",
                    window_id,
                    position.0,
                    position.1,
                    size.0,
                    size.1,
                    tab_cwds.len().max(1),
                );

                Some(window_id)
            }
            Err(e) => {
                log::error!("Failed to create arranged window: {}", e);
                None
            }
        }
    }
}
