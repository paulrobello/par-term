//! Constructor and async initialization for `WindowState`.

use super::impl_agent::merge_custom_ai_inspector_agents;
use super::{
    EguiState, FocusState, OverlayState, RenderLoopState, TriggerState, UpdateState, WatcherState,
    WindowState,
};
use crate::badge::BadgeState;
use crate::config::Config;
use crate::input::InputHandler;
use crate::keybindings::{KeyCombo, KeybindingRegistry};
use crate::smart_selection::SmartSelectionCache;
use crate::status_bar::StatusBarUI;
use crate::tab::TabManager;
use crate::tab_bar_ui::TabBarUI;
use anyhow::Result;
use arc_swap::ArcSwap;
use par_term_acp::discover_agents;
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::window::Window;

impl WindowState {
    pub(crate) fn parse_custom_action_prefix_combo(prefix_key: &str) -> Option<KeyCombo> {
        let trimmed = prefix_key.trim();
        if trimmed.is_empty() {
            return None;
        }

        match crate::keybindings::parser::parse_key_combo(trimmed) {
            Ok(combo) => Some(combo),
            Err(error) => {
                log::warn!(
                    "Invalid custom action prefix key '{}': {}",
                    prefix_key,
                    error
                );
                None
            }
        }
    }

    /// Create a new window state with the given configuration
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Self {
        let keybinding_registry = KeybindingRegistry::from_config(&config.keybindings);
        let custom_action_prefix_combo =
            Self::parse_custom_action_prefix_combo(&config.custom_action_prefix_key);
        let shaders_dir = Config::shaders_dir();
        let tmux_prefix_key = crate::tmux::PrefixKey::parse(&config.tmux_prefix_key);

        let mut input_handler = InputHandler::new();
        // Initialize Option/Alt key modes from config
        input_handler
            .update_option_key_modes(config.left_option_key_mode, config.right_option_key_mode);

        // Create badge state and overlay UI before wrapping config in ArcSwap
        let badge_state = BadgeState::new(&config);
        let overlay_ui = crate::app::window_state::overlay_ui_state::OverlayUiState::new(&config);

        // Discover available ACP agents
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        let available_agents = merge_custom_ai_inspector_agents(
            discovered_agents,
            &config.ai_inspector.ai_inspector_custom_agents,
        );

        Self {
            config: ArcSwap::from(Arc::new(config)),
            window: None,
            renderer: None,
            input_handler,
            runtime,

            tab_manager: TabManager::new(),
            tab_bar_ui: TabBarUI::new(),
            status_bar_ui: StatusBarUI::new(),

            debug: crate::app::window_state::debug_state::DebugState::new(),

            cursor_anim: crate::app::window_state::cursor_anim_state::CursorAnimState::default(),
            is_fullscreen: false,
            egui: EguiState::default(),
            shader_state: crate::app::window_state::shader_state::ShaderState::new(shaders_dir),
            overlay_ui,
            agent_state: super::agent_state::AgentState::new(available_agents),
            is_recording: false,
            is_shutting_down: false,
            window_index: 1, // Will be set by WindowManager when window is created

            focus_state: FocusState::default(),

            render_loop: RenderLoopState::default(),

            watcher_state: WatcherState::default(),

            clipboard_image_click_guard: None,

            overlay_state: OverlayState::default(),

            keybinding_registry,
            custom_action_prefix_combo,
            custom_action_prefix_state: crate::tmux::PrefixState::default(),

            smart_selection_cache: SmartSelectionCache::new(),

            tmux_state: super::TmuxState::new(tmux_prefix_key),

            broadcast_input: false,

            badge_state,

            copy_mode: crate::copy_mode::CopyModeState::new(),

            file_transfer_state: crate::app::file_transfers::FileTransferState::default(),

            update_state: UpdateState::default(),

            trigger_state: TriggerState::default(),

            pending_snap_size: None,

            last_workflow_context: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Format window title with optional window number
    /// This should be used everywhere a title is set to ensure consistency
    pub(crate) fn format_title(&self, base_title: &str) -> String {
        if self.config.load().show_window_number {
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
    /// `skip_default_tab` - When `true`, skips creating the default shell tab.
    /// Used by the "Move Tab to New Window" path to initialize a fully functional
    /// window that starts with an empty `TabManager`, ready to receive a transferred
    /// tab via `insert_tab_at`. All normal callers pass `false`.
    ///
    /// `first_tab_cwd` - Optional working directory for the first tab.
    /// Used by arrangement restore to set the CWD before the shell spawns.
    pub(crate) async fn initialize_async(
        &mut self,
        window: Window,
        skip_default_tab: bool,
        first_tab_cwd: Option<String>,
    ) -> Result<()> {
        use crate::app::window_state::renderer_init::RendererInitParams;

        // Enable IME (Input Method Editor) to receive all character events including Space
        window.set_ime_allowed(true);
        log::debug!("IME enabled for character input");

        // Detect system theme at startup and apply if auto_dark_mode is enabled
        {
            let cfg = self.config.load();
            if cfg.auto_dark_mode {
                let is_dark = window
                    .theme()
                    .is_none_or(|t| t == winit::window::Theme::Dark);
                drop(cfg); // release guard before mutation via rcu
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    if new.apply_system_theme(is_dark) {
                        Arc::new(new)
                    } else {
                        Arc::clone(old)
                    }
                });
                let cfg = self.config.load();
                log::info!(
                    "Auto dark mode: detected {} system theme, using theme: {}",
                    if is_dark { "dark" } else { "light" },
                    cfg.theme
                );
            }
        }

        // Detect system theme at startup and apply tab style if tab_style is Automatic
        {
            let is_dark = window
                .theme()
                .is_none_or(|t| t == winit::window::Theme::Dark);
            let should_apply = {
                let mut probe = (**self.config.load()).clone();
                probe.apply_system_tab_style(is_dark)
            };
            if should_apply {
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    if new.apply_system_tab_style(is_dark) {
                        Arc::new(new)
                    } else {
                        Arc::clone(old)
                    }
                });
                let cfg = self.config.load();
                log::info!(
                    "Auto tab style: detected {} system theme, applying {} tab style",
                    if is_dark { "dark" } else { "light" },
                    if is_dark {
                        cfg.dark_tab_style.display_name()
                    } else {
                        cfg.light_tab_style.display_name()
                    }
                );
            }
        }

        let window = Arc::new(window);

        // Initialize egui context and state (no memory to preserve on first init)
        self.init_egui(&window, false);

        // Create renderer using DRY init params
        let cfg = self.config.load();
        let theme = cfg.load_theme();
        // Get shader metadata from cache for full 3-tier resolution
        let metadata = cfg
            .shader
            .custom_shader
            .as_ref()
            .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
        // Get cursor shader metadata from cache for full 3-tier resolution
        let cursor_metadata = cfg.shader.cursor_shader.as_ref().and_then(|name| {
            self.shader_state
                .cursor_shader_metadata_cache
                .get(name)
                .cloned()
        });
        let params = RendererInitParams::from_config(
            &cfg,
            &theme,
            metadata.as_ref(),
            cursor_metadata.as_ref(),
        );
        drop(cfg); // release guard before moving to macOS section

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
            {
                let cfg = self.config.load();
                if cfg.window.blur_enabled
                    && cfg.window.window_opacity < 1.0
                    && let Err(e) =
                        crate::macos_blur::set_window_blur(&window, cfg.window.blur_radius)
                {
                    log::warn!("Failed to set initial window blur: {}", e);
                }
            }
        }

