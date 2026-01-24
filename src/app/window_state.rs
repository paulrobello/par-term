//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, terminal, input handler, and UI components.

use crate::app::bell::BellState;
use crate::app::debug_state::DebugState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::clipboard_history_ui::{ClipboardHistoryAction, ClipboardHistoryUI};
use crate::config::Config;
use crate::help_ui::HelpUI;
use crate::input::InputHandler;
use crate::renderer::Renderer;
use crate::scroll_state::ScrollState;
use crate::selection::SelectionMode;
use crate::settings_ui::{CursorShaderEditorResult, SettingsUI, ShaderEditorResult};
use crate::terminal::TerminalManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use wgpu::SurfaceError;
use winit::event::KeyEvent;
use winit::window::Window;

/// Per-window state that manages a single terminal window
pub struct WindowState {
    pub(crate) config: Config,
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) renderer: Option<Renderer>,
    pub(crate) terminal: Option<Arc<Mutex<TerminalManager>>>,
    pub(crate) input_handler: InputHandler,
    pub(crate) refresh_task: Option<JoinHandle<()>>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) scroll_state: ScrollState,

    pub(crate) mouse: MouseState,
    pub(crate) debug: DebugState,
    pub(crate) bell: BellState,
    pub(crate) cache: RenderCache,

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
}

impl WindowState {
    /// Create a new window state with the given configuration
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Self {
        let initial_opacity = config.window_opacity;
        let settings_ui = SettingsUI::new(config.clone());

        Self {
            config,
            window: None,
            renderer: None,
            terminal: None,
            input_handler: InputHandler::new(),
            refresh_task: None,
            runtime,
            scroll_state: ScrollState::new(),

            mouse: MouseState::new(),
            debug: DebugState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(initial_opacity),

            cursor_opacity: 1.0,
            last_cursor_blink: None,
            last_key_press: None,
            is_fullscreen: false,
            egui_ctx: None,
            egui_state: None,
            settings_ui,
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            is_recording: false,
            recording_start_time: None,
            is_shutting_down: false,

            needs_redraw: true,
            cursor_blink_timer: None,
            pending_font_rebuild: false,
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

    /// Rebuild the renderer after font-related changes and resize the terminal accordingly
    pub(crate) fn rebuild_renderer(&mut self) -> Result<()> {
        let window = if let Some(w) = &self.window {
            Arc::clone(w)
        } else {
            return Ok(()); // Nothing to rebuild yet
        };

        let theme = self.config.load_theme();
        let font_family = if self.config.font_family.is_empty() {
            None
        } else {
            Some(self.config.font_family.as_str())
        };

        let mut renderer = self.runtime.block_on(Renderer::new(
            Arc::clone(&window),
            font_family,
            self.config.font_family_bold.as_deref(),
            self.config.font_family_italic.as_deref(),
            self.config.font_family_bold_italic.as_deref(),
            &self.config.font_ranges,
            self.config.font_size,
            self.config.window_padding,
            self.config.line_spacing,
            self.config.char_spacing,
            &self.config.scrollbar_position,
            self.config.scrollbar_width,
            self.config.scrollbar_thumb_color,
            self.config.scrollbar_track_color,
            self.config.enable_text_shaping,
            self.config.enable_ligatures,
            self.config.enable_kerning,
            self.config.vsync_mode,
            self.config.window_opacity,
            theme.background.as_array(),
            self.config.background_image.as_deref(),
            self.config.background_image_enabled,
            self.config.background_image_mode,
            self.config.background_image_opacity,
            self.config.custom_shader.as_deref(),
            self.config.custom_shader_enabled,
            self.config.custom_shader_animation,
            self.config.custom_shader_animation_speed,
            self.config.custom_shader_text_opacity,
            self.config.custom_shader_full_content,
            // Cursor shader settings
            self.config.cursor_shader.as_deref(),
            self.config.cursor_shader_enabled,
            self.config.cursor_shader_animation,
            self.config.cursor_shader_animation_speed,
        ))?;

        let (cols, rows) = renderer.grid_size();
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (cols as f32 * cell_width) as usize;
        let height_px = (rows as f32 * cell_height) as usize;

        if let Some(terminal) = &self.terminal
            && let Ok(mut term) = terminal.try_lock()
        {
            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
            term.set_theme(self.config.load_theme());
        }

        // Initialize cursor shader config
        renderer.update_cursor_shader_config(
            self.config.cursor_shader_color,
            self.config.cursor_shader_trail_duration,
            self.config.cursor_shader_glow_radius,
            self.config.cursor_shader_glow_intensity,
        );

        // Initialize cursor color from config
        renderer.update_cursor_color(self.config.cursor_color);

        self.renderer = Some(renderer);
        self.cache.cells = None;
        self.needs_redraw = true;

        // Reset egui GPU textures so the new renderer has a fresh atlas, but
        // preserve window positions/collapse state by cloning the previous
        // egui memory into the new context (otherwise the Settings window
        // snaps to the top-left and all panels collapse after font changes).
        let previous_memory = self
            .egui_ctx
            .as_ref()
            .map(|ctx| ctx.memory(|mem| mem.clone()));

        let scale_factor = window.scale_factor() as f32;
        let egui_ctx = egui::Context::default();
        if let Some(memory) = previous_memory {
            egui_ctx.memory_mut(|mem| *mem = memory);
        }
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(scale_factor),
            None,
            None,
        );
        self.egui_ctx = Some(egui_ctx);
        self.egui_state = Some(egui_state);

        if let Some(window) = &self.window {
            window.request_redraw();
        }

        Ok(())
    }

    /// Initialize the window asynchronously
    pub(crate) async fn initialize_async(&mut self, window: Window) -> Result<()> {
        // Enable IME (Input Method Editor) to receive all character events including Space
        window.set_ime_allowed(true);
        log::debug!("IME enabled for character input");

        let window = Arc::new(window);

        // Initialize egui context and state
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None, // max_texture_side
        );
        self.egui_ctx = Some(egui_ctx);
        self.egui_state = Some(egui_state);

        // Create renderer with font family from config
        let font_family = if self.config.font_family.is_empty() {
            None
        } else {
            Some(self.config.font_family.as_str())
        };
        let theme = self.config.load_theme();
        let mut renderer = Renderer::new(
            Arc::clone(&window),
            font_family,
            self.config.font_family_bold.as_deref(),
            self.config.font_family_italic.as_deref(),
            self.config.font_family_bold_italic.as_deref(),
            &self.config.font_ranges,
            self.config.font_size,
            self.config.window_padding,
            self.config.line_spacing,
            self.config.char_spacing,
            &self.config.scrollbar_position,
            self.config.scrollbar_width,
            self.config.scrollbar_thumb_color,
            self.config.scrollbar_track_color,
            self.config.enable_text_shaping,
            self.config.enable_ligatures,
            self.config.enable_kerning,
            self.config.vsync_mode,
            self.config.window_opacity,
            theme.background.as_array(),
            self.config.background_image.as_deref(),
            self.config.background_image_enabled,
            self.config.background_image_mode,
            self.config.background_image_opacity,
            self.config.custom_shader.as_deref(),
            self.config.custom_shader_enabled,
            self.config.custom_shader_animation,
            self.config.custom_shader_animation_speed,
            self.config.custom_shader_text_opacity,
            self.config.custom_shader_full_content,
            // Cursor shader settings
            self.config.cursor_shader.as_deref(),
            self.config.cursor_shader_enabled,
            self.config.cursor_shader_animation,
            self.config.cursor_shader_animation_speed,
        )
        .await?;

        // macOS: also update the NSWindow alpha so the OS compositor reflects live opacity
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
        }

