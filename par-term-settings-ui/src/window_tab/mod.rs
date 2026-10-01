//! Window, tab bar, scrollbar, pane, and performance sections. Where each
//! one is drawn is set in [`crate::layout`].
//!
//! - Display settings (title, dimensions, padding)
//! - Transparency settings (opacity, blur)
//! - Performance settings (FPS, VSync, power saving)
//! - Window behavior (decorations, always on top, etc.)
//! - Tab bar settings and colors
//! - Split panes settings and appearance
//! - Scrollbar

pub(crate) mod behavior;
pub(crate) mod display;
pub(crate) mod panes;
pub(crate) mod performance;
pub(crate) mod scrollbar;
pub(crate) mod tab_bar;
mod tab_bar_appearance;
mod tab_bar_behavior;
pub(crate) mod transparency;
