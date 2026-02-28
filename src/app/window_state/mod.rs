//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | Type definitions (`WindowState`, `ConfigSaveState`, …) |
//! | `impl_init.rs` | Constructor (`new`) and async initialization |
//! | `impl_agent.rs` | ACP agent connection and lifecycle methods |
//! | `impl_helpers.rs` | DRY helpers, debounced config-save, anti-idle, egui queries, `Drop` |
//! | `prettify_helpers.rs` | Markdown reconstruction / Claude Code segment preprocessing |
//! | `action_handlers.rs` | Keyboard / menu action dispatch |
//! | `agent_messages.rs` | Incoming ACP message processing |
//! | `config_watchers.rs` | Config-file and screenshot-request watchers |
//! | `render_pipeline/` | Frame-data gather, rendering passes, post-render actions |
//! | `renderer_ops.rs` | Low-level renderer operations |
//! | `shader_ops.rs` | Shader hot-reload and cursor-shader helpers |

mod action_handlers;
mod agent_messages;
mod config_watchers;
mod impl_agent;
mod impl_helpers;
mod impl_init;
mod prettify_helpers;
mod render_pipeline;
mod renderer_ops;
mod shader_ops;

// Re-export the prettify helpers so sub-modules in render_pipeline/ can reach them
// via `super::super::` (i.e. `window_state::reconstruct_markdown_from_cells`).
pub(crate) use prettify_helpers::{
    preprocess_claude_code_segment, reconstruct_markdown_from_cells,
};

use crate::app::debug_state::DebugState;
use crate::badge::BadgeState;
use crate::config::Config;
use crate::input::InputHandler;
use crate::keybindings::KeybindingRegistry;
use crate::renderer::Renderer;
use crate::smart_selection::SmartSelectionCache;
use crate::status_bar::StatusBarUI;
use crate::tab::TabManager;
use crate::tab_bar_ui::TabBarUI;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;
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

/// Debounce state for config saves to prevent rapid concurrent writes.
///
/// Multiple code paths may request config saves in quick succession (e.g., user
/// changes a setting, an agent modifies config, update checker records timestamp).
/// This struct tracks when the last save happened and whether a pending save is needed.
///
/// The debounced save approach:
/// - If called within DEBOUNCE_INTERVAL of last save, mark `pending_save = true` and return
/// - On the next frame check, if `pending_save` and debounce has expired, perform the save
#[derive(Default)]
pub(crate) struct ConfigSaveState {
    /// When the last config save was performed
    pub(crate) last_save: Option<std::time::Instant>,
    /// Whether a save was deferred and needs to be executed
    pub(crate) pending_save: bool,
}

impl ConfigSaveState {
    /// Minimum time between config saves (in milliseconds).
    /// Rapid saves within this window are debounced.
    const DEBOUNCE_INTERVAL_MS: u64 = 100;
}