        // Create terminal with scrollback from config
        let mut terminal = TerminalManager::new_with_scrollback(
            self.config.cols,
            self.config.rows,
            self.config.scrollback_lines,
        )?;

        // Set theme from config
        terminal.set_theme(self.config.load_theme());

        // Apply clipboard history limits from config
        terminal.set_max_clipboard_sync_events(self.config.clipboard_max_sync_events);
        terminal.set_max_clipboard_event_bytes(self.config.clipboard_max_event_bytes);

        // Ensure PTY dimensions match the renderer's computed grid
        let (renderer_cols, renderer_rows) = renderer.grid_size();
        log::info!(
            "Initial terminal dimensions: {}x{}",
            renderer_cols,
            renderer_rows
        );

        // Calculate pixel dimensions and resize PTY with both character and pixel dimensions
        // This is required for applications like kitty icat that query pixel dimensions via TIOCGWINSZ
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (renderer_cols as f32 * cell_width) as usize;
        let height_px = (renderer_rows as f32 * cell_height) as usize;
        terminal.resize_with_pixels(renderer_cols, renderer_rows, width_px, height_px)?;
        log::info!(
            "Initial terminal pixel dimensions: {}x{} px",
            width_px,
            height_px
        );

        // Spawn shell (custom or default) with optional working directory, args, and env vars
        let working_dir = self.config.working_directory.as_deref();
        let shell_env = self.config.shell_env.as_ref();

        // Determine the shell command to use
        let (shell_cmd, mut shell_args) = if let Some(ref custom) = self.config.custom_shell {
            (custom.clone(), self.config.shell_args.clone())
        } else {
            #[cfg(target_os = "windows")]
            {
                ("powershell.exe".to_string(), None)
            }
            #[cfg(not(target_os = "windows"))]
            {
                (
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
                    None,
                )
            }
        };

        // On Unix-like systems, spawn as login shell if configured (default: true)
        // This ensures PATH is properly initialized from /etc/paths, ~/.zprofile, etc.
        #[cfg(not(target_os = "windows"))]
        if self.config.login_shell {
            let args = shell_args.get_or_insert_with(Vec::new);
            // Only add -l if not already present
            if !args.iter().any(|a| a == "-l" || a == "--login") {
                args.insert(0, "-l".to_string());
            }
        }

