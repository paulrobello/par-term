//! Application module for par-term
//!
//! This module contains the main application logic, including:
//! - `App`: Entry point that initializes and runs the event loop
//! - `WindowManager`: Manages multiple windows and coordinates menu events
//! - `WindowState`: Per-window state including terminal, renderer, and UI

use crate::config::Config;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::event_loop::{ControlFlow, EventLoop};

pub mod bell;
pub mod config_updates;
pub mod debug_state;
pub mod handler;
pub mod input_events;
pub mod keyboard_handlers;
pub mod mouse;
pub mod mouse_events;
mod notifications;
pub mod render_cache;
pub mod renderer_init;
pub mod scroll_ops;
pub mod tab_ops;
pub mod text_selection;
pub mod url_hover;
pub mod window_manager;
pub mod window_state;

pub use window_manager::WindowManager;

/// Main application entry point
pub struct App {
    config: Config,
    runtime: Arc<Runtime>,
}

impl App {
    /// Create a new application
    pub fn new(runtime: Arc<Runtime>) -> Result<Self> {
        let config = Config::load()?;
        Ok(Self { config, runtime })
    }

    /// Run the application
    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        // Use Wait for power-efficient event handling
        // Combined with WaitUntil in about_to_wait for precise timing
        event_loop.set_control_flow(ControlFlow::Wait);

        let mut window_manager = WindowManager::new(self.config, self.runtime);

        event_loop.run_app(&mut window_manager)?;

        Ok(())
    }
}
