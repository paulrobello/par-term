//! Core types for the pane system.
//!
//! This module defines the fundamental data structures for split panes:
//! - Binary tree structure for arbitrary nesting
//! - Per-pane state (terminal, scroll, mouse, etc.)
//! - Bounds calculation for rendering
//!
//! Sub-modules:
//! - [`bounds`]    — `PaneBounds` pixel-space bounding box
//! - [`common`]    — `RestartState`, `SplitDirection`, `NavigationDirection`, re-exports
//! - [`pane`]      — `Pane` struct, constructors, methods, and `Drop`
//! - [`pane_node`] — `PaneNode` binary tree for pane layout

mod bounds;
mod common;
mod pane;
mod pane_node;

#[cfg(test)]
mod tests;

// Re-export all public types so `pane::types::Foo` still resolves correctly
// and `pane/mod.rs` re-exports are unchanged.
pub use bounds::PaneBounds;
pub use common::{DividerRect, NavigationDirection, PaneBackground, PaneId, RestartState,
    SplitDirection};
pub use pane::Pane;
pub use pane_node::PaneNode;
