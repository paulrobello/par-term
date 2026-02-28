//! Tab session logging methods.
//!
//! Provides methods for toggling session logging on/off and querying its state.

use crate::config::Config;
use crate::session_logger::SessionLogger;
use crate::tab::Tab;
use std::sync::Arc;

impl Tab {
    /// Toggle session logging on/off.
    ///
    /// Returns `Ok(true)` if logging is now active, `Ok(false)` if stopped.
    /// If logging wasn't active and no logger exists, creates a new one.
    pub fn toggle_session_logging(&mut self, config: &Config) -> anyhow::Result<bool> {
        let mut logger_guard = self.session_logger.lock();

        if let Some(ref mut logger) = *logger_guard {
            // Logger exists - toggle based on current state
            if logger.is_active() {
                logger.stop()?;
                log::info!("Session logging stopped via hotkey");
                Ok(false)
            } else {
                logger.start()?;
                log::info!("Session logging started via hotkey");
                Ok(true)
            }
        } else {
            // No logger exists - create one and start it
            let logs_dir = config.logs_dir();
            if let Err(e) = std::fs::create_dir_all(&logs_dir) {
                log::warn!("Failed to create logs directory: {}", e);
                return Err(anyhow::anyhow!("Failed to create logs directory: {}", e));
            }

            // Get terminal dimensions
            let dimensions = if let Ok(term) = self.terminal.try_write() {
                term.dimensions()
            } else {
                (80, 24) // fallback
            };

            let session_title = Some(format!(
                "{} - {}",
                self.title,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            let mut logger = SessionLogger::new(
                config.session_log_format,
                &logs_dir,
                dimensions,
                session_title,
            )?;

            logger.set_redact_passwords(config.session_log_redact_passwords);
            logger.start()?;

            // Set up output callback to record PTY output
            let logger_clone = Arc::clone(&self.session_logger);
            if let Ok(term) = self.terminal.try_write() {
                term.set_output_callback(move |data: &[u8]| {
                    if let Some(ref mut logger) = *logger_clone.lock() {
                        logger.record_output(data);
                    }
                });
            }

            *logger_guard = Some(logger);
            log::info!("Session logging created and started via hotkey");
            Ok(true)
        }
    }

    /// Check if session logging is currently active.
    pub fn is_session_logging_active(&self) -> bool {
        if let Some(ref logger) = *self.session_logger.lock() {
            logger.is_active()
        } else {
            false
        }
    }
}
