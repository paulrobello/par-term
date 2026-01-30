//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.

use crate::app::debug_state::DebugState;
use crate::clipboard_history_ui::{ClipboardHistoryAction, ClipboardHistoryUI};
use crate::config::{Config, CursorStyle};
use crate::help_ui::HelpUI;
use crate::input::InputHandler;
use crate::renderer::Renderer;
use crate::selection::SelectionMode;
use crate::settings_ui::SettingsUI;
use crate::shader_watcher::{ShaderReloadEvent, ShaderType, ShaderWatcher};
use crate::tab::TabManager;
use crate::tab_bar_ui::{TabBarAction, TabBarUI};
use anyhow::Result;
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use std::sync::Arc;
use tokio::runtime::Runtime;
use wgpu::SurfaceError;
use winit::window::Window;

/// Per-window state that manages a single terminal window with multiple tabs
pub struct WindowState {
    pub(crate) config: Config,
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) renderer: Option<Renderer>,
    pub(crate) input_handler: InputHandler,
    pub(crate) runtime: Arc<Runtime>,

    /// Tab manager for handling multiple terminal tabs
    pub(crate) tab_manager: TabManager,
    /// Tab bar UI
    pub(crate) tab_bar_ui: TabBarUI,

    pub(crate) debug: DebugState,

    /// Cursor opacity for smooth fade animation (0.0 = invisible, 1.0 = fully visible)
    pub(crate) cursor_opacity: f32,
    /// Time of last cursor blink toggle
    pub(crate) last_cursor_blink: Option<std::time::Instant>,
    /// Time of last key press (to reset cursor blink)
    pub(crate) last_key_press: Option<std::time::Instant>,
    /// Whether window is currently in fullscreen mode
    pub(crate) is_fullscreen: bool,
    /// egui context for GUI rendering
    pub(crate) egui_ctx: Option<egui::Context>,
    /// egui-winit state for event handling
    pub(crate) egui_state: Option<egui_winit::State>,
    /// Whether egui has completed its first ctx.run() call
    /// Before first run, egui's is_using_pointer() returns unreliable results
    pub(crate) egui_initialized: bool,
    /// Settings UI manager
    pub(crate) settings_ui: SettingsUI,
    /// Help UI manager
    pub(crate) help_ui: HelpUI,
    /// Clipboard history UI manager
    pub(crate) clipboard_history_ui: ClipboardHistoryUI,
    /// Whether terminal session recording is active
    pub(crate) is_recording: bool,
    /// When recording started
    #[allow(dead_code)]
    pub(crate) recording_start_time: Option<std::time::Instant>,
    /// Flag to indicate shutdown is in progress
    pub(crate) is_shutting_down: bool,

    // Smart redraw tracking (event-driven rendering)
    /// Whether we need to render next frame
    pub(crate) needs_redraw: bool,
    /// When to blink cursor next
    pub(crate) cursor_blink_timer: Option<std::time::Instant>,
    /// Whether we need to rebuild renderer after font-related changes
    pub(crate) pending_font_rebuild: bool,

    // Focus state for power saving
    /// Whether the window currently has focus
    pub(crate) is_focused: bool,
    /// Last time a frame was rendered (for FPS throttling when unfocused)
    pub(crate) last_render_time: Option<std::time::Instant>,

    // Shader hot reload
    /// Shader file watcher for hot reload support
    pub(crate) shader_watcher: Option<ShaderWatcher>,
    /// Last shader reload error message (for display in UI)
    pub(crate) shader_reload_error: Option<String>,
    /// Background shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) background_shader_reload_result: Option<Option<String>>,
    /// Cursor shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) cursor_shader_reload_result: Option<Option<String>>,

    /// Flag to signal that the settings window should be opened
    /// This is set by keyboard handlers and consumed by the window manager
    pub(crate) open_settings_window_requested: bool,

    // Resize overlay state
    /// Whether the resize overlay is currently visible
    pub(crate) resize_overlay_visible: bool,
    /// When to hide the resize overlay (after resize stops)
    pub(crate) resize_overlay_hide_time: Option<std::time::Instant>,
    /// Current resize dimensions: (width_px, height_px, cols, rows)
    pub(crate) resize_dimensions: Option<(u32, u32, usize, usize)>,
}

impl WindowState {
    /// Create a new window state with the given configuration
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Self {
        let settings_ui = SettingsUI::new(config.clone());

        Self {
            config,
            window: None,
            renderer: None,
            input_handler: InputHandler::new(),
            runtime,

            tab_manager: TabManager::new(),
            tab_bar_ui: TabBarUI::new(),

            debug: DebugState::new(),

            cursor_opacity: 1.0,
            last_cursor_blink: None,
            last_key_press: None,
            is_fullscreen: false,
            egui_ctx: None,
            egui_state: None,
            egui_initialized: false,
            settings_ui,
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            is_recording: false,
            recording_start_time: None,
            is_shutting_down: false,

            needs_redraw: true,
            cursor_blink_timer: None,
            pending_font_rebuild: false,

            is_focused: true, // Assume focused on creation
            last_render_time: None,

            shader_watcher: None,
            shader_reload_error: None,
            background_shader_reload_result: None,
            cursor_shader_reload_result: None,

            open_settings_window_requested: false,

            resize_overlay_visible: false,
            resize_overlay_hide_time: None,
            resize_dimensions: None,
        }
    }

    // ========================================================================
    // Active Tab State Accessors (compatibility - may be useful later)
    // ========================================================================
    #[allow(dead_code)]
    pub(crate) fn terminal(
        &self,
    ) -> Option<&Arc<tokio::sync::Mutex<crate::terminal::TerminalManager>>> {
        self.active_terminal()
    }

    #[allow(dead_code)]
    pub(crate) fn scroll_state(&self) -> Option<&crate::scroll_state::ScrollState> {
        self.tab_manager.active_tab().map(|t| &t.scroll_state)
    }

    #[allow(dead_code)]
    pub(crate) fn scroll_state_mut(&mut self) -> Option<&mut crate::scroll_state::ScrollState> {
        self.tab_manager
            .active_tab_mut()
            .map(|t| &mut t.scroll_state)
    }

    #[allow(dead_code)]
    pub(crate) fn mouse(&self) -> Option<&crate::app::mouse::MouseState> {
        self.tab_manager.active_tab().map(|t| &t.mouse)
    }

    #[allow(dead_code)]
    pub(crate) fn mouse_mut(&mut self) -> Option<&mut crate::app::mouse::MouseState> {
        self.tab_manager.active_tab_mut().map(|t| &mut t.mouse)
    }

    #[allow(dead_code)]
    pub(crate) fn bell(&self) -> Option<&crate::app::bell::BellState> {
        self.tab_manager.active_tab().map(|t| &t.bell)
    }

