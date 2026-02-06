// Hide console window on Windows release builds
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use anyhow::Result;
use par_term::app::App;
use par_term::cli;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    // Process CLI arguments first (before logging init for cleaner output)
    let runtime_options = match cli::process_cli() {
        cli::CliResult::Exit(code) => {
            std::process::exit(code);
        }
        cli::CliResult::Continue(options) => options,
    };
    // Initialize unified logging — routes all log::info!() etc. to /tmp/par_term_debug.log.
    // When RUST_LOG is set, also mirrors to stderr for terminal debugging.
    // This ensures logs are always captured, even in macOS app bundles and Windows GUI apps
    // where stderr is invisible.
    par_term::debug::init_log_bridge();

    log::info!("Starting par-term terminal emulator");

    // Create Tokio runtime for async operations (PTY, etc.)
    let runtime = Arc::new(Runtime::new()?);

    // Create and run the application
    let app = App::new(Arc::clone(&runtime), runtime_options)?;
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
