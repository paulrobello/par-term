//! Pane management for split terminal support
//!
//! This module provides the pane infrastructure for split terminals:
//! - `Pane`: Represents a single terminal pane with its own state
//! - `PaneNode`: Tree structure for nested pane splits
//! - `PaneManager`: Coordinates pane operations within a tab
//! - `PaneId`: Unique identifier for each pane

pub mod bell;
mod manager;
pub mod mouse;
pub mod render_cache;
mod tmux_helpers;
mod types;

pub use manager::PaneManager;
pub use types::{
    DividerRect, NavigationDirection, Pane, PaneBackground, PaneBounds, PaneId, PaneNode,
    RestartState, SplitDirection,
};