    #[allow(dead_code)]
    pub(crate) fn bell_mut(&mut self) -> Option<&mut crate::app::bell::BellState> {
        self.tab_manager.active_tab_mut().map(|t| &mut t.bell)
    }

    #[allow(dead_code)]
    pub(crate) fn cache(&self) -> Option<&crate::app::render_cache::RenderCache> {
        self.tab_manager.active_tab().map(|t| &t.cache)
    }

    #[allow(dead_code)]
    pub(crate) fn cache_mut(&mut self) -> Option<&mut crate::app::render_cache::RenderCache> {
        self.tab_manager.active_tab_mut().map(|t| &mut t.cache)
    }

    #[allow(dead_code)]
    pub(crate) fn refresh_task(&self) -> Option<&Option<tokio::task::JoinHandle<()>>> {
        self.tab_manager.active_tab().map(|t| &t.refresh_task)
    }

    #[allow(dead_code)]
    pub(crate) fn abort_refresh_task(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(task) = tab.refresh_task.take()
        {
            task.abort();
        }
    }

    /// Extract a substring based on character columns to avoid UTF-8 slicing panics
    pub(crate) fn extract_columns(line: &str, start_col: usize, end_col: Option<usize>) -> String {
        let mut extracted = String::new();
        let end_bound = end_col.unwrap_or(usize::MAX);

        if start_col > end_bound {
            return extracted;
        }

        for (idx, ch) in line.chars().enumerate() {
            if idx > end_bound {
                break;
            }

            if idx >= start_col {
                extracted.push(ch);
            }
        }

        extracted
    }

    // ========================================================================
    // DRY Helper Methods
    // ========================================================================

