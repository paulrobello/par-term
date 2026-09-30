//! Shared snapshot types for session and arrangement persistence.
//!
//! Both the automatic session restore (`crate::session`) and the named arrangements
//! feature (`crate::arrangements`) capture the same per-tab state when saving a
//! window layout. This module defines the common base type that both hierarchies
//! share so that the field definitions are not duplicated.
//!
//! # Type relationships
//!
//! ```text
//! par-term-config::snapshot_types::TabSnapshot   (shared base)
//!         ↑                                ↑
//! par-term-settings-ui::arrangements       src/session
//!   TabSnapshot (re-export)                SessionTab { #[serde(flatten)] TabSnapshot }
//! ```
//!
//! The pane tree ([`SessionPaneNode`]) lives on [`TabSnapshot`] so that both
//! session restore and named arrangements keep split layouts (UX.md PN11).
//!
//! # Serialization compatibility
//!
//! All types derive `Serialize`/`Deserialize`.  The `#[serde(flatten)]` usage in
//! `SessionTab` means existing YAML files do not need to change — all fields are
//! written at the same level as before.

use serde::{Deserialize, Serialize};

/// Snapshot of a single tab's state.
///
/// This is the common base shared between the session-restore module
/// (`SessionTab`) and the named-arrangements module (`TabSnapshot`).
/// Both hierarchies capture exactly these fields; session additionally
/// stores `pane_layout` on top of them.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TabSnapshot {
    /// Working directory (from `Tab::get_cwd()`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Tab title
    #[serde(default)]
    pub title: String,

    /// Custom tab color set by the user
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<[u8; 3]>,

    /// User-set tab title (present only when the user manually named the tab)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_title: Option<String>,

    /// User-set title of the tab's sole pane (present only when the user
    /// renamed a pane in a single-pane tab). Multi-pane layouts carry pane
    /// titles per-leaf in `pane_layout` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_user_title: Option<String>,

    /// Custom icon set by the user (persists across sessions)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,

    /// Pane layout tree. `None` for a single-pane tab, which restores from
    /// `cwd`; only split roots are stored, so a restore never replaces the
    /// tab's own shell with a second one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_layout: Option<SessionPaneNode>,
}

/// Direction of a split
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Panes are stacked vertically (split creates top/bottom panes)
    Horizontal,
    /// Panes are side by side (split creates left/right panes)
    Vertical,
}

/// Recursive pane tree node for session and arrangement persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionPaneNode {
    /// A terminal pane leaf
    Leaf {
        /// Working directory of this pane
        cwd: Option<String>,
        /// User-set title of this pane (present only when the pane was
        /// user-named at save time; automatic titles are re-derived)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_title: Option<String>,
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
