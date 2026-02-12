//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.

use crate::app::anti_idle::should_send_keep_alive;
use crate::app::debug_state::DebugState;
use crate::badge::{BadgeState, render_badge};
use crate::cell_renderer::PaneViewport;
use crate::clipboard_history_ui::{ClipboardHistoryAction, ClipboardHistoryUI};
use crate::close_confirmation_ui::{CloseConfirmAction, CloseConfirmationUI};
use crate::command_history::CommandHistory;
use crate::command_history_ui::{CommandHistoryAction, CommandHistoryUI};
use crate::config::{
    Config, CursorShaderMetadataCache, CursorStyle, ShaderInstallPrompt, ShaderMetadataCache,
};
use crate::help_ui::HelpUI;
use crate::input::InputHandler;
use crate::integrations_ui::{IntegrationsResponse, IntegrationsUI};
use crate::keybindings::KeybindingRegistry;
use crate::paste_special_ui::{PasteSpecialAction, PasteSpecialUI};
use crate::profile::{ProfileManager, storage as profile_storage};
use crate::profile_drawer_ui::{ProfileDrawerAction, ProfileDrawerUI};
use crate::profile_modal_ui::{ProfileModalAction, ProfileModalUI};
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};
use crate::quit_confirmation_ui::{QuitConfirmAction, QuitConfirmationUI};
use crate::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
};
use crate::scrollback_metadata::ScrollbackMark;
use crate::search::SearchUI;
use crate::selection::SelectionMode;
use crate::shader_install_ui::{ShaderInstallResponse, ShaderInstallUI};
use crate::shader_watcher::{ShaderReloadEvent, ShaderType, ShaderWatcher};
use crate::smart_selection::SmartSelectionCache;
use crate::tab::{TabId, TabManager};
use crate::tab_bar_ui::{TabBarAction, TabBarUI};
use crate::tmux::{TmuxSession, TmuxSync};
use crate::tmux_session_picker_ui::{SessionPickerAction, TmuxSessionPickerUI};
use crate::tmux_status_bar_ui::TmuxStatusBarUI;
use anyhow::Result;
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use std::sync::Arc;
use tokio::runtime::Runtime;
use wgpu::SurfaceError;
use winit::dpi::PhysicalSize;
use winit::window::Window;

/// Renderer sizing info needed for split pane calculations
struct RendererSizing {
    size: PhysicalSize<u32>,
    content_offset_y: f32,
    cell_width: f32,
    cell_height: f32,
    padding: f32,
    status_bar_height: f32,
    scale_factor: f32,
}

/// Pane render data tuple for split pane rendering
type PaneRenderData = (
    PaneViewport,
    Vec<crate::cell_renderer::Cell>,
    (usize, usize),
    Option<(usize, usize)>,
    f32,
    Vec<ScrollbackMark>,
    usize, // scrollback_len
    usize, // scroll_offset
);

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
    /// tmux status bar UI
    pub(crate) tmux_status_bar_ui: TmuxStatusBarUI,

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
    /// Cache for parsed shader metadata (used for config resolution)
    pub(crate) shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor shader metadata (used for config resolution)
    pub(crate) cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Help UI manager
    pub(crate) help_ui: HelpUI,
    /// Clipboard history UI manager
    pub(crate) clipboard_history_ui: ClipboardHistoryUI,
    /// Command history UI manager (fuzzy search)
    pub(crate) command_history_ui: CommandHistoryUI,
    /// Persistent command history
    pub(crate) command_history: CommandHistory,
    /// Commands already synced from marks to persistent history (avoids repeated adds)
    synced_commands: std::collections::HashSet<String>,
    /// Paste special UI manager (text transformations)
    pub(crate) paste_special_ui: PasteSpecialUI,
    /// tmux session picker UI
    pub(crate) tmux_session_picker_ui: TmuxSessionPickerUI,
    /// Search UI manager
    pub(crate) search_ui: SearchUI,
    /// Shader install prompt UI
    pub(crate) shader_install_ui: ShaderInstallUI,
    /// Receiver for shader installation results (from background thread)
    pub(crate) shader_install_receiver: Option<std::sync::mpsc::Receiver<Result<usize, String>>>,
    /// Combined integrations welcome dialog UI
    pub(crate) integrations_ui: IntegrationsUI,
    /// Close confirmation dialog UI (for tabs with running jobs)
    pub(crate) close_confirmation_ui: CloseConfirmationUI,
    /// Quit confirmation dialog UI (prompt before closing window)
    pub(crate) quit_confirmation_ui: QuitConfirmationUI,
    /// Whether terminal session recording is active
    pub(crate) is_recording: bool,
    /// When recording started
    #[allow(dead_code)]
    pub(crate) recording_start_time: Option<std::time::Instant>,
    /// Flag to indicate shutdown is in progress
    pub(crate) is_shutting_down: bool,
    /// Window index (1-based) for display in title bar
    pub(crate) window_index: usize,

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

    // Flicker reduction state
    /// When cursor was last hidden (for reduce_flicker feature)
    pub(crate) cursor_hidden_since: Option<std::time::Instant>,
    /// Whether we have pending terminal updates deferred due to cursor being hidden
    pub(crate) flicker_pending_render: bool,

    // Throughput mode state
    /// When throughput mode batching started (for render interval timing)
    pub(crate) throughput_batch_start: Option<std::time::Instant>,

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

    /// Pending arrangement restore request (name of arrangement to restore)
    pub(crate) pending_arrangement_restore: Option<String>,

    // Profile management
    /// Profile manager for storing and managing terminal profiles
    pub(crate) profile_manager: ProfileManager,
    /// Profile drawer UI (collapsible side panel)
    pub(crate) profile_drawer_ui: ProfileDrawerUI,
    /// Profile modal UI (management dialog)
    pub(crate) profile_modal_ui: ProfileModalUI,
    /// Flag to indicate profiles menu needs to be updated in the main menu
    pub(crate) profiles_menu_needs_update: bool,
    /// Track if we blocked a mouse press for UI - also block the corresponding release
    pub(crate) ui_consumed_mouse_press: bool,

    // Resize overlay state
    /// Whether the resize overlay is currently visible
    pub(crate) resize_overlay_visible: bool,
    /// When to hide the resize overlay (after resize stops)
    pub(crate) resize_overlay_hide_time: Option<std::time::Instant>,
    /// Current resize dimensions: (width_px, height_px, cols, rows)
    pub(crate) resize_dimensions: Option<(u32, u32, usize, usize)>,

    // Toast notification state
    /// Current toast message to display
    pub(crate) toast_message: Option<String>,
    /// When to hide the toast notification
    pub(crate) toast_hide_time: Option<std::time::Instant>,

    /// Keybinding registry for user-defined keyboard shortcuts
    pub(crate) keybinding_registry: KeybindingRegistry,

    /// Cache for compiled smart selection regex patterns
    pub(crate) smart_selection_cache: SmartSelectionCache,

    // tmux integration state
    /// tmux control mode session (if connected)
    pub(crate) tmux_session: Option<TmuxSession>,
    /// tmux state synchronization manager
    pub(crate) tmux_sync: TmuxSync,
    /// Current tmux session name (for window title display)
    pub(crate) tmux_session_name: Option<String>,
    /// Tab ID where the tmux gateway connection lives (where we write commands)
    pub(crate) tmux_gateway_tab_id: Option<TabId>,
    /// Parsed prefix key from config (cached for performance)
    pub(crate) tmux_prefix_key: Option<crate::tmux::PrefixKey>,
    /// Prefix key state (whether we're waiting for command key)
    pub(crate) tmux_prefix_state: crate::tmux::PrefixState,
    /// Mapping from tmux pane IDs to native pane IDs for output routing
    pub(crate) tmux_pane_to_native_pane:
        std::collections::HashMap<crate::tmux::TmuxPaneId, crate::pane::PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs for input routing
    pub(crate) native_pane_to_tmux_pane:
        std::collections::HashMap<crate::pane::PaneId, crate::tmux::TmuxPaneId>,

    // Broadcast input mode
    /// Whether keyboard input is broadcast to all panes in current tab
    pub(crate) broadcast_input: bool,

    // Badge overlay
    /// Badge state for session information display
    pub(crate) badge_state: BadgeState,

    // Copy mode (vi-style keyboard text selection)
    /// Copy mode state machine
    pub(crate) copy_mode: crate::copy_mode::CopyModeState,
}