    /// Invalidate the active tab's cell cache, forcing regeneration on next render
    #[inline]
    pub(crate) fn invalidate_tab_cache(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.cells = None;
        }
    }

    /// Request window redraw if window exists
    #[inline]
    pub(crate) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Invalidate cache and request redraw (common pattern after state changes)
    #[inline]
    #[allow(dead_code)] // Available for future use, cannot be used inside renderer borrow blocks
    pub(crate) fn invalidate_and_redraw(&mut self) {
        self.invalidate_tab_cache();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Clear renderer cells and invalidate cache (used when switching tabs)
    pub(crate) fn clear_and_invalidate(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.invalidate_tab_cache();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Rebuild the renderer after font-related changes and resize the terminal accordingly
    pub(crate) fn rebuild_renderer(&mut self) -> Result<()> {
        use crate::app::renderer_init::RendererInitParams;

        let window = if let Some(w) = &self.window {
            Arc::clone(w)
        } else {
            return Ok(()); // Nothing to rebuild yet
        };

        // Create renderer using DRY init params
        let theme = self.config.load_theme();
        // Get shader metadata from cache for full 3-tier resolution
        let metadata = self
            .config
            .custom_shader
            .as_ref()
            .and_then(|name| self.settings_ui.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self.config.cursor_shader.as_ref().and_then(|name| {
            self.settings_ui
                .cursor_shader_metadata_cache
                .get(name)
                .cloned()
        });
        let params = RendererInitParams::from_config(
            &self.config,
            &theme,
            metadata.as_ref(),
            cursor_metadata.as_ref(),
        );
        let mut renderer = self
            .runtime
            .block_on(params.create_renderer(Arc::clone(&window)))?;

        let (cols, rows) = renderer.grid_size();
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (cols as f32 * cell_width) as usize;
        let height_px = (rows as f32 * cell_height) as usize;

        // Resize all tabs' terminals
        for tab in self.tab_manager.tabs_mut() {
            if let Ok(mut term) = tab.terminal.try_lock() {
                let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                term.set_theme(self.config.load_theme());
            }
            tab.cache.cells = None;
        }

        // Apply cursor shader configuration
        self.apply_cursor_shader_config(&mut renderer, &params);

        // Update settings UI with supported vsync modes
        let supported_modes: Vec<crate::config::VsyncMode> = [
            crate::config::VsyncMode::Immediate,
            crate::config::VsyncMode::Mailbox,
            crate::config::VsyncMode::Fifo,
        ]
        .into_iter()
        .filter(|mode| renderer.is_vsync_mode_supported(*mode))
        .collect();
        self.settings_ui
            .update_supported_vsync_modes(supported_modes);

        self.renderer = Some(renderer);
        self.needs_redraw = true;

        // Reset egui with preserved memory (window positions, collapse state)
        self.init_egui(&window, true);
        self.request_redraw();

        Ok(())
    }

    /// Initialize the window asynchronously
    pub(crate) async fn initialize_async(&mut self, window: Window) -> Result<()> {
        use crate::app::renderer_init::RendererInitParams;

        // Enable IME (Input Method Editor) to receive all character events including Space
        window.set_ime_allowed(true);
        log::debug!("IME enabled for character input");

        let window = Arc::new(window);

        // Initialize egui context and state (no memory to preserve on first init)
        self.init_egui(&window, false);

        // Create renderer using DRY init params
        let theme = self.config.load_theme();
        // Get shader metadata from cache for full 3-tier resolution
        let metadata = self
            .config
            .custom_shader
            .as_ref()
            .and_then(|name| self.settings_ui.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self.config.cursor_shader.as_ref().and_then(|name| {
            self.settings_ui
                .cursor_shader_metadata_cache
                .get(name)
                .cloned()
        });
        let params = RendererInitParams::from_config(
            &self.config,
            &theme,
            metadata.as_ref(),
            cursor_metadata.as_ref(),
        );
        let mut renderer = params.create_renderer(Arc::clone(&window)).await?;

        // macOS: Configure CAMetalLayer (transparency + performance)
        // This MUST be done AFTER creating the wgpu surface/renderer
        // so that the CAMetalLayer has been created by wgpu
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = crate::macos_metal::configure_metal_layer_for_performance(&window) {
                log::warn!("Failed to configure Metal layer: {}", e);
                log::warn!(
                    "Continuing anyway - may experience reduced FPS or missing transparency on macOS"
                );
            }
            // Set initial layer opacity to match config (content only, frame unaffected)
            if let Err(e) = crate::macos_metal::set_layer_opacity(&window, 1.0) {
                log::warn!("Failed to set initial Metal layer opacity: {}", e);
            }
            // Apply initial blur settings if enabled
            if self.config.blur_enabled
                && self.config.window_opacity < 1.0
                && let Err(e) = crate::macos_blur::set_window_blur(&window, self.config.blur_radius)
            {
                log::warn!("Failed to set initial window blur: {}", e);
            }
        }

        // Apply cursor shader configuration
        self.apply_cursor_shader_config(&mut renderer, &params);

        // Update settings UI with supported vsync modes
        let supported_modes: Vec<crate::config::VsyncMode> = [
            crate::config::VsyncMode::Immediate,
            crate::config::VsyncMode::Mailbox,
            crate::config::VsyncMode::Fifo,
        ]
        .into_iter()
        .filter(|mode| renderer.is_vsync_mode_supported(*mode))
        .collect();
        self.settings_ui
            .update_supported_vsync_modes(supported_modes);

        self.window = Some(Arc::clone(&window));
        self.renderer = Some(renderer);

        // Initialize shader watcher if hot reload is enabled
        self.init_shader_watcher();

        // Create the first tab
        let tab_id = self.tab_manager.new_tab(
            &self.config,
            Arc::clone(&self.runtime),
            false, // First tab doesn't inherit cwd
        )?;

        // Resize the tab's terminal to match renderer grid
        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
            if let Some(renderer) = &self.renderer {
                let (renderer_cols, renderer_rows) = renderer.grid_size();
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let width_px = (renderer_cols as f32 * cell_width) as usize;
                let height_px = (renderer_rows as f32 * cell_height) as usize;

                if let Ok(mut term) = tab.terminal.try_lock() {
                    let _ =
                        term.resize_with_pixels(renderer_cols, renderer_rows, width_px, height_px);
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    log::info!(
                        "Initial terminal dimensions: {}x{} ({}x{} px)",
                        renderer_cols,
                        renderer_rows,
                        width_px,
                        height_px
                    );
                }

                // Update settings UI with initial terminal dimensions
                self.settings_ui
                    .update_current_size(renderer_cols, renderer_rows);
            }

            // Start refresh task for the first tab
            tab.start_refresh_task(
                Arc::clone(&self.runtime),
                Arc::clone(&window),
                self.config.max_fps,
            );
        }

        Ok(())
    }

    /// Force surface reconfiguration - useful when rendering becomes corrupted
    /// after moving between monitors or when automatic detection fails.
    /// Also clears glyph cache to ensure fonts render correctly.
    pub(crate) fn force_surface_reconfigure(&mut self) {
        log::info!("Force surface reconfigure triggered");

        if let Some(renderer) = &mut self.renderer {
            // Reconfigure the surface
            renderer.reconfigure_surface();

            // Clear glyph cache to force re-rasterization at correct DPI
            renderer.clear_glyph_cache();

            // Invalidate cached cells to force full re-render
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.cells = None;
            }
        }

        // On macOS, reconfigure the Metal layer
        #[cfg(target_os = "macos")]
        {
            if let Some(window) = &self.window
                && let Err(e) = crate::macos_metal::configure_metal_layer_for_performance(window)
            {
                log::warn!("Failed to reconfigure Metal layer: {}", e);
            }
        }

        // Request redraw
        self.needs_redraw = true;
        self.request_redraw();
    }

    // ========================================================================
    // Shader Hot Reload
    // ========================================================================

    /// Initialize the shader watcher for hot reload support
    pub(crate) fn init_shader_watcher(&mut self) {
        debug_info!(
            "SHADER",
            "init_shader_watcher: hot_reload={}",
            self.config.shader_hot_reload
        );

        if !self.config.shader_hot_reload {
            log::debug!("Shader hot reload disabled");
            return;
        }

        let background_path = self
            .config
            .custom_shader
            .as_ref()
            .filter(|_| self.config.custom_shader_enabled)
            .map(|s| Config::shader_path(s));

        let cursor_path = self
            .config
            .cursor_shader
            .as_ref()
            .filter(|_| self.config.cursor_shader_enabled)
            .map(|s| Config::shader_path(s));

        debug_info!(
            "SHADER",
            "Shader paths: background={:?}, cursor={:?}",
            background_path,
            cursor_path
        );

        if background_path.is_none() && cursor_path.is_none() {
            debug_info!("SHADER", "No shaders to watch for hot reload");
            return;
        }

        match ShaderWatcher::new(
            background_path.as_deref(),
            cursor_path.as_deref(),
            self.config.shader_hot_reload_delay,
        ) {
            Ok(watcher) => {
                debug_info!(
                    "SHADER",
                    "Shader hot reload initialized (debounce: {}ms)",
                    self.config.shader_hot_reload_delay
                );
                self.shader_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!("SHADER", "Failed to initialize shader hot reload: {}", e);
            }
        }
    }

    /// Reinitialize shader watcher when shader paths change
    pub(crate) fn reinit_shader_watcher(&mut self) {
        debug_info!(
            "SHADER",
            "reinit_shader_watcher CALLED: shader={:?}, cursor={:?}",
            self.config.custom_shader,
            self.config.cursor_shader
        );
        // Drop existing watcher
        self.shader_watcher = None;
        self.shader_reload_error = None;

        // Reinitialize if hot reload is still enabled
        self.init_shader_watcher();
    }

    /// Check for and handle shader reload events
    ///
    /// Should be called periodically (e.g., in about_to_wait or render loop).
    /// Returns true if a shader was reloaded.
    pub(crate) fn check_shader_reload(&mut self) -> bool {
        let Some(watcher) = &self.shader_watcher else {
            return false;
        };

        let Some(event) = watcher.try_recv() else {
            return false;
        };

        self.handle_shader_reload_event(event)
    }

    /// Handle a shader reload event
    ///
    /// On success: clears errors, triggers redraw, optionally shows notification
    /// On failure: preserves the old working shader, logs error, shows notification
    fn handle_shader_reload_event(&mut self, event: ShaderReloadEvent) -> bool {
        let shader_name = match event.shader_type {
            ShaderType::Background => "Background shader",
            ShaderType::Cursor => "Cursor shader",
        };
        let file_name = event
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("shader");

        log::info!("Hot reload: {} from {}", shader_name, event.path.display());

        // Read the shader source
        let source = match std::fs::read_to_string(&event.path) {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("Cannot read '{}': {}", file_name, e);
                log::error!("Shader hot reload failed: {}", error_msg);
                self.shader_reload_error = Some(error_msg.clone());
                match event.shader_type {
                    ShaderType::Background => {
                        self.settings_ui.set_shader_error(Some(error_msg.clone()));
                        // Track error for standalone settings window propagation
                        self.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.settings_ui
                            .set_cursor_shader_error(Some(error_msg.clone()));
                        // Track error for standalone settings window propagation
                        self.cursor_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                }
                // Notify user of the error
                self.deliver_notification(
                    "Shader Reload Failed",
                    &format!("{} - {}", shader_name, error_msg),
                );
                // Trigger visual bell if enabled to alert user
                if self.config.notification_bell_visual
                    && let Some(tab) = self.tab_manager.active_tab_mut()
                {
                    tab.bell.visual_flash = Some(std::time::Instant::now());
                }
                return false;
            }
        };

        let Some(renderer) = &mut self.renderer else {
            log::error!("Cannot reload shader: no renderer available");
            return false;
        };

        // Attempt to reload the shader
        // Note: On compilation failure, the old shader pipeline is preserved
        let result = match event.shader_type {
            ShaderType::Background => renderer.reload_shader_from_source(&source),
            ShaderType::Cursor => renderer.reload_cursor_shader_from_source(&source),
        };

        match result {
            Ok(()) => {
                log::info!("{} reloaded successfully from {}", shader_name, file_name);
                self.shader_reload_error = None;
                match event.shader_type {
                    ShaderType::Background => {
                        self.settings_ui.clear_shader_error();
                        // Track success for standalone settings window propagation
                        self.background_shader_reload_result = Some(None);
                    }
                    ShaderType::Cursor => {
                        self.settings_ui.clear_cursor_shader_error();
                        // Track success for standalone settings window propagation
                        self.cursor_shader_reload_result = Some(None);
                    }
                }
                self.needs_redraw = true;
                self.request_redraw();
                true
            }
            Err(e) => {
                // Extract the most relevant error message from the chain
                let root_cause = e.root_cause().to_string();
                let error_msg = if root_cause.len() > 200 {
                    // Truncate very long error messages
                    format!("{}...", &root_cause[..200])
                } else {
                    root_cause
                };

                log::error!(
                    "{} compilation failed (old shader preserved): {}",
                    shader_name,
                    error_msg
                );
                log::debug!("Full error chain: {:#}", e);

                self.shader_reload_error = Some(error_msg.clone());
                match event.shader_type {
                    ShaderType::Background => {
                        self.settings_ui.set_shader_error(Some(error_msg.clone()));
                        // Track error for standalone settings window propagation
                        self.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.settings_ui
                            .set_cursor_shader_error(Some(error_msg.clone()));
                        // Track error for standalone settings window propagation
                        self.cursor_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                }

                // Notify user of the compilation error
                self.deliver_notification(
                    "Shader Compilation Error",
                    &format!("{}: {}", file_name, error_msg),
                );

                // Trigger visual bell if enabled to alert user
                if self.config.notification_bell_visual
                    && let Some(tab) = self.tab_manager.active_tab_mut()
                {
                    tab.bell.visual_flash = Some(std::time::Instant::now());
                }

                false
            }
        }
    }

    /// Check if egui is currently using the pointer (mouse is over an egui UI element)
    pub(crate) fn is_egui_using_pointer(&self) -> bool {
        // Before first render, egui state is unreliable - allow mouse events through
        if !self.egui_initialized {
            return false;
        }
        // Always check egui context - the tab bar is always rendered via egui
        // and can consume pointer events (e.g., close button clicks)
        if let Some(ctx) = &self.egui_ctx {
            ctx.is_using_pointer() || ctx.wants_pointer_input()
        } else {
            false
        }
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        let any_ui_visible = self.settings_ui.visible
            || self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.settings_ui.is_shader_editor_visible()
            || self.settings_ui.is_cursor_shader_editor_visible();
        if !any_ui_visible {
            return false;
        }

        // Check egui context for keyboard usage
        if let Some(ctx) = &self.egui_ctx {
            ctx.wants_keyboard_input()
        } else {
            false
        }
    }

    /// Determine if scrollbar should be visible based on autohide setting and recent activity
    pub(crate) fn should_show_scrollbar(&self) -> bool {
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return false,
        };

        // No scrollbar needed if no scrollback available
        if tab.cache.scrollback_len == 0 {
            return false;
        }

        // Always show when dragging or moving
        if tab.scroll_state.dragging {
            return true;
        }

        // If autohide disabled, always show
        if self.config.scrollbar_autohide_delay == 0 {
            return true;
        }

        // If scrolled away from bottom, keep visible
        if tab.scroll_state.offset > 0 || tab.scroll_state.target_offset > 0 {
            return true;
        }

        // Show when pointer is near the scrollbar edge (hover reveal)
        if let Some(window) = &self.window {
            let padding = 32.0; // px hover band
            let width = window.inner_size().width as f64;
            let near_right = self.config.scrollbar_position != "left"
                && (width - tab.mouse.position.0) <= padding;
            let near_left =
                self.config.scrollbar_position == "left" && tab.mouse.position.0 <= padding;
            if near_left || near_right {
                return true;
            }
        }

        // Otherwise, hide after delay
        tab.scroll_state.last_activity.elapsed().as_millis()
            < self.config.scrollbar_autohide_delay as u128
    }

    /// Update cursor blink state based on configured interval and DECSCUSR style
    ///
    /// The cursor blink state is determined by:
    /// 1. If lock_cursor_style is enabled: use config.cursor_blink
    /// 2. If lock_cursor_blink is enabled and cursor_blink is false: force no blink
    /// 3. Otherwise: terminal's cursor style (set via DECSCUSR escape sequence)
    /// 4. Fallback: user's config setting (cursor_blink)
    ///
    /// DECSCUSR values: odd = blinking, even = steady
    /// - 0/1: Blinking block (default)
    /// - 2: Steady block
    /// - 3: Blinking underline
    /// - 4: Steady underline
    /// - 5: Blinking bar
    /// - 6: Steady bar
    pub(crate) fn update_cursor_blink(&mut self) {
        // If cursor style is locked, use the config's blink setting directly
        if self.config.lock_cursor_style {
            if !self.config.cursor_blink {
                self.cursor_opacity = (self.cursor_opacity + 0.1).min(1.0);
                return;
            }
        } else if self.config.lock_cursor_blink && !self.config.cursor_blink {
            // If blink is locked off, don't blink regardless of terminal style
            self.cursor_opacity = (self.cursor_opacity + 0.1).min(1.0);
            return;
        }

        // Get cursor style from terminal to check if DECSCUSR specified blinking
        let cursor_should_blink = if self.config.lock_cursor_style {
            // Style is locked, use config's blink setting
            self.config.cursor_blink
        } else if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
        {
            use par_term_emu_core_rust::cursor::CursorStyle;
            let style = term.cursor_style();
            // DECSCUSR: odd values (1,3,5) = blinking, even values (2,4,6) = steady
            matches!(
                style,
                CursorStyle::BlinkingBlock
                    | CursorStyle::BlinkingUnderline
                    | CursorStyle::BlinkingBar
            )
        } else {
            // Fallback to config setting if terminal lock unavailable
            self.config.cursor_blink
        };

        if !cursor_should_blink {
            // Smoothly fade to full visibility if blinking disabled (by DECSCUSR or config)
            self.cursor_opacity = (self.cursor_opacity + 0.1).min(1.0);
            return;
        }

        let now = std::time::Instant::now();

        // If key was pressed recently (within 500ms), smoothly fade in cursor and reset blink timer
        if let Some(last_key) = self.last_key_press
            && now.duration_since(last_key).as_millis() < 500
        {
            self.cursor_opacity = (self.cursor_opacity + 0.1).min(1.0);
            self.last_cursor_blink = Some(now);
            return;
        }

        // Smooth cursor blink animation using sine wave for natural fade
        let blink_interval = std::time::Duration::from_millis(self.config.cursor_blink_interval);

        if let Some(last_blink) = self.last_cursor_blink {
            let elapsed = now.duration_since(last_blink);
            let progress = (elapsed.as_millis() as f32) / (blink_interval.as_millis() as f32);

            // Use cosine wave for smooth fade in/out (starts at 1.0, fades to 0.0, back to 1.0)
            self.cursor_opacity = ((progress * std::f32::consts::PI).cos())
                .abs()
                .clamp(0.0, 1.0);

            // Reset timer after full cycle (2x interval for full on+off)
            if elapsed >= blink_interval * 2 {
                self.last_cursor_blink = Some(now);
            }
        } else {
            // First time, start the blink timer with cursor fully visible
            self.cursor_opacity = 1.0;
            self.last_cursor_blink = Some(now);
        }
    }

    /// Main render function for this window
    pub(crate) fn render(&mut self) {
        // Skip rendering if shutting down
        if self.is_shutting_down {
            return;
        }

        // FPS throttling when unfocused with pause_refresh_on_blur enabled
        // This ensures rendering is capped at unfocused_fps even if multiple
        // sources are requesting redraws (refresh task, shader animations, etc.)
        if self.config.pause_refresh_on_blur && !self.is_focused {
            let frame_interval_ms = 1000 / self.config.unfocused_fps.max(1);
            let frame_interval = std::time::Duration::from_millis(frame_interval_ms as u64);

            if let Some(last_render) = self.last_render_time {
                let elapsed = last_render.elapsed();
                if elapsed < frame_interval {
                    // Not enough time has passed, skip this render
                    return;
                }
            }
        }

        // Update last render time for FPS throttling
        self.last_render_time = Some(std::time::Instant::now());

        let absolute_start = std::time::Instant::now();

        // Reset redraw flag after rendering
        // This flag will be set again in about_to_wait if another redraw is needed
        self.needs_redraw = false;

        // Track frame timing
        let frame_start = std::time::Instant::now();

        // Calculate frame time from last render
        if let Some(last_start) = self.debug.last_frame_start {
            let frame_time = frame_start.duration_since(last_start);
            self.debug.frame_times.push(frame_time);
            if self.debug.frame_times.len() > 60 {
                self.debug.frame_times.remove(0);
            }
        }
        self.debug.last_frame_start = Some(frame_start);

        // Update scroll animation
        let animation_running = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scroll_state.update_animation()
        } else {
            false
        };

        // Update tab titles from terminal OSC sequences
        self.tab_manager.update_all_titles();

        // Rebuild renderer if font-related settings changed
        if self.pending_font_rebuild {
            if let Err(e) = self.rebuild_renderer() {
                log::error!("Failed to rebuild renderer after font change: {}", e);
            }
            self.pending_font_rebuild = false;
        }

        // Sync tab bar height with renderer's content offset
        // This ensures the terminal grid correctly accounts for the tab bar
        let tab_count = self.tab_manager.tab_count();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config);
        crate::debug_trace!(
            "TAB_SYNC",
            "Tab count={}, tab_bar_height={:.0}, mode={:?}",
            tab_count,
            tab_bar_height,
            self.config.tab_bar_mode
        );
        if let Some(renderer) = &mut self.renderer {
            let current_offset = renderer.content_offset_y();
            if (current_offset - tab_bar_height).abs() > 0.1 {
                crate::debug_info!(
                    "TAB_SYNC",
                    "Content offset changing: {:.0} -> {:.0}",
                    current_offset,
                    tab_bar_height
                );
            }
            if let Some((new_cols, new_rows)) = renderer.set_content_offset_y(tab_bar_height) {
                // Grid size changed - resize all tab terminals
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let size = renderer.size();
                let width_px = size.width as usize;
                let height_px = size.height as usize;

                for tab in self.tab_manager.tabs_mut() {
                    if let Ok(mut term) = tab.terminal.try_lock() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                    }
                    // Invalidate cache since grid size changed
                    tab.cache.cells = None;
                }
                crate::debug_info!(
                    "TAB_SYNC",
                    "Tab bar height changed to {:.0}, resized terminals to {}x{}",
                    tab_bar_height,
                    new_cols,
                    new_rows
                );
            }
        }

        let (renderer_size, visible_lines) = if let Some(renderer) = &self.renderer {
            (renderer.size(), renderer.grid_size().1)
        } else {
            return;
        };

        // Get active tab's terminal
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return,
        };
        let terminal = &tab.terminal;

        // Check if shell has exited
        let _is_running = if let Ok(term) = terminal.try_lock() {
            term.is_running()
        } else {
            true // Assume running if locked
        };

        // Request another redraw if animation is still running
        if animation_running && let Some(window) = &self.window {
            window.request_redraw();
        }

        // Get scroll offset and selection from active tab
        let scroll_offset = tab.scroll_state.offset;
        let mouse_selection = tab.mouse.selection;

        // Get terminal cells for rendering (with dirty tracking optimization)
        // Also capture alt screen state to disable cursor shader for TUI apps
        let (cells, current_cursor_pos, cursor_style, is_alt_screen) = if let Ok(term) =
            terminal.try_lock()
        {
            // Get current generation to check if terminal content has changed
            let current_generation = term.update_generation();

            // Normalize selection if it exists and extract mode
            let (selection, rectangular) = if let Some(sel) = mouse_selection {
                (
                    Some(sel.normalized()),
                    sel.mode == SelectionMode::Rectangular,
                )
            } else {
                (None, false)
            };

            // Get cursor position and opacity (only show if we're at the bottom with no scroll offset
            // and the cursor is visible - TUI apps hide cursor via DECTCEM escape sequence)
            // If lock_cursor_visibility is enabled, ignore the terminal's visibility state
            let cursor_visible = self.config.lock_cursor_visibility || term.is_cursor_visible();
            let current_cursor_pos = if scroll_offset == 0 && cursor_visible {
                Some(term.cursor_position())
            } else {
                None
            };

            let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_opacity));

            // Get cursor style for geometric rendering
            // If lock_cursor_style is enabled, use the config's cursor style instead of terminal's
            // If lock_cursor_blink is enabled and cursor_blink is false, force steady cursor
            let cursor_style = if current_cursor_pos.is_some() {
                if self.config.lock_cursor_style {
                    // Convert config cursor style to terminal cursor style
                    let style = if self.config.cursor_blink {
                        match self.config.cursor_style {
                            CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                            CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                            CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                        }
                    } else {
                        match self.config.cursor_style {
                            CursorStyle::Block => TermCursorStyle::SteadyBlock,
                            CursorStyle::Beam => TermCursorStyle::SteadyBar,
                            CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                        }
                    };
                    Some(style)
                } else {
                    let mut style = term.cursor_style();
                    // If blink is locked off, convert blinking styles to steady
                    if self.config.lock_cursor_blink && !self.config.cursor_blink {
                        style = match style {
                            TermCursorStyle::BlinkingBlock => TermCursorStyle::SteadyBlock,
                            TermCursorStyle::BlinkingBar => TermCursorStyle::SteadyBar,
                            TermCursorStyle::BlinkingUnderline => TermCursorStyle::SteadyUnderline,
                            other => other,
                        };
                    }
                    Some(style)
                }
            } else {
                None
            };

            log::trace!(
                "Cursor: pos={:?}, opacity={:.2}, style={:?}, scroll={}, visible={}",
                current_cursor_pos,
                self.cursor_opacity,
                cursor_style,
                scroll_offset,
                term.is_cursor_visible()
            );

            // Check if we need to regenerate cells
            // Only regenerate when content actually changes, not on every cursor blink
            let needs_regeneration = tab.cache.cells.is_none()
                || current_generation != tab.cache.generation
                || scroll_offset != tab.cache.scroll_offset
                || current_cursor_pos != tab.cache.cursor_pos // Regenerate if cursor position changed
                || mouse_selection != tab.cache.selection; // Regenerate if selection changed (including clearing)

            let cell_gen_start = std::time::Instant::now();
            let (cells, is_cache_hit) = if needs_regeneration {
                // Generate fresh cells
                let fresh_cells =
                    term.get_cells_with_scrollback(scroll_offset, selection, rectangular, cursor);

                (fresh_cells, false)
            } else {
                // Use cached cells - clone is still needed because of apply_url_underlines
                // but we track it accurately for debug logging
                (tab.cache.cells.as_ref().unwrap().clone(), true)
            };
            self.debug.cache_hit = is_cache_hit;
            self.debug.cell_gen_time = cell_gen_start.elapsed();

            // Check if alt screen is active (TUI apps like vim, htop)
            let is_alt_screen = term.is_alt_screen_active();

            (cells, current_cursor_pos, cursor_style, is_alt_screen)
        } else {
            return; // Terminal locked, skip this frame
        };

        // Ensure cursor visibility flag for cell renderer reflects current config every frame
        // (so toggling "Hide default cursor" takes effect immediately even if no other changes)
        // Resolve hides_cursor: per-shader override -> metadata defaults -> global config
        let resolved_hides_cursor = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.hides_cursor)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.settings_ui.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.hides_cursor)
            })
            .unwrap_or(self.config.cursor_shader_hides_cursor);
        // Resolve disable_in_alt_screen: per-shader override -> metadata defaults -> global config
        let resolved_disable_in_alt_screen = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.disable_in_alt_screen)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.settings_ui.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.disable_in_alt_screen)
            })
            .unwrap_or(self.config.cursor_shader_disable_in_alt_screen);
        let hide_cursor_for_shader = self.config.cursor_shader_enabled
            && resolved_hides_cursor
            && !(resolved_disable_in_alt_screen && is_alt_screen);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_cursor_hidden_for_shader(hide_cursor_for_shader);
        }

        // Update cache with regenerated cells (if needed)
        // Need to re-borrow as mutable after the terminal lock is released
        if !self.debug.cache_hit
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Ok(term) = tab.terminal.try_lock()
        {
            let current_generation = term.update_generation();
            tab.cache.cells = Some(cells.clone());
            tab.cache.generation = current_generation;
            tab.cache.scroll_offset = tab.scroll_state.offset;
            tab.cache.cursor_pos = current_cursor_pos;
            tab.cache.selection = tab.mouse.selection;
        }

        // Get scrollback length and terminal title from terminal
        // Note: When terminal width changes during resize, the core library clears
        // scrollback because the old cells would be misaligned with the new column count.
        // This is a limitation of the current implementation - proper reflow is not yet supported.
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return,
        };
        let terminal = &tab.terminal;
        let cached_scrollback_len = tab.cache.scrollback_len;
        let cached_terminal_title = tab.cache.terminal_title.clone();
        let hovered_url = tab.mouse.hovered_url.clone();

        let (scrollback_len, terminal_title) = if let Ok(term) = terminal.try_lock() {
            (term.scrollback_len(), term.get_title())
        } else {
            (cached_scrollback_len, cached_terminal_title.clone())
        };

        // Update cache scrollback and clamp scroll state
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.scrollback_len = scrollback_len;
            tab.scroll_state
                .clamp_to_scrollback(tab.cache.scrollback_len);
        }

        // Update window title if terminal has set one via OSC sequences
        // Only if allow_title_change is enabled and we're not showing a URL tooltip
        if self.config.allow_title_change
            && hovered_url.is_none()
            && terminal_title != cached_terminal_title
        {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.terminal_title = terminal_title.clone();
            }
            if let Some(window) = &self.window {
                if terminal_title.is_empty() {
                    // Restore configured title when terminal clears title
                    window.set_title(&self.config.window_title);
                } else {
                    // Use terminal-set title
                    window.set_title(&terminal_title);
                }
            }
        }

        // Total lines = visible lines + actual scrollback content
        let total_lines = visible_lines + scrollback_len;

        // Detect URLs in visible area (only when content changed)
        // This optimization saves ~0.26ms per frame on cache hits
        let url_detect_start = std::time::Instant::now();
        let debug_url_detect_time = if !self.debug.cache_hit {
            // Content changed - re-detect URLs
            self.detect_urls();
            url_detect_start.elapsed()
        } else {
            // Content unchanged - use cached URL detection
            std::time::Duration::ZERO
        };

        // Apply URL underlining to cells (always apply, since cells might be regenerated)
        let url_underline_start = std::time::Instant::now();
        let mut cells = cells; // Make cells mutable
        self.apply_url_underlines(&mut cells, &renderer_size);
        let _debug_url_underline_time = url_underline_start.elapsed();

        // Update cursor blink state
        self.update_cursor_blink();

        let render_start = std::time::Instant::now();

        let mut debug_update_cells_time = std::time::Duration::ZERO;
        #[allow(unused_assignments)]
        let mut debug_graphics_time = std::time::Duration::ZERO;
        #[allow(unused_assignments)]
        let mut debug_actual_render_time = std::time::Duration::ZERO;
        let _ = &debug_actual_render_time;
        // Clipboard action to handle after rendering (declared here to survive renderer borrow)
        let mut pending_clipboard_action = ClipboardHistoryAction::None;
        // Tab bar action to handle after rendering (declared here to survive renderer borrow)
        let mut pending_tab_action = TabBarAction::None;

        let show_scrollbar = self.should_show_scrollbar();

        if let Some(renderer) = &mut self.renderer {
            // Disable cursor shader when alt screen is active (TUI apps like vim, htop)
            let disable_cursor_shader =
                self.config.cursor_shader_disable_in_alt_screen && is_alt_screen;
            renderer.set_cursor_shader_disabled_for_alt_screen(disable_cursor_shader);

            // Only update renderer with cells if they changed (cache MISS)
            // This avoids re-uploading the same cell data to GPU on every frame
            if !self.debug.cache_hit {
                let t = std::time::Instant::now();
                renderer.update_cells(&cells);
                debug_update_cells_time = t.elapsed();
            }

            // Update cursor position and style for geometric rendering
            if let (Some(pos), Some(opacity), Some(style)) =
                (current_cursor_pos, Some(self.cursor_opacity), cursor_style)
            {
                renderer.update_cursor(pos, opacity, style);
                // Forward cursor state to custom shader for Ghostty-compatible cursor animations
                // Use the configured cursor color
                let cursor_color = [
                    self.config.cursor_color[0] as f32 / 255.0,
                    self.config.cursor_color[1] as f32 / 255.0,
                    self.config.cursor_color[2] as f32 / 255.0,
                    1.0,
                ];
                renderer.update_shader_cursor(pos.0, pos.1, opacity, cursor_color, style);
            } else {
                renderer.clear_cursor();
            }

            // If settings UI is visible, sync app config to UI working copy and push opacity
            if self.settings_ui.visible {
                let ui_cfg = self.settings_ui.current_config().clone();
                if (ui_cfg.window_opacity - self.config.window_opacity).abs() > 1e-4 {
                    log::info!(
                        "Syncing live opacity from UI {:.3} (app {:.3})",
                        ui_cfg.window_opacity,
                        self.config.window_opacity
                    );
                    self.config.window_opacity = ui_cfg.window_opacity;
                }

                renderer.update_opacity(self.config.window_opacity);
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.applied_opacity = self.config.window_opacity;
                    tab.cache.cells = None;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            // Update scrollbar
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            renderer.update_scrollbar(scroll_offset, visible_lines, total_lines);

            // Update animations and request redraw if frames changed
            let anim_start = std::time::Instant::now();
            if let Some(tab) = self.tab_manager.active_tab() {
                let terminal = tab.terminal.blocking_lock();
                if terminal.update_animations() {
                    // Animation frame changed - request continuous redraws while animations are playing
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            let debug_anim_time = anim_start.elapsed();

            // Update graphics from terminal (pass scroll_offset for view adjustment)
            // Include both current screen graphics and scrollback graphics
            // Use get_graphics_with_animations() to get current animation frames
            let graphics_start = std::time::Instant::now();
            if let Some(tab) = self.tab_manager.active_tab() {
                let terminal = tab.terminal.blocking_lock();
                let mut graphics = terminal.get_graphics_with_animations();
                let scrollback_len = terminal.scrollback_len();

                // Always include scrollback graphics (renderer will calculate visibility)
                let scrollback_graphics = terminal.get_scrollback_graphics();
                let scrollback_count = scrollback_graphics.len();
                graphics.extend(scrollback_graphics);

                debug_info!(
                    "APP",
                    "Got {} graphics ({} scrollback) from terminal (scroll_offset={}, scrollback_len={})",
                    graphics.len(),
                    scrollback_count,
                    scroll_offset,
                    scrollback_len
                );
                if let Err(e) = renderer.update_graphics(
                    &graphics,
                    scroll_offset,
                    scrollback_len,
                    visible_lines,
                ) {
                    log::error!("Failed to update graphics: {}", e);
                }
            }
            debug_graphics_time = graphics_start.elapsed();

            // Calculate visual bell flash intensity (0.0 = no flash, 1.0 = full flash)
            let visual_bell_flash = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.bell.visual_flash);
            let visual_bell_intensity = if let Some(flash_start) = visual_bell_flash {
                const FLASH_DURATION_MS: u128 = 150;
                let elapsed = flash_start.elapsed().as_millis();
                if elapsed < FLASH_DURATION_MS {
                    // Request continuous redraws while flash is active
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    // Fade out: start at 0.3 intensity, fade to 0
                    0.3 * (1.0 - (elapsed as f32 / FLASH_DURATION_MS as f32))
                } else {
                    // Flash complete - clear it
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.bell.visual_flash = None;
                    }
                    0.0
                }
            } else {
                0.0
            };

            // Update renderer with visual bell intensity
            renderer.set_visual_bell_intensity(visual_bell_intensity);

            // Prepare egui output if settings UI is visible
            let egui_start = std::time::Instant::now();

            // Capture values for FPS overlay before closure
            let show_fps = self.debug.show_fps_overlay;
            let fps_value = self.debug.fps_value;
            let frame_time_ms = if !self.debug.frame_times.is_empty() {
                let avg = self.debug.frame_times.iter().sum::<std::time::Duration>()
                    / self.debug.frame_times.len() as u32;
                avg.as_secs_f64() * 1000.0
            } else {
                0.0
            };

            let egui_data = if let (Some(egui_ctx), Some(egui_state)) =
                (&self.egui_ctx, &mut self.egui_state)
            {
                let raw_input = egui_state.take_egui_input(self.window.as_ref().unwrap());

                let egui_output = egui_ctx.run(raw_input, |ctx| {
                    // Show FPS overlay if enabled (top-right corner)
                    if show_fps {
                        egui::Area::new(egui::Id::new("fps_overlay"))
                            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-30.0, 10.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                                    .inner_margin(egui::Margin::same(8))
                                    .corner_radius(4.0)
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(0, 255, 0));
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "FPS: {:.1}\nFrame: {:.2}ms",
                                                fps_value, frame_time_ms
                                            ))
                                            .monospace()
                                            .size(14.0),
                                        );
                                    });
                            });
                    }

                    // Show resize overlay if active (centered)
                    if self.resize_overlay_visible
                        && let Some((width_px, height_px, cols, rows)) = self.resize_dimensions
                    {
                        egui::Area::new(egui::Id::new("resize_overlay"))
                            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                                    .inner_margin(egui::Margin::same(16))
                                    .corner_radius(8.0)
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(255, 255, 255));
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}×{}\n{}×{} px",
                                                cols, rows, width_px, height_px
                                            ))
                                            .monospace()
                                            .size(24.0),
                                        );
                                    });
                            });
                    }

                    // Render tab bar if visible (action handled after closure)
                    pending_tab_action =
                        self.tab_bar_ui.render(ctx, &self.tab_manager, &self.config);

                    // Settings are now handled by standalone SettingsWindow only
                    // No overlay settings UI rendering needed

                    // Show help UI
                    self.help_ui.show(ctx);

                    // Show clipboard history UI and collect action
                    pending_clipboard_action = self.clipboard_history_ui.show(ctx);
                });

                // Handle egui platform output (clipboard, cursor changes, etc.)
                // This enables cut/copy/paste in egui text editors
                egui_state.handle_platform_output(
                    self.window.as_ref().unwrap(),
                    egui_output.platform_output.clone(),
                );

                Some((egui_output, egui_ctx))
            } else {
                None
            };

            // Mark egui as initialized after first ctx.run() - makes is_using_pointer() reliable
            if !self.egui_initialized && egui_data.is_some() {
                self.egui_initialized = true;
            }

            // Settings are now handled exclusively by standalone SettingsWindow
            // Config changes are applied via window_manager.apply_config_to_windows()

            let debug_egui_time = egui_start.elapsed();

            // Calculate FPS and timing stats
            let avg_frame_time = if !self.debug.frame_times.is_empty() {
                self.debug.frame_times.iter().sum::<std::time::Duration>()
                    / self.debug.frame_times.len() as u32
            } else {
                std::time::Duration::ZERO
            };
            let fps = if avg_frame_time.as_secs_f64() > 0.0 {
                1.0 / avg_frame_time.as_secs_f64()
            } else {
                0.0
            };

            // Update FPS value for overlay display
            self.debug.fps_value = fps;

            // Log debug info every 60 frames (about once per second at 60 FPS)
            if self.debug.frame_times.len() >= 60 {
                let (cache_gen, cache_has_cells) = self
                    .tab_manager
                    .active_tab()
                    .map(|t| (t.cache.generation, t.cache.cells.is_some()))
                    .unwrap_or((0, false));
                log::info!(
                    "PERF: FPS={:.1} Frame={:.2}ms CellGen={:.2}ms({}) URLDetect={:.2}ms Anim={:.2}ms Graphics={:.2}ms egui={:.2}ms UpdateCells={:.2}ms ActualRender={:.2}ms Total={:.2}ms Cells={} Gen={} Cache={}",
                    fps,
                    avg_frame_time.as_secs_f64() * 1000.0,
                    self.debug.cell_gen_time.as_secs_f64() * 1000.0,
                    if self.debug.cache_hit { "HIT" } else { "MISS" },
                    debug_url_detect_time.as_secs_f64() * 1000.0,
                    debug_anim_time.as_secs_f64() * 1000.0,
                    debug_graphics_time.as_secs_f64() * 1000.0,
                    debug_egui_time.as_secs_f64() * 1000.0,
                    debug_update_cells_time.as_secs_f64() * 1000.0,
                    debug_actual_render_time.as_secs_f64() * 1000.0,
                    self.debug.render_time.as_secs_f64() * 1000.0,
                    cells.len(),
                    cache_gen,
                    if cache_has_cells { "YES" } else { "NO" }
                );
            }

            // Render (with dirty tracking optimization)
            let actual_render_start = std::time::Instant::now();
            match renderer.render(egui_data, self.settings_ui.visible, show_scrollbar) {
                Ok(rendered) => {
                    if !rendered {
                        log::trace!("Skipped rendering - no changes");
                    }
                }
                Err(e) => {
                    // Check if this is a wgpu surface error that requires reconfiguration
                    // This commonly happens when dragging windows between displays
                    if let Some(surface_error) = e.downcast_ref::<SurfaceError>() {
                        match surface_error {
                            SurfaceError::Outdated | SurfaceError::Lost => {
                                log::warn!(
                                    "Surface error detected ({:?}), reconfiguring...",
                                    surface_error
                                );
                                self.force_surface_reconfigure();
                            }
                            SurfaceError::Timeout => {
                                log::warn!("Surface timeout, will retry next frame");
                                if let Some(window) = &self.window {
                                    window.request_redraw();
                                }
                            }
                            SurfaceError::OutOfMemory => {
                                log::error!("Surface out of memory: {:?}", surface_error);
                            }
                            _ => {
                                log::error!("Surface error: {:?}", surface_error);
                            }
                        }
                    } else {
                        log::error!("Render error: {}", e);
                    }
                }
            }
            debug_actual_render_time = actual_render_start.elapsed();
            let _ = debug_actual_render_time;

            self.debug.render_time = render_start.elapsed();
        }

        // Handle tab bar actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match pending_tab_action {
            TabBarAction::SwitchTo(id) => {
                self.tab_manager.switch_to(id);
                // Clear renderer cells and invalidate cache to ensure clean switch
                if let Some(renderer) = &mut self.renderer {
                    renderer.clear_all_cells();
                }
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::Close(id) => {
                let was_last = self.tab_manager.close_tab(id);
                if was_last {
                    // Last tab closed - close window
                    self.is_shutting_down = true;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::NewTab => {
                self.new_tab();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::SetColor(id, color) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.set_custom_color(color);
                    log::info!(
                        "Set custom color for tab {}: RGB({}, {}, {})",
                        id,
                        color[0],
                        color[1],
                        color[2]
                    );
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::ClearColor(id) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.clear_custom_color();
                    log::info!("Cleared custom color for tab {}", id);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::None | TabBarAction::Reorder(_, _) => {}
        }

        // Handle clipboard actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match pending_clipboard_action {
            ClipboardHistoryAction::Paste(content) => {
                self.paste_text(&content);
            }
            ClipboardHistoryAction::ClearAll => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    term.clear_all_clipboard_history();
                    log::info!("Cleared all clipboard history");
                }
                self.clipboard_history_ui.update_entries(Vec::new());
            }
            ClipboardHistoryAction::ClearSlot(slot) => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    term.clear_clipboard_history(slot);
                    log::info!("Cleared clipboard history for slot {:?}", slot);
                }
            }
            ClipboardHistoryAction::None => {}
        }

        let absolute_total = absolute_start.elapsed();
        if absolute_total.as_millis() > 10 {
            log::debug!(
                "TIMING: AbsoluteTotal={:.2}ms (from function start to end)",
                absolute_total.as_secs_f64() * 1000.0
            );
        }
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
        log::info!("Shutting down window");

        // Set shutdown flag
        self.is_shutting_down = true;

        // Clean up all tabs
        let tab_count = self.tab_manager.tab_count();
        log::info!("Cleaning up {} tabs", tab_count);

        // Stop all refresh tasks first
        for tab in self.tab_manager.tabs_mut() {
            tab.stop_refresh_task();
        }
        log::info!("All refresh tasks aborted");

        // Give abort time to take effect and any pending operations to complete
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Kill all PTY processes
        for tab in self.tab_manager.tabs_mut() {
            if let Ok(mut term) = tab.terminal.try_lock() {
                if term.is_running() {
                    log::info!("Killing PTY process for tab {}", tab.id);
                    match term.kill() {
                        Ok(()) => {
                            log::info!("PTY process killed successfully for tab {}", tab.id);
                        }
                        Err(e) => {
                            log::warn!("Failed to kill PTY process for tab {}: {:?}", tab.id, e);
                        }
                    }
                } else {
                    log::info!("PTY process already stopped for tab {}", tab.id);
                }
            } else {
                log::warn!(
                    "Could not acquire terminal lock to kill PTY for tab {}",
                    tab.id
                );
            }
        }

        // Give the PTY time to clean up after kill signal
        std::thread::sleep(std::time::Duration::from_millis(100));

        log::info!("Window shutdown complete");
    }
}