/// Per-window state that manages a single terminal window with multiple tabs.
///
/// ## Field Groups
///
/// This struct contains ~50 fields organized into the following logical groups:
///
/// | Group | Fields | Purpose |
/// |-------|--------|---------|
/// | **Core infrastructure** | `config`, `window`, `renderer`, `input_handler`, `runtime` | Foundational subsystems |
/// | **Tab & UI management** | `tab_manager`, `tab_bar_ui`, `status_bar_ui` | Multi-tab coordination and built-in UI bars |
/// | **egui overlay layer** | `egui_ctx`, `egui_state`, `pending_egui_events`, `egui_initialized` | Settings/modal GUI framework |
/// | **Specialized UI state** | `shader_state`, `overlay_ui`, `agent_state` | Sub-system state bundles (already extracted) |
/// | **Render loop control** | `needs_redraw`, `pending_font_rebuild`, `config_save_state`, `last_render_time` | Event-driven render scheduling |
/// | **Focus & mouse** | `is_focused`, `focus_click_pending`, `focus_click_suppressed_while_unfocused_at`, `clipboard_image_click_guard`, `ui_consumed_mouse_press` | Focus-click suppression and clipboard protection |
/// | **Performance modes** | `cursor_hidden_since`, `flicker_pending_render`, `throughput_batch_start` | Flicker reduction and throughput batching |
/// | **File watchers** | `config_watcher`, `config_update_watcher`, `screenshot_request_watcher` | Config-file and MCP request watchers |
/// | **Window flags** | `is_fullscreen`, `is_recording`, `is_shutting_down`, `window_index`, `config_changed_by_agent` | Miscellaneous window-level boolean flags |
/// | **Pending actions** | `open_settings_window_requested`, `open_settings_profiles_tab`, `pending_arrangement_restore`, `reload_dynamic_profiles_requested`, `profiles_menu_needs_update` | One-shot flags consumed by the window manager |
/// | **Transient overlays** | `resize_overlay_visible`, `resize_overlay_hide_time`, `resize_dimensions`, `toast_message`, `toast_hide_time`, `pane_identify_hide_time` | Short-lived UI decorations |
/// | **Feature state** | `broadcast_input`, `badge_state`, `copy_mode`, `file_transfer_state`, `cursor_anim` | Per-feature state |
/// | **Self-update** | `show_update_dialog`, `last_update_result`, `installation_type`, `update_installing`, `update_install_status`, `update_install_receiver` | In-app update flow |
/// | **Trigger management** | `trigger_spawned_processes`, `trigger_regex_cache` | RunCommand process tracking and regex cache |
/// | **Keybinding & selection** | `keybinding_registry`, `smart_selection_cache` | User input caches |
/// | **tmux integration** | `tmux_state` | tmux session/pane state |
/// | **History** | `closed_tabs` | Session undo (reopen closed tab) |
/// | **Debug** | `debug` | Dev-only diagnostics |
///
/// ### Future decomposition candidates
///
/// - `FocusState` — `is_focused`, `focus_click_pending`, `focus_click_suppressed_while_unfocused_at`,
///   `clipboard_image_click_guard`, `ui_consumed_mouse_press`
/// - `RenderScheduleState` — `needs_redraw`, `pending_font_rebuild`, `last_render_time`,
///   `cursor_hidden_since`, `flicker_pending_render`, `throughput_batch_start`
/// - `UpdateState` — all six `update_*` / `last_update_result` / `installation_type` fields
/// - `TransientOverlayState` — resize overlay + toast + pane identify fields
pub struct WindowState {
    // =========================================================================
    // Core infrastructure — foundational subsystems required by all other groups
    // =========================================================================
    /// Global configuration loaded from `~/.config/par-term/config.yaml`
    pub(crate) config: Config,
    /// The winit window handle (None briefly during initialization)
    pub(crate) window: Option<Arc<Window>>,
    /// GPU renderer (None during init / after surface loss)
    pub(crate) renderer: Option<Renderer>,
    /// Keyboard and mouse input handler
    pub(crate) input_handler: InputHandler,
    /// Tokio runtime shared with async PTY tasks
    pub(crate) runtime: Arc<Runtime>,

    // =========================================================================
    // Tab & built-in UI bars — multi-tab coordination and top/bottom bar widgets
    // =========================================================================
    /// Tab manager for handling multiple terminal tabs
    pub(crate) tab_manager: TabManager,
    /// Tab bar UI
    pub(crate) tab_bar_ui: TabBarUI,
    /// Custom status bar UI
    pub(crate) status_bar_ui: StatusBarUI,

    // =========================================================================
    // Window flags — miscellaneous window-level boolean state
    // =========================================================================
    /// Whether window is currently in fullscreen mode
    pub(crate) is_fullscreen: bool,
    /// Whether terminal session recording is active
    pub(crate) is_recording: bool,
    /// Flag to indicate shutdown is in progress
    pub(crate) is_shutting_down: bool,
    /// Window index (1-based) for display in title bar
    pub(crate) window_index: usize,

    // =========================================================================
    // egui overlay layer — settings window / modal GUI framework
    // =========================================================================
    /// egui context for GUI rendering
    pub(crate) egui_ctx: Option<egui::Context>,
    /// egui-winit state for event handling
    pub(crate) egui_state: Option<egui_winit::State>,
    /// Pending egui events to inject into next frame's raw_input.
    /// Used when macOS menu accelerators intercept Cmd+V/C/A before egui sees them
    /// while an egui overlay (profile modal, search, etc.) is active.
    pub(crate) pending_egui_events: Vec<egui::Event>,
    /// Whether egui has completed its first ctx.run() call.
    /// Before first run, egui's `is_using_pointer()` returns unreliable results.
    pub(crate) egui_initialized: bool,

    // =========================================================================
    // Specialized UI sub-system state bundles (already extracted into own types)
    // =========================================================================
    /// Shader hot-reload watcher, metadata caches, and reload-error state
    pub(crate) shader_state: crate::app::shader_state::ShaderState,
    /// Overlay / modal / side-panel UI state
    pub(crate) overlay_ui: crate::app::overlay_ui_state::OverlayUiState,
    /// ACP agent connection and runtime state
    pub(crate) agent_state: crate::app::agent_state::AgentState,
    /// Cursor animation state (opacity, blink timers)
    pub(crate) cursor_anim: crate::app::cursor_anim_state::CursorAnimState,
    /// Debug / diagnostics state
    pub(crate) debug: DebugState,

