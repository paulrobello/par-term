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

pub(crate) mod copy_mode;
mod file_transfers;
pub mod handler;
pub mod input_events;
pub mod mouse_events;
pub(crate) mod render_pipeline;
pub mod tab_ops;
mod tmux_handler;
mod triggers;
pub mod window_manager;
pub mod window_state;

pub(crate) use window_manager::WindowManager;

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
            config.shader.custom_shader = Some(shader.clone());
            config.shader.custom_shader_enabled = true;
            // Disable background image so it doesn't compete with the shader
            config.background_image_enabled = false;
            log::info!("CLI override: using shader '{}'", shader);
        }

        // Apply CLI session logging override if specified
        if runtime_options.log_session {
            config.auto_log_sessions = true;
            log::info!("CLI override: session logging enabled");
        }

        // Apply config log level (unless CLI --log-level was specified)
        if runtime_options.log_level.is_none() {
            crate::debug::set_log_level(config.log_level.to_level_filter());
            log::info!("Config log level: {}", config.log_level.display_name());
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
