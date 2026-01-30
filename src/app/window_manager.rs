//! Multi-window manager for the terminal emulator
//!
//! This module contains `WindowManager`, which coordinates multiple terminal windows,
//! handles the native menu system, and manages shared resources.

use crate::app::window_state::WindowState;
use crate::config::{Config, resolve_shader_config};
use crate::menu::{MenuAction, MenuManager};
use crate::settings_window::{SettingsWindow, SettingsWindowAction};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::event::WindowEvent;
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
    /// Separate settings window (if open)
    pub(crate) settings_window: Option<SettingsWindow>,
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
            settings_window: None,
        }
    }

    /// Create a new window with a fresh terminal session
    pub fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        use crate::font_metrics::window_size_from_config;
        use winit::window::Window;

        // Calculate window size from cols/rows BEFORE window creation.
        // This ensures the window opens at the exact correct size with no visible resize.
        // We use scale_factor=1.0 here since we don't have the actual display scale yet;
        // the window will be resized correctly once we know the actual scale factor.
        // Fallback to reasonable defaults (800x600) if font metrics calculation fails.
        let (width, height) = window_size_from_config(&self.config, 1.0).unwrap_or((800, 600));

        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
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
                // Don't copy from terminal if settings window is open (let egui handle it)
                if self.settings_window.is_some() {
                    return;
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.get_selected_text()
                    && let Err(e) = window_state.input_handler.copy_to_clipboard(&text)
                {
                    log::error!("Failed to copy to clipboard: {}", e);
                }
            }
            MenuAction::Paste => {
                // Don't paste to terminal if settings window is open (let egui handle it)
                if self.settings_window.is_some() {
                    return;
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.input_handler.paste_from_clipboard()
                {
                    window_state.paste_text(&text);
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
                self.open_settings_window(event_loop);
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
            MenuAction::ToggleBackgroundShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_background_shader();
                }
            }
            MenuAction::ToggleCursorShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_cursor_shader();
                }
            }
            MenuAction::ReloadConfig => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.reload_config();
                }
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

    /// Open the settings window (or focus if already open)
    pub fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        // If already open, bring to front and focus
        if let Some(settings_window) = &self.settings_window {
            settings_window.focus();
            return;
        }

        // Create new settings window using shared runtime
        let config = self.config.clone();
        let runtime = Arc::clone(&self.runtime);

        // Get supported vsync modes from the first window's renderer
        let supported_vsync_modes: Vec<crate::config::VsyncMode> = self
            .windows
            .values()
            .next()
            .and_then(|ws| ws.renderer.as_ref())
            .map(|renderer| {
                [
                    crate::config::VsyncMode::Immediate,
                    crate::config::VsyncMode::Mailbox,
                    crate::config::VsyncMode::Fifo,
                ]
                .into_iter()
                .filter(|mode| renderer.is_vsync_mode_supported(*mode))
                .collect()
            })
            .unwrap_or_else(|| vec![crate::config::VsyncMode::Fifo]); // Fifo always supported

        match runtime.block_on(SettingsWindow::new(
            event_loop,
            config,
            supported_vsync_modes,
        )) {
            Ok(settings_window) => {
                log::info!("Opened settings window {:?}", settings_window.window_id());
                self.settings_window = Some(settings_window);
            }
            Err(e) => {
                log::error!("Failed to create settings window: {}", e);
            }
        }
    }

    /// Close the settings window
    pub fn close_settings_window(&mut self) {
        if self.settings_window.take().is_some() {
            log::info!("Closed settings window");
        }
    }

    /// Check if a window ID belongs to the settings window
    pub fn is_settings_window(&self, window_id: WindowId) -> bool {
        self.settings_window
            .as_ref()
            .is_some_and(|sw| sw.window_id() == window_id)
    }

    /// Handle an event for the settings window
    pub fn handle_settings_window_event(
        &mut self,
        event: WindowEvent,
    ) -> Option<SettingsWindowAction> {
        if let Some(settings_window) = &mut self.settings_window {
            let action = settings_window.handle_window_event(event);

            // Handle close action
            if settings_window.should_close() {
                self.close_settings_window();
                return Some(SettingsWindowAction::Close);
            }

            return Some(action);
        }
        None
    }

    /// Apply config changes from settings window to all terminal windows
    pub fn apply_config_to_windows(&mut self, config: &Config) {
        use crate::app::config_updates::ConfigChanges;

        // Track shader errors for the standalone settings window
        // Option<Option<String>>: None = no change attempted, Some(None) = success, Some(Some(err)) = error
        let mut last_shader_result: Option<Option<String>> = None;
        let mut last_cursor_shader_result: Option<Option<String>> = None;

        for window_state in self.windows.values_mut() {
            // Detect what changed
            let changes = ConfigChanges::detect(&window_state.config, config);

            // Update the config
            window_state.config = config.clone();

            // Rebuild keybinding registry if keybindings changed
            if changes.keybindings {
                window_state.keybinding_registry =
                    crate::keybindings::KeybindingRegistry::from_config(&config.keybindings);
                log::info!(
                    "Keybinding registry rebuilt with {} bindings",
                    config.keybindings.len()
                );
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
                    window_state.needs_redraw = true;
                }

                // Update text opacity mode if changed
                if changes.keep_text_opaque {
                    renderer.set_keep_text_opaque(config.keep_text_opaque);
                    window_state.needs_redraw = true;
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
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_cursor_style(term_style);
                        }
                        tab.cache.cells = None; // Invalidate cache to redraw cursor
                    }
                    window_state.needs_redraw = true;
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
                    window_state.needs_redraw = true;
                }

                // Apply theme changes
                if changes.theme
                    && let Some(tab) = window_state.tab_manager.active_tab()
                    && let Ok(mut term) = tab.terminal.try_lock()
                {
                    term.set_theme(config.load_theme());
                }

                // Resolve per-shader settings (user override -> metadata defaults -> global)
                // This is computed once and used for both shader enable and background-as-channel0
                let shader_override = config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| config.shader_configs.get(name));
                // Get shader metadata from cache for full 3-tier resolution
                let metadata = config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| window_state.shader_metadata_cache.get(name).cloned());
                let resolved = resolve_shader_config(shader_override, metadata.as_ref(), config);

                // Apply shader changes - track if change was attempted and result
                // Option<Option<String>>: None = no change attempted, Some(None) = success, Some(Some(err)) = error
                let shader_result =
                    if changes.any_shader_change() || changes.shader_per_shader_config {
                        Some(
                            renderer
                                .set_custom_shader_enabled(
                                    config.custom_shader_enabled,
                                    config.custom_shader.as_deref(),
                                    config.window_opacity,
                                    resolved.text_opacity,
                                    config.custom_shader_animation,
                                    resolved.animation_speed,
                                    resolved.full_content,
                                    resolved.brightness,
                                    &resolved.channel_paths(),
                                    resolved.cubemap_path().map(|p| p.as_path()),
                                )
                                .err(),
                        )
                    } else {
                        None // No change attempted
                    };

                // Apply use_background_as_channel0 setting
                // This needs to be applied after the shader is loaded but before it renders
                // Include any_shader_change() to ensure the setting is applied when a new shader is loaded
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
            // shader_result: None = no change attempted, Some(None) = success, Some(Some(err)) = error
            if let Some(result) = shader_result {
                last_shader_result = Some(result);
            }
            if let Some(result) = cursor_result {
                last_cursor_shader_result = Some(result);
            }

            // Apply window-related changes
            if let Some(window) = &window_state.window {
                if changes.window_title {
                    window.set_title(&config.window_title);
                }
                if changes.window_decorations {
                    window.set_decorations(config.window_decorations);
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

            // Queue font rebuild if needed
            if changes.font {
                window_state.pending_font_rebuild = true;
            }

            // Reinitialize shader watcher if shader paths changed
            if changes.needs_watcher_reinit() {
                window_state.reinit_shader_watcher();
            }

            // Invalidate cache
            if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                tab.cache.cells = None;
            }
            window_state.needs_redraw = true;
        }

        // Also update the shared config
        self.config = config.clone();

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

    /// Apply shader changes from settings window editor
    pub fn apply_shader_from_editor(&mut self, source: &str) -> Result<(), String> {
        let mut last_error = None;

        for window_state in self.windows.values_mut() {
            if let Some(renderer) = &mut window_state.renderer {
                match renderer.reload_shader_from_source(source) {
                    Ok(()) => {
                        window_state.needs_redraw = true;
                        if let Some(window) = &window_state.window {
                            window.request_redraw();
                        }
                    }
                    Err(e) => {
                        last_error = Some(format!("{:#}", e));
                    }
                }
            }
        }

        // Update settings window with error status
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(ref err) = last_error {
                settings_window.set_shader_error(Some(err.clone()));
            } else {
                settings_window.clear_shader_error();
            }
        }

        last_error.map_or(Ok(()), Err)
    }

    /// Apply cursor shader changes from settings window editor
    pub fn apply_cursor_shader_from_editor(&mut self, source: &str) -> Result<(), String> {
        let mut last_error = None;

        for window_state in self.windows.values_mut() {
            if let Some(renderer) = &mut window_state.renderer {
                match renderer.reload_cursor_shader_from_source(source) {
                    Ok(()) => {
                        window_state.needs_redraw = true;
                        if let Some(window) = &window_state.window {
                            window.request_redraw();
                        }
                    }
                    Err(e) => {
                        last_error = Some(format!("{:#}", e));
                    }
                }
            }
        }

        // Update settings window with error status
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(ref err) = last_error {
                settings_window.set_cursor_shader_error(Some(err.clone()));
            } else {
                settings_window.clear_cursor_shader_error();
            }
        }

        last_error.map_or(Ok(()), Err)
    }

    /// Request redraw for settings window
    pub fn request_settings_redraw(&self) {
        if let Some(settings_window) = &self.settings_window {
            settings_window.request_redraw();
        }
    }
}