impl WindowState {
    /// Create a new window state with the given configuration
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Self {
        let keybinding_registry = KeybindingRegistry::from_config(&config.keybindings);
        let shaders_dir = Config::shaders_dir();
        let tmux_prefix_key = crate::tmux::PrefixKey::parse(&config.tmux_prefix_key);

        let mut input_handler = InputHandler::new();
        // Initialize Option/Alt key modes from config
        input_handler
            .update_option_key_modes(config.left_option_key_mode, config.right_option_key_mode);

        // Load profiles from disk
        let profile_manager = match profile_storage::load_profiles() {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to load profiles: {}", e);
                ProfileManager::new()
            }
        };

        // Create badge state before moving config
        let badge_state = BadgeState::new(&config);
        let command_history_max = config.command_history_max_entries;

        Self {
            config,
            window: None,
            renderer: None,
            input_handler,
            runtime,

            tab_manager: TabManager::new(),
            tab_bar_ui: TabBarUI::new(),
            tmux_status_bar_ui: TmuxStatusBarUI::new(),

            debug: DebugState::new(),

            cursor_opacity: 1.0,
            last_cursor_blink: None,
            last_key_press: None,
            is_fullscreen: false,
            egui_ctx: None,
            egui_state: None,
            egui_initialized: false,
            shader_metadata_cache: ShaderMetadataCache::with_shaders_dir(shaders_dir.clone()),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(shaders_dir),
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            command_history_ui: CommandHistoryUI::new(),
            command_history: {
                let mut ch = CommandHistory::new(command_history_max);
                ch.load();
                ch
            },
            synced_commands: std::collections::HashSet::new(),
            paste_special_ui: PasteSpecialUI::new(),
            tmux_session_picker_ui: TmuxSessionPickerUI::new(),
            search_ui: SearchUI::new(),
            shader_install_ui: ShaderInstallUI::new(),
            shader_install_receiver: None,
            integrations_ui: IntegrationsUI::new(),
            close_confirmation_ui: CloseConfirmationUI::new(),
            quit_confirmation_ui: QuitConfirmationUI::new(),
            is_recording: false,
            recording_start_time: None,
            is_shutting_down: false,
            window_index: 1, // Will be set by WindowManager when window is created

            needs_redraw: true,
            cursor_blink_timer: None,
            pending_font_rebuild: false,

            is_focused: true, // Assume focused on creation
            last_render_time: None,

            cursor_hidden_since: None,
            flicker_pending_render: false,

            throughput_batch_start: None,

            shader_watcher: None,
            shader_reload_error: None,
            background_shader_reload_result: None,
            cursor_shader_reload_result: None,

            open_settings_window_requested: false,
            pending_arrangement_restore: None,

            profile_manager,
            profile_drawer_ui: ProfileDrawerUI::new(),
            profile_modal_ui: ProfileModalUI::new(),
            profiles_menu_needs_update: true, // Update menu on startup
            ui_consumed_mouse_press: false,

            resize_overlay_visible: false,
            resize_overlay_hide_time: None,
            resize_dimensions: None,

            toast_message: None,
            toast_hide_time: None,

            keybinding_registry,

            smart_selection_cache: SmartSelectionCache::new(),

            tmux_session: None,
            tmux_sync: TmuxSync::new(),
            tmux_session_name: None,
            tmux_gateway_tab_id: None,
            tmux_prefix_key,
            tmux_prefix_state: crate::tmux::PrefixState::new(),
            tmux_pane_to_native_pane: std::collections::HashMap::new(),
            native_pane_to_tmux_pane: std::collections::HashMap::new(),

            broadcast_input: false,

            badge_state,

            copy_mode: crate::copy_mode::CopyModeState::new(),
        }
    }

    /// Format window title with optional window number
    /// This should be used everywhere a title is set to ensure consistency
    pub(crate) fn format_title(&self, base_title: &str) -> String {
        if self.config.show_window_number {
            format!("{} [{}]", base_title, self.window_index)
        } else {
            base_title.to_string()
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
            crate::debug_trace!("REDRAW", "request_redraw called");
            window.request_redraw();
        } else {
            crate::debug_trace!("REDRAW", "request_redraw called but no window");
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
            .and_then(|name| self.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.cursor_shader_metadata_cache.get(name).cloned());
        let params = RendererInitParams::from_config(
            &self.config,
            &theme,
            metadata.as_ref(),
            cursor_metadata.as_ref(),
        );

        // Drop the old renderer BEFORE creating a new one.
        // wgpu only allows one surface per window, so the old surface must be
        // released before we can create a new one.
        self.renderer = None;

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
            .and_then(|name| self.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.cursor_shader_metadata_cache.get(name).cloned());
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

        // Set tab bar height BEFORE creating the first tab
        // This ensures the terminal is sized correctly from the start
        // Use 1 as tab count since we're about to create the first tab
        let initial_tab_bar_height = self.tab_bar_ui.get_height(1, &self.config);
        let (initial_cols, initial_rows) = renderer.grid_size();
        log::info!(
            "Tab bar init: mode={:?}, height={:.1}, initial_grid={}x{}, content_offset_before={:.1}",
            self.config.tab_bar_mode,
            initial_tab_bar_height,
            initial_cols,
            initial_rows,
            renderer.content_offset_y()
        );
        if initial_tab_bar_height > 0.0 {
            if let Some((new_cols, new_rows)) =
                renderer.set_content_offset_y(initial_tab_bar_height)
            {
                log::info!(
                    "Tab bar height {:.0}px applied, grid resized: {}x{} -> {}x{}",
                    initial_tab_bar_height,
                    initial_cols,
                    initial_rows,
                    new_cols,
                    new_rows
                );
            } else {
                log::info!(
                    "Tab bar height {:.0}px applied, grid unchanged: {}x{}, content_offset_after={:.1}",
                    initial_tab_bar_height,
                    initial_cols,
                    initial_rows,
                    renderer.content_offset_y()
                );
            }
        }

        // Get the renderer's grid size BEFORE storing it (and before creating tabs)
        // This ensures the shell starts with correct dimensions that account for tab bar
        let (renderer_cols, renderer_rows) = renderer.grid_size();
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();

        self.window = Some(Arc::clone(&window));
        self.renderer = Some(renderer);

        // Initialize shader watcher if hot reload is enabled
        self.init_shader_watcher();

        // Create the first tab with the correct grid size from the renderer
        // This ensures the shell is spawned with dimensions that account for tab bar
        log::info!(
            "Creating first tab with grid size {}x{} (accounting for tab bar)",
            renderer_cols,
            renderer_rows
        );
        let tab_id = self.tab_manager.new_tab(
            &self.config,
            Arc::clone(&self.runtime),
            false,                                // First tab doesn't inherit cwd
            Some((renderer_cols, renderer_rows)), // Pass correct grid size
        )?;

        // Set cell dimensions on the terminal (for TIOCGWINSZ pixel size reporting)
        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
            let width_px = (renderer_cols as f32 * cell_width) as usize;
            let height_px = (renderer_rows as f32 * cell_height) as usize;

            if let Ok(mut term) = tab.terminal.try_lock() {
                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                // Send resize to ensure PTY has correct pixel dimensions
                let _ = term.resize_with_pixels(renderer_cols, renderer_rows, width_px, height_px);
                log::info!(
                    "Initial terminal dimensions: {}x{} ({}x{} px)",
                    renderer_cols,
                    renderer_rows,
                    width_px,
                    height_px
                );
            }

            // Start refresh task for the first tab
            tab.start_refresh_task(
                Arc::clone(&self.runtime),
                Arc::clone(&window),
                self.config.max_fps,
            );
        }

        // Check if we should prompt user to install integrations (shaders and/or shell integration)
        if self.config.should_prompt_integrations() {
            log::info!("Integrations not installed - showing welcome dialog");
            self.integrations_ui.show_dialog();
            self.needs_redraw = true;
            window.request_redraw();
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

    /// Check anti-idle timers and send keep-alive codes when due.
    ///
    /// Returns the next Instant when anti-idle should run, or None if disabled.
    pub(crate) fn handle_anti_idle(
        &mut self,
        now: std::time::Instant,
    ) -> Option<std::time::Instant> {
        if !self.config.anti_idle_enabled {
            return None;
        }

        let idle_threshold = std::time::Duration::from_secs(self.config.anti_idle_seconds.max(1));
        let keep_alive_code = [self.config.anti_idle_code];
        let mut next_due: Option<std::time::Instant> = None;

        for tab in self.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_lock() {
                // Treat new terminal output as activity
                let current_generation = term.update_generation();
                if current_generation > tab.anti_idle_last_generation {
                    tab.anti_idle_last_generation = current_generation;
                    tab.anti_idle_last_activity = now;
                }

                // If idle long enough, send keep-alive code
                if should_send_keep_alive(tab.anti_idle_last_activity, now, idle_threshold) {
                    if let Err(e) = term.write(&keep_alive_code) {
                        log::warn!(
                            "Failed to send anti-idle keep-alive for tab {}: {}",
                            tab.id,
                            e
                        );
                    } else {
                        tab.anti_idle_last_activity = now;
                    }
                }

                // Compute next due time for this tab
                let elapsed = now.duration_since(tab.anti_idle_last_activity);
                let remaining = if elapsed >= idle_threshold {
                    idle_threshold
                } else {
                    idle_threshold - elapsed
                };
                let candidate = now + remaining;
                next_due = Some(next_due.map_or(candidate, |prev| prev.min(candidate)));
            }
        }

        next_due
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
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
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
                // Track success for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.background_shader_reload_result = Some(None);
                    }
                    ShaderType::Cursor => {
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
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
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
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        let any_ui_visible = self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.command_history_ui.visible
            || self.shader_install_ui.visible
            || self.integrations_ui.visible
            || self.profile_modal_ui.visible;
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

        // FPS throttling to enforce max_fps (focused) or unfocused_fps (unfocused)
        // This ensures rendering is capped even if VSync runs at a higher rate
        // or multiple sources are requesting redraws (refresh task, shader animations, etc.)
        let target_fps = if self.config.pause_refresh_on_blur && !self.is_focused {
            self.config.unfocused_fps
        } else {
            self.config.max_fps
        };
        let frame_interval_ms = 1000 / target_fps.max(1);
        let frame_interval = std::time::Duration::from_millis(frame_interval_ms as u64);

        if let Some(last_render) = self.last_render_time {
            let elapsed = last_render.elapsed();
            if elapsed < frame_interval {
                // Not enough time has passed, skip this render
                return;
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
            // Compare in physical pixels (content_offset_y is physical, tab_bar_height is logical)
            let expected_offset = tab_bar_height * renderer.scale_factor();
            if (current_offset - expected_offset).abs() > 0.1 {
                crate::debug_info!(
                    "TAB_SYNC",
                    "Content offset changing: {:.0} -> {:.0} (logical {:.0} * scale {:.1})",
                    current_offset,
                    expected_offset,
                    tab_bar_height,
                    renderer.scale_factor()
                );
            }
            if let Some((new_cols, new_rows)) = renderer.set_content_offset_y(tab_bar_height) {
                // Grid size changed - resize all tab terminals
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                // Calculate pixel dimensions from grid size (not window size)
                // This ensures TIOCGWINSZ reports the correct terminal content dimensions
                let width_px = (new_cols as f32 * cell_width) as usize;
                let height_px = (new_rows as f32 * cell_height) as usize;

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

        let (renderer_size, visible_lines, grid_cols) = if let Some(renderer) = &self.renderer {
            let (cols, rows) = renderer.grid_size();
            (renderer.size(), rows, cols)
        } else {
            return;
        };

        // Get active tab's terminal and immediate state snapshots (avoid long borrows)
        let (
            terminal,
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            cached_scrollback_len,
            cached_terminal_title,
            hovered_url,
        ) = match self.tab_manager.active_tab() {
            Some(t) => (
                t.terminal.clone(),
                t.scroll_state.offset,
                t.mouse.selection,
                t.cache.cells.clone(),
                t.cache.generation,
                t.cache.scroll_offset,
                t.cache.cursor_pos,
                t.cache.selection,
                t.cache.scrollback_len,
                t.cache.terminal_title.clone(),
                t.mouse.hovered_url.clone(),
            ),
            None => return,
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

        // Get scroll offset and selection from active tab

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
            // In copy mode, use the copy mode cursor position instead
            let cursor_visible = self.config.lock_cursor_visibility || term.is_cursor_visible();
            let current_cursor_pos = if self.copy_mode.active {
                self.copy_mode.screen_cursor_pos(scroll_offset)
            } else if scroll_offset == 0 && cursor_visible {
                Some(term.cursor_position())
            } else {
                None
            };

            let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_opacity));

            // Get cursor style for geometric rendering
            // In copy mode, always use SteadyBlock for clear visibility
            // If lock_cursor_style is enabled, use the config's cursor style instead of terminal's
            // If lock_cursor_blink is enabled and cursor_blink is false, force steady cursor
            let cursor_style = if self.copy_mode.active && current_cursor_pos.is_some() {
                Some(TermCursorStyle::SteadyBlock)
            } else if current_cursor_pos.is_some() {
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
            let needs_regeneration = cache_cells.is_none()
                || current_generation != cache_generation
                || scroll_offset != cache_scroll_offset
                || current_cursor_pos != cache_cursor_pos // Regenerate if cursor position changed
                || mouse_selection != cache_selection; // Regenerate if selection changed (including clearing)

            let cell_gen_start = std::time::Instant::now();
            let (cells, is_cache_hit) = if needs_regeneration {
                // Generate fresh cells
                let fresh_cells =
                    term.get_cells_with_scrollback(scroll_offset, selection, rectangular, cursor);

                (fresh_cells, false)
            } else {
                // Use cached cells - clone is still needed because of apply_url_underlines
                // but we track it accurately for debug logging
                (cache_cells.as_ref().unwrap().clone(), true)
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
                    .and_then(|name| self.cursor_shader_metadata_cache.get(name))
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
                    .and_then(|name| self.cursor_shader_metadata_cache.get(name))
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

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title) = if let Ok(mut term) = terminal.try_lock() {
            // Use cursor row 0 when cursor not visible (e.g., alt screen)
            let cursor_row = current_cursor_pos.map(|(_, row)| row).unwrap_or(0);
            let sb_len = term.scrollback_len();
            term.update_scrollback_metadata(sb_len, cursor_row);

            // Feed newly completed commands into persistent history from two sources:
            // 1. Scrollback marks (populated via set_mark_command_at from grid text extraction)
            // 2. Core library command history (populated by the terminal emulator core)
            // Both sources are checked because command text may come from either path
            // depending on shell integration quality. The synced_commands set prevents
            // duplicate adds across frames and sources.
            for mark in term.scrollback_marks() {
                if let Some(ref cmd) = mark.command
                    && !cmd.is_empty()
                    && self.synced_commands.insert(cmd.clone())
                {
                    self.command_history
                        .add(cmd.clone(), mark.exit_code, mark.duration_ms);
                }
            }
            for (cmd, exit_code, duration_ms) in term.core_command_history() {
                if !cmd.is_empty() && self.synced_commands.insert(cmd.clone()) {
                    self.command_history.add(cmd, exit_code, duration_ms);
                }
            }

            (sb_len, term.get_title())
        } else {
            (cached_scrollback_len, cached_terminal_title.clone())
        };

        // Update cache scrollback and clamp scroll state
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.scrollback_len = scrollback_len;
            tab.scroll_state
                .clamp_to_scrollback(tab.cache.scrollback_len);
        }

        // Keep copy mode dimensions in sync with terminal
        if self.copy_mode.active
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, rows) = term.dimensions();
            self.copy_mode.update_dimensions(cols, rows, scrollback_len);
        }

        let need_marks =
            self.config.scrollbar_command_marks || self.config.command_separator_enabled;
        let mut scrollback_marks = if need_marks {
            if let Ok(term) = terminal.try_lock() {
                term.scrollback_marks()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Append trigger-generated marks
        if let Some(tab) = self.tab_manager.active_tab() {
            scrollback_marks.extend(tab.trigger_marks.iter().cloned());
        }

        // Keep scrollbar visible when mark indicators exist (even if no scrollback).
        if !scrollback_marks.is_empty() {
            show_scrollbar = true;
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
                    window.set_title(&self.format_title(&self.config.window_title));
                } else {
                    // Use terminal-set title with window number
                    window.set_title(&self.format_title(&terminal_title));
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

        // Update search and apply search highlighting
        if self.search_ui.visible {
            // Get all searchable lines from cells (ensures consistent wide character handling)
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_lock()
            {
                let lines_iter =
                    crate::app::search_highlight::get_all_searchable_lines(&term, visible_lines);
                self.search_ui.update_search(lines_iter);
            }

            // Apply search highlighting to visible cells
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            // Use actual terminal grid columns from renderer
            self.apply_search_highlights(
                &mut cells,
                grid_cols,
                scroll_offset,
                scrollback_len,
                visible_lines,
            );
        }

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
        // Command history action to handle after rendering
        let mut pending_command_history_action = CommandHistoryAction::None;
        // Paste special action to handle after rendering
        let mut pending_paste_special_action = PasteSpecialAction::None;
        // tmux session picker action to handle after rendering
        let mut pending_session_picker_action = SessionPickerAction::None;
        // Tab bar action to handle after rendering (declared here to survive renderer borrow)
        let mut pending_tab_action = TabBarAction::None;
        // Shader install response to handle after rendering
        let mut pending_shader_install_response = ShaderInstallResponse::None;
        // Integrations welcome dialog response to handle after rendering
        let mut pending_integrations_response = IntegrationsResponse::default();
        // Search action to handle after rendering
        let mut pending_search_action = crate::search::SearchAction::None;
        // Profile drawer action to handle after rendering
        let mut pending_profile_drawer_action = ProfileDrawerAction::None;
        // Profile modal action to handle after rendering
        let mut pending_profile_modal_action = ProfileModalAction::None;
        // Close confirmation action to handle after rendering
        let mut pending_close_confirm_action = CloseConfirmAction::None;
        // Quit confirmation action to handle after rendering
        let mut pending_quit_confirm_action = QuitConfirmAction::None;

        // Check tmux gateway state before renderer borrow to avoid borrow conflicts
        // When tmux controls the layout, we don't use pane padding
        // Note: pane_padding is in logical pixels (config); we defer DPI scaling to
        // where it's used with physical pixel coordinates (via sizing.scale_factor).
        let is_tmux_gateway = self.is_gateway_active();
        let effective_pane_padding = if is_tmux_gateway {
            0.0
        } else {
            self.config.pane_padding
        };

        // Calculate status bar height before mutable renderer borrow
        // Note: This is in logical pixels; it gets scaled to physical in RendererSizing.
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);

        // Capture window size before mutable borrow (for badge rendering in egui)
        let window_size_for_badge = self.renderer.as_ref().map(|r| r.size());

        // Capture progress bar snapshot before mutable borrow
        let progress_snapshot = if self.config.progress_bar_enabled {
            self.tab_manager.active_tab().and_then(|tab| {
                tab.terminal
                    .try_lock()
                    .ok()
                    .map(|term| ProgressBarSnapshot {
                        simple: term.progress_bar(),
                        named: term.named_progress_bars(),
                    })
            })
        } else {
            None
        };

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

            // Update progress bar state for shader uniforms
            if let Some(ref snap) = progress_snapshot {
                use par_term_emu_core_rust::terminal::ProgressState;
                let state_val = match snap.simple.state {
                    ProgressState::Hidden => 0.0,
                    ProgressState::Normal => 1.0,
                    ProgressState::Error => 2.0,
                    ProgressState::Indeterminate => 3.0,
                    ProgressState::Warning => 4.0,
                };
                let active_count = (if snap.simple.is_active() { 1 } else { 0 })
                    + snap.named.values().filter(|b| b.state.is_active()).count();
                renderer.update_shader_progress(
                    state_val,
                    snap.simple.progress as f32 / 100.0,
                    if snap.has_active() { 1.0 } else { 0.0 },
                    active_count as f32,
                );
            } else {
                renderer.update_shader_progress(0.0, 0.0, 0.0, 0.0);
            }

            // Update scrollbar
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            renderer.update_scrollbar(scroll_offset, visible_lines, total_lines, &scrollback_marks);

            // Compute and set command separator marks for single-pane rendering
            if self.config.command_separator_enabled {
                let separator_marks = crate::renderer::compute_visible_separator_marks(
                    &scrollback_marks,
                    scrollback_len,
                    scroll_offset,
                    visible_lines,
                );
                renderer.set_separator_marks(separator_marks);
            } else {
                renderer.set_separator_marks(Vec::new());
            }

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

            // Capture badge state for closure (uses window_size_for_badge captured earlier)
            let badge_enabled = self.badge_state.enabled;
            let badge_state = if badge_enabled {
                // Update variables if dirty
                if self.badge_state.is_dirty() {
                    self.badge_state.interpolate();
                }
                Some(self.badge_state.clone())
            } else {
                None
            };

            // Capture hovered scrollbar mark for tooltip display
            let hovered_mark: Option<crate::scrollback_metadata::ScrollbackMark> =
                if self.config.scrollbar_mark_tooltips && self.config.scrollbar_command_marks {
                    self.tab_manager
                        .active_tab()
                        .map(|tab| tab.mouse.position)
                        .and_then(|(mx, my)| {
                            renderer.scrollbar_mark_at_position(mx as f32, my as f32, 8.0)
                        })
                        .cloned()
                } else {
                    None
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

                    // Show copy mode status bar overlay (when enabled in config)
                    if self.copy_mode.active && self.config.copy_mode_show_status {
                        let status = self.copy_mode.status_text();
                        let (mode_text, color) = if self.copy_mode.is_searching {
                            ("SEARCH", egui::Color32::from_rgb(255, 165, 0))
                        } else {
                            match self.copy_mode.visual_mode {
                                crate::copy_mode::VisualMode::None => {
                                    ("COPY", egui::Color32::from_rgb(100, 200, 100))
                                }
                                crate::copy_mode::VisualMode::Char => {
                                    ("VISUAL", egui::Color32::from_rgb(100, 150, 255))
                                }
                                crate::copy_mode::VisualMode::Line => {
                                    ("V-LINE", egui::Color32::from_rgb(100, 150, 255))
                                }
                                crate::copy_mode::VisualMode::Block => {
                                    ("V-BLOCK", egui::Color32::from_rgb(100, 150, 255))
                                }
                            }
                        };

                        egui::Area::new(egui::Id::new("copy_mode_status_bar"))
                            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, 0.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                let available_width = ui.available_width();
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 230))
                                    .inner_margin(egui::Margin::symmetric(12, 6))
                                    .show(ui, |ui| {
                                        ui.set_min_width(available_width);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(mode_text)
                                                    .monospace()
                                                    .size(13.0)
                                                    .color(color)
                                                    .strong(),
                                            );
                                            ui.separator();
                                            ui.label(
                                                egui::RichText::new(&status)
                                                    .monospace()
                                                    .size(12.0)
                                                    .color(egui::Color32::from_rgb(200, 200, 200)),
                                            );
                                        });
                                    });
                            });
                    }

                    // Show toast notification if active (top center)
                    if let Some(ref message) = self.toast_message {
                        egui::Area::new(egui::Id::new("toast_notification"))
                            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                                    .inner_margin(egui::Margin::symmetric(20, 12))
                                    .corner_radius(8.0)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(80, 80, 80),
                                    ))
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(255, 255, 255));
                                        ui.label(egui::RichText::new(message).size(16.0));
                                    });
                            });
                    }

                    // Show scrollbar mark tooltip if hovering over a mark
                    if let Some(ref mark) = hovered_mark {
                        // Format the tooltip content
                        let mut lines = Vec::new();

                        if let Some(ref cmd) = mark.command {
                            let truncated = if cmd.len() > 50 {
                                format!("{}...", &cmd[..47])
                            } else {
                                cmd.clone()
                            };
                            lines.push(format!("Command: {}", truncated));
                        }

                        if let Some(start_time) = mark.start_time {
                            use chrono::{DateTime, Local, Utc};
                            let dt =
                                DateTime::<Utc>::from_timestamp_millis(start_time as i64).unwrap();
                            let local: DateTime<Local> = dt.into();
                            lines.push(format!("Time: {}", local.format("%H:%M:%S")));
                        }

                        if let Some(duration_ms) = mark.duration_ms {
                            if duration_ms < 1000 {
                                lines.push(format!("Duration: {}ms", duration_ms));
                            } else if duration_ms < 60000 {
                                lines
                                    .push(format!("Duration: {:.1}s", duration_ms as f64 / 1000.0));
                            } else {
                                let mins = duration_ms / 60000;
                                let secs = (duration_ms % 60000) / 1000;
                                lines.push(format!("Duration: {}m {}s", mins, secs));
                            }
                        }

                        if let Some(exit_code) = mark.exit_code {
                            lines.push(format!("Exit: {}", exit_code));
                        }

                        let tooltip_text = lines.join("\n");

                        // Calculate tooltip position, clamped to stay on screen
                        let mouse_pos = ctx.pointer_hover_pos().unwrap_or(egui::pos2(100.0, 100.0));
                        let tooltip_x = (mouse_pos.x - 180.0).max(10.0);
                        let tooltip_y = (mouse_pos.y - 20.0).max(10.0);

                        // Show tooltip near mouse position (offset to the left of scrollbar)
                        egui::Area::new(egui::Id::new("scrollbar_mark_tooltip"))
                            .order(egui::Order::Tooltip)
                            .fixed_pos(egui::pos2(tooltip_x, tooltip_y))
                            .show(ctx, |ui| {
                                ui.set_min_width(150.0);
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                                    .inner_margin(egui::Margin::same(8))
                                    .corner_radius(4.0)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(80, 80, 80),
                                    ))
                                    .show(ui, |ui| {
                                        ui.set_min_width(140.0);
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(220, 220, 220));
                                        ui.label(
                                            egui::RichText::new(&tooltip_text)
                                                .monospace()
                                                .size(12.0),
                                        );
                                    });
                            });
                    }

                    // Render tab bar if visible (action handled after closure)
                    pending_tab_action =
                        self.tab_bar_ui.render(ctx, &self.tab_manager, &self.config);

                    // Render tmux status bar if connected
                    self.tmux_status_bar_ui.render(
                        ctx,
                        &self.config,
                        self.tmux_session.as_ref(),
                        self.tmux_session_name.as_deref(),
                    );

                    // Settings are now handled by standalone SettingsWindow only
                    // No overlay settings UI rendering needed

                    // Show help UI
                    self.help_ui.show(ctx);

                    // Show clipboard history UI and collect action
                    pending_clipboard_action = self.clipboard_history_ui.show(ctx);

                    // Show command history UI and collect action
                    pending_command_history_action = self.command_history_ui.show(ctx);

                    // Show paste special UI and collect action
                    pending_paste_special_action = self.paste_special_ui.show(ctx);

                    // Show search UI and collect action
                    pending_search_action = self.search_ui.show(ctx, visible_lines, scrollback_len);

                    // Show tmux session picker UI and collect action
                    let tmux_path = self.config.resolve_tmux_path();
                    pending_session_picker_action =
                        self.tmux_session_picker_ui.show(ctx, &tmux_path);

                    // Show shader install dialog if visible
                    pending_shader_install_response = self.shader_install_ui.show(ctx);

                    // Show integrations welcome dialog if visible
                    pending_integrations_response = self.integrations_ui.show(ctx);

                    // Show close confirmation dialog if visible
                    pending_close_confirm_action = self.close_confirmation_ui.show(ctx);

                    // Show quit confirmation dialog if visible
                    pending_quit_confirm_action = self.quit_confirmation_ui.show(ctx);

                    // Render profile drawer (right side panel)
                    pending_profile_drawer_action = self.profile_drawer_ui.render(
                        ctx,
                        &self.profile_manager,
                        &self.config,
                        self.profile_modal_ui.visible,
                    );

                    // Render profile modal (management dialog)
                    pending_profile_modal_action = self.profile_modal_ui.show(ctx);

                    // Render progress bar overlay
                    if let (Some(snap), Some(size)) = (&progress_snapshot, window_size_for_badge) {
                        render_progress_bars(
                            ctx,
                            snap,
                            &self.config,
                            size.width as f32,
                            size.height as f32,
                        );
                    }

                    // Render badge overlay (top-right corner)
                    if let (Some(badge), Some(size)) = (&badge_state, window_size_for_badge) {
                        render_badge(ctx, badge, size.width as f32, size.height as f32);
                    }
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
            // Settings are handled by standalone SettingsWindow, not embedded UI

            // Extract renderer sizing info for split pane calculations
            let sizing = RendererSizing {
                size: renderer.size(),
                content_offset_y: renderer.content_offset_y(),
                cell_width: renderer.cell_width(),
                cell_height: renderer.cell_height(),
                padding: renderer.window_padding(),
                status_bar_height: status_bar_height * renderer.scale_factor(),
                scale_factor: renderer.scale_factor(),
            };

            // Check if we have a pane manager with panes - this just checks without modifying
            // We use pane_count() > 0 instead of has_multiple_panes() because even with a
            // single pane in the manager (e.g., after closing one tmux split), we need to
            // render via the pane manager path since cells are in the pane's terminal,
            // not the main renderer buffer.
            let (has_pane_manager, pane_count) = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager.as_ref())
                .map(|pm| (pm.pane_count() > 0, pm.pane_count()))
                .unwrap_or((false, 0));

            crate::debug_trace!(
                "RENDER",
                "has_pane_manager={}, pane_count={}",
                has_pane_manager,
                pane_count
            );

            let render_result = if has_pane_manager {
                // Render panes from pane manager - inline data gathering to avoid borrow conflicts
                let content_width = sizing.size.width as f32 - sizing.padding * 2.0;
                let content_height = sizing.size.height as f32
                    - sizing.content_offset_y
                    - sizing.padding
                    - sizing.status_bar_height;

                // Gather all necessary data upfront while we can borrow tab_manager
                #[allow(clippy::type_complexity)]
                let pane_render_data: Option<(
                    Vec<PaneRenderData>,
                    Vec<crate::pane::DividerRect>,
                    Vec<PaneTitleInfo>,
                    Option<PaneViewport>,
                )> = {
                    let tab = self.tab_manager.active_tab_mut();
                    if let Some(tab) = tab {
                        if let Some(pm) = &mut tab.pane_manager {
                            // Update bounds
                            let bounds = crate::pane::PaneBounds::new(
                                sizing.padding,
                                sizing.content_offset_y,
                                content_width,
                                content_height,
                            );
                            pm.set_bounds(bounds);

                            // Calculate title bar height offset for terminal sizing
                            // Scale from logical pixels (config) to physical pixels
                            let title_height_offset = if self.config.show_pane_titles {
                                self.config.pane_title_height * sizing.scale_factor
                            } else {
                                0.0
                            };

                            // Resize all pane terminals to match their new bounds
                            // Scale pane_padding from logical to physical pixels
                            pm.resize_all_terminals_with_padding(
                                sizing.cell_width,
                                sizing.cell_height,
                                effective_pane_padding * sizing.scale_factor,
                                title_height_offset,
                            );

                            // Gather pane info
                            let focused_pane_id = pm.focused_pane_id();
                            let all_pane_ids: Vec<_> =
                                pm.all_panes().iter().map(|p| p.id).collect();
                            let dividers = pm.get_dividers();

                            let pane_bg_opacity = self.config.pane_background_opacity;
                            let inactive_opacity = if self.config.dim_inactive_panes {
                                self.config.inactive_pane_opacity
                            } else {
                                1.0
                            };
                            let cursor_opacity = self.cursor_opacity;

                            // Pane title settings
                            // Scale from logical pixels (config) to physical pixels
                            let show_titles = self.config.show_pane_titles;
                            let title_height = self.config.pane_title_height * sizing.scale_factor;
                            let title_position = self.config.pane_title_position;
                            let title_text_color = [
                                self.config.pane_title_color[0] as f32 / 255.0,
                                self.config.pane_title_color[1] as f32 / 255.0,
                                self.config.pane_title_color[2] as f32 / 255.0,
                            ];
                            let title_bg_color = [
                                self.config.pane_title_bg_color[0] as f32 / 255.0,
                                self.config.pane_title_bg_color[1] as f32 / 255.0,
                                self.config.pane_title_bg_color[2] as f32 / 255.0,
                            ];

                            let mut pane_data = Vec::new();
                            let mut pane_titles = Vec::new();
                            let mut focused_viewport: Option<PaneViewport> = None;

                            for pane_id in &all_pane_ids {
                                if let Some(pane) = pm.get_pane(*pane_id) {
                                    let is_focused = Some(*pane_id) == focused_pane_id;
                                    let bounds = pane.bounds;

                                    // Calculate viewport, adjusting for title bar if shown
                                    let (viewport_y, viewport_height) = if show_titles {
                                        use crate::config::PaneTitlePosition;
                                        match title_position {
                                            PaneTitlePosition::Top => (
                                                bounds.y + title_height,
                                                (bounds.height - title_height).max(0.0),
                                            ),
                                            PaneTitlePosition::Bottom => {
                                                (bounds.y, (bounds.height - title_height).max(0.0))
                                            }
                                        }
                                    } else {
                                        (bounds.y, bounds.height)
                                    };

                                    // Create viewport with padding for content inset
                                    // Scale pane_padding from logical to physical pixels
                                    let physical_pane_padding =
                                        effective_pane_padding * sizing.scale_factor;
                                    let viewport = PaneViewport::with_padding(
                                        bounds.x,
                                        viewport_y,
                                        bounds.width,
                                        viewport_height,
                                        is_focused,
                                        if is_focused {
                                            pane_bg_opacity
                                        } else {
                                            pane_bg_opacity * inactive_opacity
                                        },
                                        physical_pane_padding,
                                    );

                                    if is_focused {
                                        focused_viewport = Some(viewport);
                                    }

                                    // Build pane title info
                                    if show_titles {
                                        use crate::config::PaneTitlePosition;
                                        let title_y = match title_position {
                                            PaneTitlePosition::Top => bounds.y,
                                            PaneTitlePosition::Bottom => {
                                                bounds.y + bounds.height - title_height
                                            }
                                        };
                                        pane_titles.push(PaneTitleInfo {
                                            x: bounds.x,
                                            y: title_y,
                                            width: bounds.width,
                                            height: title_height,
                                            title: pane.get_title(),
                                            focused: is_focused,
                                            text_color: title_text_color,
                                            bg_color: title_bg_color,
                                        });
                                    }

                                    let cells = if let Ok(term) = pane.terminal.try_lock() {
                                        let scroll_offset = pane.scroll_state.offset;
                                        let selection =
                                            pane.mouse.selection.map(|sel| sel.normalized());
                                        let rectangular = pane
                                            .mouse
                                            .selection
                                            .map(|sel| sel.mode == SelectionMode::Rectangular)
                                            .unwrap_or(false);
                                        term.get_cells_with_scrollback(
                                            scroll_offset,
                                            selection,
                                            rectangular,
                                            None,
                                        )
                                    } else {
                                        Vec::new()
                                    };

                                    let need_marks = self.config.scrollbar_command_marks
                                        || self.config.command_separator_enabled;
                                    let (marks, pane_scrollback_len) = if need_marks {
                                        if let Ok(mut term) = pane.terminal.try_lock() {
                                            // Use cursor row 0 when unknown in split panes
                                            let sb_len = term.scrollback_len();
                                            term.update_scrollback_metadata(sb_len, 0);
                                            (term.scrollback_marks(), sb_len)
                                        } else {
                                            (Vec::new(), 0)
                                        }
                                    } else {
                                        (Vec::new(), 0)
                                    };
                                    let pane_scroll_offset = pane.scroll_state.offset;

                                    let cursor_pos = if let Ok(term) = pane.terminal.try_lock() {
                                        if term.is_cursor_visible() {
                                            Some(term.cursor_position())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                    // Grid size must match the terminal's actual size
                                    // (accounting for padding and title bar, same as resize_all_terminals_with_padding)
                                    let content_width = (bounds.width
                                        - physical_pane_padding * 2.0)
                                        .max(sizing.cell_width);
                                    let content_height = (viewport_height
                                        - physical_pane_padding * 2.0)
                                        .max(sizing.cell_height);
                                    let cols = (content_width / sizing.cell_width).floor() as usize;
                                    let rows =
                                        (content_height / sizing.cell_height).floor() as usize;
                                    let cols = cols.max(1);
                                    let rows = rows.max(1);

                                    pane_data.push((
                                        viewport,
                                        cells,
                                        (cols, rows),
                                        cursor_pos,
                                        if is_focused { cursor_opacity } else { 0.0 },
                                        marks,
                                        pane_scrollback_len,
                                        pane_scroll_offset,
                                    ));
                                }
                            }

                            Some((pane_data, dividers, pane_titles, focused_viewport))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                if let Some((pane_data, dividers, pane_titles, focused_viewport)) = pane_render_data
                {
                    // Get hovered divider index for hover color rendering
                    let hovered_divider_index = self
                        .tab_manager
                        .active_tab()
                        .and_then(|t| t.mouse.hovered_divider_index);

                    // Render split panes
                    Self::render_split_panes_with_data(
                        renderer,
                        pane_data,
                        dividers,
                        pane_titles,
                        focused_viewport,
                        &self.config,
                        egui_data,
                        hovered_divider_index,
                    )
                } else {
                    // Fallback to single pane render
                    renderer.render(egui_data, false, show_scrollbar)
                }
            } else {
                // Single pane - use standard render path
                renderer.render(egui_data, false, show_scrollbar)
            };

            match render_result {
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
            TabBarAction::Reorder(id, target_index) => {
                if self.tab_manager.move_tab_to_index(id, target_index) {
                    self.needs_redraw = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            TabBarAction::None => {}
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

        // Handle command history actions collected during egui rendering
        match pending_command_history_action {
            CommandHistoryAction::Insert(command) => {
                self.paste_text(&command);
                log::info!(
                    "Inserted command from history: {}",
                    &command[..command.len().min(60)]
                );
            }
            CommandHistoryAction::None => {}
        }

        // Handle close confirmation dialog actions
        match pending_close_confirm_action {
            CloseConfirmAction::Close { tab_id, pane_id } => {
                // User confirmed close - close the tab/pane
                if let Some(pane_id) = pane_id {
                    // Close specific pane
                    if let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                        && let Some(pm) = tab.pane_manager_mut()
                    {
                        pm.close_pane(pane_id);
                        log::info!("Force-closed pane {} in tab {}", pane_id, tab_id);
                    }
                } else {
                    // Close entire tab
                    self.tab_manager.close_tab(tab_id);
                    log::info!("Force-closed tab {}", tab_id);
                }
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            CloseConfirmAction::Cancel => {
                // User cancelled - do nothing, dialog already hidden
                log::debug!("Close confirmation cancelled");
            }
            CloseConfirmAction::None => {}
        }

        // Handle quit confirmation dialog actions
        match pending_quit_confirm_action {
            QuitConfirmAction::Quit => {
                // User confirmed quit - proceed with shutdown
                log::info!("Quit confirmed by user");
                self.perform_shutdown();
            }
            QuitConfirmAction::Cancel => {
                log::debug!("Quit confirmation cancelled");
            }
            QuitConfirmAction::None => {}
        }

        // Handle paste special actions collected during egui rendering
        match pending_paste_special_action {
            PasteSpecialAction::Paste(content) => {
                self.paste_text(&content);
                log::debug!("Pasted transformed text ({} chars)", content.len());
            }
            PasteSpecialAction::None => {}
        }

        // Handle search actions collected during egui rendering
        match pending_search_action {
            crate::search::SearchAction::ScrollToMatch(offset) => {
                self.set_scroll_target(offset);
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            crate::search::SearchAction::Close => {
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            crate::search::SearchAction::None => {}
        }

        // Handle tmux session picker actions collected during egui rendering
        // Uses gateway mode: writes tmux commands to existing PTY instead of spawning process
        match pending_session_picker_action {
            SessionPickerAction::Attach(session_name) => {
                crate::debug_info!(
                    "TMUX",
                    "Session picker: attaching to '{}' via gateway",
                    session_name
                );
                if let Err(e) = self.attach_tmux_gateway(&session_name) {
                    log::error!("Failed to attach to tmux session '{}': {}", session_name, e);
                    self.show_toast(format!("Failed to attach: {}", e));
                } else {
                    crate::debug_info!("TMUX", "Gateway initiated for session '{}'", session_name);
                    self.show_toast(format!("Connecting to session '{}'...", session_name));
                }
                self.needs_redraw = true;
            }
            SessionPickerAction::CreateNew(name) => {
                crate::debug_info!(
                    "TMUX",
                    "Session picker: creating new session {:?} via gateway",
                    name
                );
                if let Err(e) = self.initiate_tmux_gateway(name.as_deref()) {
                    log::error!("Failed to create tmux session: {}", e);
                    crate::debug_error!("TMUX", "Failed to initiate gateway: {}", e);
                    self.show_toast(format!("Failed to create session: {}", e));
                } else {
                    let msg = match name {
                        Some(ref n) => format!("Creating session '{}'...", n),
                        None => "Creating new tmux session...".to_string(),
                    };
                    crate::debug_info!("TMUX", "Gateway initiated: {}", msg);
                    self.show_toast(msg);
                }
                self.needs_redraw = true;
            }
            SessionPickerAction::None => {}
        }

        // Check for shader installation completion from background thread
        if let Some(ref rx) = self.shader_install_receiver
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(count) => {
                    log::info!("Successfully installed {} shaders", count);
                    self.shader_install_ui
                        .set_success(&format!("Installed {} shaders!", count));

                    // Update config to mark as installed
                    self.config.shader_install_prompt = ShaderInstallPrompt::Installed;
                    if let Err(e) = self.config.save() {
                        log::error!("Failed to save config after shader install: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Failed to install shaders: {}", e);
                    self.shader_install_ui.set_error(&e);
                }
            }
            self.shader_install_receiver = None;
            self.needs_redraw = true;
        }

        // Handle shader install responses
        match pending_shader_install_response {
            ShaderInstallResponse::Install => {
                log::info!("User requested shader installation");
                self.shader_install_ui
                    .set_installing("Downloading shaders...");
                self.needs_redraw = true;

                // Spawn installation in background thread so UI can show progress
                let (tx, rx) = std::sync::mpsc::channel();
                self.shader_install_receiver = Some(rx);

                std::thread::spawn(move || {
                    let result = crate::shader_install_ui::install_shaders_headless();
                    let _ = tx.send(result);
                });

                // Request redraw so the spinner shows
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            ShaderInstallResponse::Never => {
                log::info!("User declined shader installation (never ask again)");
                self.shader_install_ui.hide();

                // Update config to never ask again
                self.config.shader_install_prompt = ShaderInstallPrompt::Never;
                if let Err(e) = self.config.save() {
                    log::error!("Failed to save config after declining shaders: {}", e);
                }
            }
            ShaderInstallResponse::Later => {
                log::info!("User deferred shader installation");
                self.shader_install_ui.hide();
                // Config remains "ask" - will prompt again on next startup
            }
            ShaderInstallResponse::None => {}
        }

        // Handle integrations welcome dialog responses
        self.handle_integrations_response(&pending_integrations_response);

        // Handle profile drawer actions
        match pending_profile_drawer_action {
            ProfileDrawerAction::OpenProfile(id) => {
                self.open_profile(id);
            }
            ProfileDrawerAction::ManageProfiles => {
                self.profile_modal_ui.open(&self.profile_manager);
            }
            ProfileDrawerAction::None => {}
        }

        // Handle profile modal actions
        match pending_profile_modal_action {
            ProfileModalAction::Save => {
                // Apply working profiles to manager and save to disk
                // Note: get_working_profiles() must be called before close()
                let profiles = self.profile_modal_ui.get_working_profiles().to_vec();
                self.profile_modal_ui.close();
                self.apply_profile_changes(profiles);
            }
            ProfileModalAction::OpenProfile(id) => {
                self.open_profile(id);
            }
            ProfileModalAction::Cancel | ProfileModalAction::None => {}
        }

        let absolute_total = absolute_start.elapsed();
        if absolute_total.as_millis() > 10 {
            log::debug!(
                "TIMING: AbsoluteTotal={:.2}ms (from function start to end)",
                absolute_total.as_secs_f64() * 1000.0
            );
        }
    }

    /// Render split panes when the active tab has multiple panes
    #[allow(clippy::too_many_arguments)]
    fn render_split_panes_with_data(
        renderer: &mut Renderer,
        pane_data: Vec<PaneRenderData>,
        dividers: Vec<crate::pane::DividerRect>,
        pane_titles: Vec<PaneTitleInfo>,
        focused_viewport: Option<PaneViewport>,
        config: &Config,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        hovered_divider_index: Option<usize>,
    ) -> Result<bool> {
        // Build pane render infos - we need to leak the cells temporarily
        let mut pane_render_infos: Vec<PaneRenderInfo> = Vec::new();
        let mut leaked_cells: Vec<*mut [crate::cell_renderer::Cell]> = Vec::new();

        for (
            viewport,
            cells,
            grid_size,
            cursor_pos,
            cursor_opacity,
            marks,
            scrollback_len,
            scroll_offset,
        ) in pane_data
        {
            let cells_boxed = cells.into_boxed_slice();
            let cells_ptr = Box::into_raw(cells_boxed);
            leaked_cells.push(cells_ptr);

            pane_render_infos.push(PaneRenderInfo {
                viewport,
                // SAFETY: We just allocated this, and we'll free it after rendering
                cells: unsafe { &*cells_ptr },
                grid_size,
                cursor_pos,
                cursor_opacity,
                show_scrollbar: false,
                marks,
                scrollback_len,
                scroll_offset,
            });
        }

        // Build divider render info
        let divider_render_infos: Vec<DividerRenderInfo> = dividers
            .iter()
            .enumerate()
            .map(|(i, d)| DividerRenderInfo::from_rect(d, hovered_divider_index == Some(i)))
            .collect();

        // Build divider settings from config
        let divider_settings = PaneDividerSettings {
            divider_color: [
                config.pane_divider_color[0] as f32 / 255.0,
                config.pane_divider_color[1] as f32 / 255.0,
                config.pane_divider_color[2] as f32 / 255.0,
            ],
            hover_color: [
                config.pane_divider_hover_color[0] as f32 / 255.0,
                config.pane_divider_hover_color[1] as f32 / 255.0,
                config.pane_divider_hover_color[2] as f32 / 255.0,
            ],
            show_focus_indicator: config.pane_focus_indicator,
            focus_color: [
                config.pane_focus_color[0] as f32 / 255.0,
                config.pane_focus_color[1] as f32 / 255.0,
                config.pane_focus_color[2] as f32 / 255.0,
            ],
            focus_width: config.pane_focus_width * renderer.scale_factor(),
            divider_style: config.pane_divider_style,
        };

        // Call the split pane renderer
        let result = renderer.render_split_panes(
            &pane_render_infos,
            &divider_render_infos,
            &pane_titles,
            focused_viewport.as_ref(),
            &divider_settings,
            egui_data,
            false,
        );

        // Clean up leaked cell memory
        for ptr in leaked_cells {
            // SAFETY: We just allocated these above
            let _ = unsafe { Box::from_raw(ptr) };
        }

        result
    }

    /// Handle responses from the integrations welcome dialog
    fn handle_integrations_response(&mut self, response: &IntegrationsResponse) {
        // Nothing to do if dialog wasn't interacted with
        if !response.install_shaders
            && !response.install_shell_integration
            && !response.skipped
            && !response.never_ask
            && !response.closed
            && response.shader_conflict_action.is_none()
        {
            return;
        }

        let current_version = env!("CARGO_PKG_VERSION").to_string();

        // Determine install intent and overwrite behavior
        let mut install_shaders = false;
        let mut install_shell_integration = false;
        let mut force_overwrite_modified_shaders = false;
        let mut triggered_install = false;

        // If we're waiting on a shader overwrite decision, handle that first
        if let Some(action) = response.shader_conflict_action {
            triggered_install = true;
            install_shaders = self.integrations_ui.pending_install_shaders;
            install_shell_integration = self.integrations_ui.pending_install_shell_integration;

            match action {
                crate::integrations_ui::ShaderConflictAction::Overwrite => {
                    force_overwrite_modified_shaders = true;
                }
                crate::integrations_ui::ShaderConflictAction::SkipModified => {
                    force_overwrite_modified_shaders = false;
                }
                crate::integrations_ui::ShaderConflictAction::Cancel => {
                    // Reset pending state and exit without installing
                    self.integrations_ui.awaiting_shader_overwrite = false;
                    self.integrations_ui.shader_conflicts.clear();
                    self.integrations_ui.pending_install_shaders = false;
                    self.integrations_ui.pending_install_shell_integration = false;
                    self.integrations_ui.error_message = None;
                    self.integrations_ui.success_message = None;
                    self.needs_redraw = true;
                    return;
                }
            }

            // Clear the conflict prompt regardless of choice
            self.integrations_ui.awaiting_shader_overwrite = false;
            self.integrations_ui.shader_conflicts.clear();
            self.integrations_ui.error_message = None;
            self.integrations_ui.success_message = None;
            self.integrations_ui.installing = false;
        } else if response.install_shaders || response.install_shell_integration {
            triggered_install = true;
            install_shaders = response.install_shaders;
            install_shell_integration = response.install_shell_integration;

            if install_shaders {
                match crate::shader_installer::detect_modified_bundled_shaders() {
                    Ok(conflicts) if !conflicts.is_empty() => {
                        log::info!(
                            "Detected {} modified bundled shaders; prompting for overwrite",
                            conflicts.len()
                        );
                        self.integrations_ui.awaiting_shader_overwrite = true;
                        self.integrations_ui.shader_conflicts = conflicts;
                        self.integrations_ui.pending_install_shaders = install_shaders;
                        self.integrations_ui.pending_install_shell_integration =
                            install_shell_integration;
                        self.integrations_ui.installing = false;
                        self.integrations_ui.error_message = None;
                        self.integrations_ui.success_message = None;
                        self.needs_redraw = true;
                        return; // Wait for user decision
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!(
                            "Unable to check existing shaders for modifications: {}. Proceeding without overwrite prompt.",
                            e
                        );
                    }
                }
            }
        }

        // Handle "Install Selected" - user wants to install one or both integrations
        if triggered_install {
            log::info!(
                "User requested installations: shaders={}, shell_integration={}, overwrite_modified={}",
                install_shaders,
                install_shell_integration,
                force_overwrite_modified_shaders
            );

            let mut success_parts = Vec::new();
            let mut error_parts = Vec::new();

            // Install shaders if requested
            if install_shaders {
                self.integrations_ui.set_installing("Installing shaders...");
                self.needs_redraw = true;
                self.request_redraw();

                match crate::shader_installer::install_shaders_with_manifest(
                    force_overwrite_modified_shaders,
                ) {
                    Ok(result) => {
                        log::info!(
                            "Installed {} shader files ({} skipped, {} removed)",
                            result.installed,
                            result.skipped,
                            result.removed
                        );
                        let detail = if result.skipped > 0 {
                            format!("{} shaders ({} skipped)", result.installed, result.skipped)
                        } else {
                            format!("{} shaders", result.installed)
                        };
                        success_parts.push(detail);
                        self.config.integration_versions.shaders_installed_version =
                            Some(current_version.clone());
                        self.config.integration_versions.shaders_prompted_version =
                            Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shaders: {}", e);
                        error_parts.push(format!("Shaders: {}", e));
                    }
                }
            }

            // Install shell integration if requested
            if install_shell_integration {
                self.integrations_ui
                    .set_installing("Installing shell integration...");
                self.needs_redraw = true;
                self.request_redraw();

                match crate::shell_integration_installer::install(None) {
                    Ok(result) => {
                        log::info!(
                            "Installed shell integration for {}",
                            result.shell.display_name()
                        );
                        success_parts.push(format!(
                            "shell integration ({})",
                            result.shell.display_name()
                        ));
                        self.config
                            .integration_versions
                            .shell_integration_installed_version = Some(current_version.clone());
                        self.config
                            .integration_versions
                            .shell_integration_prompted_version = Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shell integration: {}", e);
                        error_parts.push(format!("Shell: {}", e));
                    }
                }
            }

            // Show result
            if error_parts.is_empty() {
                self.integrations_ui
                    .set_success(&format!("Installed: {}", success_parts.join(", ")));
            } else if success_parts.is_empty() {
                self.integrations_ui
                    .set_error(&format!("Installation failed: {}", error_parts.join("; ")));
            } else {
                // Partial success
                self.integrations_ui.set_success(&format!(
                    "Installed: {}. Errors: {}",
                    success_parts.join(", "),
                    error_parts.join("; ")
                ));
            }

            // Save config
            if let Err(e) = self.config.save() {
                log::error!("Failed to save config after integration install: {}", e);
            }

            // Clear pending flags
            self.integrations_ui.pending_install_shaders = false;
            self.integrations_ui.pending_install_shell_integration = false;

            self.needs_redraw = true;
        }

        // Handle "Skip" - just close the dialog for this session
        if response.skipped {
            log::info!("User skipped integrations dialog for this session");
            self.integrations_ui.hide();
            // Update prompted versions so we don't ask again this version
            self.config.integration_versions.shaders_prompted_version =
                Some(current_version.clone());
            self.config
                .integration_versions
                .shell_integration_prompted_version = Some(current_version.clone());
            if let Err(e) = self.config.save() {
                log::error!("Failed to save config after skipping integrations: {}", e);
            }
        }

        // Handle "Never Ask" - disable prompting permanently
        if response.never_ask {
            log::info!("User declined integrations (never ask again)");
            self.integrations_ui.hide();
            // Set install prompts to Never
            self.config.shader_install_prompt = ShaderInstallPrompt::Never;
            self.config.shell_integration_state = crate::config::InstallPromptState::Never;
            if let Err(e) = self.config.save() {
                log::error!("Failed to save config after declining integrations: {}", e);
            }
        }

        // Handle dialog closed (OK button after success)
        if response.closed {
            self.integrations_ui.hide();
        }
    }

    /// Perform the shutdown sequence (save state and set shutdown flag)
    pub(crate) fn perform_shutdown(&mut self) {
        // Save last working directory for "previous session" mode
        if self.config.startup_directory_mode == crate::config::StartupDirectoryMode::Previous
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
        log::info!("Refresh tasks aborted, shutdown initiated");
    }
}

impl Drop for WindowState {
    fn drop(&mut self) {
        log::info!("Shutting down window");

        // Save command history before shutdown
        self.command_history.save();

        // Set shutdown flag
        self.is_shutting_down = true;

        // Clean up egui state FIRST before any other resources are dropped
        // This prevents egui from accessing freed resources during its own cleanup
        log::info!("Cleaning up egui state");
        self.egui_state = None;
        self.egui_ctx = None;

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
