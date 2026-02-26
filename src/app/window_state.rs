//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.

use crate::ai_inspector::chat::{
    ChatMessage, extract_inline_config_update, extract_inline_tool_function_name,
};
use crate::ai_inspector::panel::{AIInspectorPanel, InspectorAction};
use crate::app::anti_idle::should_send_keep_alive;
use crate::app::debug_state::DebugState;
use crate::badge::{BadgeState, render_badge};
use crate::cell_renderer::PaneViewport;
use crate::clipboard_history_ui::{ClipboardHistoryAction, ClipboardHistoryUI};
use crate::close_confirmation_ui::{CloseConfirmAction, CloseConfirmationUI};
use crate::command_history::CommandHistory;
use crate::command_history_ui::{CommandHistoryAction, CommandHistoryUI};
use crate::config::{
    Config, CursorStyle, CustomAcpAgentConfig, ShaderInstallPrompt, color_u8_to_f32,
    color_u8_to_f32_a,
};
use crate::help_ui::HelpUI;
use crate::input::InputHandler;
use crate::integrations_ui::{IntegrationsResponse, IntegrationsUI};
use crate::keybindings::KeybindingRegistry;
use crate::paste_special_ui::{PasteSpecialAction, PasteSpecialUI};
use crate::profile::{ProfileManager, storage as profile_storage};
use crate::profile_drawer_ui::{ProfileDrawerAction, ProfileDrawerUI};
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};
use crate::quit_confirmation_ui::{QuitConfirmAction, QuitConfirmationUI};
use crate::remote_shell_install_ui::{RemoteShellInstallAction, RemoteShellInstallUI};
use crate::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
};
use crate::scrollback_metadata::ScrollbackMark;
use crate::search::SearchUI;
use crate::selection::SelectionMode;
use crate::shader_install_ui::{ShaderInstallResponse, ShaderInstallUI};
use crate::shader_watcher::{ShaderReloadEvent, ShaderType, ShaderWatcher};
use crate::smart_selection::SmartSelectionCache;
use crate::ssh_connect_ui::{SshConnectAction, SshConnectUI};
use crate::status_bar::StatusBarUI;
use crate::tab::{TabId, TabManager};
use crate::tab_bar_ui::{TabBarAction, TabBarUI};
use crate::tmux::{TmuxSession, TmuxSync};
use crate::tmux_session_picker_ui::{SessionPickerAction, TmuxSessionPickerUI};
use crate::tmux_status_bar_ui::TmuxStatusBarUI;
use anyhow::Result;
use base64::Engine as _;
use par_term_acp::{
    Agent, AgentConfig, AgentMessage, AgentStatus, ClientCapabilities, FsCapabilities, SafePaths,
    discover_agents,
};
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use par_term_mcp::{
    SCREENSHOT_REQUEST_FILENAME, SCREENSHOT_RESPONSE_FILENAME, TerminalScreenshotRequest,
    TerminalScreenshotResponse,
};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use wgpu::SurfaceError;
use winit::dpi::PhysicalSize;
use winit::window::Window;

/// Renderer sizing info needed for split pane calculations
struct RendererSizing {
    size: PhysicalSize<u32>,
    content_offset_y: f32,
    content_offset_x: f32,
    content_inset_bottom: f32,
    content_inset_right: f32,
    cell_width: f32,
    cell_height: f32,
    padding: f32,
    status_bar_height: f32,
    scale_factor: f32,
}

/// Pane render data for split pane rendering
struct PaneRenderData {
    /// Viewport bounds and state for this pane
    viewport: PaneViewport,
    /// Cells to render (should match viewport grid size)
    cells: Vec<crate::cell_renderer::Cell>,
    /// Grid dimensions (cols, rows)
    grid_size: (usize, usize),
    /// Cursor position within this pane (col, row), or None if no cursor visible
    cursor_pos: Option<(usize, usize)>,
    /// Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    cursor_opacity: f32,
    /// Scrollback marks for this pane
    marks: Vec<ScrollbackMark>,
    /// Scrollback length for this pane (needed for separator mark mapping)
    scrollback_len: usize,
    /// Current scroll offset for this pane (needed for separator mark mapping)
    scroll_offset: usize,
    /// Per-pane background image override (None = use global background)
    background: Option<crate::pane::PaneBackground>,
    /// Inline graphics (Sixel/iTerm2/Kitty) to render for this pane
    graphics: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
}

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
    /// tmux status bar UI
    pub(crate) tmux_status_bar_ui: TmuxStatusBarUI,
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
    /// AI Inspector side panel
    pub(crate) ai_inspector: AIInspectorPanel,
    /// Last known AI Inspector panel consumed width (logical pixels).
    /// Used to detect width changes from drag-resizing and trigger terminal reflow.
    pub(crate) last_inspector_width: f32,
    /// ACP agent connection and runtime state
    pub(crate) agent_state: crate::app::agent_state::AgentState,
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
    /// Remote shell integration install dialog UI
    pub(crate) remote_shell_install_ui: RemoteShellInstallUI,
    /// SSH Quick Connect dialog UI
    pub(crate) ssh_connect_ui: SshConnectUI,
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
    /// Profile manager for storing and managing terminal profiles
    pub(crate) profile_manager: ProfileManager,
    /// Profile drawer UI (collapsible side panel)
    pub(crate) profile_drawer_ui: ProfileDrawerUI,
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

/// Extract an `f32` from a JSON value that may be an integer or float.
fn json_as_f32(value: &serde_json::Value) -> Result<f32, String> {
    if let Some(f) = value.as_f64() {
        Ok(f as f32)
    } else if let Some(i) = value.as_i64() {
        Ok(i as f32)
    } else {
        Err("expected number".to_string())
    }
}

const AUTO_CONTEXT_MIN_INTERVAL_MS: u64 = 1200;
const AUTO_CONTEXT_MAX_COMMAND_LEN: usize = 400;

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "pass",
        "password",
        "token",
        "secret",
        "key",
        "apikey",
        "api_key",
        "auth",
        "credential",
        "session",
        "cookie",
    ];
    MARKERS.iter().any(|marker| key.contains(marker))
}

