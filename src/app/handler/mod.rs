//! Application event handler
//!
//! This module implements the winit `ApplicationHandler` trait for `WindowManager`,
//! routing window events to the appropriate `WindowState` and handling menu events.
//!
//! ## Sub-modules
//!
//! - `window_state_impl`: `impl WindowState` — shell integration, window event routing,
//!   focus change, and per-frame `about_to_wait` polling.
//! - `app_handler_impl`: `impl ApplicationHandler for WindowManager` — winit event loop
//!   entry points (`resumed`, `window_event`, `about_to_wait`).

mod app_handler_impl;
mod window_state_impl;
