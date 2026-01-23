use crate::cell_renderer::Cell;
use crate::clipboard_history_ui::{ClipboardHistoryAction, ClipboardHistoryUI};
use crate::config::Config;
use crate::help_ui::HelpUI;
use crate::input::InputHandler;
use crate::renderer::Renderer;
use crate::scroll_state::ScrollState;
use crate::selection::{Selection, SelectionMode};
use crate::settings_ui::{CursorShaderEditorResult, SettingsUI, ShaderEditorResult};
use crate::terminal::{ClipboardSlot, TerminalManager};
use crate::url_detection;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Main application state
pub struct App {
    config: Config,
    runtime: Arc<Runtime>,
}

impl App {
    /// Create a new application
    pub fn new(runtime: Arc<Runtime>) -> Result<Self> {
        let config = Config::load()?;
        Ok(Self { config, runtime })
    }

    /// Run the application
    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        // Use Poll instead of Wait to enable continuous rendering at 60 FPS
        // Combined with PresentMode::Immediate for maximum performance
        event_loop.set_control_flow(ControlFlow::Wait);

        let mut app_state = AppState::new(self.config, self.runtime);

        event_loop.run_app(&mut app_state)?;

        Ok(())
    }
}

/// Application state that handles events
struct AppState {
    config: Config,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: Option<Arc<Mutex<TerminalManager>>>,
    input_handler: InputHandler,
    refresh_task: Option<JoinHandle<()>>,
    runtime: Arc<Runtime>,
    scroll_state: ScrollState,

    selection: Option<Selection>, // Current text selection
    is_selecting: bool,           // Whether currently dragging to select

    mouse_position: (f64, f64),   // Current mouse position in pixels
    cached_scrollback_len: usize, // Last known scrollback length
    mouse_button_pressed: bool, // Whether any mouse button is currently pressed (for motion tracking)
    last_click_time: Option<std::time::Instant>, // Time of last mouse click
    click_count: u32,           // Number of sequential clicks (1 = single, 2 = double, 3 = triple)
    click_position: Option<(usize, usize)>, // Position of last click in cell coordinates
    detected_urls: Vec<url_detection::DetectedUrl>, // URLs detected in visible terminal area
    hovered_url: Option<String>, // URL currently under mouse cursor
    cursor_opacity: f32, // Cursor opacity for smooth fade animation (0.0 = invisible, 1.0 = fully visible)
    last_cursor_blink: Option<std::time::Instant>, // Time of last cursor blink toggle
    last_key_press: Option<std::time::Instant>, // Time of last key press (to reset cursor blink)
    is_fullscreen: bool, // Whether window is currently in fullscreen mode
    audio_bell: Option<crate::audio_bell::AudioBell>, // Audio bell for terminal bell sounds
    last_bell_count: u64, // Last bell event count from terminal
    visual_bell_flash: Option<std::time::Instant>, // When visual bell flash started (None = not flashing)
    egui_ctx: Option<egui::Context>,               // egui context for GUI rendering
    egui_state: Option<egui_winit::State>,         // egui-winit state for event handling
    settings_ui: SettingsUI,                       // Settings UI manager
    help_ui: HelpUI,                               // Help UI manager
    clipboard_history_ui: ClipboardHistoryUI,      // Clipboard history UI manager
    is_recording: bool,                            // Whether terminal session recording is active
    #[allow(dead_code)] // Used in recording feature but clippy doesn't detect it
    recording_start_time: Option<std::time::Instant>, // When recording started
    is_shutting_down: bool,                        // Flag to indicate shutdown is in progress
    cached_cells: Option<Vec<Cell>>,               // Cached cells from last render (dirty tracking)
    last_generation: u64, // Last terminal generation number (for dirty tracking)
    last_scroll_offset: usize, // Last scroll offset (for cache invalidation)
    last_cursor_pos: Option<(usize, usize)>, // Last cursor position (for cache invalidation)
    last_selection: Option<Selection>, // Last selection state (for cache invalidation)
    last_applied_opacity: f32, // Last opacity value sent to renderer

    // Smart redraw tracking (event-driven rendering)
    needs_redraw: bool, // Whether we need to render next frame
    cursor_blink_timer: Option<std::time::Instant>, // When to blink cursor next
    // Debug timing info
    debug_frame_times: Vec<std::time::Duration>, // Last 60 frame times for FPS calculation
    debug_cell_gen_time: std::time::Duration,    // Time spent generating cells last frame
    debug_render_time: std::time::Duration,      // Time spent rendering last frame
    debug_cache_hit: bool,                       // Whether last frame used cached cells
    debug_last_frame_start: Option<std::time::Instant>, // Start time of last frame
    // FPS overlay
    show_fps_overlay: bool,     // Whether to show FPS overlay (toggle with F3)
    fps_value: f64,             // Current FPS value for overlay display
    pending_font_rebuild: bool, // Whether we need to rebuild renderer after font-related changes
    cached_terminal_title: String, // Last known terminal title (for change detection)
}

impl AppState {
    /// Extract a substring based on character columns to avoid UTF-8 slicing panics
    fn extract_columns(line: &str, start_col: usize, end_col: Option<usize>) -> String {
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

    fn new(config: Config, runtime: Arc<Runtime>) -> Self {
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

            selection: None,
            is_selecting: false,

            mouse_position: (0.0, 0.0),
            cached_scrollback_len: 0,
            mouse_button_pressed: false,
            last_click_time: None,
            click_count: 0,
            click_position: None,
            detected_urls: Vec::new(),
            hovered_url: None,
            cursor_opacity: 1.0,
            last_cursor_blink: None,
            last_key_press: None,
            is_fullscreen: false,
            audio_bell: {
                match crate::audio_bell::AudioBell::new() {
                    Ok(bell) => {
                        log::info!("Audio bell initialized successfully");
                        Some(bell)
                    }
                    Err(e) => {
                        log::warn!("Failed to initialize audio bell: {}", e);
                        None
                    }
                }
            },
            last_bell_count: 0,
            visual_bell_flash: None,
            egui_ctx: None,
            egui_state: None,
            settings_ui,
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            is_recording: false,
            recording_start_time: None,
            is_shutting_down: false,
            cached_cells: None,
            last_generation: 0,
            last_scroll_offset: 0,
            last_cursor_pos: None,
            last_selection: None,
            last_applied_opacity: initial_opacity,

            needs_redraw: true,
            cursor_blink_timer: None,
            debug_frame_times: Vec::with_capacity(60),
            debug_cell_gen_time: std::time::Duration::ZERO,
            debug_render_time: std::time::Duration::ZERO,
            debug_cache_hit: false,
            debug_last_frame_start: None,
            show_fps_overlay: false,
            fps_value: 0.0,
            pending_font_rebuild: false,
            cached_terminal_title: String::new(),
        }
    }

