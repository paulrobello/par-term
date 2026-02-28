//! Session state types for save/restore on startup.
//!
//! This module provides **automatic session persistence**: save the current window
//! layout, tabs, and pane splits on clean exit, then restore them on next launch.
//!
//! # Relationship to `crate::arrangements`
//!
//! par-term has two overlapping session persistence mechanisms:
//!
//! | Feature | Module | Trigger | Scope |
//! |---------|--------|---------|-------|
//! | Auto session restore | `crate::session` (this module) | Automatic on clean exit / next launch | Last-session state only (single slot) |
//! | Named arrangements | `crate::arrangements` | User-initiated save/restore via UI | Multiple named snapshots, monitor-aware |
//!
//! Both capture window positions, sizes, tab CWDs, and tab titles using similar
//! serialization patterns (serde JSON via `storage` submodule). The key distinction
//! is lifecycle: session state is ephemeral (overwritten on each clean exit), while
//! arrangements are user-named and persist indefinitely.
//!
//! The `SessionTab` / `SessionWindow` types in this module and the `TabSnapshot` /
//! `WindowSnapshot` types in `crate::arrangements` serve analogous roles and have
//! similar shapes. A future refactor could unify them under a shared snapshot type
//! in `par-term-config`, but doing so would require coordinating the different
//! restore semantics (arrangements are monitor-aware; session restore is not).

pub mod capture;
pub mod restore;
pub mod storage;

use crate::pane::SplitDirection;
use serde::{Deserialize, Serialize};

/// Top-level session state: all windows at the time of save
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Timestamp when the session was saved (ISO 8601)
    pub saved_at: String,
    /// All windows in the session
    pub windows: Vec<SessionWindow>,
}

/// A single window in the saved session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWindow {
    /// Window position (x, y) in physical pixels
    pub position: (i32, i32),
    /// Window size (width, height) in physical pixels
    pub size: (u32, u32),
    /// Tabs in this window
    pub tabs: Vec<SessionTab>,
    /// Index of the active tab
    pub active_tab_index: usize,
}

/// A single tab in a saved session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTab {
    /// Working directory of the primary/focused pane
    pub cwd: Option<String>,
    /// Tab title
    pub title: String,
    /// Custom tab color (only saved when user set a color)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<[u8; 3]>,
    /// User-set tab title (present only when user manually named the tab)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_title: Option<String>,
    /// Custom icon set by user (persists across sessions)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
    /// Pane layout tree (None = single pane, use cwd above)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_layout: Option<SessionPaneNode>,
}

/// Recursive pane tree node for session persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionPaneNode {
    /// A terminal pane leaf
    Leaf {
        /// Working directory of this pane
        cwd: Option<String>,
    },
    /// A split containing two children
    Split {
        /// Split direction
        direction: SplitDirection,
        /// Split ratio (0.0-1.0)
        ratio: f32,
        /// First child (top/left)
        first: Box<SessionPaneNode>,
        /// Second child (bottom/right)
        second: Box<SessionPaneNode>,
    },
}
