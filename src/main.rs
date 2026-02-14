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
    // CLI --log-level flag takes highest precedence, then RUST_LOG, then config (applied later).
    par_term::debug::init_log_bridge(runtime_options.log_level);

    log::info!("Starting par-term terminal emulator");

    // Clean up leftover .old binary from a previous self-update (Windows)
    par_term::self_updater::cleanup_old_binary();

    // Create Tokio runtime for async operations (PTY, etc.)
    let runtime = Arc::new(Runtime::new()?);

    // Create and run the application
    let app = App::new(Arc::clone(&runtime), runtime_options)?;
    let result = app.run();

    // All windows are closed and cleanup threads are running in background.
    // Force-exit the process immediately to avoid blocking on tokio runtime
    // shutdown or PtySession::drop timeouts. Background cleanup threads and
    // the OS will handle any remaining resource cleanup.
    log::info!("Event loop exited, force-exiting process");
    let exit_code = if result.is_ok() { 0 } else { 1 };
    std::process::exit(exit_code);
}
