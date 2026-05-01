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

        // Re-apply CLI shader override (fresh config load above would wipe it)
        if let Some(ref shader) = self.runtime_options.shader {
            self.config.shader.custom_shader = Some(shader.clone());
            self.config.shader.custom_shader_enabled = true;
            self.config.background_image_enabled = false;
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
            .with_decorations(self.config.window.window_decorations);

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
        if self.config.window.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            log::info!("Window always-on-top enabled");
        }

        // Always enable window transparency support for runtime opacity changes
        window_attrs = window_attrs.with_transparent(true);
        log::info!(
            "Window transparency enabled (opacity: {})",
            self.config.window.window_opacity
        );

        // macOS: accept the first mouse click so that clicking the tab bar or
        // any other UI element while the window is in the background both brings
        // the window into focus AND delivers the click to the application.
        // Without this, macOS silently eats the activation click and the user
        // must click a second time to interact.  The existing focus-click
        // suppression path (`focus_click_pending`) still guards the terminal
        // area against forwarding clicks to PTY mouse-tracking apps.
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS as _;
            window_attrs = window_attrs.with_accepts_first_mouse(true);
        }

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
                if let Err(e) = runtime.block_on(window_state.initialize_async(window, false, None))
                {
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

    /// Create a window that will immediately receive a tab transferred via
    /// `move_tab`. Unlike [`Self::create_window`], this helper:
    ///
    /// - uses the caller-supplied `size` and `outer_position` instead of the
    ///   config default, so the new window matches the source window's geometry
    /// - calls `initialize_async(..., skip_default_tab = true, None)` so the new
    ///   window starts with an empty `TabManager`
    /// - skips tmux auto-attach and "first-window-only" side effects (menu init
    ///   is still performed once globally via `self.menu.is_none()`)
    ///
    /// Returns the new `WindowId`, or `None` on failure.
    pub(crate) fn create_window_for_moved_tab(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        size: winit::dpi::PhysicalSize<u32>,
        outer_position: winit::dpi::PhysicalPosition<i32>,
    ) -> Option<winit::window::WindowId> {
        use winit::window::Window;

        // Reload config from disk so the new window picks up latest settings.
        if let Ok(fresh_config) = Config::load() {
            self.config = fresh_config;
        }

        // Re-apply CLI shader override (fresh config load would wipe it).
        if let Some(ref shader) = self.runtime_options.shader {
            self.config.shader.custom_shader = Some(shader.clone());
            self.config.shader.custom_shader_enabled = true;
            self.config.background_image_enabled = false;
        }

        let window_number = self.windows.len() + 1;
        let title = if self.config.show_window_number {
            format!("{} [{}]", self.config.window_title, window_number)
        } else {
            self.config.window_title.clone()
        };

        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(size)
            .with_decorations(self.config.window.window_decorations)
            .with_transparent(true);

        if self.config.lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
        }

        // Icon
        let icon_bytes = include_bytes!("../../../assets/icon.png");
        if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
            let rgba = icon_image.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        }

        if self.config.window.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }

        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS as _;
            window_attrs = window_attrs.with_accepts_first_mouse(true);
        }

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => w,
            Err(e) => {
                crate::debug_error!(
                    "TAB",
                    "create_window_for_moved_tab: winit create_window failed: {}",
                    e
                );
                return None;
            }
        };
        let window_id = window.id();

        // Menu init (idempotent — only runs once globally).
        if self.menu.is_none() {
            match MenuManager::new() {
                Ok(menu) => {
                    if let Err(e) = menu.init_global() {
                        log::warn!("Failed to initialize global menu: {}", e);
                    }
                    self.menu = Some(menu);
                }
                Err(e) => log::warn!("Failed to create menu: {}", e),
            }
        }

        let mut window_state = WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
        window_state.window_index = window_number;

        let runtime = Arc::clone(&self.runtime);
        if let Err(e) = runtime.block_on(window_state.initialize_async(window, true, None)) {
            crate::debug_error!(
                "TAB",
                "create_window_for_moved_tab: initialize_async failed: {}",
                e
            );
            return None;
        }

        // Attach menu per-window (platform-specific).
        if let Some(menu) = &self.menu
            && let Some(win) = &window_state.window
            && let Err(e) = menu.init_for_window(win)
        {
            log::warn!("Failed to initialize menu for moved-tab window: {}", e);
        }

        // Apply the requested outer position. Clamp is the caller's responsibility.
        if let Some(win) = &window_state.window {
            win.set_outer_position(outer_position);
        }

        self.windows.insert(window_id, window_state);
        self.pending_window_count += 1;

        crate::debug_info!(
            "TAB",
            "Created new window {:?} for moved tab at {:?} size {:?}",
            window_id,
            outer_position,
            size
        );

        Some(window_id)
    }

    /// Compute the outer position for a newly-spawned "move to new window" window.
    ///
    /// Starts from the source window's outer position + (30, 30) and clamps so the
    /// full rect of the new window stays inside the source's monitor. If clamping
    /// would require moving back across the source, returns the source's exact
    /// outer position (new window stacks directly on top of the source).
    pub(crate) fn compute_moved_tab_outer_position(
        event_loop: &winit::event_loop::ActiveEventLoop,
        source_outer_pos: winit::dpi::PhysicalPosition<i32>,
        new_window_size: winit::dpi::PhysicalSize<u32>,
    ) -> winit::dpi::PhysicalPosition<i32> {
        const OFFSET: i32 = 30;
        let desired = winit::dpi::PhysicalPosition::new(
            source_outer_pos.x + OFFSET,
            source_outer_pos.y + OFFSET,
        );

        let monitors: Vec<_> = event_loop.available_monitors().collect();
        let source_monitor = monitors
            .iter()
            .find(|m| {
                let mp = m.position();
                let ms = m.size();
                source_outer_pos.x >= mp.x
                    && source_outer_pos.y >= mp.y
                    && source_outer_pos.x < mp.x + ms.width as i32
                    && source_outer_pos.y < mp.y + ms.height as i32
            })
            .or_else(|| monitors.first());

        let Some(monitor) = source_monitor else {
            return desired;
        };

        let mp = monitor.position();
        let ms = monitor.size();
        let max_x = mp.x + ms.width as i32 - new_window_size.width as i32;
        let max_y = mp.y + ms.height as i32 - new_window_size.height as i32;

        let clamped_x = desired.x.min(max_x).max(mp.x);
        let clamped_y = desired.y.min(max_y).max(mp.y);

        // If clamping pushed us back above the source (source is huge relative to
        // the monitor), stack on top of the source instead.
        if clamped_x <= source_outer_pos.x && clamped_y <= source_outer_pos.y {
            source_outer_pos
        } else {
            winit::dpi::PhysicalPosition::new(clamped_x, clamped_y)
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

    /// Produce `(WindowId, display_label)` pairs for every window *other than*
    /// `source_window_id`, suitable for the "Move Tab to Window ->" submenu.
    ///
    /// Label format: `Window N - <active tab title>`, falling back to `Window N`
    /// if the active tab has no meaningful title.
    pub(crate) fn other_window_labels(
        &self,
        source_window_id: WindowId,
    ) -> Vec<(WindowId, String)> {
        self.windows
            .iter()
            .filter(|(id, _)| **id != source_window_id)
            .map(|(id, ws)| {
                let active_title = ws
                    .tab_manager
                    .active_tab()
                    .map(|t| t.title.trim().to_string())
                    .filter(|s| !s.is_empty());
                let label = match active_title {
                    Some(title) => format!("Window {} - {}", ws.window_index, title),
                    None => format!("Window {}", ws.window_index),
                };
                (*id, label)
            })
            .collect()
    }

    /// Move a live `Tab` from `source_window` to `destination`, preserving the
    /// PTY, scrollback, split panes, and all other per-tab state.
    ///
    /// Orchestration:
    /// 1. Pre-flight validation (source/tab exist, no active tmux gateway,
    ///    not a solo-tab pop-out, destination distinct and present).
    /// 2. Record source geometry, resolve/create destination window.
    /// 3. `remove_tab` from source (which also flags source empty if this was
    ///    the last tab), `stop_refresh_task` so the captured source `Arc<Window>`
    ///    is released.
    /// 4. `insert_tab_at` end of destination, rebind `start_refresh_task` against
    ///    the destination's `Arc<Window>`.
    /// 5. Focus destination window. If the source is now empty, close it.
    pub(crate) fn move_tab(
        &mut self,
        event_loop: &ActiveEventLoop,
        source_window: WindowId,
        tab_id: crate::tab::TabId,
        destination: super::MoveDestination,
    ) {
        use super::MoveDestination;

        // --- Pre-flight validation ---
        let Some(source_state) = self.windows.get(&source_window) else {
            log::warn!("move_tab: source window {:?} not found", source_window);
            return;
        };

        if source_state.tab_manager.get_tab(tab_id).is_none() {
            log::warn!("move_tab: tab {} not in window {:?}", tab_id, source_window);
            return;
        }

        // Reject if tmux gateway is active anywhere in this window.
        if source_state.is_gateway_active() {
            log::warn!(
                "move_tab: refusing to move tab {} - source window has an active tmux gateway",
                tab_id
            );
            return;
        }

        match destination {
            MoveDestination::NewWindow => {
                if source_state.tab_manager.tab_count() <= 1 {
                    log::warn!(
                        "move_tab: refusing solo-tab pop-out for tab {} (would be a no-op)",
                        tab_id
                    );
                    return;
                }
            }
            MoveDestination::ExistingWindow(dest_id) => {
                if dest_id == source_window {
                    log::warn!("move_tab: destination == source, ignoring");
                    return;
                }
                if !self.windows.contains_key(&dest_id) {
                    log::warn!("move_tab: destination window {:?} not found", dest_id);
                    return;
                }
            }
        }

        crate::debug_info!(
            "TAB",
            "Moving tab {} from window {:?} to {:?}",
            tab_id,
            source_window,
            destination
        );

        // --- Record source geometry before we mutate anything ---
        let (source_size, source_outer_pos) = {
            let ws = self.windows.get(&source_window).expect("validated above");
            let win = ws.window.as_ref();
            let size = win
                .map(|w| w.inner_size())
                .unwrap_or(winit::dpi::PhysicalSize::new(800, 600));
            let pos = win
                .and_then(|w| w.outer_position().ok())
                .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
            (size, pos)
        };

        // --- Resolve destination ---
        let dest_window_id = match destination {
            MoveDestination::ExistingWindow(id) => id,
            MoveDestination::NewWindow => {
                let clamped_pos = Self::compute_moved_tab_outer_position(
                    event_loop,
                    source_outer_pos,
                    source_size,
                );
                match self.create_window_for_moved_tab(event_loop, source_size, clamped_pos) {
                    Some(id) => id,
                    None => {
                        crate::debug_error!(
                            "TAB",
                            "move_tab: create_window_for_moved_tab failed - source state untouched"
                        );
                        return;
                    }
                }
            }
        };

        // --- Extract from source ---
        let Some(source_state) = self.windows.get_mut(&source_window) else {
            crate::debug_error!("TAB", "move_tab: source window disappeared mid-flight");
            return;
        };
        let Some((mut live_tab, source_is_empty)) = source_state.tab_manager.remove_tab(tab_id)
        else {
            crate::debug_error!(
                "TAB",
                "move_tab: remove_tab returned None for tab {}",
                tab_id
            );
            return;
        };

        // Stop the refresh task - its captured Arc<Window> still points at the source.
        live_tab.stop_refresh_task();

        // --- Insert into destination ---
        let Some(dest_state) = self.windows.get_mut(&dest_window_id) else {
            crate::debug_error!(
                "TAB",
                "move_tab: destination window {:?} disappeared - dropping tab",
                dest_window_id
            );
            return;
        };
        let insert_at = dest_state.tab_manager.tab_count();
        dest_state.tab_manager.insert_tab_at(live_tab, insert_at);

        // --- Rebind refresh task to the destination window ---
        if let Some(dest_win_arc) = dest_state.window.clone() {
            let active_fps = dest_state.config.max_fps;
            let inactive_fps = dest_state.config.inactive_tab_fps;
            if let Some(tab) = dest_state.tab_manager.get_tab_mut(tab_id) {
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    dest_win_arc,
                    active_fps,
                    inactive_fps,
                );
            }
        }

        // --- Request redraw, raise, and focus destination ---
        if let Some(win) = dest_state.window.as_ref() {
            win.request_redraw();
            win.set_visible(true);
            win.focus_window();
        }

        crate::debug_info!(
            "TAB",
            "move_tab complete (source empty: {})",
            source_is_empty
        );

        // --- Close source if emptied ---
        if source_is_empty {
            self.close_window(source_window);
        }
    }
}
