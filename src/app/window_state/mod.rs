//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.

pub(crate) mod agent_messages;
pub(crate) mod render_pipeline;

use crate::ai_inspector::chat::ChatMessage;
use crate::ai_inspector::panel::InspectorAction;
use crate::app::anti_idle::should_send_keep_alive;
use crate::app::debug_state::DebugState;
use crate::badge::BadgeState;
use crate::clipboard_history_ui::ClipboardHistoryAction;
use crate::config::{
    Config, CustomAcpAgentConfig, ShaderInstallPrompt,
};
use crate::input::InputHandler;
use crate::integrations_ui::IntegrationsResponse;
use crate::keybindings::KeybindingRegistry;
use crate::renderer::Renderer;
use crate::shader_watcher::{ShaderReloadEvent, ShaderType, ShaderWatcher};
use crate::smart_selection::SmartSelectionCache;
use crate::status_bar::StatusBarUI;
use crate::tab::TabManager;
use crate::tab_bar_ui::{TabBarAction, TabBarUI};
use anyhow::Result;
use par_term_acp::{
    Agent, AgentConfig, AgentMessage, AgentStatus, ClientCapabilities, FsCapabilities, SafePaths,
    discover_agents,
};
use par_term_mcp::{
    SCREENSHOT_REQUEST_FILENAME, SCREENSHOT_RESPONSE_FILENAME, TerminalScreenshotRequest,
    TerminalScreenshotResponse,
};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use winit::window::Window;

#[derive(Clone)]
pub(crate) struct PreservedClipboardImage {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ClipboardImageClickGuard {
    pub(crate) image: PreservedClipboardImage,
    pub(crate) press_position: (f64, f64),
    pub(crate) suppress_terminal_mouse_click: bool,
}

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
    /// Custom status bar UI
    pub(crate) status_bar_ui: StatusBarUI,

    pub(crate) debug: DebugState,

    /// Cursor animation state (opacity, blink timers)
    pub(crate) cursor_anim: crate::app::cursor_anim_state::CursorAnimState,
    /// Whether window is currently in fullscreen mode
    pub(crate) is_fullscreen: bool,
    /// egui context for GUI rendering
    pub(crate) egui_ctx: Option<egui::Context>,
    /// egui-winit state for event handling
    pub(crate) egui_state: Option<egui_winit::State>,
    /// Pending egui events to inject into next frame's raw_input.
    /// Used when macOS menu accelerators intercept Cmd+V/C/A before egui sees them
    /// while an egui overlay (profile modal, search, etc.) is active.
    pub(crate) pending_egui_events: Vec<egui::Event>,
    /// Whether egui has completed its first ctx.run() call
    /// Before first run, egui's is_using_pointer() returns unreliable results
    pub(crate) egui_initialized: bool,
    /// Shader hot-reload watcher, metadata caches, and reload-error state
    pub(crate) shader_state: crate::app::shader_state::ShaderState,
    /// Overlay / modal / side-panel UI state
    pub(crate) overlay_ui: crate::app::overlay_ui_state::OverlayUiState,
    /// ACP agent connection and runtime state
    pub(crate) agent_state: crate::app::agent_state::AgentState,
    /// Whether terminal session recording is active
    pub(crate) is_recording: bool,
    /// Flag to indicate shutdown is in progress
    pub(crate) is_shutting_down: bool,
    /// Window index (1-based) for display in title bar
    pub(crate) window_index: usize,

    // Smart redraw tracking (event-driven rendering)
    /// Whether we need to render next frame
    pub(crate) needs_redraw: bool,
    /// Set when an agent/MCP config update was applied — signals WindowManager to
    /// sync its own config copy so subsequent saves don't overwrite agent changes.
    pub(crate) config_changed_by_agent: bool,
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

    // Config and screenshot watchers
    /// Config file watcher for automatic reload (e.g., when ACP agent modifies config.yaml)
    pub(crate) config_watcher: Option<crate::config::watcher::ConfigWatcher>,
    /// Watcher for `.config-update.json` written by the MCP server
    pub(crate) config_update_watcher: Option<crate::config::watcher::ConfigWatcher>,
    /// Watcher for `.screenshot-request.json` written by the MCP server
    pub(crate) screenshot_request_watcher: Option<crate::config::watcher::ConfigWatcher>,

    /// Flag to signal that the settings window should be opened
    /// This is set by keyboard handlers and consumed by the window manager
    pub(crate) open_settings_window_requested: bool,

    /// Pending arrangement restore request (name of arrangement to restore)
    pub(crate) pending_arrangement_restore: Option<String>,

    /// Flag to request reload of dynamic profiles
    pub(crate) reload_dynamic_profiles_requested: bool,

    // Profile management
    /// Flag to signal that the settings window should open to the Profiles tab
    pub(crate) open_settings_profiles_tab: bool,
    /// Flag to indicate profiles menu needs to be updated in the main menu
    pub(crate) profiles_menu_needs_update: bool,
    /// Track if we blocked a mouse press for UI - also block the corresponding release
    pub(crate) ui_consumed_mouse_press: bool,
    /// Eat the first mouse click after window focus to prevent forwarding to PTY.
    /// Without this, clicking to focus the window sends a mouse event to tmux (or
    /// other mouse-aware apps), which can trigger a zero-char selection that clears
    /// the system clipboard — destroying any clipboard image.
    pub(crate) focus_click_pending: bool,
    /// Timestamp of a mouse press we already suppressed while the window was still
    /// unfocused. Used to avoid arming a second suppression when the OS delivers
    /// the `Focused(true)` event after the click press/release.
    pub(crate) focus_click_suppressed_while_unfocused_at: Option<std::time::Instant>,
    /// Snapshot of clipboard image content captured on mouse-down so we can restore it
    /// after a plain click if a terminal app/tmux clears the clipboard on click.
    pub(crate) clipboard_image_click_guard: Option<ClipboardImageClickGuard>,

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

    // Pane identification overlay
    /// When to hide the pane index overlay
    pub(crate) pane_identify_hide_time: Option<std::time::Instant>,

    /// Recently closed tab metadata for session undo (reopen closed tab)
    pub(crate) closed_tabs: std::collections::VecDeque<super::tab_ops::ClosedTabInfo>,

    /// Keybinding registry for user-defined keyboard shortcuts
    pub(crate) keybinding_registry: KeybindingRegistry,

    /// Cache for compiled smart selection regex patterns
    pub(crate) smart_selection_cache: SmartSelectionCache,

    // tmux integration state
    /// tmux integration state (session, sync, pane mappings, prefix key)
    pub(crate) tmux_state: crate::app::tmux_state::TmuxState,

    // Broadcast input mode
    /// Whether keyboard input is broadcast to all panes in current tab
    pub(crate) broadcast_input: bool,

    // Badge overlay
    /// Badge state for session information display
    pub(crate) badge_state: BadgeState,

    // Copy mode (vi-style keyboard text selection)
    /// Copy mode state machine
    pub(crate) copy_mode: crate::copy_mode::CopyModeState,

