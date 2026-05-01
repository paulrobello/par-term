//! Per-window state for multi-window terminal emulator
//!
//! This module contains `WindowState`, which holds all state specific to a single window,
//! including its renderer, tab manager, input handler, and UI components.
//!
//! Architectural Note: `WindowState` is being decomposed from a God Object
//! into cohesive sub-state structs (ARC-001). See `focus_state.rs`,
//! `overlay_state.rs`, `update_state.rs`, `watcher_state.rs`, `trigger_state.rs`,
//! and `render_loop_state.rs`.
//!
//! # ARC-002: Remaining God-Object Decomposition (Requires Manual Intervention)
//!
//! `WindowState` currently has 30+ fields and 84 separate `impl WindowState` blocks
//! scattered across the codebase. Several sub-state bundles have already been extracted
//! (see `EguiState`, `FocusState`, `OverlayState`, `RenderLoopState`, `ShaderState`,
//! `AgentState`, `CursorAnimState`, `OverlayUiState`, `TriggerState`, `WatcherState`,
//! `UpdateState`, `DebugState`). The remaining work deferred to a future session:
//!
//! **Suggested next extractions (in order of isolation):**
//!
//! 1. `TmuxSubsystem` — owns `tmux_state` and all methods in `src/app/tmux_handler/`.
//!    Safe to extract once `TmuxState` has no shared borrow with other sub-state.
//!
//! 2. `SelectionSubsystem` — owns `smart_selection_cache`, `copy_mode`, and the
//!    text-selection helpers in `text_selection.rs`. These three fields form a tight
//!    read-only cluster during rendering.
//!
//! 3. `WindowInfrastructure` — groups `window`, `renderer`, `runtime` as the GPU/OS
//!    surface layer; separates it from application-level state.
//!
//! **Blocker:** All 84 `impl WindowState` blocks must be audited before moving any
//! field to ensure no method holds simultaneous mutable borrows across sub-systems.
//! Recommend using `cargo expand` or GitNexus impact analysis on each field before
//! moving it. The `#[path]` redirect blocker (ARC-003) has been resolved — field
//! extraction from step 3+ can now proceed.
//!
//! **Tracking:** Issue ARC-002 in AUDIT.md.
//!
//! # ARC-003: render_pipeline `#[path]` Redirect — RESOLVED
//!
//! The `#[path = "../render_pipeline/mod.rs"]` redirect has been removed.
//! `render_pipeline` is now declared as a first-class module in `src/app/mod.rs`,
//! matching the physical directory layout (`src/app/render_pipeline/`).
//! All `super::` references inside `render_pipeline/*.rs` correctly resolve to
//! the `render_pipeline` module itself (unchanged). See ARC-001 in AUDIT.md.

mod action_handlers;
mod agent_config;
mod agent_message_helpers;
mod agent_messages;
mod agent_screenshot;
pub(crate) mod agent_state;
mod agent_tick_helpers;
pub(crate) mod anti_idle;
pub(crate) mod config_updates;
mod config_watchers;
pub(crate) mod cursor_anim_state;
pub(crate) mod debug_state;
mod egui_state;
mod focus_state;
mod impl_agent;
mod impl_helpers;
mod impl_init;
pub(crate) mod keyboard_handlers;
mod notifications;
mod overlay_state;
pub(crate) mod overlay_ui_state;
mod render_loop_state;
pub(crate) mod renderer_init;
mod renderer_ops;
pub(crate) mod scroll_ops;
pub(crate) mod search_highlight;
mod shader_ops;
pub(crate) mod shader_state;
pub(crate) mod text_selection;
mod trigger_state;
mod ui_query_helpers;
mod update_state;
pub(crate) mod url_hover;
mod watcher_state;

// Re-export the sub-state types
pub(crate) use crate::app::tmux_handler::tmux_state::TmuxState;
pub(crate) use egui_state::EguiState;
pub(crate) use focus_state::FocusState;
pub(crate) use overlay_state::OverlayState;
pub(crate) use render_loop_state::{ConfigSaveState, RenderLoopState};
pub(crate) use trigger_state::{PendingTriggerAction, TriggerState};
pub(crate) use update_state::UpdateState;
pub(crate) use watcher_state::WatcherState;

use crate::app::window_state::debug_state::DebugState;
use crate::badge::BadgeState;
use crate::config::Config;
use crate::input::InputHandler;
use crate::keybindings::{KeyCombo, KeybindingRegistry};
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

/// Transient context shared between chained workflow actions.
///
/// Created by `ShellCommand` actions with `capture_output: true` and
/// consumed by `Condition` checks (`ExitCode`, `OutputContains`).
/// Never serialized to disk.
#[derive(Debug, Clone)]
pub(crate) struct WorkflowContext {
    /// Exit code from the last captured shell command.
    pub(crate) last_exit_code: Option<i32>,
    /// Captured stdout+stderr from the last shell command (capped at 64 KB).
    pub(crate) last_output: Option<String>,
}

/// Per-window state that manages a single terminal window with multiple tabs.
pub struct WindowState {
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
    pub(crate) shader_state: crate::app::window_state::shader_state::ShaderState,
    /// Overlay / modal / side-panel UI state
    pub(crate) overlay_ui: crate::app::window_state::overlay_ui_state::OverlayUiState,
    /// ACP agent connection and runtime state
    pub(crate) agent_state: agent_state::AgentState,
    /// Cursor animation state (opacity, blink timers)
    pub(crate) cursor_anim: crate::app::window_state::cursor_anim_state::CursorAnimState,
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
    // Render loop control & config management (ARC-001 extraction: RenderLoopState)
    // =========================================================================
    /// Pending-work flags for the render loop (agent config change, font rebuild, config save)
    pub(crate) render_loop: RenderLoopState,

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
    /// Shared transient context for chained workflow actions (Sequence / Condition / Repeat).
    /// Written by background ShellCommand threads (capture_output=true); read by Condition checks.
    pub(crate) last_workflow_context: std::sync::Arc<std::sync::Mutex<Option<WorkflowContext>>>,

    // =========================================================================
    // Keybinding & smart selection caches
    // =========================================================================
    pub(crate) keybinding_registry: KeybindingRegistry,
    pub(crate) custom_action_prefix_combo: Option<KeyCombo>,
    pub(crate) custom_action_prefix_state: crate::tmux::PrefixState,
    pub(crate) smart_selection_cache: SmartSelectionCache,

    // =========================================================================
    // tmux integration
    // =========================================================================
    pub(crate) tmux_state: TmuxState,

    // =========================================================================
    // Window snap-to-grid
    // =========================================================================
    /// Tracks the last size we requested via `request_inner_size` for snap-to-grid.
    /// Cleared once we receive a Resized event matching this size, preventing infinite re-snap.
    pub(crate) pending_snap_size: Option<winit::dpi::PhysicalSize<u32>>,
}