    // =========================================================================
    // Render loop scheduling — event-driven render and font-rebuild flags
    // =========================================================================
    /// Whether we need to render next frame
    pub(crate) needs_redraw: bool,
    /// Set when an agent/MCP config update was applied — signals WindowManager to
    /// sync its own config copy so subsequent saves don't overwrite agent changes.
    pub(crate) config_changed_by_agent: bool,
    /// Whether we need to rebuild renderer after font-related changes
    pub(crate) pending_font_rebuild: bool,
    /// Debounce state for config saves to prevent rapid concurrent writes
    pub(crate) config_save_state: ConfigSaveState,
    /// Last time a frame was rendered (for FPS throttling when unfocused)
    pub(crate) last_render_time: Option<std::time::Instant>,

    // =========================================================================
    // Focus & power saving — window focus state and FPS throttling
    // =========================================================================
    /// Whether the window currently has focus
    pub(crate) is_focused: bool,

    // =========================================================================
    // Focus-click suppression & clipboard protection
    //
    // Prevents the first click after a window-focus event from being forwarded
    // to the PTY, which can cause tmux/mouse-aware apps to clear the clipboard.
    // =========================================================================
    /// Track if we blocked a mouse press for UI — also block the corresponding release
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

    // =========================================================================
    // Performance modes — flicker reduction and throughput batching
    // =========================================================================
    /// When cursor was last hidden (for `reduce_flicker` feature)
    pub(crate) cursor_hidden_since: Option<std::time::Instant>,
    /// Whether we have pending terminal updates deferred due to cursor being hidden
    pub(crate) flicker_pending_render: bool,
    /// When throughput mode batching started (for render interval timing)
    pub(crate) throughput_batch_start: Option<std::time::Instant>,

    // =========================================================================
    // File watchers — config-file and MCP request watchers
    // =========================================================================
    /// Config file watcher for automatic reload (e.g., when ACP agent modifies config.yaml)
    pub(crate) config_watcher: Option<crate::config::watcher::ConfigWatcher>,
    /// Watcher for `.config-update.json` written by the MCP server
    pub(crate) config_update_watcher: Option<crate::config::watcher::ConfigWatcher>,
    /// Watcher for `.screenshot-request.json` written by the MCP server
    pub(crate) screenshot_request_watcher: Option<crate::config::watcher::ConfigWatcher>,

    // =========================================================================
    // Pending window-manager actions — one-shot flags consumed by WindowManager
    // =========================================================================
    /// Flag to signal that the settings window should be opened.
    /// Set by keyboard handlers and consumed by the window manager.
    pub(crate) open_settings_window_requested: bool,
    /// Flag to signal that the settings window should open to the Profiles tab
    pub(crate) open_settings_profiles_tab: bool,
    /// Pending arrangement restore request (name of arrangement to restore)
    pub(crate) pending_arrangement_restore: Option<String>,
    /// Flag to request reload of dynamic profiles
    pub(crate) reload_dynamic_profiles_requested: bool,
    /// Flag to indicate profiles menu needs to be updated in the main menu
    pub(crate) profiles_menu_needs_update: bool,

    // =========================================================================
    // Transient overlays — short-lived UI decorations with auto-hide timers
    // =========================================================================

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

    // =========================================================================
    // Feature state — per-feature state bundles
    // =========================================================================
    /// Whether keyboard input is broadcast to all panes in current tab
    pub(crate) broadcast_input: bool,
    /// Badge state for session information display
    pub(crate) badge_state: BadgeState,
    /// Copy mode state machine (vi-style keyboard text selection)
    pub(crate) copy_mode: crate::copy_mode::CopyModeState,
    /// File transfer UI state (active transfers, pending saves/uploads, dialog state)
    pub(crate) file_transfer_state: crate::app::file_transfers::FileTransferState,

    // =========================================================================
    // Self-update flow — in-app update dialog and install state
    // =========================================================================
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

    // =========================================================================
    // Trigger management — RunCommand process tracking and regex caching
    // =========================================================================
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: std::collections::HashMap<u32, std::time::Instant>,
    /// Compiled regex cache for prettify trigger patterns (command_filter and block_end).
    /// Keyed by pattern string; avoids recompiling the same pattern every frame.
    pub(crate) trigger_regex_cache: std::collections::HashMap<String, regex::Regex>,

    // =========================================================================
    // Keybinding & smart selection caches
    // =========================================================================
    /// Keybinding registry for user-defined keyboard shortcuts
    pub(crate) keybinding_registry: KeybindingRegistry,
    /// Cache for compiled smart selection regex patterns
    pub(crate) smart_selection_cache: SmartSelectionCache,

    // =========================================================================
    // tmux integration — tmux session/pane state machine
    // =========================================================================
    /// tmux integration state (session, sync, pane mappings, prefix key)
    pub(crate) tmux_state: crate::app::tmux_state::TmuxState,

    // =========================================================================
    // Session history — recently closed tab undo queue
    // =========================================================================
    /// Recently closed tab metadata for session undo (reopen closed tab)
    pub(crate) closed_tabs: std::collections::VecDeque<super::tab_ops::ClosedTabInfo>,
}
