//! Constructor and async initialization for `WindowState`.

use super::impl_agent::merge_custom_ai_inspector_agents;
use super::{
    ConfigSaveState, EguiState, FocusState, OverlayState, TriggerState, UpdateState, WatcherState,
    WindowState,
};
use crate::badge::BadgeState;
use crate::config::Config;
use crate::input::InputHandler;
use crate::keybindings::KeybindingRegistry;
use crate::smart_selection::SmartSelectionCache;
use crate::status_bar::StatusBarUI;
use crate::tab::TabManager;
use crate::tab_bar_ui::TabBarUI;
use anyhow::Result;
use par_term_acp::discover_agents;
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::window::Window;

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

        // Create badge state and overlay UI before moving config
        let badge_state = BadgeState::new(&config);
        let overlay_ui = crate::app::overlay_ui_state::OverlayUiState::new(&config);

        // Discover available ACP agents
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        let available_agents = merge_custom_ai_inspector_agents(
            discovered_agents,
            &config.ai_inspector.ai_inspector_custom_agents,
        );

        Self {
            config,
            window: None,
            renderer: None,
            input_handler,
            runtime,

            tab_manager: TabManager::new(),
            tab_bar_ui: TabBarUI::new(),
            status_bar_ui: StatusBarUI::new(),

            debug: crate::app::debug_state::DebugState::new(),

            cursor_anim: crate::app::cursor_anim_state::CursorAnimState::default(),
            is_fullscreen: false,
            egui: EguiState::default(),
            shader_state: crate::app::shader_state::ShaderState::new(shaders_dir),
            overlay_ui,
            agent_state: crate::app::agent_state::AgentState::new(available_agents),
            is_recording: false,
            is_shutting_down: false,
            window_index: 1, // Will be set by WindowManager when window is created

            focus_state: FocusState::default(),

            config_changed_by_agent: false,
            pending_font_rebuild: false,
            config_save_state: ConfigSaveState::default(),

            watcher_state: WatcherState::default(),

            clipboard_image_click_guard: None,

            overlay_state: OverlayState::default(),

            keybinding_registry,

            smart_selection_cache: SmartSelectionCache::new(),

            tmux_state: crate::app::tmux_state::TmuxState::new(tmux_prefix_key),

            broadcast_input: false,

            badge_state,

            copy_mode: crate::copy_mode::CopyModeState::new(),

            file_transfer_state: crate::app::file_transfers::FileTransferState::default(),

            update_state: UpdateState::default(),

            trigger_state: TriggerState::default(),
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

    /// Initialize the window asynchronously
    ///
    /// `first_tab_cwd` - Optional working directory for the first tab.
    /// Used by arrangement restore to set the CWD before the shell spawns.
    pub(crate) async fn initialize_async(
        &mut self,
        window: Window,
        first_tab_cwd: Option<String>,
    ) -> Result<()> {
        use crate::app::renderer_init::RendererInitParams;

        // Enable IME (Input Method Editor) to receive all character events including Space
        window.set_ime_allowed(true);
        log::debug!("IME enabled for character input");

        // Detect system theme at startup and apply if auto_dark_mode is enabled
        if self.config.auto_dark_mode {
            let is_dark = window
                .theme()
                .is_none_or(|t| t == winit::window::Theme::Dark);
            if self.config.apply_system_theme(is_dark) {
                log::info!(
                    "Auto dark mode: detected {} system theme, using theme: {}",
                    if is_dark { "dark" } else { "light" },
                    self.config.theme
                );
            }
        }

        // Detect system theme at startup and apply tab style if tab_style is Automatic
        {
            let is_dark = window
                .theme()
                .is_none_or(|t| t == winit::window::Theme::Dark);
            if self.config.apply_system_tab_style(is_dark) {
                log::info!(
                    "Auto tab style: detected {} system theme, applying {} tab style",
                    if is_dark { "dark" } else { "light" },
                    if is_dark {
                        self.config.dark_tab_style.display_name()
                    } else {
                        self.config.light_tab_style.display_name()
                    }
                );
            }
        }

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
            .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = self.config.cursor_shader.as_ref().and_then(|name| {
            self.shader_state
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

        // Set tab bar offsets BEFORE creating the first tab
        // This ensures the terminal is sized correctly from the start
        // Use 1 as tab count since we're about to create the first tab
        let initial_tab_bar_height = self.tab_bar_ui.get_height(1, &self.config);
        let initial_tab_bar_width = self.tab_bar_ui.get_width(1, &self.config);
        let (initial_cols, initial_rows) = renderer.grid_size();
        log::info!(
            "Tab bar init: mode={:?}, position={:?}, height={:.1}, width={:.1}, initial_grid={}x{}, content_offset_y_before={:.1}",
            self.config.tab_bar_mode,
            self.config.tab_bar_position,
            initial_tab_bar_height,
            initial_tab_bar_width,
            initial_cols,
            initial_rows,
            renderer.content_offset_y()
        );
        self.apply_tab_bar_offsets(&mut renderer, initial_tab_bar_height, initial_tab_bar_width);

        // Get the renderer's grid size BEFORE storing it (and before creating tabs)
        // This ensures the shell starts with correct dimensions that account for tab bar
        let (renderer_cols, renderer_rows) = renderer.grid_size();
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();

        self.window = Some(Arc::clone(&window));
        self.renderer = Some(renderer);

        // Initialize shader watcher if hot reload is enabled
        self.init_shader_watcher();

        // Initialize config file watcher for automatic reload
        self.init_config_watcher();

        // Initialize config-update file watcher (MCP server writes here)
        self.init_config_update_watcher();

        // Initialize screenshot-request watcher (MCP server screenshot tool writes here)
        self.init_screenshot_request_watcher();

        // Sync status bar monitor state based on config
        self.status_bar_ui.sync_monitor_state(&self.config);

        // Create the first tab with the correct grid size from the renderer
        // This ensures the shell is spawned with dimensions that account for tab bar
        log::info!(
            "Creating first tab with grid size {}x{} (accounting for tab bar)",
            renderer_cols,
            renderer_rows
        );
        let tab_id = self.tab_manager.new_tab_with_cwd(
            &self.config,
            Arc::clone(&self.runtime),
            first_tab_cwd,
            Some((renderer_cols, renderer_rows)), // Pass correct grid size
        )?;

        // Set cell dimensions on the terminal (for TIOCGWINSZ pixel size reporting)
        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
            let width_px = (renderer_cols as f32 * cell_width) as usize;
            let height_px = (renderer_rows as f32 * cell_height) as usize;

            if let Ok(mut term) = tab.terminal.try_write() {
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
                self.config.inactive_tab_fps,
            );
        }

        // Auto-connect agent if panel is open on startup and auto-launch is enabled
        if self.overlay_ui.ai_inspector.open {
            self.try_auto_connect_agent();
        }

        // Check if we should prompt user to install integrations (shaders and/or shell integration)
        if self.config.should_prompt_integrations(crate::VERSION) {
            log::info!("Integrations not installed - showing welcome dialog");
            self.overlay_ui.integrations_ui.show_dialog();
            self.focus_state.needs_redraw = true;
            window.request_redraw();
        }

        Ok(())
    }
}
