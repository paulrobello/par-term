//! Shared enums for the pane system.

// Re-export PaneId from par-term-config for shared access across subcrates
pub use par_term_config::PaneId;

// Re-export rendering types from par-term-config
pub use par_term_config::{DividerRect, PaneBackground};

/// State for shell restart behavior
#[derive(Debug, Clone)]
pub enum RestartState {
    /// Waiting for user to press Enter to restart
    AwaitingInput,
    /// Waiting for delay timer before restart
    AwaitingDelay(std::time::Instant),
}

/// Direction of a split
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDirection {
    /// Panes are stacked vertically (split creates top/bottom panes)
    Horizontal,
    /// Panes are side by side (split creates left/right panes)
    Vertical,
}

/// Direction for pane navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    Left,
    Right,
    Up,
    Down,
}