    // File transfer state
    /// File transfer UI state (active transfers, pending saves/uploads, dialog state)
    pub(crate) file_transfer_state: crate::app::file_transfers::FileTransferState,

    /// Whether to show the update dialog overlay (set when user clicks the update widget)
    pub(crate) show_update_dialog: bool,

    /// Last update check result (for update dialog)
    pub(crate) last_update_result: Option<crate::update_checker::UpdateCheckResult>,
    /// Detected installation type
    pub(crate) installation_type: par_term_settings_ui::InstallationType,

    /// Whether an update install is in progress (from the update dialog)
    pub(crate) update_installing: bool,
    /// Status message from the update install
    pub(crate) update_install_status: Option<String>,
    /// Channel receiver for async update install result
    pub(crate) update_install_receiver:
        Option<std::sync::mpsc::Receiver<Result<crate::self_updater::UpdateResult, String>>>,
}

fn merge_custom_ai_inspector_agents(
    mut agents: Vec<AgentConfig>,
    custom_agents: &[CustomAcpAgentConfig],
) -> Vec<AgentConfig> {
    for custom in custom_agents {
        if custom.identity.trim().is_empty()
            || custom.short_name.trim().is_empty()
            || custom.name.trim().is_empty()
            || custom.run_command.is_empty()
        {
            log::warn!(
                "Skipping invalid custom ACP agent entry identity='{}' short_name='{}'",
                custom.identity,
                custom.short_name
            );
            continue;
        }

        let actions: std::collections::HashMap<
            String,
            std::collections::HashMap<String, par_term_acp::agents::ActionConfig>,
        > = custom
            .actions
            .iter()
            .map(|(action_name, variants)| {
                let mapped_variants = variants
                    .iter()
                    .map(|(variant_name, action)| {
                        (
                            variant_name.clone(),
                            par_term_acp::agents::ActionConfig {
                                command: action.command.clone(),
                                description: action.description.clone(),
                            },
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>();
                (action_name.clone(), mapped_variants)
            })
            .collect::<std::collections::HashMap<_, _>>();

        let mut env = custom.env.clone();
        if !env.contains_key("OLLAMA_CONTEXT_LENGTH")
            && let Some(ctx) = custom.ollama_context_length
            && ctx > 0
        {
            env.insert("OLLAMA_CONTEXT_LENGTH".to_string(), ctx.to_string());
        }

        let mut custom_agent = AgentConfig {
            identity: custom.identity.clone(),
            name: custom.name.clone(),
            short_name: custom.short_name.clone(),
            protocol: if custom.protocol.trim().is_empty() {
                "acp".to_string()
            } else {
                custom.protocol.clone()
            },
            r#type: if custom.r#type.trim().is_empty() {
                "coding".to_string()
            } else {
                custom.r#type.clone()
            },
            active: custom.active,
            run_command: custom.run_command.clone(),
            env,
            install_command: custom.install_command.clone(),
            actions,
            connector_installed: false,
        };

        custom_agent.detect_connector();
        agents.retain(|existing| existing.identity != custom_agent.identity);
        agents.push(custom_agent);
    }

    agents.retain(|agent| agent.is_active());
    agents
}

/// Reconstruct markdown syntax from cell attributes for Claude Code output.
///
/// Claude Code pre-renders markdown with ANSI sequences (bold for headers/emphasis,
/// italic for emphasis), stripping the original syntax markers. This function
/// reconstructs markdown syntax from cell attributes so the prettifier's markdown
/// detector can recognize patterns like `# Header` and `**bold**`.
pub(super) fn reconstruct_markdown_from_cells(cells: &[par_term_config::Cell]) -> String {
    // First, extract plain text and trim trailing whitespace.
    let trimmed_len = cells
        .iter()
        .rposition(|c| {
            let g = c.grapheme.as_str();
            !(g.is_empty() || g == "\0" || g == " ")
        })
        .map(|i| i + 1)
        .unwrap_or(0);

    if trimmed_len == 0 {
        return String::new();
    }

    let cells = &cells[..trimmed_len];

    // Find the first non-whitespace cell index.
    let first_nonws = cells
        .iter()
        .position(|c| {
            let g = c.grapheme.as_str();
            !(g.is_empty() || g == "\0" || g == " ")
        })
        .unwrap_or(0);

    // Check if all non-whitespace cells share the same attribute pattern (for header detection).
    let all_bold = cells[first_nonws..].iter().all(|c| {
        let g = c.grapheme.as_str();
        (g.is_empty() || g == "\0" || g == " ") || c.bold
    });

    let all_underline = all_bold
        && cells[first_nonws..].iter().all(|c| {
            let g = c.grapheme.as_str();
            (g.is_empty() || g == "\0" || g == " ") || c.underline
        });

    // Extract the plain text content.
    let plain_text: String = cells
        .iter()
        .map(|c| {
            let g = c.grapheme.as_str();
            if g.is_empty() || g == "\0" { " " } else { g }
        })
        .collect::<String>()
        .trim_end()
        .to_string();

    // Header detection: if every non-ws cell is bold, it's a header.
    // Bold + underline → H1 (`# `), bold only → H2 (`## `).
    if all_bold && !plain_text.trim().is_empty() {
        let trimmed = plain_text.trim_start();
        // Don't add header markers to lines that already look like list items, tables, etc.
        if !trimmed.starts_with('-')
            && !trimmed.starts_with('*')
            && !trimmed.starts_with('|')
            && !trimmed.starts_with('#')
        {
            return if all_underline {
                format!("# {trimmed}")
            } else {
                format!("## {trimmed}")
            };
        }
    }

    // Inline bold/italic reconstruction: track attribute transitions.
    let mut result = String::with_capacity(plain_text.len() + 32);
    let mut in_bold = false;
    let mut in_italic = false;

    for cell in cells {
        let g = cell.grapheme.as_str();
        let ch = if g.is_empty() || g == "\0" { " " } else { g };

        // Bold transitions (skip if whole line is bold — already handled as header).
        if !all_bold {
            if cell.bold && !in_bold {
                result.push_str("**");
                in_bold = true;
            } else if !cell.bold && in_bold {
                result.push_str("**");
                in_bold = false;
            }
        }

        // Italic transitions.
        if cell.italic && !in_italic {
            result.push('*');
            in_italic = true;
        } else if !cell.italic && in_italic {
            result.push('*');
            in_italic = false;
        }

        result.push_str(ch);
    }

    // Close any open markers.
    if in_bold {
        result.push_str("**");
    }
    if in_italic {
        result.push('*');
    }

    result.trim_end().to_string()
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

        // Create badge state and overlay UI before moving config
        let badge_state = BadgeState::new(&config);
        let overlay_ui = crate::app::overlay_ui_state::OverlayUiState::new(&config);

        // Discover available ACP agents
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        let available_agents =
            merge_custom_ai_inspector_agents(discovered_agents, &config.ai_inspector_custom_agents);

        Self {
            config,
            window: None,
            renderer: None,
            input_handler,
            runtime,

            tab_manager: TabManager::new(),
            tab_bar_ui: TabBarUI::new(),
            status_bar_ui: StatusBarUI::new(),

            debug: DebugState::new(),

            cursor_anim: crate::app::cursor_anim_state::CursorAnimState::default(),
            is_fullscreen: false,
            egui_ctx: None,
            egui_state: None,
            pending_egui_events: Vec::new(),
            egui_initialized: false,
            shader_state: crate::app::shader_state::ShaderState::new(shaders_dir),
            overlay_ui,
            agent_state: crate::app::agent_state::AgentState::new(available_agents),
            is_recording: false,
            is_shutting_down: false,
            window_index: 1, // Will be set by WindowManager when window is created

            needs_redraw: true,
            config_changed_by_agent: false,
            pending_font_rebuild: false,

            is_focused: true, // Assume focused on creation
            last_render_time: None,

            cursor_hidden_since: None,
            flicker_pending_render: false,

            throughput_batch_start: None,

            config_watcher: None,
            config_update_watcher: None,
            screenshot_request_watcher: None,

            open_settings_window_requested: false,
            pending_arrangement_restore: None,
            reload_dynamic_profiles_requested: false,

            open_settings_profiles_tab: false,
            profiles_menu_needs_update: true, // Update menu on startup
            ui_consumed_mouse_press: false,
            focus_click_pending: false,
            focus_click_suppressed_while_unfocused_at: None,
            clipboard_image_click_guard: None,

            resize_overlay_visible: false,
            resize_overlay_hide_time: None,
            resize_dimensions: None,

            toast_message: None,
            toast_hide_time: None,
            pane_identify_hide_time: None,
            closed_tabs: std::collections::VecDeque::new(),

            keybinding_registry,

            smart_selection_cache: SmartSelectionCache::new(),

            tmux_state: crate::app::tmux_state::TmuxState::new(tmux_prefix_key),

            broadcast_input: false,

            badge_state,

            copy_mode: crate::copy_mode::CopyModeState::new(),

            file_transfer_state: crate::app::file_transfers::FileTransferState::default(),

            show_update_dialog: false,

            last_update_result: None,
            installation_type: par_term_settings_ui::InstallationType::StandaloneBinary,

            update_installing: false,
            update_install_status: None,
            update_install_receiver: None,
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

    /// Recompute available ACP agents from discovered + custom definitions.
    pub(crate) fn refresh_available_agents(&mut self) {
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        self.agent_state.available_agents = merge_custom_ai_inspector_agents(
            discovered_agents,
            &self.config.ai_inspector_custom_agents,
        );
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

        // Re-apply AI Inspector panel inset to the new renderer.
        // The old renderer had the correct content_inset_right but the new one
        // starts with 0.0. Force last_inspector_width to 0 so sync detects the change.
        self.overlay_ui.last_inspector_width = 0.0;
        self.sync_ai_inspector_width();

        // Reset egui with preserved memory (window positions, collapse state)
        self.init_egui(&window, true);
        self.request_redraw();

        Ok(())
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
                self.config.inactive_tab_fps,
            );
        }

        // Auto-connect agent if panel is open on startup and auto-launch is enabled
        if self.overlay_ui.ai_inspector.open {
            self.try_auto_connect_agent();
        }

        // Check if we should prompt user to install integrations (shaders and/or shell integration)
        if self.config.should_prompt_integrations() {
            log::info!("Integrations not installed - showing welcome dialog");
            self.overlay_ui.integrations_ui.show_dialog();
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
    // Tab Bar Offsets
    // ========================================================================

    /// Apply tab bar offsets based on the current position configuration.
    /// Sets content_offset_y (top), content_offset_x (left), and content_inset_bottom (bottom).
    /// Returns Some((cols, rows)) if any offset changed and caused a grid resize.
    pub(crate) fn apply_tab_bar_offsets(
        &self,
        renderer: &mut crate::renderer::Renderer,
        tab_bar_height: f32,
        tab_bar_width: f32,
    ) -> Option<(usize, usize)> {
        Self::apply_tab_bar_offsets_for_position(
            self.config.tab_bar_position,
            renderer,
            tab_bar_height,
            tab_bar_width,
        )
    }

    /// Static helper to apply tab bar offsets (avoids borrowing self).
    pub(crate) fn apply_tab_bar_offsets_for_position(
        position: crate::config::TabBarPosition,
        renderer: &mut crate::renderer::Renderer,
        tab_bar_height: f32,
        tab_bar_width: f32,
    ) -> Option<(usize, usize)> {
        use crate::config::TabBarPosition;
        let (offset_y, offset_x, inset_bottom) = match position {
            TabBarPosition::Top => (tab_bar_height, 0.0, 0.0),
            TabBarPosition::Bottom => (0.0, 0.0, tab_bar_height),
            TabBarPosition::Left => (0.0, tab_bar_width, 0.0),
        };

        let mut result = None;
        if let Some(grid) = renderer.set_content_offset_y(offset_y) {
            result = Some(grid);
        }
        if let Some(grid) = renderer.set_content_offset_x(offset_x) {
            result = Some(grid);
        }
        if let Some(grid) = renderer.set_content_inset_bottom(inset_bottom) {
            result = Some(grid);
        }
        result
    }

    // AI Inspector Panel Width Sync
    // ========================================================================

    /// Sync the AI Inspector panel consumed width with the renderer.
    ///
    /// When the panel opens, closes, or is resized by dragging, the terminal
    /// column count must be updated so text reflows to fit the available space.
    /// This method checks whether the consumed width has changed and, if so,
    /// updates the renderer's right content inset and resizes all terminals.
    pub(crate) fn sync_ai_inspector_width(&mut self) {
        let current_width = self.overlay_ui.ai_inspector.consumed_width();

        if let Some(renderer) = &mut self.renderer {
            // Always verify the renderer's content_inset_right matches the expected
            // physical value. This catches cases where content_inset_right was reset
            // (e.g., renderer rebuild, scale factor change) even when the logical
            // panel width hasn't changed.
            // The renderer's set_content_inset_right() has its own guard that only
            // triggers a resize when the physical value actually differs.
            if let Some((new_cols, new_rows)) = renderer.set_content_inset_right(current_width) {
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let width_px = (new_cols as f32 * cell_width) as usize;
                let height_px = (new_rows as f32 * cell_height) as usize;

                for tab in self.tab_manager.tabs_mut() {
                    if let Ok(mut term) = tab.terminal.try_lock() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                    }
                    tab.cache.cells = None;
                }

                crate::debug_info!(
                    "AI_INSPECTOR",
                    "Panel width synced to {:.0}px, resized terminals to {}x{}",
                    current_width,
                    new_cols,
                    new_rows
                );
                self.needs_redraw = true;
            } else if (current_width - self.overlay_ui.last_inspector_width).abs() >= 1.0 {
                // Logical width changed but physical grid didn't resize
                // (could happen with very small changes below cell width threshold)
                self.needs_redraw = true;
            }
        }

        // Persist panel width to config when the user finishes resizing.
        if !self.overlay_ui.ai_inspector.is_resizing()
            && (current_width - self.overlay_ui.last_inspector_width).abs() >= 1.0
            && current_width > 0.0
            && self.overlay_ui.ai_inspector.open
        {
            self.config.ai_inspector_width = self.overlay_ui.ai_inspector.width;
            // Save to disk so the width is remembered across sessions.
            if let Err(e) = self.config.save() {
                log::error!("Failed to save AI inspector width: {}", e);
            }
        }

        self.overlay_ui.last_inspector_width = current_width;
    }

    /// Connect to an ACP agent by identity string.
    ///
    /// This extracts the agent connection logic so it can be called both from
    /// `InspectorAction::ConnectAgent` and from the auto-connect-on-open path.
    pub(crate) fn connect_agent(&mut self, identity: &str) {
        if let Some(agent_config) = self
            .agent_state
            .available_agents
            .iter()
            .find(|a| a.identity == identity)
        {
            self.agent_state.pending_agent_context_replay = self
                .overlay_ui
                .ai_inspector
                .chat
                .build_context_replay_prompt();
            self.overlay_ui.ai_inspector.connected_agent_name = Some(agent_config.name.clone());
            self.overlay_ui.ai_inspector.connected_agent_identity =
                Some(agent_config.identity.clone());

            // Clean up any previous agent before starting a new connection.
            if let Some(old_agent) = self.agent_state.agent.take() {
                let runtime = self.runtime.clone();
                runtime.spawn(async move {
                    let mut agent = old_agent.lock().await;
                    agent.disconnect().await;
                });
            }
            self.agent_state.agent_rx = None;
            self.agent_state.agent_tx = None;
            self.agent_state.agent_client = None;

            let (tx, rx) = mpsc::unbounded_channel();
            self.agent_state.agent_rx = Some(rx);
            self.agent_state.agent_tx = Some(tx.clone());
            let ui_tx = tx.clone();
            let safe_paths = SafePaths {
                config_dir: Config::config_dir(),
                shaders_dir: Config::shaders_dir(),
            };
            let mcp_server_bin =
                std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("par-term"));
            let agent = Agent::new(agent_config.clone(), tx, safe_paths, mcp_server_bin);
            agent.auto_approve.store(
                self.config.ai_inspector_auto_approve,
                std::sync::atomic::Ordering::Relaxed,
            );
            let agent = Arc::new(tokio::sync::Mutex::new(agent));
            self.agent_state.agent = Some(agent.clone());

            // Determine CWD for the agent session
            let fallback_cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let cwd = if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_lock() {
                    term.shell_integration_cwd()
                        .unwrap_or_else(|| fallback_cwd.clone())
                } else {
                    fallback_cwd.clone()
                }
            } else {
                fallback_cwd
            };

            let capabilities = ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                    list_directory: true,
                    find: true,
                },
                terminal: self.config.ai_inspector_agent_terminal_access,
                config: true,
            };

            let auto_approve = self.config.ai_inspector_auto_approve;
            let runtime = self.runtime.clone();
            runtime.spawn(async move {
                let mut agent = agent.lock().await;
                if let Err(e) = agent.connect(&cwd, capabilities).await {
                    log::error!("ACP: failed to connect to agent: {e}");
                    return;
                }
                if let Some(client) = &agent.client {
                    let _ = ui_tx.send(AgentMessage::ClientReady(Arc::clone(client)));
                }
                if auto_approve && let Err(e) = agent.set_mode("bypassPermissions").await {
                    log::error!("ACP: failed to set bypassPermissions mode: {e}");
                }
            });
        }
    }

    /// Auto-connect to the configured agent if auto-launch is enabled and no agent is connected.
    pub(crate) fn try_auto_connect_agent(&mut self) {
        if self.config.ai_inspector_auto_launch
            && self.overlay_ui.ai_inspector.agent_status == AgentStatus::Disconnected
            && self.agent_state.agent.is_none()
        {
            let identity = self.config.ai_inspector_agent.clone();
            if !identity.is_empty() {
                log::info!("ACP: auto-connecting to agent '{}'", identity);
                self.connect_agent(&identity);
            }
        }
    }

    // Status Bar Inset Sync
    // ========================================================================

    /// Sync the status bar bottom inset with the renderer so that the terminal
    /// grid does not extend behind the status bar.
    ///
    /// Must be called before cells are gathered each frame so the grid size
    /// is correct. Only triggers a terminal resize when the height changes
    /// (e.g., status bar toggled on/off or height changed in settings).
    pub(crate) fn sync_status_bar_inset(&mut self) {
        let is_tmux = self.is_tmux_connected();
        let tmux_bar = crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux);
        let custom_bar = self.status_bar_ui.height(&self.config, self.is_fullscreen);
        let total = tmux_bar + custom_bar;

        if let Some(renderer) = &mut self.renderer
            && let Some((new_cols, new_rows)) = renderer.set_egui_bottom_inset(total)
        {
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (new_cols as f32 * cell_width) as usize;
            let height_px = (new_rows as f32 * cell_height) as usize;

            for tab in self.tab_manager.tabs_mut() {
                if let Ok(mut term) = tab.terminal.try_lock() {
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                }
                tab.cache.cells = None;
            }
        }
    }

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
                self.shader_state.shader_watcher = Some(watcher);
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
        self.shader_state.shader_watcher = None;
        self.shader_state.shader_reload_error = None;

        // Reinitialize if hot reload is still enabled
        self.init_shader_watcher();
    }

    /// Initialize the config file watcher for automatic reload.
    ///
    /// Watches `config.yaml` for changes so that when an ACP agent modifies
    /// the config, par-term can auto-reload shader and other settings.
    pub(crate) fn init_config_watcher(&mut self) {
        let config_path = Config::config_path();
        if !config_path.exists() {
            debug_info!("CONFIG", "Config file does not exist, skipping watcher");
            return;
        }
        match crate::config::watcher::ConfigWatcher::new(&config_path, 500) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Config watcher initialized");
                self.config_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!("CONFIG", "Failed to initialize config watcher: {}", e);
            }
        }
    }

    /// Initialize the watcher for `.config-update.json` (MCP server config updates).
    ///
    /// The MCP server (spawned by the ACP agent) writes config updates to this
    /// file. We watch it, apply the updates in-memory, and delete it.
    pub(crate) fn init_config_update_watcher(&mut self) {
        let update_path = Config::config_dir().join(".config-update.json");

        // Create the file if it doesn't exist so the watcher can start
        if !update_path.exists() {
            if let Some(parent) = update_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&update_path, "");
        }

        match crate::config::watcher::ConfigWatcher::new(&update_path, 200) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Config-update watcher initialized");
                self.config_update_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!(
                    "CONFIG",
                    "Failed to initialize config-update watcher: {}",
                    e
                );
            }
        }
    }

    /// Initialize the watcher for `.screenshot-request.json` (MCP screenshot tool).
    ///
    /// The MCP server writes screenshot requests to this file. We watch it,
    /// capture the current renderer output, write a response to
    /// `.screenshot-response.json`, and clear the request file.
    pub(crate) fn init_screenshot_request_watcher(&mut self) {
        let request_path = Config::config_dir().join(SCREENSHOT_REQUEST_FILENAME);

        if !request_path.exists() {
            if let Some(parent) = request_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&request_path, "");
        }

        let response_path = Config::config_dir().join(SCREENSHOT_RESPONSE_FILENAME);
        if !response_path.exists() {
            if let Some(parent) = response_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&response_path, "");
        }

        match crate::config::watcher::ConfigWatcher::new(&request_path, 100) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Screenshot-request watcher initialized");
                self.screenshot_request_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!(
                    "CONFIG",
                    "Failed to initialize screenshot-request watcher: {}",
                    e
                );
            }
        }
    }

    /// Check for pending config update file changes (from MCP server).
    ///
    /// When the MCP server writes `.config-update.json`, this reads it,
    /// applies the updates in-memory, saves to disk, and removes the file.
    pub(crate) fn check_config_update_file(&mut self) {
        let Some(watcher) = &self.config_update_watcher else {
            return;
        };
        if watcher.try_recv().is_none() {
            return;
        }

        let update_path = Config::config_dir().join(".config-update.json");
        let content = match std::fs::read_to_string(&update_path) {
            Ok(c) if c.trim().is_empty() => return,
            Ok(c) => c,
            Err(e) => {
                log::warn!("CONFIG: failed to read config-update file: {e}");
                return;
            }
        };

        match serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content)
        {
            Ok(updates) => {
                log::info!(
                    "CONFIG: applying MCP config update ({} keys): {:?}",
                    updates.len(),
                    updates
                );
                if let Err(e) = self.apply_agent_config_updates(&updates) {
                    log::error!("CONFIG: MCP config update failed: {e}");
                } else {
                    self.config_changed_by_agent = true;
                }
                self.needs_redraw = true;
            }
            Err(e) => {
                log::error!("CONFIG: invalid JSON in config-update file: {e}");
            }
        }

        // Clear the file so we don't re-process it
        let _ = std::fs::write(&update_path, "");
    }

    /// Check for pending screenshot request file changes (from MCP server).
    ///
    /// When the MCP server writes `.screenshot-request.json`, this captures the
    /// active terminal renderer output and writes a response to
    /// `.screenshot-response.json`.
    pub(crate) fn check_screenshot_request_file(&mut self) {
        let Some(watcher) = &self.screenshot_request_watcher else {
            return;
        };
        if watcher.try_recv().is_none() {
            return;
        }

        let request_path = Config::config_dir().join(SCREENSHOT_REQUEST_FILENAME);
        let response_path = Config::config_dir().join(SCREENSHOT_RESPONSE_FILENAME);

        let content = match std::fs::read_to_string(&request_path) {
            Ok(c) if c.trim().is_empty() => return,
            Ok(c) => c,
            Err(e) => {
                log::warn!("ACP screenshot: failed to read request file: {e}");
                return;
            }
        };

        let request = match serde_json::from_str::<TerminalScreenshotRequest>(&content) {
            Ok(req) => req,
            Err(e) => {
                log::error!("ACP screenshot: invalid JSON in request file: {e}");
                let _ = std::fs::write(&request_path, "");
                return;
            }
        };

        let response = match self.capture_terminal_screenshot_mcp_response(&request.request_id) {
            Ok(resp) => resp,
            Err(e) => TerminalScreenshotResponse {
                request_id: request.request_id.clone(),
                ok: false,
                error: Some(e),
                mime_type: None,
                data_base64: None,
                width: None,
                height: None,
            },
        };

        match serde_json::to_vec_pretty(&response) {
            Ok(bytes) => {
                let tmp = response_path.with_extension("json.tmp");
                if let Err(e) =
                    std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &response_path))
                {
                    let _ = std::fs::remove_file(&tmp);
                    log::error!(
                        "ACP screenshot: failed to write response {}: {}",
                        response_path.display(),
                        e
                    );
                }
            }
            Err(e) => {
                log::error!("ACP screenshot: failed to serialize response: {e}");
            }
        }

        // Clear request file so it is processed only once.
        let _ = std::fs::write(&request_path, "");
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
        let Some(watcher) = &self.shader_state.shader_watcher else {
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
                self.shader_state.shader_reload_error = Some(error_msg.clone());
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result =
                            Some(Some(error_msg.clone()));
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
                self.shader_state.shader_reload_error = None;
                // Track success for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result = Some(None);
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result = Some(None);
                    }
                }
                self.needs_redraw = true;
                self.request_redraw();
                true
            }
            Err(e) => {
                // Extract the most relevant error message from the chain
                let root_cause = e.to_string();
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

                self.shader_state.shader_reload_error = Some(error_msg.clone());
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result =
                            Some(Some(error_msg.clone()));
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
        // AI Inspector resize handle uses direct pointer tracking (not egui widgets),
        // so egui doesn't know about it. Check explicitly to prevent mouse events
        // from reaching the terminal during resize drag or initial click on the handle.
        if self.overlay_ui.ai_inspector.wants_pointer() {
            return true;
        }
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

    /// Canonical check: is any modal UI overlay visible?
    ///
    /// This is the single source of truth for "should input be blocked from the terminal
    /// because a modal dialog is open?" When adding a new modal panel, add it here.
    ///
    /// Note: Side panels (ai_inspector, profile drawer) and inline edit states
    /// (tab_bar_ui.is_renaming()) are NOT modals — they are checked separately
    /// at call sites that need them. The resize overlay is also not a modal.
    pub(crate) fn any_modal_ui_visible(&self) -> bool {
        self.overlay_ui.help_ui.visible
            || self.overlay_ui.clipboard_history_ui.visible
            || self.overlay_ui.command_history_ui.visible
            || self.overlay_ui.search_ui.visible
            || self.overlay_ui.tmux_session_picker_ui.visible
            || self.overlay_ui.shader_install_ui.visible
            || self.overlay_ui.integrations_ui.visible
            || self.overlay_ui.ssh_connect_ui.is_visible()
            || self.overlay_ui.remote_shell_install_ui.is_visible()
            || self.overlay_ui.quit_confirmation_ui.is_visible()
    }

    /// Check if any egui overlay with text input is visible.
    /// Used to route clipboard operations (paste/copy/select-all) to egui
    /// instead of the terminal when a modal dialog or the AI inspector is active.
    pub(crate) fn has_egui_text_overlay_visible(&self) -> bool {
        self.any_modal_ui_visible() || self.overlay_ui.ai_inspector.open
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        // Also check ai_inspector (side panel with text input) and tab rename (inline edit)
        let any_ui_visible = self.any_modal_ui_visible()
            || self.overlay_ui.ai_inspector.open
            || self.tab_bar_ui.is_renaming();
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
                self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
                return;
            }
        } else if self.config.lock_cursor_blink && !self.config.cursor_blink {
            // If blink is locked off, don't blink regardless of terminal style
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
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
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
            return;
        }

        let now = std::time::Instant::now();

        // If key was pressed recently (within 500ms), smoothly fade in cursor and reset blink timer
        if let Some(last_key) = self.cursor_anim.last_key_press
            && now.duration_since(last_key).as_millis() < 500
        {
            self.cursor_anim.cursor_opacity = (self.cursor_anim.cursor_opacity + 0.1).min(1.0);
            self.cursor_anim.last_cursor_blink = Some(now);
            return;
        }

        // Smooth cursor blink animation using sine wave for natural fade
        let blink_interval = std::time::Duration::from_millis(self.config.cursor_blink_interval);

        if let Some(last_blink) = self.cursor_anim.last_cursor_blink {
            let elapsed = now.duration_since(last_blink);
            let progress = (elapsed.as_millis() as f32) / (blink_interval.as_millis() as f32);

            // Use cosine wave for smooth fade in/out (starts at 1.0, fades to 0.0, back to 1.0)
            self.cursor_anim.cursor_opacity = ((progress * std::f32::consts::PI).cos())
                .abs()
                .clamp(0.0, 1.0);

            // Reset timer after full cycle (2x interval for full on+off)
            if elapsed >= blink_interval * 2 {
                self.cursor_anim.last_cursor_blink = Some(now);
            }
        } else {
            // First time, start the blink timer with cursor fully visible
            self.cursor_anim.cursor_opacity = 1.0;
            self.cursor_anim.last_cursor_blink = Some(now);
        }
    }
    /// Handle tab bar actions collected during egui rendering (called after renderer borrow released).
    fn handle_tab_bar_action_after_render(&mut self, action: crate::tab_bar_ui::TabBarAction) {
        // Handle tab bar actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
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
                // Switch to the tab first so close_current_tab() operates on it.
                // This routes through the full close path: running-jobs confirmation,
                // session undo capture, and preserve-shell logic.
                self.tab_manager.switch_to(id);
                let was_last = self.close_current_tab();
                if was_last {
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
            TabBarAction::NewTabWithProfile(profile_id) => {
                self.open_profile(profile_id);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::RenameTab(id, name) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    if name.is_empty() {
                        // Blank name: revert to auto title mode
                        tab.user_named = false;
                        tab.has_default_title = true;
                        // Trigger immediate title update
                        tab.update_title(self.config.tab_title_mode);
                    } else {
                        tab.title = name;
                        tab.user_named = true;
                        tab.has_default_title = false;
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::Duplicate(id) => {
                self.duplicate_tab_by_id(id);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::ToggleAssistantPanel => {
                let just_opened = self.overlay_ui.ai_inspector.toggle();
                self.sync_ai_inspector_width();
                if just_opened {
                    self.try_auto_connect_agent();
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::SetTabIcon(tab_id, icon) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.custom_icon = icon;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::None => {}
        }
    }

    /// Handle clipboard history actions collected during egui rendering.
    fn handle_clipboard_history_action_after_render(
        &mut self,
        action: crate::clipboard_history_ui::ClipboardHistoryAction,
    ) {
        // Handle clipboard actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
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
                self.overlay_ui
                    .clipboard_history_ui
                    .update_entries(Vec::new());
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
    }

    /// Handle AI Inspector panel actions collected during egui rendering.
    fn handle_inspector_action_after_render(
        &mut self,
        action: crate::ai_inspector::panel::InspectorAction,
    ) {
        // Handle AI Inspector actions collected during egui rendering
        match action {
            InspectorAction::Close => {
                self.overlay_ui.ai_inspector.open = false;
                self.sync_ai_inspector_width();
            }
            InspectorAction::CopyJson(json) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(json);
                }
            }
            InspectorAction::SaveToFile(json) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(format!(
                        "par-term-snapshot-{}.json",
                        chrono::Local::now().format("%Y-%m-%d-%H%M%S")
                    ))
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    let _ = std::fs::write(path, json);
                }
            }
            InspectorAction::WriteToTerminal(cmd) => {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    let _ = term.write(cmd.as_bytes());
                }
            }
            InspectorAction::RunCommandAndNotify(cmd) => {
                // Write command + Enter to terminal
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    let _ = term.write(format!("{cmd}\n").as_bytes());
                }
                // Record command count before execution so we can detect completion
                let history_len = self
                    .tab_manager
                    .active_tab()
                    .and_then(|tab| tab.terminal.try_lock().ok())
                    .map(|term| term.core_command_history().len())
                    .unwrap_or(0);
                // Spawn a task that polls for command completion and notifies the agent
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    let tx = self.agent_state.agent_tx.clone();
                    let terminal = self
                        .tab_manager
                        .active_tab()
                        .map(|tab| tab.terminal.clone());
                    let cmd_for_msg = cmd.clone();
                    self.runtime.spawn(async move {
                        // Poll for command completion (up to 30 seconds)
                        let mut exit_code: Option<i32> = None;
                        for _ in 0..300 {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            if let Some(ref terminal) = terminal
                                && let Ok(term) = terminal.try_lock()
                            {
                                let history = term.core_command_history();
                                if history.len() > history_len {
                                    // New command finished
                                    if let Some(last) = history.last() {
                                        exit_code = last.1;
                                    }
                                    break;
                                }
                            }
                        }
                        // Send feedback to agent
                        let exit_str = exit_code
                            .map(|c| format!("exit code {c}"))
                            .unwrap_or_else(|| "unknown exit code".to_string());
                        let feedback = format!(
                            "[System: The user executed `{cmd_for_msg}` in their terminal ({exit_str}). \
                             The output is available through the normal terminal capture.]"
                        );
                        let content = vec![par_term_acp::ContentBlock::Text {
                            text: feedback,
                        }];
                        let agent = agent.lock().await;
                        let _ = agent.send_prompt(content).await;
                        if let Some(tx) = tx {
                            let _ = tx.send(par_term_acp::AgentMessage::PromptComplete);
                        }
                    });
                }
                self.needs_redraw = true;
            }
            InspectorAction::ConnectAgent(identity) => {
                self.connect_agent(&identity);
            }
            InspectorAction::DisconnectAgent => {
                if let Some(agent) = self.agent_state.agent.take() {
                    self.runtime.spawn(async move {
                        let mut agent = agent.lock().await;
                        agent.disconnect().await;
                    });
                }
                self.agent_state.agent_rx = None;
                self.agent_state.agent_tx = None;
                self.agent_state.agent_client = None;
                self.overlay_ui.ai_inspector.connected_agent_name = None;
                self.overlay_ui.ai_inspector.connected_agent_identity = None;
                // Abort any queued send tasks.
                for handle in self.agent_state.pending_send_handles.drain(..) {
                    handle.abort();
                }
                self.overlay_ui.ai_inspector.agent_status = AgentStatus::Disconnected;
                self.agent_state.pending_agent_context_replay = None;
                self.needs_redraw = true;
            }
            InspectorAction::RevokeAlwaysAllowSelections => {
                if let Some(identity) = self
                    .overlay_ui
                    .ai_inspector
                    .connected_agent_identity
                    .clone()
                {
                    // Cancel any queued prompts before replacing the session.
                    for handle in self.agent_state.pending_send_handles.drain(..) {
                        handle.abort();
                    }
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Resetting agent session to revoke all \"Always allow\" permissions. Local chat context will be replayed on your next prompt (best effort)."
                            .to_string(),
                    );
                    self.connect_agent(&identity);
                } else {
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Cannot reset permissions: no connected agent identity.".to_string(),
                    );
                }
                self.needs_redraw = true;
            }
            InspectorAction::SendPrompt(text) => {
                // Reset one-shot local backend recovery for each user prompt.
                self.agent_state.agent_skill_failure_detected = false;
                self.agent_state.agent_skill_recovery_attempts = 0;
                self.overlay_ui
                    .ai_inspector
                    .chat
                    .add_user_message(text.clone());
                self.overlay_ui.ai_inspector.chat.streaming = true;
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    // Build structured prompt blocks so system/context/user roles
                    // stay explicit and stable on every turn.
                    let mut content: Vec<par_term_acp::ContentBlock> =
                        vec![par_term_acp::ContentBlock::Text {
                            text: format!(
                                "{}[End system instructions]",
                                crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE
                            ),
                        }];

                    // Inject shader context when relevant (keyword match or active shaders).
                    if crate::ai_inspector::shader_context::should_inject_shader_context(
                        &text,
                        &self.config,
                    ) {
                        content.push(par_term_acp::ContentBlock::Text {
                            text: crate::ai_inspector::shader_context::build_shader_context(
                                &self.config,
                            ),
                        });
                    }

                    if let Some(replay_prompt) =
                        self.agent_state.pending_agent_context_replay.take()
                    {
                        content.push(par_term_acp::ContentBlock::Text {
                            text: replay_prompt,
                        });
                    }

                    content.push(par_term_acp::ContentBlock::Text {
                        text: format!("[User message]\n{text}"),
                    });
                    let tx = self.agent_state.agent_tx.clone();
                    let handle = self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        // Ensure each user prompt starts in executable mode even if
                        // a previous response switched the session to plan mode.
                        if let Err(e) = agent.set_mode("default").await {
                            log::warn!("ACP: failed to pre-set default mode before prompt: {e}");
                        }
                        // Signal that we've acquired the lock — the prompt
                        // is no longer cancellable.
                        if let Some(ref tx) = tx {
                            let _ = tx.send(AgentMessage::PromptStarted);
                        }
                        let _ = agent.send_prompt(content).await;
                        // Signal the UI to flush the agent text buffer so
                        // command suggestions are extracted.
                        if let Some(tx) = tx {
                            let _ = tx.send(AgentMessage::PromptComplete);
                        }
                    });
                    self.agent_state.pending_send_handles.push_back(handle);
                }
                self.needs_redraw = true;
            }
            InspectorAction::SetTerminalAccess(enabled) => {
                self.config.ai_inspector_agent_terminal_access = enabled;
                self.needs_redraw = true;
            }
            InspectorAction::RespondPermission {
                request_id,
                option_id,
                cancelled,
            } => {
                if let Some(client) = &self.agent_state.agent_client {
                    let client = client.clone();
                    let action = if cancelled { "cancelled" } else { "selected" };
                    log::info!("ACP: sending permission response id={request_id} action={action}");
                    self.runtime.spawn(async move {
                        use par_term_acp::{PermissionOutcome, RequestPermissionResponse};
                        let outcome = if cancelled {
                            PermissionOutcome {
                                outcome: "cancelled".to_string(),
                                option_id: None,
                            }
                        } else {
                            PermissionOutcome {
                                outcome: "selected".to_string(),
                                option_id: Some(option_id),
                            }
                        };
                        let result = RequestPermissionResponse { outcome };
                        if let Err(e) = client
                            .respond(
                                request_id,
                                Some(serde_json::to_value(&result).expect("window_state: RequestPermissionResponse must be serializable to JSON")),
                                None,
                            )
                            .await
                        {
                            log::error!("ACP: failed to send permission response: {e}");
                        }
                    });
                } else {
                    log::error!(
                        "ACP: cannot send permission response id={request_id} — agent_client is None!"
                    );
                }
                // Mark the permission as resolved in the chat.
                for msg in &mut self.overlay_ui.ai_inspector.chat.messages {
                    if let ChatMessage::Permission {
                        request_id: rid,
                        resolved,
                        ..
                    } = msg
                        && *rid == request_id
                    {
                        *resolved = true;
                        break;
                    }
                }
                self.needs_redraw = true;
            }
            InspectorAction::SetAgentMode(mode_id) => {
                let is_yolo = mode_id == "bypassPermissions";
                self.config.ai_inspector_auto_approve = is_yolo;
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        agent
                            .auto_approve
                            .store(is_yolo, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) = agent.set_mode(&mode_id).await {
                            log::error!("ACP: failed to set mode '{mode_id}': {e}");
                        }
                    });
                }
                self.needs_redraw = true;
            }
            InspectorAction::CancelPrompt => {
                if let Some(agent) = &self.agent_state.agent {
                    let agent = agent.clone();
                    self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        if let Err(e) = agent.cancel().await {
                            log::error!("ACP: failed to cancel prompt: {e}");
                        }
                    });
                }
                self.overlay_ui.ai_inspector.chat.flush_agent_message();
                self.overlay_ui
                    .ai_inspector
                    .chat
                    .add_system_message("Cancelled.".to_string());
                self.needs_redraw = true;
            }
            InspectorAction::CancelQueuedPrompt => {
                if self.overlay_ui.ai_inspector.chat.cancel_last_pending() {
                    // Abort the most recent queued send task.
                    if let Some(handle) = self.agent_state.pending_send_handles.pop_back() {
                        handle.abort();
                    }
                    self.overlay_ui
                        .ai_inspector
                        .chat
                        .add_system_message("Queued message cancelled.".to_string());
                }
                self.needs_redraw = true;
            }
            InspectorAction::ClearChat => {
                let reconnect_identity = self
                    .overlay_ui
                    .ai_inspector
                    .connected_agent_identity
                    .clone();
                self.overlay_ui.ai_inspector.chat.clear();
                self.agent_state.pending_agent_context_replay = None;
                self.agent_state.agent_skill_failure_detected = false;
                self.agent_state.agent_skill_recovery_attempts = 0;
                // Abort any queued send tasks so stale prompts do not continue
                // after the conversation/session reset.
                for handle in self.agent_state.pending_send_handles.drain(..) {
                    handle.abort();
                }
                if let Some(identity) = reconnect_identity
                    && (self.agent_state.agent.is_some()
                        || self.overlay_ui.ai_inspector.agent_status != AgentStatus::Disconnected)
                {
                    self.connect_agent(&identity);
                    self.overlay_ui.ai_inspector.chat.add_system_message(
                        "Conversation cleared. Reconnected agent to reset session state."
                            .to_string(),
                    );
                }
                self.needs_redraw = true;
            }
            InspectorAction::None => {}
        }
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
            install_shaders = self.overlay_ui.integrations_ui.pending_install_shaders;
            install_shell_integration = self
                .overlay_ui
                .integrations_ui
                .pending_install_shell_integration;

            match action {
                crate::integrations_ui::ShaderConflictAction::Overwrite => {
                    force_overwrite_modified_shaders = true;
                }
                crate::integrations_ui::ShaderConflictAction::SkipModified => {
                    force_overwrite_modified_shaders = false;
                }
                crate::integrations_ui::ShaderConflictAction::Cancel => {
                    // Reset pending state and exit without installing
                    self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
                    self.overlay_ui.integrations_ui.shader_conflicts.clear();
                    self.overlay_ui.integrations_ui.pending_install_shaders = false;
                    self.overlay_ui
                        .integrations_ui
                        .pending_install_shell_integration = false;
                    self.overlay_ui.integrations_ui.error_message = None;
                    self.overlay_ui.integrations_ui.success_message = None;
                    self.needs_redraw = true;
                    return;
                }
            }

            // Clear the conflict prompt regardless of choice
            self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
            self.overlay_ui.integrations_ui.shader_conflicts.clear();
            self.overlay_ui.integrations_ui.error_message = None;
            self.overlay_ui.integrations_ui.success_message = None;
            self.overlay_ui.integrations_ui.installing = false;
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
                        self.overlay_ui.integrations_ui.awaiting_shader_overwrite = true;
                        self.overlay_ui.integrations_ui.shader_conflicts = conflicts;
                        self.overlay_ui.integrations_ui.pending_install_shaders = install_shaders;
                        self.overlay_ui
                            .integrations_ui
                            .pending_install_shell_integration = install_shell_integration;
                        self.overlay_ui.integrations_ui.installing = false;
                        self.overlay_ui.integrations_ui.error_message = None;
                        self.overlay_ui.integrations_ui.success_message = None;
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
                self.overlay_ui
                    .integrations_ui
                    .set_installing("Installing shaders...");
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
                self.overlay_ui
                    .integrations_ui
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
                self.overlay_ui
                    .integrations_ui
                    .set_success(&format!("Installed: {}", success_parts.join(", ")));
            } else if success_parts.is_empty() {
                self.overlay_ui
                    .integrations_ui
                    .set_error(&format!("Installation failed: {}", error_parts.join("; ")));
            } else {
                // Partial success
                self.overlay_ui.integrations_ui.set_success(&format!(
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
            self.overlay_ui.integrations_ui.pending_install_shaders = false;
            self.overlay_ui
                .integrations_ui
                .pending_install_shell_integration = false;

            self.needs_redraw = true;
        }

        // Handle "Skip" - just close the dialog for this session
        if response.skipped {
            log::info!("User skipped integrations dialog for this session");
            self.overlay_ui.integrations_ui.hide();
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
            self.overlay_ui.integrations_ui.hide();
            // Set install prompts to Never
            self.config.shader_install_prompt = ShaderInstallPrompt::Never;
            self.config.shell_integration_state = crate::config::InstallPromptState::Never;
            if let Err(e) = self.config.save() {
                log::error!("Failed to save config after declining integrations: {}", e);
            }
        }

        // Handle dialog closed (OK button after success)
        if response.closed {
            self.overlay_ui.integrations_ui.hide();
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

// ---------------------------------------------------------------------------
impl Drop for WindowState {
    fn drop(&mut self) {
        let t0 = std::time::Instant::now();
        log::info!("Shutting down window (fast path)");

        // Signal status bar polling threads to stop immediately.
        // They check the flag every 50ms, so by the time the auto-drop
        // calls join() later, the threads will already be exiting.
        self.status_bar_ui.signal_shutdown();

        // Save command history on a background thread (serializes in-memory, writes async)
        self.overlay_ui.command_history.save_background();

        // Set shutdown flag
        self.is_shutting_down = true;

        // Hide the window immediately for instant visual feedback
        if let Some(ref window) = self.window {
            window.set_visible(false);
            log::info!(
                "Window hidden for instant visual close (+{:.1}ms)",
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }

        // Clean up egui state FIRST before any other resources are dropped
        self.egui_state = None;
        self.egui_ctx = None;

        // Drain all tabs from the manager (takes ownership without dropping)
        let mut tabs = self.tab_manager.drain_tabs();
        let tab_count = tabs.len();
        log::info!(
            "Fast shutdown: draining {} tabs (+{:.1}ms)",
            tab_count,
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Collect terminal Arcs and session loggers from all tabs and panes
        // BEFORE setting shutdown_fast. Cloning the Arc keeps TerminalManager
        // alive even after Tab/Pane is dropped. Session loggers are collected
        // so they can be stopped on a background thread instead of blocking.
        let mut terminal_arcs = Vec::new();
        let mut session_loggers = Vec::new();

        for tab in &mut tabs {
            // Stop refresh tasks (fast - just aborts tokio tasks)
            tab.stop_refresh_task();

            // Collect session logger for background stop
            session_loggers.push(Arc::clone(&tab.session_logger));

            // Clone terminal Arc before we mark shutdown_fast
            terminal_arcs.push(Arc::clone(&tab.terminal));

            // Also handle panes if this tab has splits
            if let Some(ref mut pm) = tab.pane_manager {
                for pane in pm.all_panes_mut() {
                    pane.stop_refresh_task();
                    session_loggers.push(Arc::clone(&pane.session_logger));
                    terminal_arcs.push(Arc::clone(&pane.terminal));
                    pane.shutdown_fast = true;
                }
            }

            // Mark tab for fast drop (skips sleep + kill in Tab::drop)
            tab.shutdown_fast = true;
        }

        // Pre-kill all PTY processes (sends SIGKILL, fast non-blocking)
        for arc in &terminal_arcs {
            if let Ok(mut term) = arc.try_lock()
                && term.is_running()
            {
                let _ = term.kill();
            }
        }
        log::info!(
            "Pre-killed {} terminal sessions (+{:.1}ms)",
            terminal_arcs.len(),
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Drop tabs on main thread (fast - Tab::drop just returns immediately)
        drop(tabs);
        log::info!(
            "Tabs dropped (+{:.1}ms)",
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Fire-and-forget: stop session loggers on a background thread.
        // Each logger.stop() flushes buffered I/O which can block.
        if !session_loggers.is_empty() {
            let _ = std::thread::Builder::new()
                .name("logger-cleanup".into())
                .spawn(move || {
                    for logger_arc in session_loggers {
                        if let Some(ref mut logger) = *logger_arc.lock() {
                            let _ = logger.stop();
                        }
                    }
                });
        }

        // Fire-and-forget: drop the cloned terminal Arcs on background threads.
        // When our clone is the last reference, TerminalManager::drop runs,
        // which triggers PtySession::drop (up to 2s reader thread wait).
        // By running these in parallel, all sessions clean up concurrently.
        // We intentionally do NOT join these threads — the process is exiting
        // and the OS will reclaim all resources.
        for (i, arc) in terminal_arcs.into_iter().enumerate() {
            let _ = std::thread::Builder::new()
                .name(format!("pty-cleanup-{}", i))
                .spawn(move || {
                    let t = std::time::Instant::now();
                    drop(arc);
                    log::info!(
                        "pty-cleanup-{} finished in {:.1}ms",
                        i,
                        t.elapsed().as_secs_f64() * 1000.0
                    );
                });
        }

        log::info!(
            "Window shutdown complete ({} tabs, main thread blocked {:.1}ms)",
            tab_count,
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}
