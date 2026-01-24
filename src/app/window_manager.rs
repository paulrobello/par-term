//! Multi-window manager for the terminal emulator
//!
//! This module contains `WindowManager`, which coordinates multiple terminal windows,
//! handles the native menu system, and manages shared resources.

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::menu::{MenuAction, MenuManager};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

/// Manages multiple terminal windows and shared resources
pub struct WindowManager {
    /// Per-window state indexed by window ID
    pub(crate) windows: HashMap<WindowId, WindowState>,
    /// Native menu manager
    pub(crate) menu: Option<MenuManager>,
    /// Shared configuration (read at startup, each window gets a clone)
    pub(crate) config: Config,
    /// Shared async runtime
    pub(crate) runtime: Arc<Runtime>,
    /// Flag to indicate if app should exit
    pub(crate) should_exit: bool,
    /// Counter for generating unique window IDs during creation
    pending_window_count: usize,
}

impl WindowManager {
    /// Create a new window manager
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Self {
        Self {
            windows: HashMap::new(),
            menu: None,
            config,
            runtime,
            should_exit: false,
            pending_window_count: 0,
        }
    }

    /// Create a new window with a fresh terminal session
    pub fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        use winit::window::Window;

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

                // Initialize async components using the shared runtime
                let runtime = Arc::clone(&self.runtime);
                if let Err(e) = runtime.block_on(window_state.initialize_async(window)) {
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

                self.windows.insert(window_id, window_state);
                self.pending_window_count += 1;
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

    /// Close a specific window
    pub fn close_window(&mut self, window_id: WindowId) {
        if let Some(window_state) = self.windows.remove(&window_id) {
            log::info!(
                "Closing window {:?} (remaining: {})",
                window_id,
                self.windows.len()
            );
            // WindowState's Drop impl handles cleanup
            drop(window_state);
        }

        // Exit app when last window closes
        if self.windows.is_empty() {
            log::info!("Last window closed, exiting application");
            self.should_exit = true;
        }
    }

    /// Get mutable reference to a window's state
    #[allow(dead_code)]
    pub fn get_window_mut(&mut self, window_id: WindowId) -> Option<&mut WindowState> {
        self.windows.get_mut(&window_id)
    }

    /// Get reference to a window's state
    #[allow(dead_code)]
    pub fn get_window(&self, window_id: WindowId) -> Option<&WindowState> {
        self.windows.get(&window_id)
    }

    /// Handle a menu action
    pub fn handle_menu_action(
        &mut self,
        action: MenuAction,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        match action {
            MenuAction::NewWindow => {
                self.create_window(event_loop);
            }
            MenuAction::CloseWindow => {
                if let Some(window_id) = focused_window {
                    self.close_window(window_id);
                }
            }
            MenuAction::NewTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.new_tab();
                }
            }
            MenuAction::CloseTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::NextTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.next_tab();
                }
            }
            MenuAction::PreviousTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.prev_tab();
                }
            }
            MenuAction::SwitchToTab(index) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.switch_to_tab_index(index);
                }
            }
            MenuAction::MoveTabLeft => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_left();
                }
            }
            MenuAction::MoveTabRight => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_right();
                }
            }
            MenuAction::DuplicateTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.duplicate_tab();
                }
            }
            MenuAction::Quit => {
                // Close all windows
                let window_ids: Vec<_> = self.windows.keys().copied().collect();
                for window_id in window_ids {
                    self.close_window(window_id);
                }
            }
            MenuAction::Copy => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.get_selected_text()
                    && let Err(e) = window_state.input_handler.copy_to_clipboard(&text)
                {
                    log::error!("Failed to copy to clipboard: {}", e);
                }
            }
            MenuAction::Paste => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.input_handler.paste_from_clipboard()
                    && let Ok(text_str) = std::str::from_utf8(&text)
                {
                    window_state.paste_text(text_str);
                }
            }
            MenuAction::SelectAll => {
                // Not implemented for terminal - would select all visible text
                log::debug!("SelectAll menu action (not implemented for terminal)");
            }
            MenuAction::ClearScrollback => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    // Clear scrollback in active tab
                    let cleared = if let Some(tab) = window_state.tab_manager.active_tab() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.clear_scrollback();
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if cleared {
                        if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                            tab.cache.scrollback_len = 0;
                        }
                        window_state.set_scroll_target(0);
                        log::info!("Cleared scrollback buffer");
                    }
                }
            }
            MenuAction::ClipboardHistory => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.clipboard_history_ui.toggle();
                    window_state.needs_redraw = true;
                }
            }
            MenuAction::ToggleFullscreen => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window_state.is_fullscreen = !window_state.is_fullscreen;
                    if window_state.is_fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            }
            MenuAction::IncreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size + 1.0).min(72.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::DecreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size - 1.0).max(6.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ResetFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = 14.0;
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ToggleFpsOverlay => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.debug.show_fps_overlay = !window_state.debug.show_fps_overlay;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::OpenSettings => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.settings_ui.toggle();
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::Minimize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_minimized(true);
                }
            }
            MenuAction::Zoom => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_maximized(!window.is_maximized());
                }
            }
            MenuAction::ShowHelp => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.help_ui.toggle();
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::About => {
                log::info!("About par-term v{}", env!("CARGO_PKG_VERSION"));
                // Could show an about dialog here
            }
        }
    }

    /// Process any pending menu events
    pub fn process_menu_events(
        &mut self,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        if let Some(menu) = &self.menu {
            // Collect actions to avoid borrow conflicts
            let actions: Vec<_> = menu.poll_events().collect();
            for action in actions {
                self.handle_menu_action(action, event_loop, focused_window);
            }
        }
    }
}