        // Apply cursor shader configuration
        self.apply_cursor_shader_config(&mut renderer, &params);

        // Set tab bar offsets BEFORE creating the first tab
        // This ensures the terminal is sized correctly from the start
        // Use 1 as tab count since we're about to create the first tab
        let (initial_tab_bar_height, initial_tab_bar_width, tab_bar_mode, tab_bar_position) = {
            let cfg = self.config.load();
            (
                self.tab_bar_ui.get_height(1, &cfg),
                self.tab_bar_ui.get_width(1, &cfg),
                cfg.tab_bar_mode,
                cfg.tab_bar_position,
            )
        };
        let (initial_cols, initial_rows) = renderer.grid_size();
        log::info!(
            "Tab bar init: mode={:?}, position={:?}, height={:.1}, width={:.1}, initial_grid={}x{}, content_offset_y_before={:.1}",
            tab_bar_mode,
            tab_bar_position,
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

        // Initialize shader-diagnostics-request watcher (MCP server diagnostics tool writes here)
        self.init_shader_diagnostics_request_watcher();

        // Sync status bar monitor state based on config
        {
            let cfg = self.config.load();
            self.status_bar_ui.sync_monitor_state(&cfg);
        }

        // Create the first tab with the correct grid size from the renderer
        // This ensures the shell is spawned with dimensions that account for tab bar.
        // Skipped when `skip_default_tab` is true (e.g. "Move Tab to New Window" path).
        if !skip_default_tab {
            log::info!(
                "Creating first tab with grid size {}x{} (accounting for tab bar)",
                renderer_cols,
                renderer_rows
            );
            let (max_fps, inactive_tab_fps) = {
                let cfg = self.config.load();
                (cfg.max_fps, cfg.inactive_tab_fps)
            };
            let tab_id = self.tab_manager.new_tab_with_cwd(
                &self.config.load(),
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
                    if let Err(e) =
                        term.resize_with_pixels(renderer_cols, renderer_rows, width_px, height_px)
                    {
                        crate::debug_error!("TERMINAL", "resize_with_pixels failed (init): {e}");
                    }
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
                    max_fps,
                    inactive_tab_fps,
                );
            }
        }

        // Auto-connect agent if panel is open on startup and auto-launch is enabled
        if self.overlay_ui.ai_inspector.open {
            self.try_auto_connect_agent();
        }

        // Check if we should prompt user to install integrations (shaders and/or shell integration)
        {
            let cfg = self.config.load();
            if cfg.should_prompt_integrations(crate::VERSION) {
                drop(cfg);
                log::info!("Integrations not installed - showing welcome dialog");
                self.overlay_ui.integrations_ui.show_dialog();
                self.focus_state.needs_redraw = true;
                window.request_redraw();
            }
        }

        Ok(())
    }
}
