//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.

mod action_handlers;
mod agent_messages;
mod config_watchers;
mod render_pipeline;
mod renderer_ops;
mod shader_ops;

use crate::app::anti_idle::should_send_keep_alive;
use crate::app::debug_state::DebugState;
use crate::badge::BadgeState;
use crate::config::{Config, CustomAcpAgentConfig};
use crate::input::InputHandler;
use crate::keybindings::KeybindingRegistry;
use crate::renderer::Renderer;
use crate::smart_selection::SmartSelectionCache;
use crate::status_bar::StatusBarUI;
use crate::tab::TabManager;
use crate::tab_bar_ui::TabBarUI;
use anyhow::Result;
use par_term_acp::{
    Agent, AgentConfig, AgentMessage, AgentStatus, ClientCapabilities, FsCapabilities, SafePaths,
    discover_agents,
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

    // Trigger RunCommand process management
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: std::collections::HashMap<u32, std::time::Instant>,
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

/// Preprocess a Claude Code segment before detection.
///
/// 1. **Line-number stripping**: File previews show numbered lines like
///    `"       1 # Defect Report"`. We strip the prefix so detectors see
///    `"# Defect Report"` and can match ATX headers / fenced code.
/// 2. **UI chrome filtering**: Remove tool headers reconstructed as `## Write(`,
///    tree connectors (`└`, `├`), and other TUI elements that confuse detectors.
pub(super) fn preprocess_claude_code_segment(lines: &mut Vec<(String, usize)>) {
    use std::sync::LazyLock;

    /// Matches Claude Code line-number prefixes: leading whitespace + digits + space.
    /// Examples: "    1 ", "   10 ", "  100 "
    static LINE_NUMBER_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^\s+\d+\s").unwrap());

    /// Captures the line-number prefix for stripping.
    static LINE_NUMBER_STRIP_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^(\s+\d+) ").unwrap());

    if lines.is_empty() {
        return;
    }

    // Detect line-numbered content: if ≥50% of non-empty lines have a line-number
    // prefix, this is a file preview. Strip the prefix so detectors see raw content.
    let non_empty: Vec<&(String, usize)> =
        lines.iter().filter(|(l, _)| !l.trim().is_empty()).collect();
    if !non_empty.is_empty() {
        let numbered_count = non_empty
            .iter()
            .filter(|(l, _)| LINE_NUMBER_RE.is_match(l))
            .count();
        if numbered_count * 2 >= non_empty.len() {
            for (line, _) in lines.iter_mut() {
                if let Some(m) = LINE_NUMBER_STRIP_RE.find(line) {
                    *line = line[m.end()..].to_string();
                }
            }
        }
    }

    // Filter out Claude Code UI chrome lines that would confuse detectors:
    // - Tool headers reconstructed as ## headers: "## Write(", "## Bash(", "## Read("
    // - Result indicators with tree connectors: starts with "└"
    // - "Wrote N lines to..." result summaries
    lines.retain(|(line, _)| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true; // Keep blank lines (they serve as block separators)
        }
        // Skip tool headers that were reconstructed as markdown headers.
        if let Some(after_header) = trimmed.strip_prefix("## ")
            && (after_header.starts_with("Write(")
                || after_header.starts_with("Bash(")
                || after_header.starts_with("Read(")
                || after_header.starts_with("Glob(")
                || after_header.starts_with("Grep(")
                || after_header.starts_with("Edit(")
                || after_header.starts_with("Task(")
                || after_header.starts_with("Wrote ")
                || after_header.starts_with("Done"))
        {
            return false;
        }
        // Skip tree-connector result lines.
        if trimmed.starts_with('└') || trimmed.starts_with('├') {
            return false;
        }
        true
    });
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

            trigger_spawned_processes: std::collections::HashMap::new(),
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
        if self.config.should_prompt_integrations(crate::VERSION) {
            log::info!("Integrations not installed - showing welcome dialog");
            self.overlay_ui.integrations_ui.show_dialog();
            self.needs_redraw = true;
            window.request_redraw();
        }

        Ok(())
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
