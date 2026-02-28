// Hide console window on Windows release builds
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! par-term entry point.
//!
//! # Logging Quick Reference
//!
//! par-term has **two parallel logging systems** that both write to the same file
//! (`/tmp/par_term_debug.log`, respects `$TMPDIR`/`%TEMP%`):
//!
//! | System | Macros | Control | Best for |
//! |--------|--------|---------|----------|
//! | Custom debug | `crate::debug_info!("CAT", ...)` | `DEBUG_LEVEL=0-4` env var | High-frequency render/input events with category tags |
//! | Standard `log` crate | `log::info!(...)`, `log::warn!()`, etc. | `RUST_LOG` env var | Application lifecycle, startup/shutdown, config, I/O errors |
//!
//! **Rule of thumb**: use `log::*!()` for events that happen once (startup, config load,
//! profile switch, errors). Use `crate::debug_*!()` for events that fire every frame or
//! on every keystroke (rendering, input, shader updates). Third-party crates (wgpu, tokio,
//! etc.) emit only through `log`, never through the custom macros.
//!
//! See `src/debug.rs` for macro definitions and `docs/LOGGING.md` for full documentation.

use anyhow::Result;
use par_term::app::App;
use par_term::cli;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    // Process CLI arguments first (before logging init for cleaner output)
    let runtime_options = match cli::process_cli() {
        cli::CliResult::Exit(code) => {
            if code == 0 {
                return Ok(());
            }
            // Non-zero exit: use process::exit so the shell sees the correct
            // exit code. No app state exists yet, so no destructors are skipped.
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

    // Event loop has exited. All windows have already been closed and their
    // Drop impls have run (PTY cleanup, session saves, etc.).
    // Drop the runtime explicitly so Tokio can shut down its worker threads
    // before main returns. Use `shutdown_timeout` to avoid blocking forever
    // if a background task hangs.
    log::info!("Event loop exited, shutting down runtime");
    let rt = Arc::try_unwrap(runtime).ok();
    if let Some(rt) = rt {
        rt.shutdown_timeout(std::time::Duration::from_secs(2));
    }

    match result {
        Ok(_) => Ok(()),
        Err(ref e) => {
            eprintln!("par-term: error: {e:#}");
            // On Linux, provide a hint when the error looks like a missing display server
            #[cfg(target_os = "linux")]
            {
                let msg = format!("{e:?}").to_lowercase();
                if msg.contains("display")
                    || msg.contains("wayland")
                    || msg.contains("xcb")
                    || msg.contains("x server")
                    || msg.contains("compositor")
                {
                    eprintln!(
                        "par-term: hint: no display server found — ensure DISPLAY (X11) or \
                         WAYLAND_DISPLAY (Wayland) is set and a compositor is running"
                    );
                }
            }
            // Return the original error so main exits with code 1 (anyhow default)
            result
        }
    }
}