        let shell_args_deref = shell_args.as_deref();
        terminal.spawn_custom_shell_with_dir(
            &shell_cmd,
            shell_args_deref,
            working_dir,
            shell_env,
        )?;

        // Set cell dimensions on terminal for proper graphics scroll calculations
        let cell_width = renderer.cell_width() as u32;
        let cell_height = renderer.cell_height() as u32;
        log::info!("Setting cell dimensions: {}x{}", cell_width, cell_height);
        terminal.set_cell_dimensions(cell_width, cell_height);

        // Initialize cursor shader config
        renderer.update_cursor_shader_config(
            self.config.cursor_shader_color,
            self.config.cursor_shader_trail_duration,
            self.config.cursor_shader_glow_radius,
            self.config.cursor_shader_glow_intensity,
        );

        // Initialize cursor color from config
        renderer.update_cursor_color(self.config.cursor_color);

        self.window = Some(Arc::clone(&window));
        self.renderer = Some(renderer);
        self.terminal = Some(Arc::new(Mutex::new(terminal)));

        // Start update polling task to check for terminal changes
        let window_clone = Arc::clone(&window);
        let terminal_clone = Arc::clone(self.terminal.as_ref().unwrap());
        let max_fps = self.config.max_fps.max(1);
        let refresh_interval_ms = 1000 / max_fps;

