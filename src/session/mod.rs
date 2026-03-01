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
//! # Shared types
//!
//! The common per-tab fields (`cwd`, `title`, `custom_color`, `user_title`,
//! `custom_icon`) are defined once in [`par_term_config::snapshot_types::TabSnapshot`]
//! and are embedded into [`SessionTab`] via `#[serde(flatten)]`.  The arrangements
//! module re-exports the same type directly, eliminating the previous duplication.
//! Existing YAML session files are fully backward-compatible — all fields remain at
//! the same nesting level.

pub mod capture;
pub mod restore;
pub mod storage;

// Re-export TabSnapshot so session consumers can use `crate::session::TabSnapshot`.
use crate::pane::SplitDirection;
pub use par_term_config::snapshot_types::TabSnapshot;
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

/// A single tab in a saved session.
///
/// The common tab fields (`cwd`, `title`, `custom_color`, `user_title`,
/// `custom_icon`) are inherited from [`TabSnapshot`] via `#[serde(flatten)]`
/// so that the serialized YAML layout is unchanged from before this refactor.
/// The session-specific field `pane_layout` is appended alongside the flattened
/// fields in the output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionTab {
    /// Common tab snapshot fields shared with the arrangements module
    #[serde(flatten)]
    pub snapshot: TabSnapshot,
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
