//! Application module for par-term
//!
//! This module contains the main application logic, including:
//! - `App`: Entry point that initializes and runs the event loop
//! - `WindowManager`: Manages multiple windows and coordinates menu events
//! - `WindowState`: Per-window state including terminal, renderer, and UI

use crate::cli::RuntimeOptions;
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
pub mod anti_idle;
mod notifications;
pub mod render_cache;
pub mod renderer_init;
pub mod scroll_ops;
pub mod search_highlight;
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
    runtime_options: RuntimeOptions,
}

impl App {
    /// Create a new application
    pub fn new(runtime: Arc<Runtime>, runtime_options: RuntimeOptions) -> Result<Self> {
        let mut config = Config::load()?;

        // Apply CLI shader override if specified
        if let Some(ref shader) = runtime_options.shader {
            config.custom_shader = Some(shader.clone());
            config.custom_shader_enabled = true;
            log::info!("CLI override: using shader '{}'", shader);
        }

        Ok(Self {
            config,
            runtime,
            runtime_options,
        })
    }

    /// Run the application
    pub fn run(self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        // Use Wait for power-efficient event handling
        // Combined with WaitUntil in about_to_wait for precise timing
        event_loop.set_control_flow(ControlFlow::Wait);

        let mut window_manager =
            WindowManager::new(self.config, self.runtime, self.runtime_options);

        event_loop.run_app(&mut window_manager)?;

        Ok(())
    }
}