        let handle = self.runtime.spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
                refresh_interval_ms as u64,
            ));
            // Track the last seen generation to detect changes
            let mut last_gen = 0;

            loop {
                interval.tick().await;

                // Check if terminal has updates (using generation counter if available, or has_updates flag)
                // We use try_lock to avoid blocking the PTY thread
                let should_redraw = if let Ok(term) = terminal_clone.try_lock() {
                    let current_gen = term.update_generation();
                    if current_gen > last_gen {
                        last_gen = current_gen;
                        true
                    } else {
                        // Also check for animations or other updates that might not bump generation
                        term.has_updates()
                    }
                } else {
                    // contention - retry next tick
                    false
                };

                if should_redraw {
                    window_clone.request_redraw();
                }
            }
        });
        self.refresh_task = Some(handle);

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
            self.cache.cells = None;
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
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        self.needs_redraw = true;
    }

    pub(crate) fn handle_fullscreen_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F11: Toggle fullscreen
        if matches!(event.logical_key, Key::Named(NamedKey::F11))
            && let Some(window) = &self.window
        {
            self.is_fullscreen = !self.is_fullscreen;

            if self.is_fullscreen {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                log::info!("Entering fullscreen mode");
            } else {
                window.set_fullscreen(None);
                log::info!("Exiting fullscreen mode");
            }

            return true;
        }

        false
    }

    pub(crate) fn handle_settings_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F12: Toggle settings UI
        if matches!(event.logical_key, Key::Named(NamedKey::F12)) {
            self.settings_ui.toggle();
            log::info!(
                "Settings UI toggled: {}",
                if self.settings_ui.visible {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide settings
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F1 key to toggle help panel
    pub(crate) fn handle_help_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F1: Toggle help UI
        if matches!(event.logical_key, Key::Named(NamedKey::F1)) {
            self.help_ui.toggle();
            log::info!(
                "Help UI toggled: {}",
                if self.help_ui.visible {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide help
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        // Escape: Close help UI if visible
        if matches!(event.logical_key, Key::Named(NamedKey::Escape)) && self.help_ui.visible {
            self.help_ui.visible = false;
            log::info!("Help UI closed via Escape");

            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F11 key to toggle shader editor
    pub(crate) fn handle_shader_editor_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F11: Toggle shader editor
        if matches!(event.logical_key, Key::Named(NamedKey::F11)) {
            if self.settings_ui.is_shader_editor_visible() {
                // Close shader editor - handled by the UI itself
                log::info!("Shader editor close requested via F11");
            } else {
                // Open shader editor
                if self.settings_ui.open_shader_editor() {
                    log::info!("Shader editor opened via F11");
                } else {
                    log::warn!("Cannot open shader editor: no shader path configured in settings");
                }
            }

            // Request redraw to show/hide shader editor
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F3 key to toggle FPS overlay
    pub(crate) fn handle_fps_overlay_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F3: Toggle FPS overlay
        if matches!(event.logical_key, Key::Named(NamedKey::F3)) {
            self.debug.show_fps_overlay = !self.debug.show_fps_overlay;
            log::info!(
                "FPS overlay toggled: {}",
                if self.debug.show_fps_overlay {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide FPS overlay
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    pub(crate) fn scroll_up_page(&mut self) {
        // Calculate page size based on visible lines
        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = self.scroll_state.target_offset.saturating_add(page_size);
            let clamped_target = new_target.min(self.cache.scrollback_len);
            self.set_scroll_target(clamped_target);
        }
    }

    pub(crate) fn scroll_down_page(&mut self) {
        // Calculate page size based on visible lines
        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = self.scroll_state.target_offset.saturating_sub(page_size);
            self.set_scroll_target(new_target);
        }
    }

    pub(crate) fn scroll_to_top(&mut self) {
        self.set_scroll_target(self.cache.scrollback_len);
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        self.set_scroll_target(0);
    }

    /// Check if egui is currently using the pointer (mouse is over an egui UI element)
    pub(crate) fn is_egui_using_pointer(&self) -> bool {
        // If any UI panel is visible, check if egui wants the pointer
        let any_ui_visible =
            self.settings_ui.visible || self.help_ui.visible || self.clipboard_history_ui.visible;
        if !any_ui_visible {
            return false;
        }

        // Check egui context for pointer usage
        if let Some(ctx) = &self.egui_ctx {
            ctx.is_using_pointer() || ctx.wants_pointer_input()
        } else {
            false
        }
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        let any_ui_visible =
            self.settings_ui.visible || self.help_ui.visible || self.clipboard_history_ui.visible;
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
        // No scrollbar needed if no scrollback available
        if self.cache.scrollback_len == 0 {
            return false;
        }

        // Always show when dragging or moving
        if self.scroll_state.dragging {
            return true;
        }

        // If autohide disabled, always show
        if self.config.scrollbar_autohide_delay == 0 {
            return true;
        }

        // If scrolled away from bottom, keep visible
        if self.scroll_state.offset > 0 || self.scroll_state.target_offset > 0 {
            return true;
        }

        // Show when pointer is near the scrollbar edge (hover reveal)
        if let Some(window) = &self.window {
            let padding = 32.0; // px hover band
            let width = window.inner_size().width as f64;
            let near_right = self.config.scrollbar_position != "left"
                && (width - self.mouse.position.0) <= padding;
            let near_left =
                self.config.scrollbar_position == "left" && self.mouse.position.0 <= padding;
            if near_left || near_right {
                return true;
            }
        }

        // Otherwise, hide after delay
        self.scroll_state.last_activity.elapsed().as_millis()
            < self.config.scrollbar_autohide_delay as u128
    }

    /// Update cursor blink state based on configured interval
    pub(crate) fn update_cursor_blink(&mut self) {
        if !self.config.cursor_blink {
            // Smoothly fade to full visibility if blinking disabled
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
        let animation_running = self.scroll_state.update_animation();

        // Rebuild renderer if font-related settings changed
        if self.pending_font_rebuild {
            if let Err(e) = self.rebuild_renderer() {
                log::error!("Failed to rebuild renderer after font change: {}", e);
            }
            self.pending_font_rebuild = false;
        }

        let (renderer_size, visible_lines) = if let Some(renderer) = &self.renderer {
            (renderer.size(), renderer.grid_size().1)
        } else {
            return;
        };

        let terminal = if let Some(terminal) = &self.terminal {
            terminal
        } else {
            return;
        };

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

        // Get terminal cells for rendering (with dirty tracking optimization)
        let (cells, current_cursor_pos, cursor_style) = if let Ok(term) = terminal.try_lock() {
            // Get current generation to check if terminal content has changed
            let current_generation = term.update_generation();

            // Normalize selection if it exists and extract mode
            let (selection, rectangular) = if let Some(sel) = self.mouse.selection {
                (
                    Some(sel.normalized()),
                    sel.mode == SelectionMode::Rectangular,
                )
            } else {
                (None, false)
            };

            // Get cursor position and opacity (only show if we're at the bottom with no scroll offset
            // and the cursor is visible - TUI apps hide cursor via DECTCEM escape sequence)
            let current_cursor_pos = if self.scroll_state.offset == 0 && term.is_cursor_visible() {
                Some(term.cursor_position())
            } else {
                None
            };

            let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_opacity));

            // Get cursor style for geometric rendering
            let cursor_style = if current_cursor_pos.is_some() {
                Some(term.cursor_style())
            } else {
                None
            };

            log::trace!(
                "Cursor: pos={:?}, opacity={:.2}, style={:?}, scroll={}, visible={}",
                current_cursor_pos,
                self.cursor_opacity,
                cursor_style,
                self.scroll_state.offset,
                term.is_cursor_visible()
            );

            // Check if we need to regenerate cells
            // Only regenerate when content actually changes, not on every cursor blink
            let needs_regeneration = self.cache.cells.is_none()
                || current_generation != self.cache.generation
                || self.scroll_state.offset != self.cache.scroll_offset
                || current_cursor_pos != self.cache.cursor_pos // Regenerate if cursor position changed
                || self.mouse.selection != self.cache.selection; // Regenerate if selection changed (including clearing)

            let cell_gen_start = std::time::Instant::now();
            let (cells, is_cache_hit) = if needs_regeneration {
                // Generate fresh cells
                let fresh_cells = term.get_cells_with_scrollback(
                    self.scroll_state.offset,
                    selection,
                    rectangular,
                    cursor,
                );

                // Update cache
                self.cache.cells = Some(fresh_cells.clone());
                self.cache.generation = current_generation;
                self.cache.scroll_offset = self.scroll_state.offset;
                self.cache.cursor_pos = current_cursor_pos;
                self.cache.selection = self.mouse.selection;

                (fresh_cells, false)
            } else {
                // Use cached cells - clone is still needed because of apply_url_underlines
                // but we track it accurately for debug logging
                (self.cache.cells.as_ref().unwrap().clone(), true)
            };
            self.debug.cache_hit = is_cache_hit;
            self.debug.cell_gen_time = cell_gen_start.elapsed();

            (cells, current_cursor_pos, cursor_style)
        } else {
            return; // Terminal locked, skip this frame
        };

        // Get scrollback length and terminal title from terminal
        // Note: When terminal width changes during resize, the core library clears
        // scrollback because the old cells would be misaligned with the new column count.
        // This is a limitation of the current implementation - proper reflow is not yet supported.
        let (scrollback_len, terminal_title) = if let Ok(term) = terminal.try_lock() {
            (term.scrollback_len(), term.get_title())
        } else {
            (self.cache.scrollback_len, self.cache.terminal_title.clone())
        };

        self.cache.scrollback_len = scrollback_len;
        self.scroll_state
            .clamp_to_scrollback(self.cache.scrollback_len);

        // Update window title if terminal has set one via OSC sequences
        // Only if allow_title_change is enabled and we're not showing a URL tooltip
        if self.config.allow_title_change
            && self.mouse.hovered_url.is_none()
            && terminal_title != self.cache.terminal_title
            && let Some(window) = &self.window
        {
            self.cache.terminal_title = terminal_title.clone();
            if terminal_title.is_empty() {
                // Restore configured title when terminal clears title
                window.set_title(&self.config.window_title);
            } else {
                // Use terminal-set title
                window.set_title(&terminal_title);
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

        let show_scrollbar = self.should_show_scrollbar();

        if let Some(renderer) = &mut self.renderer {
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
                self.cache.applied_opacity = self.config.window_opacity;
                self.cache.cells = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            // Update scrollbar
            renderer.update_scrollbar(self.scroll_state.offset, visible_lines, total_lines);

            // Update animations and request redraw if frames changed
            let anim_start = std::time::Instant::now();
            if let Some(terminal) = &self.terminal {
                let terminal = terminal.blocking_lock();
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
            if let Some(terminal) = &self.terminal {
                let terminal = terminal.blocking_lock();
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
                    self.scroll_state.offset,
                    scrollback_len
                );
                if let Err(e) = renderer.update_graphics(
                    &graphics,
                    self.scroll_state.offset,
                    scrollback_len,
                    visible_lines,
                ) {
                    log::error!("Failed to update graphics: {}", e);
                }
            }
            debug_graphics_time = graphics_start.elapsed();

            // Calculate visual bell flash intensity (0.0 = no flash, 1.0 = full flash)
            let visual_bell_intensity = if let Some(flash_start) = self.bell.visual_flash {
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
                    self.bell.visual_flash = None;
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

            // Track config changes from settings UI (to be applied after egui block)
            #[allow(clippy::type_complexity)]
            let mut pending_config_update: Option<(
                Option<crate::config::Config>,
                Option<crate::config::Config>,
                Option<ShaderEditorResult>,
                Option<CursorShaderEditorResult>,
            )> = None;

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

                    // Show settings UI and store results for later processing
                    let settings_result = self.settings_ui.show(ctx);
                    pending_config_update = Some(settings_result);

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

            // Process settings changes after egui block (to avoid borrow conflicts)
            if let Some((
                config_to_save,
                config_for_live_update,
                shader_apply,
                cursor_shader_apply,
            )) = pending_config_update
            {
                // Handle background shader apply request first
                if let Some(shader_result) = shader_apply {
                    log::info!(
                        "Applying background shader from editor ({} bytes)",
                        shader_result.source.len()
                    );
                    match renderer.reload_shader_from_source(&shader_result.source) {
                        Ok(()) => {
                            log::info!("Background shader applied successfully from editor");
                            self.settings_ui.clear_shader_error();
                        }
                        Err(e) => {
                            let error_msg = format!("{:#}", e);
                            log::error!("Background shader compilation failed: {}", error_msg);
                            self.settings_ui.set_shader_error(Some(error_msg));
                        }
                    }
                }

                // Handle cursor shader apply request
                if let Some(cursor_shader_result) = cursor_shader_apply {
                    log::info!(
                        "Applying cursor shader from editor ({} bytes)",
                        cursor_shader_result.source.len()
                    );
                    match renderer.reload_cursor_shader_from_source(&cursor_shader_result.source) {
                        Ok(()) => {
                            log::info!("Cursor shader applied successfully from editor");
                            self.settings_ui.clear_cursor_shader_error();
                        }
                        Err(e) => {
                            let error_msg = format!("{:#}", e);
                            log::error!("Cursor shader compilation failed: {}", error_msg);
                            self.settings_ui.set_cursor_shader_error(Some(error_msg));
                        }
                    }
                }
                // Apply live updates immediately (for visual feedback)
                if let Some(live_config) = config_for_live_update {
                    let theme_changed = live_config.theme != self.config.theme;
                    let shader_animation_changed =
                        live_config.custom_shader_animation != self.config.custom_shader_animation;
                    let shader_enabled_changed =
                        live_config.custom_shader_enabled != self.config.custom_shader_enabled;
                    let shader_path_changed =
                        live_config.custom_shader != self.config.custom_shader;
                    let shader_speed_changed = (live_config.custom_shader_animation_speed
                        - self.config.custom_shader_animation_speed)
                        .abs()
                        > f32::EPSILON;
                    let shader_full_content_changed = live_config.custom_shader_full_content
                        != self.config.custom_shader_full_content;
                    let shader_text_opacity_changed = (live_config.custom_shader_text_opacity
                        - self.config.custom_shader_text_opacity)
                        .abs()
                        > f32::EPSILON;
                    let cursor_shader_config_changed = live_config.cursor_shader_color
                        != self.config.cursor_shader_color
                        || (live_config.cursor_shader_trail_duration
                            - self.config.cursor_shader_trail_duration)
                            .abs()
                            > f32::EPSILON
                        || (live_config.cursor_shader_glow_radius
                            - self.config.cursor_shader_glow_radius)
                            .abs()
                            > f32::EPSILON
                        || (live_config.cursor_shader_glow_intensity
                            - self.config.cursor_shader_glow_intensity)
                            .abs()
                            > f32::EPSILON;
                    let cursor_shader_path_changed =
                        live_config.cursor_shader != self.config.cursor_shader;
                    let cursor_shader_enabled_changed =
                        live_config.cursor_shader_enabled != self.config.cursor_shader_enabled;
                    let cursor_shader_animation_changed =
                        live_config.cursor_shader_animation != self.config.cursor_shader_animation;
                    let cursor_shader_speed_changed = (live_config.cursor_shader_animation_speed
                        - self.config.cursor_shader_animation_speed)
                        .abs()
                        > f32::EPSILON;
                    let _scrollbar_position_changed =
                        live_config.scrollbar_position != self.config.scrollbar_position;
                    let window_title_changed = live_config.window_title != self.config.window_title;
                    let window_decorations_changed =
                        live_config.window_decorations != self.config.window_decorations;
                    let max_fps_changed = live_config.max_fps != self.config.max_fps;
                    let cursor_style_changed = live_config.cursor_style != self.config.cursor_style;
                    let cursor_color_changed = live_config.cursor_color != self.config.cursor_color;
                    let bg_enabled_changed = live_config.background_image_enabled
                        != self.config.background_image_enabled;
                    let bg_path_changed =
                        live_config.background_image != self.config.background_image;
                    let bg_mode_changed =
                        live_config.background_image_mode != self.config.background_image_mode;
                    let bg_opacity_changed = (live_config.background_image_opacity
                        - self.config.background_image_opacity)
                        .abs()
                        > f32::EPSILON;
                    let font_changed = live_config.font_family != self.config.font_family
                        || live_config.font_family_bold != self.config.font_family_bold
                        || live_config.font_family_italic != self.config.font_family_italic
                        || live_config.font_family_bold_italic
                            != self.config.font_family_bold_italic
                        || (live_config.font_size - self.config.font_size).abs() > f32::EPSILON
                        || (live_config.line_spacing - self.config.line_spacing).abs()
                            > f32::EPSILON
                        || (live_config.char_spacing - self.config.char_spacing).abs()
                            > f32::EPSILON;
                    let padding_changed = (live_config.window_padding - self.config.window_padding)
                        .abs()
                        > f32::EPSILON;
                    log::info!(
                        "Applying live config update - opacity: {}{}{}",
                        live_config.window_opacity,
                        if theme_changed {
                            " (theme changed)"
                        } else {
                            ""
                        },
                        if font_changed { " (font changed)" } else { "" }
                    );
                    self.config = live_config;
                    self.scroll_state.last_activity = std::time::Instant::now();

                    // Apply settings that can be updated in real-time
                    if let Some(window) = &self.window {
                        // Update window level (always on top)
                        window.set_window_level(if self.config.window_always_on_top {
                            winit::window::WindowLevel::AlwaysOnTop
                        } else {
                            winit::window::WindowLevel::Normal
                        });

                        // Update window title
                        if window_title_changed {
                            window.set_title(&self.config.window_title);
                            log::info!("Updated window title to: {}", self.config.window_title);
                        }

                        // Update window decorations
                        if window_decorations_changed {
                            window.set_decorations(self.config.window_decorations);
                            log::info!(
                                "Updated window decorations: {}",
                                self.config.window_decorations
                            );
                        }

                        // Request redraw to apply visual changes
                        window.request_redraw();
                    }

                    // Update max_fps (restart refresh timer with new interval)
                    if max_fps_changed {
                        // Abort the old timer task
                        if let Some(old_task) = self.refresh_task.take() {
                            old_task.abort();
                        }
                        // Start new timer with updated interval
                        if let Some(window) = &self.window {
                            let window_clone = Arc::clone(window);
                            let refresh_interval_ms = 1000 / self.config.max_fps.max(1); // Convert Hz to ms
                            let handle = self.runtime.spawn(async move {
                                let mut interval = tokio::time::interval(
                                    tokio::time::Duration::from_millis(refresh_interval_ms as u64),
                                );
                                loop {
                                    interval.tick().await;
                                    window_clone.request_redraw();
                                }
                            });
                            self.refresh_task = Some(handle);
                            log::info!(
                                "Updated max_fps to {} ({}ms interval)",
                                self.config.max_fps,
                                refresh_interval_ms
                            );
                        }
                    }

                    // Update renderer with real-time settings
                    renderer.update_opacity(self.config.window_opacity);
                    renderer.update_scrollbar_appearance(
                        self.config.scrollbar_width,
                        self.config.scrollbar_thumb_color,
                        self.config.scrollbar_track_color,
                    );
                    // Scrollbar position is now fixed to right; ignore config changes

                    if cursor_style_changed {
                        // Set cursor style directly on the terminal (no need to send DECSCUSR to PTY)
                        // This updates the terminal's internal cursor state without involving the shell
                        if let Some(terminal_mgr) = &self.terminal
                            && let Ok(term_mgr) = terminal_mgr.try_lock()
                        {
                            // Get the underlying Terminal from TerminalManager
                            let terminal = term_mgr.terminal();
                            if let Some(mut term) = terminal.try_lock() {
                                // Convert config cursor style to terminal cursor style
                                use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
                                let term_style = match self.config.cursor_style {
                                    crate::config::CursorStyle::Block => {
                                        TermCursorStyle::SteadyBlock
                                    }
                                    crate::config::CursorStyle::Underline => {
                                        TermCursorStyle::SteadyUnderline
                                    }
                                    crate::config::CursorStyle::Beam => TermCursorStyle::SteadyBar,
                                };
                                term.set_cursor_style(term_style);
                            }
                        }

                        // Force cell regen to reflect cursor style change
                        self.cache.cells = None;
                        self.cache.cursor_pos = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }

                    // Update cursor color
                    if cursor_color_changed {
                        renderer.update_cursor_color(self.config.cursor_color);
                        // Force cell regen to reflect cursor color change
                        self.cache.cells = None;
                        self.cache.cursor_pos = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }

                    if self.config.background_image_enabled {
                        renderer
                            .update_background_image_opacity(self.config.background_image_opacity);
                    }

                    if bg_enabled_changed
                        || bg_path_changed
                        || bg_mode_changed
                        || bg_opacity_changed
                    {
                        renderer.set_background_image_enabled(
                            self.config.background_image_enabled,
                            self.config.background_image.as_deref(),
                            self.config.background_image_mode,
                            self.config.background_image_opacity,
                        );
                    }

                    if shader_animation_changed
                        || shader_enabled_changed
                        || shader_path_changed
                        || shader_speed_changed
                        || shader_full_content_changed
                        || shader_text_opacity_changed
                    {
                        match renderer.set_custom_shader_enabled(
                            self.config.custom_shader_enabled,
                            self.config.custom_shader.as_deref(),
                            self.config.window_opacity,
                            self.config.custom_shader_text_opacity,
                            self.config.custom_shader_animation,
                            self.config.custom_shader_animation_speed,
                            self.config.custom_shader_full_content,
                        ) {
                            Ok(()) => {
                                self.settings_ui.clear_shader_error();
                            }
                            Err(error_msg) => {
                                log::error!("Shader compilation failed: {}", error_msg);
                                self.settings_ui.set_shader_error(Some(error_msg));
                            }
                        }
                    }

                    // Update cursor shader configuration
                    if cursor_shader_config_changed {
                        renderer.update_cursor_shader_config(
                            self.config.cursor_shader_color,
                            self.config.cursor_shader_trail_duration,
                            self.config.cursor_shader_glow_radius,
                            self.config.cursor_shader_glow_intensity,
                        );
                    }

                    // Handle cursor shader path/enabled/animation changes
                    if cursor_shader_path_changed
                        || cursor_shader_enabled_changed
                        || cursor_shader_animation_changed
                        || cursor_shader_speed_changed
                    {
                        match renderer.set_cursor_shader_enabled(
                            self.config.cursor_shader_enabled,
                            self.config.cursor_shader.as_deref(),
                            self.config.window_opacity,
                            self.config.cursor_shader_animation,
                            self.config.cursor_shader_animation_speed,
                        ) {
                            Ok(()) => {
                                self.settings_ui.clear_cursor_shader_error();
                            }
                            Err(error_msg) => {
                                log::error!("Cursor shader compilation failed: {}", error_msg);
                                self.settings_ui.set_cursor_shader_error(Some(error_msg));
                            }
                        }
                    }

                    // Apply theme changes immediately to the terminal
                    if theme_changed {
                        if let Some(terminal) = &self.terminal
                            && let Ok(mut term) = terminal.try_lock()
                        {
                            term.set_theme(self.config.load_theme());
                            log::info!("Applied live theme change: {}", self.config.theme);
                        }
                        // Force redraw so new theme colors show up right away
                        self.cache.cells = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }

                    if font_changed {
                        // Rebuild renderer on next frame to apply font/spacing changes without restart
                        self.pending_font_rebuild = true;
                        log::info!("Queued renderer rebuild for font change");
                    }

                    // Update window padding dynamically (no rebuild needed)
                    if padding_changed {
                        if let Some((cols, rows)) =
                            renderer.update_window_padding(self.config.window_padding)
                        {
                            // Grid size changed - resize terminal to match
                            let cell_width = renderer.cell_width();
                            let cell_height = renderer.cell_height();
                            let width_px = (cols as f32 * cell_width) as usize;
                            let height_px = (rows as f32 * cell_height) as usize;

                            if let Some(terminal) = &self.terminal
                                && let Ok(mut term) = terminal.try_lock()
                            {
                                let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                                log::info!(
                                    "Resized terminal to {}x{} due to padding change",
                                    cols,
                                    rows
                                );
                            }
                        }
                        log::info!("Updated window padding to {}", self.config.window_padding);
                    }

                    // Invalidate cell cache to force regeneration with new opacity
                    self.cache.cells = None;

                    // Track last applied opacity
                    self.cache.applied_opacity = self.config.window_opacity;

                    // Request multiple redraws to ensure changes are visible
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }

                // Persist to disk if save was clicked
                if let Some(new_config) = config_to_save {
                    if let Err(e) = new_config.save() {
                        log::error!("Failed to save config: {}", e);
                    } else {
                        log::info!("Configuration saved successfully");
                        log::info!(
                            "  Bell settings: sound={}, visual={}, desktop={}",
                            new_config.notification_bell_sound,
                            new_config.notification_bell_visual,
                            new_config.notification_bell_desktop
                        );
                        // Update settings_ui with saved config
                        self.settings_ui.update_config(new_config);
                    }
                }
            }
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
                    self.cache.generation,
                    if self.cache.cells.is_some() {
                        "YES"
                    } else {
                        "NO"
                    }
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

        // Handle clipboard actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match pending_clipboard_action {
            ClipboardHistoryAction::Paste(content) => {
                self.paste_text(&content);
            }
            ClipboardHistoryAction::ClearAll => {
                if let Some(terminal) = &self.terminal
                    && let Ok(term) = terminal.try_lock()
                {
                    term.clear_all_clipboard_history();
                    log::info!("Cleared all clipboard history");
                }
                self.clipboard_history_ui.update_entries(Vec::new());
            }
            ClipboardHistoryAction::ClearSlot(slot) => {
                if let Some(terminal) = &self.terminal
                    && let Ok(term) = terminal.try_lock()
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

        // Abort refresh task first to prevent lock contention
        if let Some(handle) = self.refresh_task.take() {
            handle.abort();
            log::info!("Refresh task aborted");

            // Give abort time to take effect and any pending operations to complete
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Kill the PTY process first (doesn't require terminal lock)
        if let Some(terminal) = &self.terminal {
            // Try to acquire lock briefly to kill the PTY
            let killed = if let Ok(mut term) = terminal.try_lock() {
                if term.is_running() {
                    log::info!("Killing PTY process during shutdown");
                    match term.kill() {
                        Ok(()) => {
                            log::info!("PTY process killed successfully");
                            true
                        }
                        Err(e) => {
                            log::warn!("Failed to kill PTY process: {:?}", e);
                            false
                        }
                    }
                } else {
                    log::info!("PTY process already stopped");
                    true
                }
            } else {
                log::warn!("Could not acquire terminal lock to kill PTY during shutdown");
                false
            };

            // Give the PTY time to clean up after kill signal
            if killed {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        // Now drop terminal - should be safe since PTY is killed and refresh task is aborted
        if let Some(terminal) = self.terminal.take() {
            // Use a shorter timeout since PTY is already killed
            let timeout = std::time::Duration::from_millis(500);
            let start = std::time::Instant::now();

            loop {
                if let Ok(_term) = terminal.try_lock() {
                    log::info!("Terminal lock acquired for cleanup");
                    // Terminal will be dropped when _term goes out of scope
                    break;
                } else if start.elapsed() >= timeout {
                    log::warn!(
                        "Could not acquire terminal lock within timeout during shutdown, forcing cleanup"
                    );
                    // Force drop by breaking - Arc will be dropped anyway
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // Arc will be dropped here regardless
        }

        log::info!("Window shutdown complete");
    }
}
