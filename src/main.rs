#[macro_use]
mod debug;

mod app;
mod audio_bell;
mod cell_renderer;
mod clipboard_history_ui;
mod config;
mod custom_shader_renderer;
mod font_manager;
mod gpu_utils;
mod graphics_renderer;
mod help_ui;
mod input;
mod macos_metal; // macOS-specific CAMetalLayer configuration
mod menu;
mod renderer;
mod scroll_state;
mod scrollbar;
mod selection;
mod settings_ui;
mod settings_window;
mod shader_watcher;
mod styled_content;
mod tab;
mod tab_bar_ui;
mod terminal;
mod text_shaper;
mod themes;
mod url_detection;

use anyhow::Result;
use app::App;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    // Initialize logging - respect RUST_LOG env var, suppress verbose wgpu logs
    env_logger::Builder::from_default_env()
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("rodio", log::LevelFilter::Error)
        .filter_module("cpal", log::LevelFilter::Error)
        .init();

    log::info!("Starting par-term terminal emulator");

    // Create Tokio runtime for async operations (PTY, etc.)
    let runtime = Arc::new(Runtime::new()?);

    // Create and run the application
    let app = App::new(Arc::clone(&runtime))?;
    let result = app.run();

    // Explicitly shutdown the runtime to ensure all tasks are terminated
    // This prevents hanging on exit due to background tasks
    log::info!("Shutting down Tokio runtime");

    // Try to get exclusive ownership of runtime for shutdown
    // This will only succeed if all other Arc references are dropped
    match Arc::try_unwrap(runtime) {
        Ok(rt) => {
            // We have exclusive ownership, can shutdown gracefully
            rt.shutdown_timeout(std::time::Duration::from_secs(2));
            log::info!("Tokio runtime shutdown complete");
        }
        Err(arc) => {
            // Other references still exist, force shutdown background tasks
            log::warn!(
                "Runtime still has {} strong references, forcing shutdown",
                Arc::strong_count(&arc)
            );
            // Note: Runtime will be dropped when last Arc is dropped
        }
    }

    result
}
