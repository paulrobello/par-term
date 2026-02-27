//! Window event handling and per-frame update methods for WindowState.
//!
//! Contains:
//! - Shell integration title/badge sync
//! - `handle_window_event`: routes winit WindowEvents to terminal/renderer
//! - `handle_focus_change`: power-saving focus logic
//! - `about_to_wait`: per-frame polling (notifications, tmux, config reload, etc.)

mod about_to_wait;
mod focus;
mod handle_window_event;
mod shell_exit;
mod shell_integration;