fn redact_auto_context_command(command: &str) -> (String, bool) {
    let mut redacted = false;
    let mut redact_next = false;
    let mut out: Vec<String> = Vec::new();

    for token in command.split_whitespace() {
        if redact_next {
            out.push("[REDACTED]".to_string());
            redacted = true;
            redact_next = false;
            continue;
        }

        let cleaned = token.trim_matches(|c| c == '"' || c == '\'');

        if let Some(flag) = cleaned.strip_prefix("--") {
            if let Some((name, _value)) = flag.split_once('=')
                && is_sensitive_key(name)
            {
                let prefix = token.split_once('=').map(|(p, _)| p).unwrap_or(token);
                out.push(format!("{prefix}=[REDACTED]"));
                redacted = true;
                continue;
            }
            if is_sensitive_key(flag) {
                out.push(token.to_string());
                redact_next = true;
                continue;
            }
        }

        if let Some((name, _value)) = cleaned.split_once('=')
            && is_sensitive_key(name)
        {
            let prefix = token.split_once('=').map(|(p, _)| p).unwrap_or(token);
            out.push(format!("{prefix}=[REDACTED]"));
            redacted = true;
            continue;
        }

        out.push(token.to_string());
    }

    let mut sanitized = out.join(" ");
    if sanitized.chars().count() > AUTO_CONTEXT_MAX_COMMAND_LEN {
        sanitized = sanitized
            .chars()
            .take(AUTO_CONTEXT_MAX_COMMAND_LEN)
            .collect();
        sanitized.push_str("...[truncated]");
    }
    (sanitized, redacted)
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

fn is_terminal_screenshot_permission_tool(tool_call: &serde_json::Value) -> bool {
    let tool_name = tool_call
        .get("kind")
        .and_then(|v| v.as_str())
        .or_else(|| tool_call.get("name").and_then(|v| v.as_str()))
        .or_else(|| tool_call.get("toolName").and_then(|v| v.as_str()))
        .or_else(|| {
            tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .and_then(|t| t.split_whitespace().next())
        })
        .unwrap_or("");
    let lower = tool_name.to_ascii_lowercase();
    lower == "terminal_screenshot" || lower.contains("par-term-config__terminal_screenshot")
}

/// Reconstruct markdown syntax from cell attributes for Claude Code output.
///
/// Claude Code pre-renders markdown with ANSI sequences (bold for headers/emphasis,
/// italic for emphasis), stripping the original syntax markers. This function
/// reconstructs markdown syntax from cell attributes so the prettifier's markdown
/// detector can recognize patterns like `# Header` and `**bold**`.
fn reconstruct_markdown_from_cells(cells: &[par_term_config::Cell]) -> String {
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

        // Load profiles from disk
        let profile_manager = match profile_storage::load_profiles() {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to load profiles: {}", e);
                ProfileManager::new()
            }
        };

        // Create badge state and AI inspector before moving config
        let badge_state = BadgeState::new(&config);
        let ai_inspector = AIInspectorPanel::new(&config);

        // Discover available ACP agents
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        let available_agents =
            merge_custom_ai_inspector_agents(discovered_agents, &config.ai_inspector_custom_agents);
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
            status_bar_ui: StatusBarUI::new(),

            debug: DebugState::new(),

            cursor_anim: crate::app::cursor_anim_state::CursorAnimState::default(),
            is_fullscreen: false,
            egui_ctx: None,
            egui_state: None,
            pending_egui_events: Vec::new(),
            egui_initialized: false,
            shader_state: crate::app::shader_state::ShaderState::new(shaders_dir),
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
            ai_inspector,
            last_inspector_width: 0.0,
            agent_state: crate::app::agent_state::AgentState::new(available_agents),
            shader_install_ui: ShaderInstallUI::new(),
            shader_install_receiver: None,
            integrations_ui: IntegrationsUI::new(),
            close_confirmation_ui: CloseConfirmationUI::new(),
            quit_confirmation_ui: QuitConfirmationUI::new(),
            remote_shell_install_ui: RemoteShellInstallUI::new(),
            ssh_connect_ui: SshConnectUI::new(),
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

            profile_manager,
            profile_drawer_ui: ProfileDrawerUI::new(),
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
        let cursor_metadata = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name).cloned());
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
        self.last_inspector_width = 0.0;
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
        let cursor_metadata = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name).cloned());
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
        if self.ai_inspector.open {
            self.try_auto_connect_agent();
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
        let current_width = self.ai_inspector.consumed_width();

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
            } else if (current_width - self.last_inspector_width).abs() >= 1.0 {
                // Logical width changed but physical grid didn't resize
                // (could happen with very small changes below cell width threshold)
                self.needs_redraw = true;
            }
        }

        // Persist panel width to config when the user finishes resizing.
        if !self.ai_inspector.is_resizing()
            && (current_width - self.last_inspector_width).abs() >= 1.0
            && current_width > 0.0
            && self.ai_inspector.open
        {
            self.config.ai_inspector_width = self.ai_inspector.width;
            // Save to disk so the width is remembered across sessions.
            if let Err(e) = self.config.save() {
                log::error!("Failed to save AI inspector width: {}", e);
            }
        }

        self.last_inspector_width = current_width;
    }

    /// Connect to an ACP agent by identity string.
    ///
    /// This extracts the agent connection logic so it can be called both from
    /// `InspectorAction::ConnectAgent` and from the auto-connect-on-open path.
    pub(crate) fn connect_agent(&mut self, identity: &str) {
        if let Some(agent_config) = self
            .agent_state.available_agents
            .iter()
            .find(|a| a.identity == identity)
        {
            self.agent_state.pending_agent_context_replay =
                self.ai_inspector.chat.build_context_replay_prompt();
            self.ai_inspector.connected_agent_name = Some(agent_config.name.clone());
            self.ai_inspector.connected_agent_identity = Some(agent_config.identity.clone());

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
            && self.ai_inspector.agent_status == AgentStatus::Disconnected
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

    fn capture_terminal_screenshot_mcp_response(
        &mut self,
        request_id: &str,
    ) -> Result<TerminalScreenshotResponse, String> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| "No renderer available for screenshot".to_string())?;

        let image = renderer
            .take_screenshot()
            .map_err(|e| format!("Renderer screenshot failed: {e}"))?;
        let width = image.width();
        let height = image.height();

        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {e}"))?;
        let png_bytes = buf.into_inner();
        let data_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

        Ok(TerminalScreenshotResponse {
            request_id: request_id.to_string(),
            ok: true,
            error: None,
            mime_type: Some("image/png".to_string()),
            data_base64: Some(data_base64),
            width: Some(width),
            height: Some(height),
        })
    }

    /// Check for pending config file changes and apply them.
    ///
    /// Called periodically from the event loop. On config change:
    /// 1. Reloads config from disk
    /// 2. Applies shader-related config changes
    /// 3. Reinitializes shader watcher if shader paths changed
    pub(crate) fn check_config_reload(&mut self) {
        let Some(watcher) = &self.config_watcher else {
            return;
        };
        let Some(_event) = watcher.try_recv() else {
            return;
        };

        log::info!("CONFIG: config file changed, reloading...");

        match Config::load() {
            Ok(new_config) => {
                use crate::app::config_updates::ConfigChanges;

                let changes = ConfigChanges::detect(&self.config, &new_config);

                // Replace the entire in-memory config so that any subsequent
                // config.save() writes the agent's changes, not stale values.
                self.config = new_config;

                log::info!(
                    "CONFIG: shader_changed={} cursor_changed={} shader={:?}",
                    changes.any_shader_change(),
                    changes.any_cursor_shader_toggle(),
                    self.config.custom_shader
                );

                // Apply shader changes to the renderer
                if let Some(renderer) = &mut self.renderer {
                    if changes.any_shader_change() || changes.shader_per_shader_config {
                        log::info!("CONFIG: applying background shader change to renderer");
                        let shader_override = self
                            .config
                            .custom_shader
                            .as_ref()
                            .and_then(|name| self.config.shader_configs.get(name));
                        let metadata = self
                            .config
                            .custom_shader
                            .as_ref()
                            .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
                        let resolved = crate::config::shader_config::resolve_shader_config(
                            shader_override,
                            metadata.as_ref(),
                            &self.config,
                        );
                        if let Err(e) = renderer.set_custom_shader_enabled(
                            self.config.custom_shader_enabled,
                            self.config.custom_shader.as_deref(),
                            self.config.window_opacity,
                            self.config.custom_shader_animation,
                            resolved.animation_speed,
                            resolved.full_content,
                            resolved.brightness,
                            &resolved.channel_paths(),
                            resolved.cubemap_path().map(|p| p.as_path()),
                        ) {
                            log::error!("Config reload: shader load failed: {e}");
                        }
                    }
                    if changes.any_cursor_shader_toggle() {
                        log::info!("CONFIG: applying cursor shader change to renderer");
                        if let Err(e) = renderer.set_cursor_shader_enabled(
                            self.config.cursor_shader_enabled,
                            self.config.cursor_shader.as_deref(),
                            self.config.window_opacity,
                            self.config.cursor_shader_animation,
                            self.config.cursor_shader_animation_speed,
                        ) {
                            log::error!("Config reload: cursor shader load failed: {e}");
                        }
                    }
                }

                // Reinit shader watcher if paths changed
                if changes.needs_watcher_reinit() {
                    self.reinit_shader_watcher();
                }

                // Rebuild prettifier pipelines if prettifier config changed.
                if changes.prettifier_changed {
                    for tab in self.tab_manager.tabs_mut() {
                        tab.prettifier =
                            crate::prettifier::config_bridge::create_pipeline_from_config(
                                &self.config,
                                self.config.cols,
                                None,
                            );
                    }
                }

                self.needs_redraw = true;
                debug_info!("CONFIG", "Config reloaded successfully");
            }
            Err(e) => {
                log::error!("Failed to reload config: {}", e);
            }
        }
    }

    /// Apply config updates from the ACP agent.
    ///
    /// Updates the in-memory config, applies changes to the renderer, and
    /// saves to disk. Returns `Ok(())` on success or an error string.
    fn apply_agent_config_updates(
        &mut self,
        updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        let old_config = self.config.clone();

        for (key, value) in updates {
            if let Err(e) = self.apply_single_config_update(key, value) {
                errors.push(format!("{key}: {e}"));
            }
        }

        if !errors.is_empty() {
            return Err(errors.join("; "));
        }

        // Detect changes and apply to renderer
        use crate::app::config_updates::ConfigChanges;
        let changes = ConfigChanges::detect(&old_config, &self.config);

        log::info!(
            "ACP config/update: shader_change={} cursor_change={} old_shader={:?} new_shader={:?}",
            changes.any_shader_change(),
            changes.any_cursor_shader_toggle(),
            old_config.custom_shader,
            self.config.custom_shader
        );

        if let Some(renderer) = &mut self.renderer {
            if changes.any_shader_change() || changes.shader_per_shader_config {
                log::info!("ACP config/update: applying background shader change to renderer");
                let shader_override = self
                    .config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| self.config.shader_configs.get(name));
                let metadata = self
                    .config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());
                let resolved = crate::config::shader_config::resolve_shader_config(
                    shader_override,
                    metadata.as_ref(),
                    &self.config,
                );
                if let Err(e) = renderer.set_custom_shader_enabled(
                    self.config.custom_shader_enabled,
                    self.config.custom_shader.as_deref(),
                    self.config.window_opacity,
                    self.config.custom_shader_animation,
                    resolved.animation_speed,
                    resolved.full_content,
                    resolved.brightness,
                    &resolved.channel_paths(),
                    resolved.cubemap_path().map(|p| p.as_path()),
                ) {
                    log::error!("ACP config/update: shader load failed: {e}");
                }
            }
            if changes.any_cursor_shader_toggle() {
                log::info!("ACP config/update: applying cursor shader change to renderer");
                if let Err(e) = renderer.set_cursor_shader_enabled(
                    self.config.cursor_shader_enabled,
                    self.config.cursor_shader.as_deref(),
                    self.config.window_opacity,
                    self.config.cursor_shader_animation,
                    self.config.cursor_shader_animation_speed,
                ) {
                    log::error!("ACP config/update: cursor shader load failed: {e}");
                }
            }
        }

        if changes.needs_watcher_reinit() {
            self.reinit_shader_watcher();
        }

        // Rebuild prettifier pipelines if prettifier config changed.
        if changes.prettifier_changed {
            for tab in self.tab_manager.tabs_mut() {
                tab.prettifier = crate::prettifier::config_bridge::create_pipeline_from_config(
                    &self.config,
                    self.config.cols,
                    None,
                );
            }
        }

        // Save to disk
        if let Err(e) = self.config.save() {
            return Err(format!("Failed to save config: {e}"));
        }

        Ok(())
    }

    /// Apply a single config key/value update to the in-memory config.
    fn apply_single_config_update(
        &mut self,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        match key {
            // -- Background shader --
            "custom_shader" => {
                self.config.custom_shader = if value.is_null() {
                    None
                } else {
                    Some(value.as_str().ok_or("expected string or null")?.to_string())
                };
                Ok(())
            }
            "custom_shader_enabled" => {
                self.config.custom_shader_enabled = value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }
            "custom_shader_animation" => {
                self.config.custom_shader_animation = value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }
            "custom_shader_animation_speed" => {
                self.config.custom_shader_animation_speed = json_as_f32(value)?;
                Ok(())
            }
            "custom_shader_brightness" => {
                self.config.custom_shader_brightness = json_as_f32(value)?;
                Ok(())
            }
            "custom_shader_text_opacity" => {
                self.config.custom_shader_text_opacity = json_as_f32(value)?;
                Ok(())
            }
            "custom_shader_full_content" => {
                self.config.custom_shader_full_content =
                    value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }

            // -- Cursor shader --
            "cursor_shader" => {
                self.config.cursor_shader = if value.is_null() {
                    None
                } else {
                    Some(value.as_str().ok_or("expected string or null")?.to_string())
                };
                Ok(())
            }
            "cursor_shader_enabled" => {
                self.config.cursor_shader_enabled = value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }
            "cursor_shader_animation" => {
                self.config.cursor_shader_animation = value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }
            "cursor_shader_animation_speed" => {
                self.config.cursor_shader_animation_speed = json_as_f32(value)?;
                Ok(())
            }
            "cursor_shader_glow_radius" => {
                self.config.cursor_shader_glow_radius = json_as_f32(value)?;
                Ok(())
            }
            "cursor_shader_glow_intensity" => {
                self.config.cursor_shader_glow_intensity = json_as_f32(value)?;
                Ok(())
            }
            "cursor_shader_trail_duration" => {
                self.config.cursor_shader_trail_duration = json_as_f32(value)?;
                Ok(())
            }
            "cursor_shader_hides_cursor" => {
                self.config.cursor_shader_hides_cursor =
                    value.as_bool().ok_or("expected boolean")?;
                Ok(())
            }

            // -- Window --
            "window_opacity" => {
                self.config.window_opacity = json_as_f32(value)?;
                Ok(())
            }
            "font_size" => {
                self.config.font_size = json_as_f32(value)?;
                Ok(())
            }

            _ => Err(format!("unknown or read-only config key: {key}")),
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
                        self.shader_state.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result = Some(Some(error_msg.clone()));
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
                        self.shader_state.background_shader_reload_result = Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result = Some(Some(error_msg.clone()));
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
        if self.ai_inspector.wants_pointer() {
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
        self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.command_history_ui.visible
            || self.search_ui.visible
            || self.tmux_session_picker_ui.visible
            || self.shader_install_ui.visible
            || self.integrations_ui.visible
            || self.ssh_connect_ui.is_visible()
            || self.remote_shell_install_ui.is_visible()
            || self.quit_confirmation_ui.is_visible()
    }

    /// Check if any egui overlay with text input is visible.
    /// Used to route clipboard operations (paste/copy/select-all) to egui
    /// instead of the terminal when a modal dialog or the AI inspector is active.
    pub(crate) fn has_egui_text_overlay_visible(&self) -> bool {
        self.any_modal_ui_visible() || self.ai_inspector.open
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        // Also check ai_inspector (side panel with text input) and tab rename (inline edit)
        let any_ui_visible =
            self.any_modal_ui_visible() || self.ai_inspector.open || self.tab_bar_ui.is_renaming();
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
            self.debug.frame_times.push_back(frame_time);
            if self.debug.frame_times.len() > 60 {
                self.debug.frame_times.pop_front();
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
        self.tab_manager
            .update_all_titles(self.config.tab_title_mode);

        // Rebuild renderer if font-related settings changed
        if self.pending_font_rebuild {
            if let Err(e) = self.rebuild_renderer() {
                log::error!("Failed to rebuild renderer after font change: {}", e);
            }
            self.pending_font_rebuild = false;
        }

        // Sync tab bar offsets with renderer's content offsets
        // This ensures the terminal grid correctly accounts for the tab bar position
        let tab_count = self.tab_manager.tab_count();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config);
        let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &self.config);
        crate::debug_trace!(
            "TAB_SYNC",
            "Tab count={}, tab_bar_height={:.0}, tab_bar_width={:.0}, position={:?}, mode={:?}",
            tab_count,
            tab_bar_height,
            tab_bar_width,
            self.config.tab_bar_position,
            self.config.tab_bar_mode
        );
        if let Some(renderer) = &mut self.renderer {
            let grid_changed = Self::apply_tab_bar_offsets_for_position(
                self.config.tab_bar_position,
                renderer,
                tab_bar_height,
                tab_bar_width,
            );
            if let Some((new_cols, new_rows)) = grid_changed {
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
                    "TAB_SYNC",
                    "Tab bar offsets changed (position={:?}), resized terminals to {}x{}",
                    self.config.tab_bar_position,
                    new_cols,
                    new_rows
                );
            }
        }

        // Sync status bar inset so the terminal grid does not extend behind it.
        // Must happen before cell gathering so the row count is correct.
        self.sync_status_bar_inset();

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
        let (mut cells, current_cursor_pos, cursor_style, is_alt_screen, current_generation) =
            if let Ok(term) = terminal.try_lock() {
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

                let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_anim.cursor_opacity));

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
                                TermCursorStyle::BlinkingUnderline => {
                                    TermCursorStyle::SteadyUnderline
                                }
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
                    self.cursor_anim.cursor_opacity,
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
                    let fresh_cells = term.get_cells_with_scrollback(
                        scroll_offset,
                        selection,
                        rectangular,
                        cursor,
                    );

                    (fresh_cells, false)
                } else {
                    // Cache hit: clone the Vec through the Arc (one allocation instead of two).
                    // apply_url_underlines needs a mutable Vec, so we still need an owned copy,
                    // but the Arc clone that extracted cache_cells from tab.cache was free.
                    (cache_cells.as_ref().unwrap().as_ref().clone(), true)
                };
                self.debug.cache_hit = is_cache_hit;
                self.debug.cell_gen_time = cell_gen_start.elapsed();

                // Check if alt screen is active (TUI apps like vim, htop)
                let is_alt_screen = term.is_alt_screen_active();

                (
                    cells,
                    current_cursor_pos,
                    cursor_style,
                    is_alt_screen,
                    current_generation,
                )
            } else if let Some(cached) = cache_cells {
                // Terminal locked (e.g., upload in progress), use cached cells so the
                // rest of the render pipeline (including file transfer overlay) can proceed.
                // Unwrap the Arc: if this is the sole reference the Vec is moved for free,
                // otherwise a clone is made (rare — only if another Arc clone is live).
                let cached_vec = Arc::try_unwrap(cached).unwrap_or_else(|a| (*a).clone());
                (cached_vec, cache_cursor_pos, None, false, cache_generation)
            } else {
                return; // Terminal locked and no cache available, skip this frame
            };

        // --- Prettifier pipeline update ---
        // Feed terminal output changes to the prettifier, check debounce, and handle
        // alt-screen transitions. This runs outside the terminal lock.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Detect alt-screen transitions
            if is_alt_screen != tab.was_alt_screen {
                if let Some(ref mut pipeline) = tab.prettifier {
                    pipeline.on_alt_screen_change(is_alt_screen);
                }
                tab.was_alt_screen = is_alt_screen;
            }

            // Always check debounce (cheap: just a timestamp comparison)
            if let Some(ref mut pipeline) = tab.prettifier {
                pipeline.check_debounce();
            }
        }

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
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
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
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
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
            tab.cache.cells = Some(Arc::new(cells.clone()));
            tab.cache.generation = current_generation;
            tab.cache.scroll_offset = tab.scroll_state.offset;
            tab.cache.cursor_pos = current_cursor_pos;
            tab.cache.selection = tab.mouse.selection;
        }

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title, shell_lifecycle_events) =
            if let Ok(mut term) = terminal.try_lock() {
                // Use cursor row 0 when cursor not visible (e.g., alt screen)
                let cursor_row = current_cursor_pos.map(|(_, row)| row).unwrap_or(0);
                let sb_len = term.scrollback_len();
                term.update_scrollback_metadata(sb_len, cursor_row);

                // Drain shell lifecycle events for the prettifier pipeline
                let shell_events = term.drain_shell_lifecycle_events();

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

                (sb_len, term.get_title(), shell_events)
            } else {
                (
                    cached_scrollback_len,
                    cached_terminal_title.clone(),
                    Vec::new(),
                )
            };

        // Capture prettifier block count before processing events/feed so we can
        // detect when new blocks are added and invalidate the cell cache.
        let prettifier_block_count_before = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.prettifier.as_ref())
            .map(|p| p.active_blocks().len())
            .unwrap_or(0);

        // Forward shell lifecycle events to the prettifier pipeline (outside terminal lock)
        if !shell_lifecycle_events.is_empty()
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
        {
            for event in &shell_lifecycle_events {
                match event {
                    par_term_terminal::ShellLifecycleEvent::CommandStarted {
                        command,
                        absolute_line,
                    } => {
                        tab.cache.prettifier_command_start_line = Some(*absolute_line);
                        tab.cache.prettifier_command_text = Some(command.clone());
                        pipeline.on_command_start(command);
                    }
                    par_term_terminal::ShellLifecycleEvent::CommandFinished { absolute_line } => {
                        if let Some(start) = tab.cache.prettifier_command_start_line.take() {
                            let cmd_text = tab.cache.prettifier_command_text.take();
                            // Read full command output from scrollback so the
                            // prettified block covers the entire output, not just
                            // the visible portion. This ensures scrolling through
                            // long output shows prettified content throughout.
                            let output_start = start + 1;
                            if let Ok(term) = terminal.try_lock() {
                                let lines = term.lines_text_range(output_start, *absolute_line);
                                crate::debug_info!(
                                    "PRETTIFIER",
                                    "submit_command_output: {} lines (rows {}..{})",
                                    lines.len(),
                                    output_start,
                                    absolute_line
                                );
                                pipeline.submit_command_output(lines, cmd_text);
                            } else {
                                // Lock failed — fall back to boundary detector state
                                pipeline.on_command_end();
                            }
                        } else {
                            pipeline.on_command_end();
                        }
                    }
                }
            }
        }

        // Feed terminal output lines to the prettifier pipeline (gated on content changes).
        // Skip per-frame viewport feed for CommandOutput scope — it reads full output
        // from scrollback on CommandFinished instead.
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
            && pipeline.is_enabled()
            && !is_alt_screen
            && pipeline.detection_scope()
                != crate::prettifier::boundary::DetectionScope::CommandOutput
            && current_generation != tab.cache.prettifier_feed_generation
        {
            tab.cache.prettifier_feed_generation = current_generation;

            // Heuristic Claude Code session detection from visible output.
            // One-time: scan for signature patterns if not yet detected.
            if !pipeline.claude_code().is_active() {
                'detect: for row_idx in 0..visible_lines {
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }
                    let row_text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();
                    // Look for Claude Code signature patterns in output.
                    if row_text.contains("Claude Code")
                        || row_text.contains("claude.ai/code")
                        || row_text.contains("Tips for getting the best")
                        || (row_text.contains("Model:")
                            && (row_text.contains("Opus")
                                || row_text.contains("Sonnet")
                                || row_text.contains("Haiku")))
                    {
                        crate::debug_info!(
                            "PRETTIFIER",
                            "Claude Code session detected from output heuristic"
                        );
                        pipeline.mark_claude_code_active();
                        break 'detect;
                    }
                }
            }

            let is_claude_session = pipeline.claude_code().is_active();

            // In Claude Code compact mode, collapse markers indicate tool
            // outputs are hidden. Don't prettify — let Claude Code's own
            // rendering show (styled responses, collapsed summaries). When
            // the user presses Ctrl+O (verbose mode), markers disappear and
            // we prettify the expanded content.
            let has_collapse_markers = is_claude_session
                && (0..visible_lines).any(|row_idx| {
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        return false;
                    }
                    let text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();
                    // Match Claude Code's specific collapse patterns:
                    //   "… +N lines (ctrl+o to expand)"
                    //   "Read N lines (ctrl+o to expand)"
                    //   "Read N files (ctrl+o to expand)"
                    //   "+N lines (ctrl+o to expand)"
                    let is_collapse_line = text.contains("lines (ctrl+o to expand)")
                        || text.contains("files (ctrl+o to expand)");
                    if is_collapse_line {
                        crate::debug_info!(
                            "PRETTIFIER",
                            "collapse marker found at row {}",
                            row_idx
                        );
                    }
                    is_collapse_line
                });

            if has_collapse_markers {
                // Compact mode — clear any existing prettified blocks
                // so cell substitution doesn't overwrite Claude Code's
                // own rendering.
                pipeline.clear_blocks();
            } else {
                // Reset the boundary detector so it gets a fresh snapshot of
                // visible content each time the terminal changes. Without this,
                // the same rows would accumulate as duplicates across frames.
                // The debounce timer (100ms) handles emission timing — the block
                // is emitted once content stabilizes.
                pipeline.reset_boundary();

                // Feed all visible rows from the current frame snapshot.
                for row_idx in 0..visible_lines {
                    let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;

                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }

                    let line = if is_claude_session {
                        // Attribute-aware markdown reconstruction for Claude Code sessions.
                        reconstruct_markdown_from_cells(&cells[start..end])
                    } else {
                        // Plain text extraction for normal output.
                        cells[start..end]
                            .iter()
                            .map(|c| {
                                let g = c.grapheme.as_str();
                                if g.is_empty() || g == "\0" { " " } else { g }
                            })
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    };

                    pipeline.process_output(&line, absolute_row);
                }
            }
        }

        // If new prettified blocks were added during event processing or per-frame feed,
        // invalidate the cell cache so the next frame runs cell substitution.
        {
            let block_count_after = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.prettifier.as_ref())
                .map(|p| p.active_blocks().len())
                .unwrap_or(0);
            if block_count_after > prettifier_block_count_before {
                crate::debug_info!(
                    "PRETTIFIER",
                    "new blocks detected ({} -> {}), invalidating cell cache",
                    prettifier_block_count_before,
                    block_count_after
                );
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
            }
        }

        // Update cache scrollback and clamp scroll state.
        //
        // In pane mode the focused pane's own terminal holds the scrollback, not
        // `tab.terminal`.  Clamping here with `tab.terminal.scrollback_len()` would
        // incorrectly cap (or zero-out) the scroll offset every frame.  The correct
        // clamp happens later in the pane render path once we know the focused pane's
        // actual scrollback length.
        let is_pane_mode = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count() > 0)
            .unwrap_or(false);
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.scrollback_len = scrollback_len;
            if !is_pane_mode {
                tab.scroll_state
                    .clamp_to_scrollback(tab.cache.scrollback_len);
            }
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
        // AI Inspector action to handle after rendering
        let mut pending_inspector_action = InspectorAction::None;
        // Profile drawer action to handle after rendering
        let mut pending_profile_drawer_action = ProfileDrawerAction::None;
        // Close confirmation action to handle after rendering
        let mut pending_close_confirm_action = CloseConfirmAction::None;
        // Quit confirmation action to handle after rendering
        let mut pending_quit_confirm_action = QuitConfirmAction::None;
        let mut pending_remote_install_action = RemoteShellInstallAction::None;
        let mut pending_ssh_connect_action = SshConnectAction::None;
        // Process agent messages and refresh AI Inspector snapshot
        self.process_agent_messages_tick();

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

        // Calculate custom status bar height
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

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

        // Sync AI Inspector panel width before scrollbar update so the scrollbar
        // position uses the current panel width on this frame (not the previous one).
        self.sync_ai_inspector_width();

        // Prettifier cell substitution — replace raw cells with rendered content.
        // Always run when blocks exist: the cell cache stores raw terminal cells
        // (set before this point), so we must re-apply styled content every frame.
        //
        // Also collect inline graphics (rendered diagrams) for GPU compositing.
        #[allow(clippy::type_complexity)]
        let mut prettifier_graphics: Vec<(
            u64,
            std::sync::Arc<Vec<u8>>,
            u32,
            u32,
            isize,
            usize,
        )> = Vec::new();
        if !is_alt_screen
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(ref pipeline) = tab.prettifier
            && pipeline.is_enabled()
        {
            let scroll_off = tab.scroll_state.offset;
            let gutter_w = tab.gutter_manager.gutter_width;

            // Track which blocks we've already collected graphics from
            // to avoid duplicates when multiple viewport rows fall in
            // the same block.
            let mut collected_block_ids = std::collections::HashSet::new();

            for viewport_row in 0..visible_lines {
                let absolute_row = scrollback_len.saturating_sub(scroll_off) + viewport_row;
                if let Some(block) = pipeline.block_at_row(absolute_row) {
                    if !block.has_rendered() {
                        continue;
                    }

                    // Collect inline graphics from this block (once per block).
                    if collected_block_ids.insert(block.block_id) {
                        let block_start = block.content().start_row;
                        for graphic in block.buffer.rendered_graphics() {
                            if !graphic.is_rgba
                                || graphic.data.is_empty()
                                || graphic.pixel_width == 0
                                || graphic.pixel_height == 0
                            {
                                continue;
                            }
                            // Compute screen row: block_start + graphic.row within block,
                            // then convert to viewport coordinates.
                            let abs_graphic_row = block_start + graphic.row;
                            let view_start = scrollback_len.saturating_sub(scroll_off);
                            let screen_row = abs_graphic_row as isize - view_start as isize;

                            // Use block_id + graphic row as a stable texture ID
                            // (offset to avoid colliding with terminal graphic IDs).
                            let texture_id = 0x8000_0000_0000_0000_u64
                                | (block.block_id << 16)
                                | (graphic.row as u64);

                            crate::debug_info!(
                                "PRETTIFIER",
                                "uploading graphic: block={}, row={}, screen_row={}, {}x{} px, {} bytes RGBA",
                                block.block_id,
                                graphic.row,
                                screen_row,
                                graphic.pixel_width,
                                graphic.pixel_height,
                                graphic.data.len()
                            );

                            prettifier_graphics.push((
                                texture_id,
                                graphic.data.clone(),
                                graphic.pixel_width,
                                graphic.pixel_height,
                                screen_row,
                                graphic.col + gutter_w,
                            ));
                        }
                    }

                    let display_lines = block.buffer.display_lines();
                    let block_start = block.content().start_row;
                    let line_offset = absolute_row.saturating_sub(block_start);
                    if let Some(styled_line) = display_lines.get(line_offset) {
                        crate::debug_trace!(
                            "PRETTIFIER",
                            "cell sub: vp_row={}, abs_row={}, block_id={}, line_off={}, segs={}",
                            viewport_row,
                            absolute_row,
                            block.block_id,
                            line_offset,
                            styled_line.segments.len()
                        );
                        let cell_start = viewport_row * grid_cols;
                        let cell_end = (cell_start + grid_cols).min(cells.len());
                        if cell_start >= cells.len() {
                            break;
                        }
                        // Clear row
                        for cell in &mut cells[cell_start..cell_end] {
                            *cell = par_term_config::Cell::default();
                        }
                        // Write styled segments (offset by gutter width to avoid clipping)
                        let mut col = gutter_w;
                        for segment in &styled_line.segments {
                            for ch in segment.text.chars() {
                                if col >= grid_cols {
                                    break;
                                }
                                let idx = cell_start + col;
                                if idx < cells.len() {
                                    cells[idx].grapheme = ch.to_string();
                                    if let Some([r, g, b]) = segment.fg {
                                        cells[idx].fg_color = [r, g, b, 0xFF];
                                    }
                                    if let Some([r, g, b]) = segment.bg {
                                        cells[idx].bg_color = [r, g, b, 0xFF];
                                    }
                                    cells[idx].bold = segment.bold;
                                    cells[idx].italic = segment.italic;
                                    cells[idx].underline = segment.underline;
                                    cells[idx].strikethrough = segment.strikethrough;
                                }
                                col += 1;
                            }
                        }
                    }
                }
            }
        }

        // Cache modal visibility before entering the renderer borrow scope.
        // Method calls borrow all of `self`, which conflicts with `&mut self.renderer`.
        let any_modal_visible = self.any_modal_ui_visible();

        if let Some(renderer) = &mut self.renderer {
            // Status bar inset is handled by sync_status_bar_inset() above,
            // before cell gathering, so the grid height is correct.

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
                (current_cursor_pos, Some(self.cursor_anim.cursor_opacity), cursor_style)
            {
                renderer.update_cursor(pos, opacity, style);
                // Forward cursor state to custom shader for Ghostty-compatible cursor animations
                // Use the configured cursor color
                let cursor_color = color_u8_to_f32_a(self.config.cursor_color, 1.0);
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

            // Compute and set gutter indicators for prettified blocks
            {
                let gutter_data = if let Some(tab) = self.tab_manager.active_tab() {
                    if let Some(ref pipeline) = tab.prettifier {
                        if pipeline.is_enabled() {
                            let indicators = tab.gutter_manager.indicators_for_viewport(
                                pipeline,
                                scroll_offset,
                                visible_lines,
                            );
                            // Default gutter indicator color: semi-transparent highlight
                            let gutter_color = [0.3, 0.5, 0.8, 0.15];
                            indicators
                                .iter()
                                .flat_map(|ind| {
                                    (ind.row..ind.row + ind.height).map(move |r| (r, gutter_color))
                                })
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                renderer.set_gutter_indicators(gutter_data);
            }

            // Update animations and request redraw if frames changed
            // Use try_lock() to avoid blocking the event loop when PTY reader holds the lock
            let anim_start = std::time::Instant::now();
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(terminal) = tab.terminal.try_lock()
                && terminal.update_animations()
            {
                // Animation frame changed - request continuous redraws while animations are playing
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            let debug_anim_time = anim_start.elapsed();

            // Update graphics from terminal (pass scroll_offset for view adjustment)
            // Include both current screen graphics and scrollback graphics
            // Use get_graphics_with_animations() to get current animation frames
            // Use try_lock() to avoid blocking the event loop when PTY reader holds the lock
            //
            // In split-pane mode each pane has its own PTY terminal; graphics are collected
            // per-pane inside the pane data gather loop below and do not go through here.
            let graphics_start = std::time::Instant::now();
            let has_pane_manager_for_graphics = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager.as_ref())
                .map(|pm| pm.pane_count() > 0)
                .unwrap_or(false);
            if !has_pane_manager_for_graphics
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(terminal) = tab.terminal.try_lock()
            {
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

            // Upload prettifier diagram graphics (rendered Mermaid, etc.) to the GPU.
            // These are appended to the sixel_graphics render list and composited in
            // the same pass as Sixel/iTerm2/Kitty graphics.
            if !prettifier_graphics.is_empty() {
                #[allow(clippy::type_complexity)]
                let refs: Vec<(u64, &[u8], u32, u32, isize, usize)> = prettifier_graphics
                    .iter()
                    .map(|(id, data, w, h, row, col)| (*id, data.as_slice(), *w, *h, *row, *col))
                    .collect();
                if let Err(e) = renderer.update_prettifier_graphics(&refs) {
                    crate::debug_error!(
                        "PRETTIFIER",
                        "Failed to upload prettifier graphics: {}",
                        e
                    );
                }
            }

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

            // Capture session variables for status bar rendering (skip if bar is hidden)
            let status_bar_session_vars = if self.config.status_bar_enabled
                && !self
                    .status_bar_ui
                    .should_hide(&self.config, self.is_fullscreen)
            {
                Some(self.badge_state.variables.read().clone())
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

            // Collect pane bounds for identify overlay (before egui borrow)
            let pane_identify_bounds: Vec<(usize, crate::pane::PaneBounds)> =
                if self.pane_identify_hide_time.is_some() {
                    self.tab_manager
                        .active_tab()
                        .and_then(|tab| tab.pane_manager())
                        .map(|pm| {
                            pm.all_panes()
                                .iter()
                                .enumerate()
                                .map(|(i, pane)| (i, pane.bounds))
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

            let egui_data = if let Some(window) = self.window.as_ref() {
                // Window is live; run egui if the context and state are also ready.
                if let (Some(egui_ctx), Some(egui_state)) = (&self.egui_ctx, &mut self.egui_state) {
                    let mut raw_input = egui_state.take_egui_input(window);

                    // Inject pending events from menu accelerators (Cmd+V/C/A intercepted by muda)
                    // when egui overlays (profile modal, search, etc.) are active
                    raw_input.events.append(&mut self.pending_egui_events);

                    // When no modal UI overlay is visible, filter out Tab key events to prevent
                    // egui's default focus navigation from stealing Tab/Shift+Tab from the terminal.
                    // Tab/Shift+Tab should only cycle focus between egui widgets when a modal is open.
                    // Note: Side panels (ai_inspector, profile drawer) are NOT modals — the terminal
                    // should still receive Tab/Shift+Tab when they are open.
                    if !any_modal_visible {
                        raw_input.events.retain(|e| {
                            !matches!(
                                e,
                                egui::Event::Key {
                                    key: egui::Key::Tab,
                                    ..
                                }
                            )
                        });
                    }

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
                    let tab_bar_right_reserved = if self.ai_inspector.open {
                        self.ai_inspector.consumed_width()
                    } else {
                        0.0
                    };
                    pending_tab_action = self.tab_bar_ui.render(
                        ctx,
                        &self.tab_manager,
                        &self.config,
                        &self.profile_manager,
                        tab_bar_right_reserved,
                    );

                    // Render tmux status bar if connected
                    self.tmux_status_bar_ui.render(
                        ctx,
                        &self.config,
                        self.tmux_session.as_ref(),
                        self.tmux_session_name.as_deref(),
                    );

                    // Render custom status bar
                    if let Some(ref session_vars) = status_bar_session_vars {
                        let (_bar_height, status_bar_action) = self.status_bar_ui.render(
                            ctx,
                            &self.config,
                            session_vars,
                            self.is_fullscreen,
                        );
                        if status_bar_action
                            == Some(crate::status_bar::StatusBarAction::ShowUpdateDialog)
                        {
                            self.show_update_dialog = true;
                        }
                    }

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

                    // Show AI Inspector panel and collect action
                    pending_inspector_action = self.ai_inspector.show(ctx, &self.agent_state.available_agents);

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

                    // Show remote shell install dialog if visible
                    pending_remote_install_action = self.remote_shell_install_ui.show(ctx);

                    // Show SSH Quick Connect dialog if visible
                    pending_ssh_connect_action = self.ssh_connect_ui.show(ctx);

                    // Render update dialog overlay
                    if self.show_update_dialog {
                        // Poll for update install completion
                        if let Some(ref rx) = self.update_install_receiver
                            && let Ok(result) = rx.try_recv()
                        {
                            match result {
                                Ok(update_result) => {
                                    self.update_install_status = Some(format!(
                                        "Updated to v{}! Restart par-term to use the new version.",
                                        update_result.new_version
                                    ));
                                    self.update_installing = false;
                                    self.status_bar_ui.update_available_version = None;
                                }
                                Err(e) => {
                                    self.update_install_status =
                                        Some(format!("Update failed: {}", e));
                                    self.update_installing = false;
                                }
                            }
                            self.update_install_receiver = None;
                        }

                        if let Some(ref update_result) = self.last_update_result {
                            let dialog_action = crate::update_dialog::render(
                                ctx,
                                update_result,
                                env!("CARGO_PKG_VERSION"),
                                self.installation_type,
                                self.update_installing,
                                self.update_install_status.as_deref(),
                            );
                            match dialog_action {
                                crate::update_dialog::UpdateDialogAction::Dismiss => {
                                    if !self.update_installing {
                                        self.show_update_dialog = false;
                                        self.update_install_status = None;
                                    }
                                }
                                crate::update_dialog::UpdateDialogAction::SkipVersion(v) => {
                                    self.config.skipped_version = Some(v);
                                    self.show_update_dialog = false;
                                    self.status_bar_ui.update_available_version = None;
                                    self.update_install_status = None;
                                    let _ = self.config.save();
                                }
                                crate::update_dialog::UpdateDialogAction::InstallUpdate(v) => {
                                    if !self.update_installing {
                                        self.update_installing = true;
                                        self.update_install_status =
                                            Some("Downloading update...".to_string());
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        self.update_install_receiver = Some(rx);
                                        let version = v.clone();
                                        std::thread::spawn(move || {
                                            let result =
                                                crate::self_updater::perform_update(&version);
                                            let _ = tx.send(result);
                                        });
                                    }
                                    // Don't close dialog while installing
                                }
                                crate::update_dialog::UpdateDialogAction::None => {}
                            }
                        } else {
                            // No update result, close dialog
                            self.show_update_dialog = false;
                        }
                    }

                    // Render profile drawer (right side panel)
                    pending_profile_drawer_action = self.profile_drawer_ui.render(
                        ctx,
                        &self.profile_manager,
                        &self.config,
                        false, // profile modal is no longer in the terminal window
                    );

                    // Render progress bar overlay
                    if let (Some(snap), Some(size)) = (&progress_snapshot, window_size_for_badge) {
                        let tab_count = self.tab_manager.tab_count();
                        let tb_height = self.tab_bar_ui.get_height(tab_count, &self.config);
                        let (top_inset, bottom_inset) = match self.config.tab_bar_position {
                            par_term_config::TabBarPosition::Top => (tb_height, 0.0),
                            par_term_config::TabBarPosition::Bottom => (0.0, tb_height),
                            par_term_config::TabBarPosition::Left => (0.0, 0.0),
                        };
                        render_progress_bars(
                            ctx,
                            snap,
                            &self.config,
                            size.width as f32,
                            size.height as f32,
                            top_inset,
                            bottom_inset,
                        );
                    }

                    // Render pane identify overlay (large index numbers centered on each pane)
                    if !pane_identify_bounds.is_empty() {
                        for (index, bounds) in &pane_identify_bounds {
                            let center_x = bounds.x + bounds.width / 2.0;
                            let center_y = bounds.y + bounds.height / 2.0;
                            egui::Area::new(egui::Id::new(format!("pane_identify_{}", index)))
                                .fixed_pos(egui::pos2(center_x - 30.0, center_y - 30.0))
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(ctx, |ui| {
                                    egui::Frame::NONE
                                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                                        .inner_margin(egui::Margin::symmetric(16, 8))
                                        .corner_radius(8.0)
                                        .stroke(egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgb(100, 200, 255),
                                        ))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(format!("Pane {}", index))
                                                    .monospace()
                                                    .size(28.0)
                                                    .color(egui::Color32::from_rgb(100, 200, 255)),
                                            );
                                        });
                                });
                        }
                    }

                    // Render file transfer progress overlay (bottom-right corner)
                    crate::app::file_transfers::render_file_transfer_overlay(
                        &self.file_transfer_state,
                        ctx,
                    );

                    // Render badge overlay (top-right corner)
                    if let (Some(badge), Some(size)) = (&badge_state, window_size_for_badge) {
                        render_badge(ctx, badge, size.width as f32, size.height as f32);
                    }
                });

                    // Handle egui platform output (clipboard, cursor changes, etc.)
                    // This enables cut/copy/paste in egui text editors
                    egui_state.handle_platform_output(window, egui_output.platform_output.clone());

                    Some((egui_output, egui_ctx))
                } else {
                    // egui context/state not yet initialised for this window.
                    None
                }
            } else {
                // Window not yet created; skip egui rendering this frame.
                crate::debug_error!("RENDER", "egui render skipped: window is None");
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
                content_offset_x: renderer.content_offset_x(),
                content_inset_bottom: renderer.content_inset_bottom(),
                content_inset_right: renderer.content_inset_right(),
                cell_width: renderer.cell_width(),
                cell_height: renderer.cell_height(),
                padding: renderer.window_padding(),
                status_bar_height: (status_bar_height + custom_status_bar_height)
                    * renderer.scale_factor(),
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

            // Per-pane backgrounds only take effect when splits are active.
            // In single-pane mode, skip per-pane background lookup.
            let pane_0_bg: Option<crate::pane::PaneBackground> = None;

            let render_result = if has_pane_manager {
                // When splits are active and hide_window_padding_on_split is enabled,
                // use 0 padding so panes extend to the window edges
                let effective_padding =
                    if pane_count > 1 && self.config.hide_window_padding_on_split {
                        0.0
                    } else {
                        sizing.padding
                    };

                // Render panes from pane manager - inline data gathering to avoid borrow conflicts
                let content_width = sizing.size.width as f32
                    - effective_padding * 2.0
                    - sizing.content_offset_x
                    - sizing.content_inset_right;
                let content_height = sizing.size.height as f32
                    - sizing.content_offset_y
                    - sizing.content_inset_bottom
                    - effective_padding
                    - sizing.status_bar_height;

                // Gather all necessary data upfront while we can borrow tab_manager
                #[allow(clippy::type_complexity)]
                let pane_render_data: Option<(
                    Vec<PaneRenderData>,
                    Vec<crate::pane::DividerRect>,
                    Vec<PaneTitleInfo>,
                    Option<PaneViewport>,
                    usize, // focused pane scrollback_len (for tab.cache update)
                )> = {
                    let tab = self.tab_manager.active_tab_mut();
                    if let Some(tab) = tab {
                        // Capture tab-level scroll offset before mutably borrowing pane_manager.
                        // In split-pane mode the focused pane uses tab.scroll_state.offset;
                        // non-focused panes always render at offset 0 (bottom).
                        let tab_scroll_offset = tab.scroll_state.offset;
                        if let Some(pm) = &mut tab.pane_manager {
                            // Update bounds
                            let bounds = crate::pane::PaneBounds::new(
                                effective_padding + sizing.content_offset_x,
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
                            let cursor_opacity = self.cursor_anim.cursor_opacity;

                            // Pane title settings
                            // Scale from logical pixels (config) to physical pixels
                            let show_titles = self.config.show_pane_titles;
                            let title_height = self.config.pane_title_height * sizing.scale_factor;
                            let title_position = self.config.pane_title_position;
                            let title_text_color = color_u8_to_f32(self.config.pane_title_color);
                            let title_bg_color = color_u8_to_f32(self.config.pane_title_bg_color);

                            let mut pane_data = Vec::new();
                            let mut pane_titles = Vec::new();
                            let mut focused_pane_scrollback_len: usize = 0;
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
                                        let scroll_offset =
                                            if is_focused { tab_scroll_offset } else { 0 };
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
                                        // Still need the actual scrollback_len even without marks:
                                        // it's used for graphics position math (tex_v_start
                                        // cropping when graphic is partially off-top, and
                                        // view_start when showing scrollback graphics).
                                        let sb_len = if let Ok(term) = pane.terminal.try_lock() {
                                            term.scrollback_len()
                                        } else {
                                            0
                                        };
                                        (Vec::new(), sb_len)
                                    };
                                    let pane_scroll_offset =
                                        if is_focused { tab_scroll_offset } else { 0 };

                                    // Cache the focused pane's scrollback_len so that scroll
                                    // operations (mouse wheel, Page Up, etc.) can use it without
                                    // needing to lock the terminal. Only update when the value is
                                    // non-zero (lock succeeded) to avoid clobbering a good cached
                                    // value with a transient lock-failure fallback of 0.
                                    if is_focused && pane_scrollback_len > 0 {
                                        focused_pane_scrollback_len = pane_scrollback_len;
                                    }

                                    // Per-pane backgrounds only apply when multiple panes exist
                                    let pane_background = if all_pane_ids.len() > 1
                                        && pane.background().has_image()
                                    {
                                        Some(pane.background().clone())
                                    } else {
                                        None
                                    };

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

                                    // Collect inline graphics (Sixel/iTerm2/Kitty) from this
                                    // pane's PTY terminal.  Each pane has its own PTY so graphics
                                    // are never in the shared tab.terminal.
                                    let pane_graphics = if let Ok(term) = pane.terminal.try_lock() {
                                        let mut g = term.get_graphics_with_animations();
                                        let sb = term.get_scrollback_graphics();
                                        crate::debug_log!(
                                            "GRAPHICS",
                                            "pane {:?}: active_graphics={}, scrollback_graphics={}, scrollback_len={}, scroll_offset={}, visible_rows={}, viewport=({},{},{}x{})",
                                            pane_id,
                                            g.len(),
                                            sb.len(),
                                            pane_scrollback_len,
                                            pane_scroll_offset,
                                            rows,
                                            viewport.x,
                                            viewport.y,
                                            viewport.width,
                                            viewport.height
                                        );
                                        for (i, gfx) in g.iter().chain(sb.iter()).enumerate() {
                                            crate::debug_log!(
                                                "GRAPHICS",
                                                "  graphic[{}]: id={}, pos=({},{}), scroll_offset_rows={}, scrollback_row={:?}, size={}x{}",
                                                i,
                                                gfx.id,
                                                gfx.position.0,
                                                gfx.position.1,
                                                gfx.scroll_offset_rows,
                                                gfx.scrollback_row,
                                                gfx.width,
                                                gfx.height
                                            );
                                        }
                                        g.extend(sb);
                                        g
                                    } else {
                                        crate::debug_log!(
                                            "GRAPHICS",
                                            "pane {:?}: try_lock() failed, no graphics",
                                            pane_id
                                        );
                                        Vec::new()
                                    };

                                    pane_data.push(PaneRenderData {
                                        viewport,
                                        cells,
                                        grid_size: (cols, rows),
                                        cursor_pos,
                                        cursor_opacity: if is_focused {
                                            cursor_opacity
                                        } else {
                                            0.0
                                        },
                                        marks,
                                        scrollback_len: pane_scrollback_len,
                                        scroll_offset: pane_scroll_offset,
                                        background: pane_background,
                                        graphics: pane_graphics,
                                    });
                                }
                            }

                            Some((
                                pane_data,
                                dividers,
                                pane_titles,
                                focused_viewport,
                                focused_pane_scrollback_len,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                if let Some((
                    pane_data,
                    dividers,
                    pane_titles,
                    focused_viewport,
                    focused_pane_scrollback_len,
                )) = pane_render_data
                {
                    // Update tab cache with the focused pane's scrollback_len so that scroll
                    // operations (mouse wheel, Page Up/Down, etc.) see the correct limit.
                    // Only update when non-zero to avoid clobbering a good value on lock failure.
                    // The `apply_scroll` function already clamps the target; we don't call
                    // `clamp_to_scrollback` here because that would reset an ongoing scroll.
                    if focused_pane_scrollback_len > 0
                        && let Some(tab) = self.tab_manager.active_tab_mut()
                    {
                        tab.cache.scrollback_len = focused_pane_scrollback_len;
                    }

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
                        show_scrollbar,
                    )
                } else {
                    // Fallback to single pane render
                    renderer.render(egui_data, false, show_scrollbar, pane_0_bg.as_ref())
                }
            } else {
                // Single pane - use standard render path
                renderer.render(egui_data, false, show_scrollbar, pane_0_bg.as_ref())
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

        // Sync AI Inspector panel width after the render pass.
        // This catches drag-resize changes that update self.ai_inspector.width during show().
        // Done here to avoid borrow conflicts with the renderer block above.
        self.sync_ai_inspector_width();

        // Handle tab bar actions collected during egui rendering
        self.handle_tab_bar_action_after_render(pending_tab_action);


        // Handle clipboard actions collected during egui rendering
        self.handle_clipboard_history_action_after_render(pending_clipboard_action);

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

        // Handle remote shell integration install action
        match pending_remote_install_action {
            RemoteShellInstallAction::Install => {
                // Send the install command via paste_text() which uses the same
                // code path as Cmd+V paste — handles bracketed paste mode and
                // correctly forwards through SSH sessions.
                let command = RemoteShellInstallUI::install_command();
                // paste_text appends \r internally via term.paste()
                self.paste_text(&format!("{}\n", command));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            RemoteShellInstallAction::Cancel => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            RemoteShellInstallAction::None => {}
        }

        // Handle SSH Quick Connect actions
        match pending_ssh_connect_action {
            SshConnectAction::Connect {
                host,
                profile_override: _,
            } => {
                // Build SSH command and write it to the active terminal's PTY
                let args = host.ssh_args();
                let ssh_cmd = format!("ssh {}\n", args.join(" "));
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    let _ = term.write_str(&ssh_cmd);
                }
                log::info!(
                    "SSH Quick Connect: connecting to {}",
                    host.connection_string()
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            SshConnectAction::Cancel => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            SshConnectAction::None => {}
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


        // Handle AI Inspector actions collected during egui rendering
        self.handle_inspector_action_after_render(pending_inspector_action);

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
                // Open settings window to Profiles tab instead of terminal-embedded modal
                self.open_settings_window_requested = true;
                self.open_settings_profiles_tab = true;
            }
            ProfileDrawerAction::None => {}
        }

        let absolute_total = absolute_start.elapsed();
        if absolute_total.as_millis() > 10 {
            log::debug!(
                "TIMING: AbsoluteTotal={:.2}ms (from function start to end)",
                absolute_total.as_secs_f64() * 1000.0
            );
        }
    }

    /// Process incoming ACP agent messages for this render tick and refresh
    /// the AI Inspector snapshot when needed.
    ///
    /// Called once per frame from `render()`. Handles the full agent message
    /// dispatch loop, deferred config updates, inline tool-markup fallback,
    /// bounded skill-failure recovery, auto-context feeding, and snapshot refresh.
    fn process_agent_messages_tick(&mut self) {
        let mut saw_prompt_complete_this_tick = false;

        // Process agent messages
        let msg_count_before = self.ai_inspector.chat.messages.len();
        // Config update requests are deferred until message processing completes.
        type ConfigUpdateEntry = (
            std::collections::HashMap<String, serde_json::Value>,
            tokio::sync::oneshot::Sender<Result<(), String>>,
        );
        let mut pending_config_updates: Vec<ConfigUpdateEntry> = Vec::new();
        let messages = self.agent_state.drain_messages();
        for msg in messages {
            match msg {
                    AgentMessage::StatusChanged(status) => {
                        // Flush any pending agent text on status change.
                        self.ai_inspector.chat.flush_agent_message();
                        self.ai_inspector.agent_status = status;
                        self.needs_redraw = true;
                    }
                    AgentMessage::SessionUpdate(update) => {
                        match &update {
                            par_term_acp::SessionUpdate::ToolCall(info) => {
                                let title_l = info.title.to_ascii_lowercase();
                                if title_l.contains("skill")
                                    || title_l.contains("todo")
                                    || title_l.contains("enterplanmode")
                                {
                                    self.agent_state.agent_skill_failure_detected = true;
                                }
                            }
                            par_term_acp::SessionUpdate::ToolCallUpdate(info) => {
                                if let Some(status) = &info.status {
                                    let status_l = status.to_ascii_lowercase();
                                    if status_l.contains("fail") || status_l.contains("error") {
                                        self.agent_state.agent_skill_failure_detected = true;
                                    }
                                }
                            }
                            par_term_acp::SessionUpdate::CurrentModeUpdate { mode_id } => {
                                if mode_id.eq_ignore_ascii_case("plan") {
                                    self.agent_state.agent_skill_failure_detected = true;
                                    self.ai_inspector.chat.add_system_message(
                                        "Agent switched to plan mode during an executable task. Requesting default mode and retry guidance."
                                            .to_string(),
                                    );
                                    if let Some(agent) = &self.agent_state.agent {
                                        let agent = agent.clone();
                                        self.runtime.spawn(async move {
                                            let agent = agent.lock().await;
                                            if let Err(e) = agent.set_mode("default").await {
                                                log::error!(
                                                    "ACP: failed to auto-reset mode from plan to default: {e}"
                                                );
                                            }
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                        self.ai_inspector.chat.handle_update(update);
                        self.needs_redraw = true;
                    }
                    AgentMessage::PermissionRequest {
                        request_id,
                        tool_call,
                        options,
                    } => {
                        log::info!(
                            "ACP: permission request id={request_id} options={}",
                            options.len()
                        );
                        let description = tool_call
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Permission requested")
                            .to_string();
                        if is_terminal_screenshot_permission_tool(&tool_call)
                            && !self.config.ai_inspector_agent_screenshot_access
                        {
                            let deny_option_id = options
                                .iter()
                                .find(|o| {
                                    matches!(
                                        o.kind.as_deref(),
                                        Some("deny")
                                            | Some("reject")
                                            | Some("cancel")
                                            | Some("disallow")
                                    ) || o.name.to_lowercase().contains("deny")
                                        || o.name.to_lowercase().contains("reject")
                                        || o.name.to_lowercase().contains("cancel")
                                })
                                .or_else(|| options.first())
                                .map(|o| o.option_id.clone());

                            if let Some(client) = &self.agent_state.agent_client {
                                let client = client.clone();
                                self.runtime.spawn(async move {
                                    use par_term_acp::{
                                        PermissionOutcome, RequestPermissionResponse,
                                    };
                                    let outcome = RequestPermissionResponse {
                                        outcome: PermissionOutcome {
                                            outcome: "selected".to_string(),
                                            option_id: deny_option_id,
                                        },
                                    };
                                    let response_json =
                                        serde_json::to_value(&outcome).unwrap_or_default();
                                    if let Err(e) =
                                        client.respond(request_id, Some(response_json), None).await
                                    {
                                        log::error!(
                                            "ACP: failed to auto-deny screenshot permission: {e}"
                                        );
                                    }
                                });
                            } else {
                                log::error!(
                                    "ACP: cannot auto-deny screenshot permission id={request_id} \
                                     — agent_client is None!"
                                );
                            }

                            self.ai_inspector.chat.add_system_message(format!(
                                "Blocked screenshot request (`{description}`) because \"Allow Agent Screenshots\" is disabled in Settings > Assistant > Permissions."
                            ));
                            self.needs_redraw = true;
                            continue;
                        }
                        self.ai_inspector
                            .chat
                            .messages
                            .push(ChatMessage::Permission {
                                request_id,
                                description,
                                options: options
                                    .iter()
                                    .map(|o| (o.option_id.clone(), o.name.clone()))
                                    .collect(),
                                resolved: false,
                            });
                        self.needs_redraw = true;
                    }
                    AgentMessage::PromptStarted => {
                        self.agent_state.agent_skill_failure_detected = false;
                        self.ai_inspector.chat.mark_oldest_pending_sent();
                        // Remove the corresponding handle (first in queue).
                        if !self.agent_state.pending_send_handles.is_empty() {
                            self.agent_state.pending_send_handles.pop_front();
                        }
                        self.needs_redraw = true;
                    }
                    AgentMessage::PromptComplete => {
                        saw_prompt_complete_this_tick = true;
                        self.ai_inspector.chat.flush_agent_message();
                        self.needs_redraw = true;
                    }
                    AgentMessage::ConfigUpdate { updates, reply } => {
                        pending_config_updates.push((updates, reply));
                    }
                    AgentMessage::ClientReady(client) => {
                        log::info!("ACP: agent_client ready");
                        self.agent_state.agent_client = Some(client);
                    }
                    AgentMessage::AutoApproved(description) => {
                        self.ai_inspector.chat.add_auto_approved(description);
                        self.needs_redraw = true;
                    }
                }
        }
        // Process deferred config updates now that message processing completes.
        for (updates, reply) in pending_config_updates {
            let result = self.apply_agent_config_updates(&updates);
            if result.is_ok() {
                self.config_changed_by_agent = true;
            }
            let _ = reply.send(result);
            self.needs_redraw = true;
        }

        // Track recoverable local backend tool failures during the current
        // prompt (for example failed `Skill`/`Write` calls).
        if !self.agent_state.agent_skill_failure_detected {
            let mut seen_user_boundary = false;
            for msg in self.ai_inspector.chat.messages.iter().rev() {
                if matches!(msg, ChatMessage::User { .. }) {
                    seen_user_boundary = true;
                    break;
                }
                if let ChatMessage::ToolCall { title, status, .. } = msg {
                    let title_l = title.to_ascii_lowercase();
                    let status_l = status.to_ascii_lowercase();
                    let is_failed = status_l.contains("fail") || status_l.contains("error");
                    let is_recoverable_tool = title_l.contains("skill")
                        || title_l == "write"
                        || title_l.starts_with("write ")
                        || title_l.contains(" write ");
                    if is_failed && is_recoverable_tool {
                        self.agent_state.agent_skill_failure_detected = true;
                        break;
                    }
                }
            }
            // If there is no user message yet, ignore stale history.
            if !seen_user_boundary {
                self.agent_state.agent_skill_failure_detected = false;
            }
        }

        // Compatibility fallback: some local ACP backends emit literal
        // `<function=...>` tool markup in chat instead of structured tool calls.
        // Parse inline `config_update` payloads from newly added agent messages
        // and apply them so config changes still work.
        let inline_updates: Vec<(usize, std::collections::HashMap<String, serde_json::Value>)> =
            self.ai_inspector
                .chat
                .messages
                .iter()
                .enumerate()
                .skip(msg_count_before)
                .filter_map(|(idx, msg)| match msg {
                    ChatMessage::Agent(text) => {
                        extract_inline_config_update(text).map(|updates| (idx, updates))
                    }
                    _ => None,
                })
                .collect();

        for (idx, updates) in inline_updates {
            match self.apply_agent_config_updates(&updates) {
                Ok(()) => {
                    self.config_changed_by_agent = true;
                    if let Some(ChatMessage::Agent(text)) =
                        self.ai_inspector.chat.messages.get_mut(idx)
                    {
                        *text = "Applied config update request.".to_string();
                    }
                    self.ai_inspector.chat.add_system_message(
                        "Applied inline config_update fallback from agent output.".to_string(),
                    );
                }
                Err(e) => {
                    self.ai_inspector
                        .chat
                        .add_system_message(format!("Inline config_update fallback failed: {e}"));
                }
            }
            self.needs_redraw = true;
        }

        // Detect other inline XML-style tool markup (we only auto-apply
        // `config_update`). Treat these as recoverable local backend tool
        // failures so we can issue a one-shot retry with stricter guidance.
        for msg in self
            .ai_inspector
            .chat
            .messages
            .iter()
            .skip(msg_count_before)
        {
            if let ChatMessage::Agent(text) = msg
                && let Some(function_name) = extract_inline_tool_function_name(text)
                && function_name != "mcp__par-term-config__config_update"
            {
                self.agent_state.agent_skill_failure_detected = true;
                self.ai_inspector.chat.add_system_message(format!(
                    "Agent emitted inline tool markup (`{function_name}`) instead of a structured ACP tool call."
                ));
                self.needs_redraw = true;
                break;
            }
        }

        let last_user_text = self
            .ai_inspector
            .chat
            .messages
            .iter()
            .rev()
            .find_map(|msg| {
                if let ChatMessage::User { text, .. } = msg {
                    Some(text.clone())
                } else {
                    None
                }
            });

        let shader_activation_incomplete = if saw_prompt_complete_this_tick {
            if let Some(user_text) = last_user_text.as_deref() {
                if crate::ai_inspector::shader_context::is_shader_activation_request(user_text) {
                    let mut saw_user_boundary = false;
                    let mut saw_config_update_for_prompt = false;
                    for msg in self.ai_inspector.chat.messages.iter().rev() {
                        match msg {
                            ChatMessage::User { .. } => {
                                saw_user_boundary = true;
                                break;
                            }
                            ChatMessage::ToolCall { title, .. } => {
                                let title_l = title.to_ascii_lowercase();
                                if title_l.contains("config_update") {
                                    saw_config_update_for_prompt = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    saw_user_boundary && !saw_config_update_for_prompt
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Bounded recovery: if the prompt failed due to a local backend tool
        // mismatch (failed Skill/Write or inline tool markup), or if a shader
        // activation request completed without a config_update call, nudge the
        // agent to continue the same task with proper ACP tool use.
        if saw_prompt_complete_this_tick
            && (self.agent_state.agent_skill_failure_detected || shader_activation_incomplete)
            && self.agent_state.agent_skill_recovery_attempts < 3
            && let Some(agent) = &self.agent_state.agent
        {
            let had_recoverable_failure = self.agent_state.agent_skill_failure_detected;
            self.agent_state.agent_skill_recovery_attempts =
                self.agent_state.agent_skill_recovery_attempts.saturating_add(1);
            self.agent_state.agent_skill_failure_detected = false;
            self.ai_inspector.chat.streaming = true;
            if shader_activation_incomplete && !had_recoverable_failure {
                self.ai_inspector.chat.add_system_message(
                    format!(
                        "Agent completed a shader task response without activating the shader via \
                         config_update. Auto-retrying (attempt {}/3) to finish the activation step.",
                        self.agent_state.agent_skill_recovery_attempts
                    ),
                );
            } else {
                self.ai_inspector.chat.add_system_message(
                    format!(
                        "Recoverable local-backend tool failure detected (failed Skill/Write or \
                         inline tool markup). Auto-retrying (attempt {}/3) with stricter ACP tool guidance.",
                        self.agent_state.agent_skill_recovery_attempts
                    ),
                );
            }

            let mut content: Vec<par_term_acp::ContentBlock> =
                vec![par_term_acp::ContentBlock::Text {
                    text: format!(
                        "{}[End system instructions]",
                        crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE
                    ),
                }];

            if let Some(ref user_text) = last_user_text
                && crate::ai_inspector::shader_context::should_inject_shader_context(
                    user_text,
                    &self.config,
                )
            {
                content.push(par_term_acp::ContentBlock::Text {
                    text: crate::ai_inspector::shader_context::build_shader_context(&self.config),
                });
            }

            let extra_recovery_strictness = if self.agent_state.agent_skill_recovery_attempts >= 2 {
                " Do not explore unrelated files or dependencies. For shader tasks, go directly \
                 to the shader file write and config_update activation steps."
            } else {
                ""
            };
            content.push(par_term_acp::ContentBlock::Text {
                text: format!(
                    "[Host recovery note]\nContinue the previous user task and stay on the \
                       same domain/problem (do not switch to unrelated examples/files). Do NOT \
                       use `Skill`, `Task`, or `TodoWrite`. Do NOT emit XML-style tool markup \
                       (`<function=...>`). Use normal ACP file/system/MCP tools directly. If \
                       a `Read` fails because the target is a directory, do not retry `Read` on \
                       that directory; use a listing/search tool or write the known target file \
                       path directly. \
                       Complete the full requested workflow before declaring success (for shader \
                       tasks: write the requested shader content, then call config_update to \
                       activate it). \
                       using `Write`, use exact parameters like `file_path` and `content` (not \
                       `filepath`). For par-term settings changes use \
                       `mcp__par-term-config__config_update` / `config_update`. If a tool \
                       fails, correct the call and retry the same task with the available \
                       tools. If you have already created the requested shader file, do not \
                       stop there: call config_update now to activate it before declaring \
                       success. Do not ask the user to restate the request unless you truly \
                       need missing information.{}",
                    extra_recovery_strictness
                ),
            });

            let agent = agent.clone();
            let tx = self.agent_state.agent_tx.clone();
            let handle = self.runtime.spawn(async move {
                let agent = agent.lock().await;
                if let Some(ref tx) = tx {
                    let _ = tx.send(AgentMessage::PromptStarted);
                }
                let _ = agent.send_prompt(content).await;
                if let Some(tx) = tx {
                    let _ = tx.send(AgentMessage::PromptComplete);
                }
            });
            self.agent_state.pending_send_handles.push_back(handle);
            self.needs_redraw = true;
        }

        // Auto-execute new CommandSuggestion messages when terminal access is enabled.
        if self.config.ai_inspector_agent_terminal_access {
            let new_messages = &self.ai_inspector.chat.messages[msg_count_before..];
            let commands_to_run: Vec<String> = new_messages
                .iter()
                .filter_map(|msg| {
                    if let ChatMessage::CommandSuggestion(cmd) = msg {
                        Some(format!("{cmd}\n"))
                    } else {
                        None
                    }
                })
                .collect();

            if !commands_to_run.is_empty()
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_lock()
            {
                for cmd in &commands_to_run {
                    let _ = term.write(cmd.as_bytes());
                }
                crate::debug_info!(
                    "AI_INSPECTOR",
                    "Auto-executed {} command(s) in terminal",
                    commands_to_run.len()
                );
            }
        }

        // Detect new command completions and auto-refresh the snapshot.
        // This is separate from agent auto-context so the panel always shows
        // up-to-date command history regardless of agent connection state.
        if self.ai_inspector.open
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
        {
            let history = term.core_command_history();
            let current_count = history.len();

            if current_count != self.ai_inspector.last_command_count {
                // Command count changed — refresh the snapshot
                let had_commands = self.ai_inspector.last_command_count > 0;
                self.ai_inspector.last_command_count = current_count;
                self.ai_inspector.needs_refresh = true;

                // Auto-context feeding: send latest command info to agent
                if had_commands
                    && current_count > 0
                    && self.config.ai_inspector_auto_context
                    && self.ai_inspector.agent_status == AgentStatus::Connected
                    && let Some((cmd, exit_code, duration_ms)) = history.last()
                {
                    let now = std::time::Instant::now();
                    let throttled = self.agent_state.last_auto_context_sent_at.is_some_and(|last_sent| {
                        now.duration_since(last_sent)
                            < std::time::Duration::from_millis(AUTO_CONTEXT_MIN_INTERVAL_MS)
                    });

                    if !throttled {
                        let exit_code_str = exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        let duration = duration_ms.unwrap_or(0);

                        let cwd = term.shell_integration_cwd().unwrap_or_default();
                        let (sanitized_cmd, was_redacted) = redact_auto_context_command(cmd);

                        let context = format!(
                            "[Auto-context event]\nCommand completed:\n$ {}\nExit code: {}\nDuration: {}ms\nCWD: {}\nSensitive arguments redacted: {}",
                            sanitized_cmd, exit_code_str, duration, cwd, was_redacted
                        );

                        if let Some(agent) = &self.agent_state.agent {
                            self.agent_state.last_auto_context_sent_at = Some(now);
                            self.ai_inspector.chat.add_system_message(if was_redacted {
                                "Auto-context sent command metadata to the agent (sensitive values redacted).".to_string()
                            } else {
                                "Auto-context sent command metadata to the agent.".to_string()
                            });
                            self.needs_redraw = true;
                            let agent = agent.clone();
                            let content = vec![par_term_acp::ContentBlock::Text { text: context }];
                            self.runtime.spawn(async move {
                                let agent = agent.lock().await;
                                let _ = agent.send_prompt(content).await;
                            });
                        }
                    }
                }
            }
        }

        // Refresh AI Inspector snapshot if needed
        if self.ai_inspector.open
            && self.ai_inspector.needs_refresh
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
        {
            let snapshot = crate::ai_inspector::snapshot::SnapshotData::gather(
                &term,
                &self.ai_inspector.scope,
                self.config.ai_inspector_context_max_lines,
            );
            self.ai_inspector.snapshot = Some(snapshot);
            self.ai_inspector.needs_refresh = false;
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
                let just_opened = self.ai_inspector.toggle();
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
    fn handle_clipboard_history_action_after_render(&mut self, action: crate::clipboard_history_ui::ClipboardHistoryAction) {
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
    }

    /// Handle AI Inspector panel actions collected during egui rendering.
    fn handle_inspector_action_after_render(&mut self, action: crate::ai_inspector::panel::InspectorAction) {
        // Handle AI Inspector actions collected during egui rendering
        match action {
            InspectorAction::Close => {
                self.ai_inspector.open = false;
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
                self.ai_inspector.connected_agent_name = None;
                self.ai_inspector.connected_agent_identity = None;
                // Abort any queued send tasks.
                for handle in self.agent_state.pending_send_handles.drain(..) {
                    handle.abort();
                }
                self.ai_inspector.agent_status = AgentStatus::Disconnected;
                self.agent_state.pending_agent_context_replay = None;
                self.needs_redraw = true;
            }
            InspectorAction::RevokeAlwaysAllowSelections => {
                if let Some(identity) = self.ai_inspector.connected_agent_identity.clone() {
                    // Cancel any queued prompts before replacing the session.
                    for handle in self.agent_state.pending_send_handles.drain(..) {
                        handle.abort();
                    }
                    self.ai_inspector.chat.add_system_message(
                        "Resetting agent session to revoke all \"Always allow\" permissions. Local chat context will be replayed on your next prompt (best effort)."
                            .to_string(),
                    );
                    self.connect_agent(&identity);
                } else {
                    self.ai_inspector.chat.add_system_message(
                        "Cannot reset permissions: no connected agent identity.".to_string(),
                    );
                }
                self.needs_redraw = true;
            }
            InspectorAction::SendPrompt(text) => {
                // Reset one-shot local backend recovery for each user prompt.
                self.agent_state.agent_skill_failure_detected = false;
                self.agent_state.agent_skill_recovery_attempts = 0;
                self.ai_inspector.chat.add_user_message(text.clone());
                self.ai_inspector.chat.streaming = true;
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

                    if let Some(replay_prompt) = self.agent_state.pending_agent_context_replay.take() {
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
                                Some(serde_json::to_value(&result).unwrap()),
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
                for msg in &mut self.ai_inspector.chat.messages {
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
                self.ai_inspector.chat.flush_agent_message();
                self.ai_inspector
                    .chat
                    .add_system_message("Cancelled.".to_string());
                self.needs_redraw = true;
            }
            InspectorAction::CancelQueuedPrompt => {
                if self.ai_inspector.chat.cancel_last_pending() {
                    // Abort the most recent queued send task.
                    if let Some(handle) = self.agent_state.pending_send_handles.pop_back() {
                        handle.abort();
                    }
                    self.ai_inspector
                        .chat
                        .add_system_message("Queued message cancelled.".to_string());
                }
                self.needs_redraw = true;
            }
            InspectorAction::ClearChat => {
                let reconnect_identity = self.ai_inspector.connected_agent_identity.clone();
                self.ai_inspector.chat.clear();
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
                        || self.ai_inspector.agent_status != AgentStatus::Disconnected)
                {
                    self.connect_agent(&identity);
                    self.ai_inspector.chat.add_system_message(
                        "Conversation cleared. Reconnected agent to reset session state."
                            .to_string(),
                    );
                }
                self.needs_redraw = true;
            }
            InspectorAction::None => {}
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
        show_scrollbar: bool,
    ) -> Result<bool> {
        // Two-phase construction: separate owned cell data from pane metadata
        // so PaneRenderInfo can borrow cell slices safely.  This replaces the
        // previous unsafe Box::into_raw / Box::from_raw pattern that leaked
        // memory if render_split_panes panicked.
        //
        // Phase 1: Extract cells into a Vec that outlives the render infos.
        // The remaining pane fields are collected into partial render infos.
        let mut owned_cells: Vec<Vec<crate::cell_renderer::Cell>> =
            Vec::with_capacity(pane_data.len());
        let mut partial_infos: Vec<PaneRenderInfo> = Vec::with_capacity(pane_data.len());

        for pane in pane_data {
            let focused = pane.viewport.focused;
            owned_cells.push(pane.cells);
            partial_infos.push(PaneRenderInfo {
                viewport: pane.viewport,
                // Placeholder — will be patched in Phase 2 once owned_cells
                // is finished growing and its elements have stable addresses.
                cells: &[],
                grid_size: pane.grid_size,
                cursor_pos: pane.cursor_pos,
                cursor_opacity: pane.cursor_opacity,
                show_scrollbar: show_scrollbar && focused,
                marks: pane.marks,
                scrollback_len: pane.scrollback_len,
                scroll_offset: pane.scroll_offset,
                background: pane.background,
                graphics: pane.graphics,
            });
        }

        // Phase 2: Patch cell references now that owned_cells won't reallocate.
        // owned_cells lives until scope exit (even on panic), so the borrows
        // are valid for the lifetime of partial_infos.
        for (info, cells) in partial_infos.iter_mut().zip(owned_cells.iter()) {
            info.cells = cells.as_slice();
        }
        let pane_render_infos = partial_infos;

        // Build divider render info
        let divider_render_infos: Vec<DividerRenderInfo> = dividers
            .iter()
            .enumerate()
            .map(|(i, d)| DividerRenderInfo::from_rect(d, hovered_divider_index == Some(i)))
            .collect();

        // Build divider settings from config
        let divider_settings = PaneDividerSettings {
            divider_color: color_u8_to_f32(config.pane_divider_color),
            hover_color: color_u8_to_f32(config.pane_divider_hover_color),
            show_focus_indicator: config.pane_focus_indicator,
            focus_color: color_u8_to_f32(config.pane_focus_color),
            focus_width: config.pane_focus_width * renderer.scale_factor(),
            divider_style: config.pane_divider_style,
        };

        // Call the split pane renderer.
        // owned_cells is dropped automatically at scope exit, even on panic.
        renderer.render_split_panes(
            &pane_render_infos,
            &divider_render_infos,
            &pane_titles,
            focused_viewport.as_ref(),
            &divider_settings,
            egui_data,
            false,
        )
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
        self.command_history.save_background();

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
