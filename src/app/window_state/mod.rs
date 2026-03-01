//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.
//!
//! Architectural Note: `WindowState` is being decomposed from a God Object
//! into cohesive sub-state structs (ARC-001). See `focus_state.rs`,
//! `overlay_state.rs`, `update_state.rs`, `watcher_state.rs`, and `trigger_state.rs`.

mod action_handlers;
mod agent_config;
mod agent_messages;
mod agent_screenshot;
mod config_watchers;
mod egui_state;
mod focus_state;
mod impl_agent;
mod impl_helpers;
mod impl_init;
mod overlay_state;
mod prettify_helpers;
mod render_pipeline;
mod renderer_ops;
mod shader_ops;
mod trigger_state;
mod update_state;
mod watcher_state;

// Re-export the sub-state types
pub(crate) use egui_state::EguiState;
pub(crate) use focus_state::FocusState;
pub(crate) use overlay_state::OverlayState;
pub(crate) use trigger_state::TriggerState;
pub(crate) use update_state::UpdateState;
pub(crate) use watcher_state::WatcherState;

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
#[derive(Default)]
pub(crate) struct ConfigSaveState {
    /// When the last config save was performed
    pub(crate) last_save: Option<std::time::Instant>,
    /// Whether a save was deferred and needs to be executed
    pub(crate) pending_save: bool,
}

impl ConfigSaveState {
    /// Minimum time between config saves (in milliseconds).
    const DEBOUNCE_INTERVAL_MS: u64 = 100;
}

/// Per-window state that manages a single terminal window with multiple tabs.
pub(crate) struct WindowState {
    // =========================================================================
    // Core infrastructure
    // =========================================================================
    /// Global configuration
    pub(crate) config: Config,
    /// The winit window handle
    pub(crate) window: Option<Arc<Window>>,
    /// GPU renderer
    pub(crate) renderer: Option<Renderer>,
    /// Keyboard and mouse input handler
    pub(crate) input_handler: InputHandler,
    /// Tokio runtime shared with async PTY tasks
    pub(crate) runtime: Arc<Runtime>,

    // =========================================================================
    // Tab & built-in UI bars
    // =========================================================================
    /// Tab manager for handling multiple terminal tabs
    pub(crate) tab_manager: TabManager,
    /// Tab bar UI
    pub(crate) tab_bar_ui: TabBarUI,
    /// Custom status bar UI
    pub(crate) status_bar_ui: StatusBarUI,

    // =========================================================================
    // Window flags
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
    // egui overlay layer (ARC-001 extraction: EguiState)
    // =========================================================================
    /// egui context, input state, and lifecycle flags (see `EguiState`)
    pub(crate) egui: EguiState,

    // =========================================================================
    // Sub-system state bundles
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
    // Decomposed state objects (ARC-001)
    // =========================================================================
    /// State for focus, redraw tracking, and render throttling
    pub(crate) focus_state: FocusState,
    /// State for the in-app self-update flow
    pub(crate) update_state: UpdateState,
    /// State for transient UI overlays and pending UI requests
    pub(crate) overlay_state: OverlayState,
    /// State for file and request watchers
    pub(crate) watcher_state: WatcherState,
    /// State for terminal triggers and their spawned processes
    pub(crate) trigger_state: TriggerState,

    // =========================================================================
    // Render loop control & config management
    // =========================================================================
    /// Set when an agent/MCP config update was applied
    pub(crate) config_changed_by_agent: bool,
    /// Whether we need to rebuild renderer after font-related changes
    pub(crate) pending_font_rebuild: bool,
    /// Debounce state for config saves
    pub(crate) config_save_state: ConfigSaveState,

    // =========================================================================
    // Feature state
    // =========================================================================
    /// Whether keyboard input is broadcast to all panes in current tab
    pub(crate) broadcast_input: bool,
    /// Badge state for session information display
    pub(crate) badge_state: BadgeState,
    /// Copy mode state machine
    pub(crate) copy_mode: crate::copy_mode::CopyModeState,
    /// File transfer UI state
    pub(crate) file_transfer_state: crate::app::file_transfers::FileTransferState,
    /// Snapshot of clipboard image for restore after tmux clicks
    pub(crate) clipboard_image_click_guard: Option<ClipboardImageClickGuard>,

    // =========================================================================
    // Keybinding & smart selection caches
    // =========================================================================
    pub(crate) keybinding_registry: KeybindingRegistry,
    pub(crate) smart_selection_cache: SmartSelectionCache,

    // =========================================================================
    // tmux integration
    // =========================================================================
    pub(crate) tmux_state: crate::app::tmux_state::TmuxState,
}
