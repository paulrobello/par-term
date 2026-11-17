//! Tab session logging methods.
//!
//! Provides methods for toggling session logging on/off and querying its state,
//! plus the shared auto-logging arming used by tab construction.

use crate::config::{Config, SessionLogFormat};
use crate::session_logger::{SessionLogger, SharedSessionLogger};
use crate::tab::Tab;
use par_term_terminal::TerminalManager;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Arm a session logger under `auto_log_sessions`, storing it in `slot` and
/// attaching the output callback on `terminal`.
///
/// Shared by `Tab::new_internal` (new tabs) and `Tab::new_from_pane`
/// (promoted tabs — a `Pane`'s logger slot is born empty, so without this the
/// promoted tab would never record a `.cast`, card 01a1dae3). Idempotent: a
/// slot that already holds a logger is preserved untouched.
pub(crate) fn arm_auto_session_logger(
    slot: &SharedSessionLogger,
    terminal: &Arc<RwLock<TerminalManager>>,
    config: &Config,
    title: &str,
) -> bool {
    if !config.session_log.auto_log_sessions {
        return false;
    }
    let mut guard = slot.lock();
    if guard.is_some() {
        return false;
    }
    let logs_dir = config.logs_dir();

    // SEC-010: Ensure the logs directory exists with owner-only permissions
    // (0o700) so session logs are not world-listable.
    if let Err(e) = std::fs::create_dir_all(&logs_dir) {
        log::warn!("Failed to create logs directory {logs_dir:?}: {e}");
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&logs_dir, std::fs::Permissions::from_mode(0o700));
    }

    let title_with_ts = Some(format!(
        "{} - {}",
        title,
        jiff::ZonedDateTime::now().strftime("%Y-%m-%d %H:%M:%S")
    ));

    match SessionLogger::new(
        config.session_log.session_log_format,
        &logs_dir,
        (config.cols, config.rows),
        title_with_ts,
    ) {
        Ok(mut logger) => {
            logger.set_redact_passwords(config.session_log.session_log_redact_passwords);
            if let Err(e) = logger.start() {
                log::warn!("Failed to start session logging: {e}");
                return false;
            }
            // v3 logs get the graphics-aware exporter: it ferries the
            // recording to the core lib's export through the terminal at
            // stop() time.
            if config.session_log.session_log_format == SessionLogFormat::AsciicastV3 {
                let tm = Arc::clone(terminal);
                logger.set_v3_exporter(Box::new(move |session| {
                    tm.try_read()
                        .ok()
                        .map(|tm| tm.export_session_asciicast_v3(session))
                }));
            }
            let logger_clone = Arc::clone(slot);
            if let Ok(term) = terminal.try_read() {
                term.set_output_callback(move |data: &[u8]| {
                    if let Some(ref mut logger) = *logger_clone.lock() {
                        logger.record_output(data);
                    }
                });
            } else {
                log::warn!(
                    "Terminal lock held while arming session logging; callback not attached"
                );
                return false;
            }
            log::info!("Session logging started: {:?}", logger.output_path());
            *guard = Some(logger);
            true
        }
        Err(e) => {
            log::warn!("Failed to create session logger: {e}");
            false
        }
    }
}

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
            // SEC-010: Enforce restrictive directory permissions so that session logs
            // (which may capture terminal output including passwords) are not
            // world-listable even if the user's umask is permissive.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&logs_dir, std::fs::Permissions::from_mode(0o700));
            }

            // Get terminal dimensions
            let dimensions = if let Ok(term) = self.terminal.try_read() {
                term.dimensions()
            } else {
                (80, 24) // fallback
            };

            let session_title = Some(format!(
                "{} - {}",
                self.title,
                jiff::ZonedDateTime::now().strftime("%Y-%m-%d %H:%M:%S")
            ));

            let mut logger = SessionLogger::new(
                config.session_log.session_log_format,
                &logs_dir,
                dimensions,
                session_title,
            )?;

            logger.set_redact_passwords(config.session_log.session_log_redact_passwords);

            // v3 logs get the graphics-aware exporter: it ferries the recording
            // to the core lib's export through the terminal at stop() time.
            if config.session_log.session_log_format == SessionLogFormat::AsciicastV3 {
                let tm = Arc::clone(&self.terminal);
                logger.set_v3_exporter(Box::new(move |session| {
                    tm.try_read()
                        .ok()
                        .map(|tm| tm.export_session_asciicast_v3(session))
                }));
            }

            logger.start()?;

            // SEC-002: Emit a prominent one-time warning when session logging is enabled.
            // Session logs capture raw terminal I/O and may include passwords, API keys,
            // tokens, and other credentials even with heuristic redaction active.
            eprintln!(
                "\n[par-term SESSION LOGGING ENABLED]\n\
                 Session output is now being recorded to: {}\n\
                 WARNING: Session logs may capture sensitive data (passwords, API keys, tokens)\n\
                 despite heuristic redaction. Password-prompt detection is enabled: {}\n\
                 Do NOT share session log files unless you have reviewed their contents.\n\
                 Disable session logging when working with sensitive credentials.\n",
                logs_dir.display(),
                config.session_log.session_log_redact_passwords,
            );

            // Set up output callback to record PTY output
            let logger_clone = Arc::clone(&self.session_logger);
            if let Ok(term) = self.terminal.try_read() {
                term.set_output_callback(move |data: &[u8]| {
                    if let Some(ref mut logger) = *logger_clone.lock() {
                        logger.record_output(data);
                    }
                });
            }

            *logger_guard = Some(logger);
            drop(logger_guard);

            // In a mux tab the panes are mirrors fed through
            // `process_mux_output`, which fires the per-terminal output
            // callback — they need the same attachment or their output
            // never reaches the log.
            self.attach_session_logging_to_panes();
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

    /// Attach the tab's session-logger output callback to every pane
    /// terminal of a tmux/mux tab.
    ///
    /// Mirror panes receive daemon output through `process_mux_output`,
    /// which fires the per-terminal output callback; the toggle/auto-log
    /// paths attach only on `tab.terminal`, which in a mux tab is the
    /// hidden shell whose output is not what the user sees. Called when
    /// logging starts and whenever a tmux layout change creates panes, so
    /// mirrors born mid-session log too. The callback captures the shared
    /// logger slot, so one attached before a later toggle still picks up
    /// the logger stored into it.
    pub(crate) fn attach_session_logging_to_panes(&self) {
        if self.session_logger.lock().is_none() {
            return;
        }
        if !(self.tmux.tmux_gateway_active || self.tmux.tmux_pane_id.is_some()) {
            return;
        }
        if let Some(pm) = self.pane_manager() {
            for pane in pm.all_panes() {
                let logger_clone = Arc::clone(&self.session_logger);
                if let Ok(term) = pane.terminal.try_read() {
                    term.set_output_callback(move |data: &[u8]| {
                        if let Some(ref mut logger) = *logger_clone.lock() {
                            logger.record_output(data);
                        }
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod arm_tests {
    use super::*;
    use crate::pane::ExtractResult;
    use crate::tab::TabInitParams;
    use par_term_terminal::TerminalManager;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;

    /// A tab built through the real `new_internal` against a PTY-free
    /// terminal, with auto-logging pointed at a temp dir.
    fn source_tab(config: &Config) -> Tab {
        let terminal = TerminalManager::new_with_scrollback(80, 24, 100).expect("terminal");
        Tab::new_internal(
            TabInitParams {
                id: 1,
                title: "Tab 1".to_string(),
                has_default_title: true,
                default_number: 1,
                user_named: false,
                working_directory: None,
                runtime: None,
            },
            terminal,
            config,
            "Tab 1".to_string(),
        )
        .expect("tab construction")
    }

    fn test_config(logs_dir: &TempDir) -> Config {
        let mut config = Config::default();
        config.session_log.auto_log_sessions = true;
        config.session_log.session_log_directory = logs_dir.path().to_string_lossy().to_string();
        config
    }

    /// Card 01a1dae3: a `Pane`'s logger slot is born empty and nothing arms
    /// it, so a promoted tab adopted a dead slot and never recorded. The
    /// promote path must arm auto-logging on the pane's terminal, and output
    /// routed through the pane must reach the recording (streamed v2: the
    /// bytes are on disk without any stop()).
    #[test]
    fn promoted_tab_arms_session_logging_and_records_output() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let runtime = Arc::new(Runtime::new().unwrap());

        let mut source = source_tab(&config);
        let pane_id = source.pane_manager.as_ref().unwrap().all_panes()[0].id;
        let pane = match source.pane_manager.as_mut().unwrap().extract_pane(pane_id) {
            ExtractResult::OnlyPane(pane) => pane,
            _ => panic!("expected OnlyPane from a single-pane tab"),
        };

        let promoted = Tab::new_from_pane(2, pane, &config, runtime, 1);
        {
            let guard = promoted.session_logger.lock();
            let logger = guard.as_ref().expect("promoted tab must arm auto-logging");
            assert!(logger.is_active(), "the armed logger must be started");
            assert!(
                logger.output_path().starts_with(temp.path()),
                "the .cast must live in the configured logs dir: {:?}",
                logger.output_path()
            );
        }

        // Output routed through the pane's terminal (the same seam a mux
        // mirror uses) reaches the recording and streams to disk.
        let pane_term = promoted
            .pane_manager
            .as_ref()
            .unwrap()
            .get_pane(pane_id)
            .expect("the promoted pane")
            .terminal
            .clone();
        pane_term
            .try_read()
            .expect("pane terminal")
            .process_mux_output(b"PROMOTE-MARKER\r\n");
        let guard = promoted.session_logger.lock();
        let logger = guard.as_ref().unwrap();
        let content = std::fs::read_to_string(logger.output_path()).expect("the .cast must exist");
        assert!(
            content.contains("PROMOTE-MARKER"),
            "output must reach the recording without a stop(): {content:?}"
        );
    }

    /// The arming is idempotent per slot: a tab whose logger slot already
    /// holds a logger keeps it untouched (a promoted mux-mirror pane with an
    /// armed slot must not get a second logger).
    #[test]
    fn arming_preserves_an_existing_logger_slot() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let runtime = Arc::new(Runtime::new().unwrap());

        let mut source = source_tab(&config);
        let pane_id = source.pane_manager.as_ref().unwrap().all_panes()[0].id;
        let pane = match source.pane_manager.as_mut().unwrap().extract_pane(pane_id) {
            ExtractResult::OnlyPane(pane) => pane,
            _ => panic!("expected OnlyPane from a single-pane tab"),
        };

        // Pre-arm the pane's slot as a mux path would.
        let prearmed_path = {
            let mut slot = pane.session_logger.lock();
            let mut logger = SessionLogger::new(
                config.session_log.session_log_format,
                temp.path(),
                (80, 24),
                Some("prearmed".to_string()),
            )
            .unwrap();
            logger.start().unwrap();
            let path = logger.output_path().clone();
            *slot = Some(logger);
            path
        };

        let promoted = Tab::new_from_pane(2, pane, &config, runtime, 1);
        let guard = promoted.session_logger.lock();
        let logger = guard.as_ref().expect("the pre-armed slot must survive");
        assert_eq!(logger.output_path(), &prearmed_path);
    }

    /// Card 01a1dbf5: a real resize of the tab terminal must land an `r`
    /// event in the recording (streamed v2), so replays reconstruct the
    /// geometry timeline instead of rendering at the header's stale size.
    #[test]
    fn tab_terminal_resize_records_r_event() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let mut tab = source_tab(&config); // armed via new_internal, terminal 80x24

        // Drive the real local relayout path to a 100x30 grid.
        tab.set_pane_bounds_with_padding(
            crate::pane::PaneBounds {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 600.0,
            },
            10.0,
            20.0,
            0.0,
        );

        let dims = tab.terminal.try_read().unwrap().dimensions();
        assert_eq!(dims, (100, 30), "relayout must resize the tab terminal");
        let guard = tab.session_logger.lock();
        let logger = guard.as_ref().unwrap();
        let content = std::fs::read_to_string(logger.output_path()).unwrap();
        assert!(
            content.contains("\"r\",\"100x30\""),
            "cast must carry the resize event: {content:?}"
        );
    }

    /// The per-pane resize seam: `Pane::resize_terminal` records onto an
    /// armed pane logger — the path the mux layout reflow
    /// (`update_layout_from_tmux`) uses to size daemon leaves.
    #[test]
    fn pane_resize_records_to_armed_pane_logger() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let source = source_tab(&config);
        let pane_id = source.pane_manager.as_ref().unwrap().all_panes()[0].id;

        // Arm the pane's own slot as a mux mirror path would.
        let pane_path = {
            let pm = source.pane_manager.as_ref().unwrap();
            let pane = pm.get_pane(pane_id).unwrap();
            let mut slot = pane.session_logger.lock();
            let mut logger = SessionLogger::new(
                config.session_log.session_log_format,
                temp.path(),
                (80, 24),
                Some("pane".to_string()),
            )
            .unwrap();
            logger.start().unwrap();
            let path = logger.output_path().clone();
            *slot = Some(logger);
            path
        };

        let resized = source
            .pane_manager
            .as_ref()
            .unwrap()
            .get_pane(pane_id)
            .unwrap()
            .resize_terminal(120, 40);
        assert!(resized, "the pane must actually resize");
        let content = std::fs::read_to_string(&pane_path).unwrap();
        assert!(
            content.contains("\"r\",\"120x40\""),
            "pane recording must carry the resize event: {content:?}"
        );
    }
}