    /// Rebuild the renderer after font-related changes and resize the terminal accordingly
    fn rebuild_renderer(&mut self) -> Result<()> {
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
        self.cached_cells = None;
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

    async fn initialize_async(&mut self, window: Window) -> Result<()> {
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

    fn handle_key_event(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        // Check if any UI panel is visible
        let any_ui_visible =
            self.settings_ui.visible || self.help_ui.visible || self.clipboard_history_ui.visible;

        // When UI panels are visible, block ALL keys from going to terminal
        // except for UI control keys (Escape handled by egui, F1/F2/F3 for toggles)
        if any_ui_visible {
            let is_ui_control_key = matches!(
                event.logical_key,
                Key::Named(NamedKey::F1)
                    | Key::Named(NamedKey::F2)
                    | Key::Named(NamedKey::F3)
                    | Key::Named(NamedKey::Escape)
            );

            if !is_ui_control_key {
                log::debug!("Blocking key while UI visible: {:?}", event.logical_key);
                return;
            }
        }

        // Check if egui UI wants keyboard input (e.g., text fields, ComboBoxes)
        if self.is_egui_using_keyboard() {
            log::debug!("Blocking key event: egui wants keyboard input");
            return;
        }

        // Check if shell has exited
        let is_running = if let Some(terminal) = &self.terminal {
            if let Ok(term) = terminal.try_lock() {
                term.is_running()
            } else {
                true
            }
        } else {
            true
        };

        // If shell exited and user presses any key, exit the application
        // (fallback behavior if close_on_shell_exit is false)
        if !is_running && event.state == ElementState::Pressed {
            log::info!("Shell has exited, closing terminal on keypress");
            // Abort the refresh task to prevent lockup on shutdown
            if let Some(task) = self.refresh_task.take() {
                task.abort();
                log::info!("Refresh task aborted");
            }
            event_loop.exit();
            return;
        }

        // Update last key press time for cursor blink reset
        if event.state == ElementState::Pressed {
            self.last_key_press = Some(std::time::Instant::now());
        }

        // Check if this is a scroll navigation key
        if self.handle_scroll_keys(&event) {
            return; // Key was handled for scrolling, don't send to terminal
        }

        // Check if this is a config reload key (F5)
        if self.handle_config_reload(&event) {
            return; // Key was handled for config reload, don't send to terminal
        }

        // Check if this is a clipboard history key (Ctrl+Shift+H)
        if self.handle_clipboard_history_keys(&event) {
            return; // Key was handled for clipboard history, don't send to terminal
        }

        // Check for fullscreen toggle (F11)
        if self.handle_fullscreen_toggle(&event) {
            return; // Key was handled for fullscreen toggle
        }

        // Check for help toggle (F1)
        if self.handle_help_toggle(&event) {
            return; // Key was handled for help toggle
        }

        // Check for settings toggle (F12)
        if self.handle_settings_toggle(&event) {
            return; // Key was handled for settings toggle
        }

        // Check for shader editor toggle (F11)
        if self.handle_shader_editor_toggle(&event) {
            return; // Key was handled for shader editor toggle
        }

        // Check for FPS overlay toggle (F3)
        if self.handle_fps_overlay_toggle(&event) {
            return; // Key was handled for FPS overlay toggle
        }

        // Check for utility shortcuts (clear scrollback, font size, etc.)
        if self.handle_utility_shortcuts(&event, event_loop) {
            return; // Key was handled by utility shortcut
        }

        // Clear selection on keyboard input (except for special keys handled above)
        if event.state == ElementState::Pressed && self.selection.is_some() {
            self.selection = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Debug: Log Tab and Space key before processing
        let is_tab = matches!(event.logical_key, Key::Named(NamedKey::Tab));
        let is_space = matches!(event.logical_key, Key::Named(NamedKey::Space));
        if is_tab {
            log::debug!("Tab key event received, state={:?}", event.state);
        }
        if is_space {
            log::debug!("Space key event received, state={:?}", event.state);
        }

        // Normal key handling - send to terminal
        if let Some(bytes) = self.input_handler.handle_key_event(event)
            && let Some(terminal) = &self.terminal
        {
            if is_tab {
                log::debug!("Sending Tab key to terminal ({} bytes)", bytes.len());
            }
            if is_space {
                log::debug!("Sending Space key to terminal ({} bytes)", bytes.len());
            }
            let terminal_clone = Arc::clone(terminal);

            self.runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let _ = term.write(&bytes);
            });
        }
    }

    fn handle_scroll_keys(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        let shift = self.input_handler.modifiers.state().shift_key();

        let handled = match &event.logical_key {
            Key::Named(NamedKey::PageUp) => {
                // Scroll up one page
                self.scroll_up_page();
                true
            }
            Key::Named(NamedKey::PageDown) => {
                // Scroll down one page
                self.scroll_down_page();
                true
            }
            Key::Named(NamedKey::Home) if shift => {
                // Shift+Home: Scroll to top
                self.scroll_to_top();
                true
            }
            Key::Named(NamedKey::End) if shift => {
                // Shift+End: Scroll to bottom
                self.scroll_to_bottom();
                true
            }
            _ => false,
        };

        if handled && let Some(window) = &self.window {
            window.request_redraw();
        }

        handled
    }

    fn handle_config_reload(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F5 to reload config
        if matches!(event.logical_key, Key::Named(NamedKey::F5)) {
            log::info!("Reloading configuration (F5 pressed)");
            self.reload_config();
            return true;
        }

        false
    }

    fn reload_config(&mut self) {
        match Config::load() {
            Ok(new_config) => {
                log::info!("Configuration reloaded successfully");

                // Apply settings that can be changed at runtime

                // Update auto_copy_selection
                self.config.auto_copy_selection = new_config.auto_copy_selection;

                // Update middle_click_paste
                self.config.middle_click_paste = new_config.middle_click_paste;

                // Update window title
                if self.config.window_title != new_config.window_title {
                    self.config.window_title = new_config.window_title.clone();
                    if let Some(window) = &self.window {
                        window.set_title(&new_config.window_title);
                    }
                }

                // Update theme
                if self.config.theme != new_config.theme {
                    self.config.theme = new_config.theme.clone();
                    if let Some(terminal) = &self.terminal
                        && let Ok(mut term) = terminal.try_lock()
                    {
                        term.set_theme(new_config.load_theme());
                        log::info!("Applied new theme: {}", new_config.theme);
                    }
                }

                // Note: Clipboard history and notification settings not yet available in core library
                // Config reloading for these features will be enabled when APIs become available

                // Note: Terminal dimensions and scrollback size still require restart
                if new_config.font_size != self.config.font_size {
                    log::info!(
                        "Font size changed from {} -> {} (applied live)",
                        self.config.font_size,
                        new_config.font_size
                    );
                }

                if new_config.cols != self.config.cols || new_config.rows != self.config.rows {
                    log::warn!("Terminal dimensions change requires restart");
                }

                // Request redraw to apply theme changes
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Err(e) => {
                log::error!("Failed to reload configuration: {}", e);
            }
        }
    }

    fn handle_clipboard_history_keys(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::Key;

        // Handle Escape to close clipboard history UI
        if self.clipboard_history_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.clipboard_history_ui.visible = false;
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                        self.clipboard_history_ui.select_previous();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                        self.clipboard_history_ui.select_next();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::Enter) => {
                        // Paste the selected entry
                        if let Some(entry) = self.clipboard_history_ui.selected_entry() {
                            let content = entry.content.clone();
                            self.clipboard_history_ui.visible = false;
                            self.paste_text(&content);
                            self.needs_redraw = true;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            // While clipboard history is visible, consume all key events
            return true;
        }

        // Ctrl+Shift+H: Toggle clipboard history UI
        if event.state == ElementState::Pressed {
            let ctrl = self.input_handler.modifiers.state().control_key();
            let shift = self.input_handler.modifiers.state().shift_key();

            if ctrl
                && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "h" || c.as_str() == "H")
            {
                self.toggle_clipboard_history();
                return true;
            }
        }

        false
    }

    fn toggle_clipboard_history(&mut self) {
        // Refresh clipboard history entries from terminal before showing
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            // Get history for all slots and merge
            let mut all_entries = Vec::new();
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Primary));
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Clipboard));
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Selection));

            // Sort by timestamp (newest first)
            all_entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));

            self.clipboard_history_ui.update_entries(all_entries);
        }

        self.clipboard_history_ui.toggle();
        self.needs_redraw = true;
        log::debug!(
            "Clipboard history UI toggled: {}",
            self.clipboard_history_ui.visible
        );
    }

    fn paste_text(&mut self, text: &str) {
        if let Some(terminal) = &self.terminal {
            let terminal_clone = Arc::clone(terminal);
            // Convert newlines to carriage returns for terminal
            let text = text.replace('\n', "\r");
            self.runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let _ = term.write(text.as_bytes());
                log::debug!("Pasted text from clipboard history ({} bytes)", text.len());
            });
        }
    }

    /// Force surface reconfiguration - useful when rendering becomes corrupted
    /// after moving between monitors or when automatic detection fails.
    /// Also clears glyph cache to ensure fonts render correctly.
    fn force_surface_reconfigure(&mut self) {
        log::info!("Force surface reconfigure triggered");

        if let Some(renderer) = &mut self.renderer {
            // Reconfigure the surface
            renderer.reconfigure_surface();

            // Clear glyph cache to force re-rasterization at correct DPI
            renderer.clear_glyph_cache();

            // Invalidate cached cells to force full re-render
            self.cached_cells = None;
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

    fn handle_utility_shortcuts(
        &mut self,
        event: &KeyEvent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::Key;

        if event.state != ElementState::Pressed {
            return false;
        }

        let ctrl = self.input_handler.modifiers.state().control_key();
        let shift = self.input_handler.modifiers.state().shift_key();

        // Ctrl+Shift+K: Clear scrollback
        if ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "k" || c.as_str() == "K")
        {
            // Clear scrollback if terminal is available
            let cleared = if let Some(terminal) = &self.terminal
                && let Ok(term) = terminal.try_lock()
            {
                term.clear_scrollback();
                true
            } else {
                false
            };

            if cleared {
                self.cached_scrollback_len = 0;
                self.set_scroll_target(0);
                log::info!("Cleared scrollback buffer");
            }
            return true;
        }

        // Ctrl+L: Clear screen (send clear sequence to shell)
        if ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "l" || c.as_str() == "L")
        {
            if let Some(terminal) = &self.terminal {
                let terminal_clone = Arc::clone(terminal);
                // Send the "clear" command sequence (Ctrl+L)
                let clear_sequence = vec![0x0C]; // Ctrl+L character
                self.runtime.spawn(async move {
                    if let Ok(term) = terminal_clone.try_lock() {
                        let _ = term.write(&clear_sequence);
                        log::debug!("Sent clear screen sequence (Ctrl+L)");
                    }
                });
            }
            return true;
        }

        // Ctrl+Plus/Equals: Increase font size (applies live)
        if ctrl
            && !shift
            && (matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "+" || c.as_str() == "="))
        {
            self.config.font_size = (self.config.font_size + 1.0).min(72.0);
            self.pending_font_rebuild = true;
            log::info!(
                "Font size increased to {} (applying live)",
                self.config.font_size
            );
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+Minus: Decrease font size (applies live)
        if ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "-" || c.as_str() == "_")
        {
            self.config.font_size = (self.config.font_size - 1.0).max(6.0);
            self.pending_font_rebuild = true;
            log::info!(
                "Font size decreased to {} (applying live)",
                self.config.font_size
            );
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+0: Reset font size to default (applies live)
        if ctrl && !shift && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "0")
        {
            self.config.font_size = 14.0; // Default font size
            self.pending_font_rebuild = true;
            log::info!("Font size reset to default (14.0, applying live)");
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+, (Cmd+, on macOS): Cycle cursor style (Block -> Beam -> Underline -> Block)
        let super_key = self.input_handler.modifiers.state().super_key();
        let ctrl_or_cmd = ctrl || super_key;

        if ctrl_or_cmd
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == ",")
        {
            use crate::config::CursorStyle;
            use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

            // Cycle to next cursor style
            self.config.cursor_style = match self.config.cursor_style {
                CursorStyle::Block => CursorStyle::Beam,
                CursorStyle::Beam => CursorStyle::Underline,
                CursorStyle::Underline => CursorStyle::Block,
            };

            log::info!("Cursor style changed to {:?}", self.config.cursor_style);

            // Apply to terminal
            if let Some(terminal_mgr) = &self.terminal
                && let Ok(term_mgr) = terminal_mgr.try_lock()
            {
                let terminal = term_mgr.terminal();
                if let Some(mut term) = terminal.try_lock() {
                    let term_style = match self.config.cursor_style {
                        CursorStyle::Block => TermCursorStyle::SteadyBlock,
                        CursorStyle::Beam => TermCursorStyle::SteadyBar,
                        CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                    };
                    term.set_cursor_style(term_style);
                }
            }

            // Force redraw to reflect cursor style change
            self.cached_cells = None;
            self.last_cursor_pos = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+Shift+S: Take screenshot
        if ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "s" || c.as_str() == "S")
        {
            self.take_screenshot();
            return true;
        }

        // Ctrl+Shift+R: Toggle session recording
        if ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "r" || c.as_str() == "R")
        {
            self.toggle_recording();
            return true;
        }

        // Ctrl+Shift+F5: Force surface reconfigure (fixes rendering corruption)
        if ctrl && shift && matches!(event.logical_key, Key::Named(winit::keyboard::NamedKey::F5)) {
            log::info!("Manual surface reconfigure triggered via Ctrl+Shift+F5");
            self.force_surface_reconfigure();
            return true;
        }

        false
    }

    fn handle_fullscreen_toggle(&mut self, event: &KeyEvent) -> bool {
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

    fn handle_settings_toggle(&mut self, event: &KeyEvent) -> bool {
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
    fn handle_help_toggle(&mut self, event: &KeyEvent) -> bool {
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
    fn handle_shader_editor_toggle(&mut self, event: &KeyEvent) -> bool {
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
    fn handle_fps_overlay_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F3: Toggle FPS overlay
        if matches!(event.logical_key, Key::Named(NamedKey::F3)) {
            self.show_fps_overlay = !self.show_fps_overlay;
            log::info!(
                "FPS overlay toggled: {}",
                if self.show_fps_overlay {
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

    fn scroll_up_page(&mut self) {
        // Calculate page size based on visible lines
        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = self.scroll_state.target_offset.saturating_add(page_size);
            let clamped_target = new_target.min(self.cached_scrollback_len);
            self.set_scroll_target(clamped_target);
        }
    }

    fn scroll_down_page(&mut self) {
        // Calculate page size based on visible lines
        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = self.scroll_state.target_offset.saturating_sub(page_size);
            self.set_scroll_target(new_target);
        }
    }

    fn scroll_to_top(&mut self) {
        self.set_scroll_target(self.cached_scrollback_len);
    }

    fn scroll_to_bottom(&mut self) {
        self.set_scroll_target(0);
    }

    /// Check if egui is currently using the pointer (mouse is over an egui UI element)
    fn is_egui_using_pointer(&self) -> bool {
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
    fn is_egui_using_keyboard(&self) -> bool {
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

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // --- 1. Mouse Tracking Protocol ---
        // Check if the terminal application (e.g., vim, htop) has requested mouse tracking.
        // If enabled, we forward wheel events to the PTY instead of scrolling locally.
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
            && term.is_mouse_tracking_enabled()
        {
            // Calculate scroll lines based on delta type (Line vs Pixel)
            let scroll_lines = match delta {
                MouseScrollDelta::LineDelta(_x, y) => y as i32,
                MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
            };

            // Map pixel position to terminal cell coordinates
            if let Some((col, row)) =
                self.pixel_to_cell(self.mouse_position.0, self.mouse_position.1)
            {
                // XTerm mouse protocol buttons: 64 = scroll up, 65 = scroll down
                let button = if scroll_lines > 0 { 64 } else { 65 };
                // Limit burst to 10 events to avoid flooding the PTY
                let count = scroll_lines.unsigned_abs().min(10);

                // Encode and send to terminal via async task
                let mut all_encoded = Vec::new();
                for _ in 0..count {
                    let encoded = term.encode_mouse_event(button, col, row, true, 0);
                    if !encoded.is_empty() {
                        all_encoded.extend(encoded);
                    }
                }

                if !all_encoded.is_empty() {
                    let terminal_clone = Arc::clone(terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.lock().await;
                        let _ = t.write(&all_encoded);
                    });
                }
            }
            return; // Exit early: terminal app handled the input
        }

        // --- 2. Local Scrolling ---
        // Normal behavior: scroll through the local scrollback buffer.
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_x, y) => (y * self.config.mouse_scroll_speed) as i32,
            MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
        };

        let scrollback_len = self.cached_scrollback_len;

        // Calculate new scroll target (positive delta = scroll up = increase offset)
        let new_target = self.scroll_state.apply_scroll(scroll_lines, scrollback_len);

        // Update target and trigger interpolation animation
        self.set_scroll_target(new_target);
    }

    /// Set scroll target and initiate smooth interpolation animation.
    /// The actual interpolation happens in `update_scroll_animation` during render.
    fn set_scroll_target(&mut self, new_offset: usize) {
        if self.scroll_state.set_target(new_offset) {
            // Request redraw to start the animation loop

            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    /// Update smooth scroll animation via interpolation.
    /// Returns true if the animation is still in progress.
    fn update_scroll_animation(&mut self) -> bool {
        self.scroll_state.update_animation()
    }

    /// Convert pixel coordinates to terminal cell coordinates
    fn pixel_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(renderer) = &self.renderer {
            // Use actual cell dimensions from renderer for accurate coordinate mapping
            let cell_width = renderer.cell_width() as f64;
            let cell_height = renderer.cell_height() as f64;
            let padding = renderer.window_padding() as f64;

            // Account for window padding (all sides)
            let adjusted_x = (x - padding).max(0.0);
            let adjusted_y = (y - padding).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }

    /// Determine if scrollbar should be visible based on autohide setting and recent activity
    fn should_show_scrollbar(&self) -> bool {
        // No scrollbar needed if no scrollback available
        if self.cached_scrollback_len == 0 {
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
                && (width - self.mouse_position.0) <= padding;
            let near_left =
                self.config.scrollbar_position == "left" && self.mouse_position.0 <= padding;
            if near_left || near_right {
                return true;
            }
        }

        // Otherwise, hide after delay
        self.scroll_state.last_activity.elapsed().as_millis()
            < self.config.scrollbar_autohide_delay as u128
    }

    /// Select word at the given position
    fn select_word_at(&mut self, col: usize, row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            let visible_cells =
                term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);
            if visible_cells.is_empty() || cols == 0 {
                return;
            }

            let cell_idx = row * cols + col;
            if cell_idx >= visible_cells.len() {
                return;
            }

            // Find word boundaries
            let mut start_col = col;
            let mut end_col = col;

            // Expand left
            for c in (0..col).rev() {
                let idx = row * cols + c;
                if idx >= visible_cells.len() {
                    break;
                }
                let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
                if ch.is_alphanumeric() || ch == '_' {
                    start_col = c;
                } else {
                    break;
                }
            }

            // Expand right
            for c in col..cols {
                let idx = row * cols + c;
                if idx >= visible_cells.len() {
                    break;
                }
                let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
                if ch.is_alphanumeric() || ch == '_' {
                    end_col = c;
                } else {
                    break;
                }
            }

            self.selection = Some(Selection::new(
                (start_col, row),
                (end_col, row),
                SelectionMode::Normal,
            ));
        }
    }

    /// Select entire line at the given row (used for triple-click)
    fn select_line_at(&mut self, row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            if cols == 0 {
                return;
            }

            // Store the row in start/end - Line mode uses rows only
            self.selection = Some(Selection::new(
                (0, row),
                (cols.saturating_sub(1), row),
                SelectionMode::Line,
            ));
        }
    }

    /// Extend line selection to include rows from anchor to current row
    fn extend_line_selection(&mut self, current_row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            if cols == 0 {
                return;
            }

            // Use click_position as the anchor row (the originally triple-clicked row)
            let anchor_row = self.click_position.map(|(_, r)| r).unwrap_or(current_row);

            if let Some(ref mut selection) = self.selection
                && selection.mode == SelectionMode::Line
            {
                // For line selection, always ensure full lines are selected
                // by setting columns appropriately based on drag direction
                if current_row >= anchor_row {
                    // Dragging down or same row: start at col 0, end at last col
                    selection.start = (0, anchor_row);
                    selection.end = (cols.saturating_sub(1), current_row);
                } else {
                    // Dragging up: start at last col (anchor row), end at col 0 (current row)
                    // After normalization, this becomes: start=(0, current_row), end=(cols-1, anchor_row)
                    selection.start = (cols.saturating_sub(1), anchor_row);
                    selection.end = (0, current_row);
                }
            }
        }
    }

    /// Extract selected text from terminal
    fn get_selected_text(&self) -> Option<String> {
        if let (Some(selection), Some(terminal)) = (&self.selection, &self.terminal) {
            if let Ok(term) = terminal.try_lock() {
                let (start, end) = selection.normalized();
                let (start_col, start_row) = start;
                let (end_col, end_row) = end;

                let (cols, rows) = term.dimensions();
                let visible_cells =
                    term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);
                if visible_cells.is_empty() || cols == 0 {
                    return None;
                }

                let mut visible_lines = Vec::with_capacity(rows);
                for row in 0..rows {
                    let start_idx = row * cols;
                    let end_idx = start_idx.saturating_add(cols);
                    if end_idx > visible_cells.len() {
                        break;
                    }

                    let mut line = String::with_capacity(cols);
                    for cell in &visible_cells[start_idx..end_idx] {
                        line.push_str(&cell.grapheme);
                    }
                    visible_lines.push(line);
                }

                if visible_lines.is_empty() {
                    return None;
                }

                let mut selected_text = String::new();
                let max_row = visible_lines.len().saturating_sub(1);
                let start_row = start_row.min(max_row);
                let end_row = end_row.min(max_row);

                if selection.mode == SelectionMode::Line {
                    // Line selection: extract full lines
                    #[allow(clippy::needless_range_loop)]
                    for row in start_row..=end_row {
                        if row > start_row {
                            selected_text.push('\n');
                        }
                        let line = &visible_lines[row];
                        // Trim trailing spaces from each line but keep the content
                        selected_text.push_str(line.trim_end());
                    }
                } else if selection.mode == SelectionMode::Rectangular {
                    // Rectangular selection: extract same columns from each row
                    let min_col = start_col.min(end_col);
                    let max_col = start_col.max(end_col);

                    #[allow(clippy::needless_range_loop)]
                    for row in start_row..=end_row {
                        if row > start_row {
                            selected_text.push('\n');
                        }
                        let line = &visible_lines[row];
                        selected_text.push_str(&Self::extract_columns(
                            line,
                            min_col,
                            Some(max_col),
                        ));
                    }
                } else if start_row == end_row {
                    // Normal single-line selection
                    let line = &visible_lines[start_row];
                    selected_text = Self::extract_columns(line, start_col, Some(end_col));
                } else {
                    // Normal multi-line selection
                    for (idx, row) in (start_row..=end_row).enumerate() {
                        let line = &visible_lines[row];
                        if idx == 0 {
                            selected_text.push_str(&Self::extract_columns(line, start_col, None));
                        } else if row == end_row {
                            selected_text.push('\n');
                            selected_text.push_str(&Self::extract_columns(line, 0, Some(end_col)));
                        } else {
                            selected_text.push('\n');
                            selected_text.push_str(line);
                        }
                    }
                }

                Some(selected_text)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Detect URLs in the visible terminal area (both regex-detected and OSC 8 hyperlinks)
    fn detect_urls(&mut self) {
        self.detected_urls.clear();

        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, rows) = term.dimensions();
            let visible_cells =
                term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);

            if visible_cells.is_empty() || cols == 0 {
                return;
            }

            // Build hyperlink ID to URL mapping from terminal
            let mut hyperlink_urls = std::collections::HashMap::new();
            let all_hyperlinks = term.get_all_hyperlinks();
            for hyperlink_info in all_hyperlinks {
                // Get the hyperlink ID from the first position
                if let Some((col, row)) = hyperlink_info.positions.first() {
                    // Get the cell at this position to find the hyperlink_id
                    let cell_idx = row * cols + col;
                    if let Some(cell) = visible_cells.get(cell_idx)
                        && let Some(id) = cell.hyperlink_id
                    {
                        hyperlink_urls.insert(id, hyperlink_info.url.clone());
                    }
                }
            }

            // Extract text from each visible line and detect URLs
            for row in 0..rows {
                let start_idx = row * cols;
                let end_idx = start_idx.saturating_add(cols);
                if end_idx > visible_cells.len() {
                    break;
                }

                let row_cells = &visible_cells[start_idx..end_idx];

                let mut line = String::with_capacity(cols);
                for cell in row_cells {
                    line.push_str(&cell.grapheme);
                }

                // Adjust row to account for scroll offset
                let absolute_row = row + self.scroll_state.offset;

                // Detect regex-based URLs in this line
                let regex_urls = url_detection::detect_urls_in_line(&line, absolute_row);
                self.detected_urls.extend(regex_urls);

                // Detect OSC 8 hyperlinks in this row
                let osc8_urls =
                    url_detection::detect_osc8_hyperlinks(row_cells, absolute_row, &hyperlink_urls);
                self.detected_urls.extend(osc8_urls);
            }
        }
    }

    /// Apply visual styling to cells that are part of detected URLs
    /// Changes the foreground color to indicate clickable URLs
    fn apply_url_underlines(
        &self,
        cells: &mut [crate::cell_renderer::Cell],
        renderer_size: &winit::dpi::PhysicalSize<u32>,
    ) {
        if self.detected_urls.is_empty() {
            return;
        }

        // Calculate grid dimensions from renderer size
        let char_width = self.config.font_size * 0.6;
        let cols = (renderer_size.width as f32 / char_width) as usize;

        // URL color: bright cyan (#4FC3F7) for visibility
        let url_color = [79, 195, 247, 255];

        // Apply color styling to cells that are part of URLs
        for url in &self.detected_urls {
            // Convert absolute row (with scroll offset) to viewport-relative row
            if url.row < self.scroll_state.offset {
                continue; // URL is above the visible area
            }
            let viewport_row = url.row - self.scroll_state.offset;

            // Calculate cell indices for this URL
            for col in url.start_col..url.end_col {
                let cell_idx = viewport_row * cols + col;
                if cell_idx < cells.len() {
                    cells[cell_idx].fg_color = url_color;
                    cells[cell_idx].underline = true; // Set for future underline rendering support
                }
            }
        }
    }

    /// Send mouse event to terminal if mouse tracking is enabled
    ///
    /// Returns true if event was consumed by terminal (mouse tracking enabled or alt screen active),
    /// false otherwise. When on alt screen, we don't want local text selection.
    fn try_send_mouse_event(&self, button: u8, pressed: bool) -> bool {
        if let Some(terminal) = &self.terminal
            && let Some((col, row)) =
                self.pixel_to_cell(self.mouse_position.0, self.mouse_position.1)
            && let Ok(term) = terminal.try_lock()
        {
            // Check if alternate screen is active - don't do local selection on alt screen
            // even if mouse tracking isn't enabled (e.g., some TUI apps don't enable mouse)
            let alt_screen_active = term.is_alt_screen_active();

            // Check if mouse tracking is enabled
            if term.is_mouse_tracking_enabled() {
                // Encode mouse event
                let encoded = term.encode_mouse_event(button, col, row, pressed, 0);

                if !encoded.is_empty() {
                    // Send to PTY using async lock to ensure write completes
                    let terminal_clone = Arc::clone(terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.lock().await;
                        let _ = t.write(&encoded);
                    });
                }
                return true; // Event consumed by mouse tracking
            }

            // On alt screen without mouse tracking - still consume event to prevent selection
            if alt_screen_active {
                return true;
            }
        }
        false // Event not consumed, handle normally
    }

    /// Update cursor blink state based on configured interval
    fn update_cursor_blink(&mut self) {
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

    fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        // Track button press state for motion tracking logic (drag selection, motion reporting)
        self.mouse_button_pressed = state == ElementState::Pressed;

        // --- 1. Shader Interaction ---
        // Update shader mouse state for left button (matches Shadertoy iMouse convention)
        if button == MouseButton::Left
            && let Some(ref mut renderer) = self.renderer
        {
            renderer.set_shader_mouse_button(
                state == ElementState::Pressed,
                self.mouse_position.0 as f32,
                self.mouse_position.1 as f32,
            );
        }

        match button {
            MouseButton::Left => {
                // --- 2. URL Clicking ---
                // Check for Ctrl+Click on URL to open it in default browser
                if state == ElementState::Pressed
                    && self.input_handler.modifiers.state().control_key()
                    && let Some((col, row)) =
                        self.pixel_to_cell(self.mouse_position.0, self.mouse_position.1)
                {
                    // Adjust row for scroll offset
                    let adjusted_row = row + self.scroll_state.offset;

                    if let Some(url) =
                        url_detection::find_url_at_position(&self.detected_urls, col, adjusted_row)
                    {
                        if let Err(e) = url_detection::open_url(&url.url) {
                            log::error!("Failed to open URL: {}", e);
                        }
                        return; // Exit early: URL click handled
                    }
                }

                // --- 3. Mouse Tracking Forwarding ---
                // Forward events to the PTY if terminal application requested tracking
                if self.try_send_mouse_event(0, state == ElementState::Pressed) {
                    return; // Exit early: terminal app handled the input
                }

                if state == ElementState::Pressed {
                    // --- 4. Scrollbar Interaction ---
                    // Check if clicking/dragging the scrollbar track or thumb
                    let mouse_x = self.mouse_position.0 as f32;
                    let mouse_y = self.mouse_position.1 as f32;

                    if let Some(renderer) = &self.renderer
                        && renderer.scrollbar_track_contains_x(mouse_x)
                    {
                        self.scroll_state.dragging = true;
                        self.scroll_state.last_activity = std::time::Instant::now();

                        let thumb_bounds = renderer.scrollbar_thumb_bounds();
                        if renderer.scrollbar_contains_point(mouse_x, mouse_y) {
                            // Clicked on thumb: track offset from thumb top for precise dragging
                            self.scroll_state.drag_offset = thumb_bounds
                                .map(|(thumb_top, thumb_height)| {
                                    (mouse_y - thumb_top).clamp(0.0, thumb_height)
                                })
                                .unwrap_or(0.0);
                        } else {
                            // Clicked on track: center thumb on mouse position
                            self.scroll_state.drag_offset = thumb_bounds
                                .map(|(_, thumb_height)| thumb_height / 2.0)
                                .unwrap_or(0.0);
                        }

                        self.drag_scrollbar_to(mouse_y);
                        return; // Exit early: scrollbar handling takes precedence over selection
                    }

                    // --- 5. Selection Anchoring & Click Counting ---
                    // Handle complex selection modes based on click sequence
                    if let Some((col, row)) =
                        self.pixel_to_cell(self.mouse_position.0, self.mouse_position.1)
                    {
                        let now = std::time::Instant::now();
                        let same_position = self.click_position == Some((col, row));

                        // Thresholds for sequential clicks (double/triple)
                        let threshold_ms = if self.click_count == 1 {
                            self.config.mouse_double_click_threshold
                        } else {
                            self.config.mouse_triple_click_threshold
                        };
                        let click_threshold = std::time::Duration::from_millis(threshold_ms);

                        // Increment click counter if within time/space constraints
                        if same_position
                            && let Some(last_time) = self.last_click_time
                            && now.duration_since(last_time) < click_threshold
                        {
                            self.click_count = (self.click_count + 1).min(3);
                        } else {
                            self.click_count = 1;
                            // Clear previous selection on new single click
                            self.selection = None;
                        }

                        self.last_click_time = Some(now);
                        self.click_position = Some((col, row));

                        // Apply immediate selection based on click count
                        if self.click_count == 2 {
                            // Double-click: Anchor word selection
                            self.select_word_at(col, row);
                            self.is_selecting = false; // Word selection is static until drag starts
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else if self.click_count == 3 {
                            // Triple-click: Anchor full-line selection
                            self.select_line_at(row);
                            self.is_selecting = true; // Triple-click usually implies immediate drag intent
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else {
                            // Single click: Reset state and wait for drag to start Normal/Rectangular selection
                            self.is_selecting = false;
                            self.selection = None;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                } else {
                    // End scrollbar drag
                    if self.scroll_state.dragging {
                        self.scroll_state.dragging = false;
                        self.scroll_state.drag_offset = 0.0;
                        return;
                    }

                    // End selection and optionally copy to clipboard/primary selection
                    self.is_selecting = false;

                    if let Some(mut selected_text) = self.get_selected_text()
                        && !selected_text.is_empty()
                    {
                        // Strip trailing newline if configured (inverted logic: copy_trailing_newline=false means strip)
                        if !self.config.copy_trailing_newline {
                            while selected_text.ends_with('\n') || selected_text.ends_with('\r') {
                                selected_text.pop();
                            }
                        }

                        // Always copy to primary selection (Linux X11 - no-op on other platforms)
                        if let Err(e) = self.input_handler.copy_to_primary_selection(&selected_text)
                        {
                            log::debug!("Failed to copy to primary selection: {}", e);
                        } else {
                            log::debug!(
                                "Copied {} chars to primary selection",
                                selected_text.len()
                            );
                        }

                        // Copy to clipboard if auto_copy is enabled
                        if self.config.auto_copy_selection {
                            if let Err(e) = self.input_handler.copy_to_clipboard(&selected_text) {
                                log::error!("Failed to copy to clipboard: {}", e);
                            } else {
                                log::debug!("Copied {} chars to clipboard", selected_text.len());
                            }
                        }

                        // Add to clipboard history (once, regardless of which clipboard was used)
                        if let Some(terminal) = &self.terminal
                            && let Ok(term) = terminal.try_lock()
                        {
                            term.add_to_clipboard_history(
                                ClipboardSlot::Clipboard,
                                selected_text.clone(),
                                None,
                            );
                        }
                    }
                }
            }
            MouseButton::Middle => {
                // Try to send to terminal if mouse tracking is enabled
                if self.try_send_mouse_event(1, state == ElementState::Pressed) {
                    return; // Event consumed by terminal
                }

                // Handle middle-click paste if configured
                if state == ElementState::Pressed && self.config.middle_click_paste {
                    // Paste from primary selection (Linux X11) or clipboard (fallback)
                    if let Some(bytes) = self.input_handler.paste_from_primary_selection()
                        && let Some(terminal) = &self.terminal
                    {
                        let terminal_clone = Arc::clone(terminal);
                        self.runtime.spawn(async move {
                            let term = terminal_clone.lock().await;
                            let _ = term.write(&bytes);
                        });
                    }
                }
            }
            MouseButton::Right => {
                // Try to send to terminal if mouse tracking is enabled
                let _ = self.try_send_mouse_event(2, state == ElementState::Pressed);
                // Event consumed by terminal (or ignored)
            }
            _ => {}
        }
    }

    fn handle_mouse_move(&mut self, position: (f64, f64)) {
        self.mouse_position = position;

        // --- 1. Shader Uniform Updates ---
        // Update current mouse position for custom shaders (iMouse.xy)
        if let Some(ref mut renderer) = self.renderer {
            renderer.set_shader_mouse_position(position.0 as f32, position.1 as f32);
        }

        // --- 2. URL Hover Detection ---
        // Identify if mouse is over a clickable link and update window UI (cursor/title)
        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1) {
            let adjusted_row = row + self.scroll_state.offset;
            let url_opt =
                url_detection::find_url_at_position(&self.detected_urls, col, adjusted_row);

            if let Some(url) = url_opt {
                // Hovering over a new/different URL
                if self.hovered_url.as_ref() != Some(&url.url) {
                    self.hovered_url = Some(url.url.clone());
                    if let Some(window) = &self.window {
                        // Visual feedback: hand pointer + URL tooltip in title
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                        let tooltip_title = format!("{} - {}", self.config.window_title, url.url);
                        window.set_title(&tooltip_title);
                    }
                }
            } else {
                // Mouse left a URL area: restore default state
                if self.hovered_url.is_some() {
                    self.hovered_url = None;
                    if let Some(window) = &self.window {
                        window.set_cursor(winit::window::CursorIcon::Text);
                        // Restore terminal-controlled title or config default
                        if self.config.allow_title_change && !self.cached_terminal_title.is_empty()
                        {
                            window.set_title(&self.cached_terminal_title);
                        } else {
                            window.set_title(&self.config.window_title);
                        }
                    }
                }
            }
        }

        // --- 3. Mouse Motion Reporting ---
        // Forward motion events to PTY if requested by terminal app (e.g., mouse tracking in vim)
        if let Some(terminal) = &self.terminal
            && let Some((col, row)) = self.pixel_to_cell(position.0, position.1)
            && let Ok(term) = terminal.try_lock()
            && term.should_report_mouse_motion(self.mouse_button_pressed)
        {
            // Encode button+motion (button 32 marker)
            let button = if self.mouse_button_pressed {
                32 // Motion while button pressed
            } else {
                35 // Motion without button pressed
            };

            let encoded = term.encode_mouse_event(button, col, row, true, 0);
            if !encoded.is_empty() {
                let terminal_clone = Arc::clone(terminal);
                let runtime = Arc::clone(&self.runtime);
                runtime.spawn(async move {
                    let t = terminal_clone.lock().await;
                    let _ = t.write(&encoded);
                });
            }
            return; // Exit early: terminal app is handling mouse motion
        }

        // --- 4. Scrollbar Dragging ---
        if self.scroll_state.dragging {
            self.scroll_state.last_activity = std::time::Instant::now();
            self.drag_scrollbar_to(position.1 as f32);
            return; // Exit early: scrollbar dragging takes precedence over selection
        }

        // --- 5. Drag Selection Logic ---
        // Perform local text selection if mouse tracking is NOT active
        let alt_screen_active = self
            .terminal
            .as_ref()
            .and_then(|t| t.try_lock().ok())
            .is_some_and(|term| term.is_alt_screen_active());

        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1)
            && self.mouse_button_pressed
            && !alt_screen_active
        {
            if self.click_count == 1 && !self.is_selecting {
                // Initial drag move: Start selection if we've moved past the click threshold
                if let Some(click_pos) = self.click_position
                    && click_pos != (col, row)
                {
                    self.is_selecting = true;
                    // Alt key triggers Rectangular/Block selection mode
                    let mode = if self.input_handler.modifiers.state().alt_key() {
                        SelectionMode::Rectangular
                    } else {
                        SelectionMode::Normal
                    };
                    self.selection = Some(Selection::new(
                        self.click_position.unwrap(),
                        (col, row),
                        mode,
                    ));
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            } else if self.is_selecting {
                // Dragging in progress: Update selection endpoints
                if let Some(ref selection) = self.selection {
                    if selection.mode == SelectionMode::Line {
                        // Triple-click mode: Selection always covers whole lines
                        self.extend_line_selection(row);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        // Normal/Rectangular mode: update end cell
                        if let Some(ref mut sel) = self.selection {
                            sel.end = (col, row);
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                }
            }
        }
    }

    fn drag_scrollbar_to(&mut self, mouse_y: f32) {
        if let Some(renderer) = &self.renderer {
            let adjusted_y = mouse_y - self.scroll_state.drag_offset;
            if let Some(new_offset) = renderer.scrollbar_mouse_y_to_scroll_offset(adjusted_y)
                && self.scroll_state.offset != new_offset
            {
                // Instant update for dragging (no animation)
                self.scroll_state.offset = new_offset;
                self.scroll_state.target_offset = new_offset;
                self.scroll_state.animated_offset = new_offset as f64;
                self.scroll_state.animation_start = None;

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    fn render(&mut self) {
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
        if let Some(last_start) = self.debug_last_frame_start {
            let frame_time = frame_start.duration_since(last_start);
            self.debug_frame_times.push(frame_time);
            if self.debug_frame_times.len() > 60 {
                self.debug_frame_times.remove(0);
            }
        }
        self.debug_last_frame_start = Some(frame_start);

        // Update scroll animation
        let animation_running = self.update_scroll_animation();

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
            let (selection, rectangular) = if let Some(sel) = self.selection {
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
            let needs_regeneration = self.cached_cells.is_none()
                || current_generation != self.last_generation
                || self.scroll_state.offset != self.last_scroll_offset
                || current_cursor_pos != self.last_cursor_pos // Regenerate if cursor position changed
                || self.selection != self.last_selection; // Regenerate if selection changed (including clearing)

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
                self.cached_cells = Some(fresh_cells.clone());
                self.last_generation = current_generation;
                self.last_scroll_offset = self.scroll_state.offset;
                self.last_cursor_pos = current_cursor_pos;
                self.last_selection = self.selection;

                (fresh_cells, false)
            } else {
                // Use cached cells - clone is still needed because of apply_url_underlines
                // but we track it accurately for debug logging
                (self.cached_cells.as_ref().unwrap().clone(), true)
            };
            self.debug_cache_hit = is_cache_hit;
            self.debug_cell_gen_time = cell_gen_start.elapsed();

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
            (
                self.cached_scrollback_len,
                self.cached_terminal_title.clone(),
            )
        };

        self.cached_scrollback_len = scrollback_len;
        self.scroll_state
            .clamp_to_scrollback(self.cached_scrollback_len);

        // Update window title if terminal has set one via OSC sequences
        // Only if allow_title_change is enabled and we're not showing a URL tooltip
        if self.config.allow_title_change
            && self.hovered_url.is_none()
            && terminal_title != self.cached_terminal_title
            && let Some(window) = &self.window
        {
            self.cached_terminal_title = terminal_title.clone();
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
        let debug_url_detect_time = if !self.debug_cache_hit {
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
            if !self.debug_cache_hit {
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
                self.last_applied_opacity = self.config.window_opacity;
                self.cached_cells = None;
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
            let visual_bell_intensity = if let Some(flash_start) = self.visual_bell_flash {
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
                    self.visual_bell_flash = None;
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
            let show_fps = self.show_fps_overlay;
            let fps_value = self.fps_value;
            let frame_time_ms = if !self.debug_frame_times.is_empty() {
                let avg = self.debug_frame_times.iter().sum::<std::time::Duration>()
                    / self.debug_frame_times.len() as u32;
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
                    let cursor_color_changed =
                        live_config.cursor_color != self.config.cursor_color;
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
                        self.cached_cells = None;
                        self.last_cursor_pos = None;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }

                    // Update cursor color
                    if cursor_color_changed {
                        renderer.update_cursor_color(self.config.cursor_color);
                        // Force cell regen to reflect cursor color change
                        self.cached_cells = None;
                        self.last_cursor_pos = None;
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
                        self.cached_cells = None;
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
                    self.cached_cells = None;

                    // Track last applied opacity
                    self.last_applied_opacity = self.config.window_opacity;

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
            let avg_frame_time = if !self.debug_frame_times.is_empty() {
                self.debug_frame_times.iter().sum::<std::time::Duration>()
                    / self.debug_frame_times.len() as u32
            } else {
                std::time::Duration::ZERO
            };
            let fps = if avg_frame_time.as_secs_f64() > 0.0 {
                1.0 / avg_frame_time.as_secs_f64()
            } else {
                0.0
            };

            // Update FPS value for overlay display
            self.fps_value = fps;

            // Log debug info every 60 frames (about once per second at 60 FPS)
            if self.debug_frame_times.len() >= 60 {
                log::info!(
                    "PERF: FPS={:.1} Frame={:.2}ms CellGen={:.2}ms({}) URLDetect={:.2}ms Anim={:.2}ms Graphics={:.2}ms egui={:.2}ms UpdateCells={:.2}ms ActualRender={:.2}ms Total={:.2}ms Cells={} Gen={} Cache={}",
                    fps,
                    avg_frame_time.as_secs_f64() * 1000.0,
                    self.debug_cell_gen_time.as_secs_f64() * 1000.0,
                    if self.debug_cache_hit { "HIT" } else { "MISS" },
                    debug_url_detect_time.as_secs_f64() * 1000.0,
                    debug_anim_time.as_secs_f64() * 1000.0,
                    debug_graphics_time.as_secs_f64() * 1000.0,
                    debug_egui_time.as_secs_f64() * 1000.0,
                    debug_update_cells_time.as_secs_f64() * 1000.0,
                    debug_actual_render_time.as_secs_f64() * 1000.0,
                    self.debug_render_time.as_secs_f64() * 1000.0,
                    cells.len(),
                    self.last_generation,
                    if self.cached_cells.is_some() {
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

            self.debug_render_time = render_start.elapsed();
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

    fn check_notifications(&mut self) {
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

    fn check_bell(&mut self) {
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

            if current_bell_count > self.last_bell_count {
                // Bell event(s) occurred
                let bell_events = current_bell_count - self.last_bell_count;
                log::info!("🔔 Bell event detected ({} bell(s))", bell_events);
                log::info!(
                    "  Config: sound={}, visual={}, desktop={}",
                    self.config.notification_bell_sound,
                    self.config.notification_bell_visual,
                    self.config.notification_bell_desktop
                );

                // Play audio bell if enabled (volume > 0)
                if self.config.notification_bell_sound > 0 {
                    if let Some(audio_bell) = &self.audio_bell {
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
                    self.visual_bell_flash = Some(std::time::Instant::now());
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

                self.last_bell_count = current_bell_count;
            }
        }
    }

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
    fn toggle_recording(&mut self) {
        log::warn!("Recording functionality not yet available in core library");
        self.deliver_notification(
            "Recording Not Available",
            "Recording APIs are not yet implemented in the core library",
        );
    }

    /*
    fn toggle_recording(&mut self) {
        if self.is_recording {
            // Stop recording and save
            self.stop_and_save_recording();
        } else {
            // Start recording
            self.start_recording();
        }
    }

    fn start_recording(&mut self) {
        log::info!("Starting terminal session recording");

        if let Some(terminal) = &self.terminal {
            if let Ok(term) = terminal.try_lock() {
                // Start recording (no title for now)
                term.start_recording(None);

                self.is_recording = true;
                self.recording_start_time = Some(std::time::Instant::now());
                log::info!("Recording started successfully");
                self.deliver_notification(
                    "Recording Started",
                    "Terminal session recording started",
                );

                // Update window title to show recording status
                if let Some(window) = &self.window {
                    let title = format!("{} [RECORDING]", self.config.window_title);
                    window.set_title(&title);
                }
            } else {
                log::error!("Failed to lock terminal");
                self.deliver_notification("Recording Error", "Terminal is busy");
            }
        } else {
            log::warn!("No terminal available for recording");
            self.deliver_notification("Recording Error", "No terminal available");
        }
    }

    fn stop_and_save_recording(&mut self) {
        log::info!("Stopping terminal session recording");

        if let Some(terminal) = &self.terminal {
            if let Ok(term) = terminal.try_lock() {
                // Stop recording and get the session
                let session_opt = term.stop_recording();

                if let Some(session) = session_opt {
                    // Generate timestamp-based filename
                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let filename = format!("par-term_recording_{}.cast", timestamp);

                    // Create recordings directory in user's home dir
                    if let Some(home_dir) = dirs::home_dir() {
                        let recording_dir = home_dir.join("par-term-recordings");
                        if let Err(e) = std::fs::create_dir_all(&recording_dir) {
                            log::error!("Failed to create recording directory: {}", e);
                            self.deliver_notification(
                                "Recording Error",
                                &format!("Failed to create directory: {}", e),
                            );
                            self.is_recording = false;
                            self.recording_start_time = None;
                            return;
                        }

                        let path = recording_dir.join(&filename);
                        let path_str = path.to_string_lossy().to_string();

                        // Export to asciicast format and write to file
                        match term.export_recording_to_file(&session, &path, "asciicast") {
                            Ok(()) => {
                                self.is_recording = false;
                                let duration = self
                                    .recording_start_time
                                    .map(|start| start.elapsed().as_secs())
                                    .unwrap_or(0);
                                self.recording_start_time = None;

                                log::info!(
                                    "Recording saved to: {} ({} seconds)",
                                    path_str,
                                    duration
                                );
                                self.deliver_notification(
                                    "Recording Saved",
                                    &format!(
                                        "Saved to: {}\nDuration: {} seconds",
                                        path_str, duration
                                    ),
                                );

                                // Restore window title
                                if let Some(window) = &self.window {
                                    window.set_title(&self.config.window_title);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to save recording: {}", e);
                                self.deliver_notification(
                                    "Recording Error",
                                    &format!("Failed to save: {}", e),
                                );
                                self.is_recording = false;
                                self.recording_start_time = None;
                            }
                        }
                    } else {
                        log::error!("Failed to get home directory");
                        self.deliver_notification(
                            "Recording Error",
                            "Failed to get home directory",
                        );
                        self.is_recording = false;
                        self.recording_start_time = None;
                    }
                } else {
                    log::warn!("No recording session available (recording was not active)");
                    self.deliver_notification("Recording Error", "No active recording to save");
                    self.is_recording = false;
                    self.recording_start_time = None;

                    // Restore window title
                    if let Some(window) = &self.window {
                        window.set_title(&self.config.window_title);
                    }
                }
            } else {
                log::error!("Failed to lock terminal");
                self.deliver_notification("Recording Error", "Terminal is busy");
                self.is_recording = false;
                self.recording_start_time = None;
            }
        } else {
            log::warn!("No terminal available");
            self.deliver_notification("Recording Error", "No terminal available");
            self.is_recording = false;
            self.recording_start_time = None;
        }
    }
    */

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
    fn update_window_title_with_shell_integration(&self) {
        // Skip if scrolled (scrollback indicator takes priority)
        if self.scroll_state.offset != 0 {
            return;
        }

        // Skip if hovering over URL (URL tooltip takes priority)
        if self.hovered_url.is_some() {
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
            let icon_bytes = include_bytes!("../assets/icon.png");
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
                        self.cached_scrollback_len = term.scrollback_len();

                        // Update scrollbar internal state
                        let total_lines = rows + self.cached_scrollback_len;
                        renderer.update_scrollbar(self.scroll_state.offset, rows, total_lines);
                    }

                    // Invalidate cell cache to force regeneration
                    self.cached_cells = None;
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
        if self.visual_bell_flash.is_some() {
            self.needs_redraw = true;
            let next_frame = now + std::time::Duration::from_millis(16);
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 4. Interactive UI Elements
        // Ensure high responsiveness during mouse dragging (text selection or scrollbar).
        if (self.is_selecting || self.selection.is_some() || self.scroll_state.dragging)
            && self.mouse_button_pressed
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

impl Drop for AppState {
    fn drop(&mut self) {
        log::info!("Shutting down application");

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

        log::info!("Application shutdown complete");
    }
}
