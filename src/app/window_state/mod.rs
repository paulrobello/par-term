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
    /// Debounce state for config saves to prevent rapid concurrent writes
    pub(crate) config_save_state: ConfigSaveState,

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

    /// Compiled regex cache for prettify trigger patterns (command_filter and block_end).
    /// Keyed by pattern string; avoids recompiling the same pattern every frame.
    pub(crate) trigger_regex_cache: std::collections::HashMap<String, regex::Regex>,
}
