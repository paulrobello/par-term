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
//!   TabSnapshot (re-export)                SessionTab { #[serde(flatten)] TabSnapshot, pane_layout }
//! ```
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

    /// Custom icon set by the user (persists across sessions)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
}
