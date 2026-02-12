//! Session state types for save/restore on startup
//!
//! This module provides automatic session persistence: save the current window
//! layout, tabs, and pane splits on clean exit, then restore them on next launch.

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
