//! Pane terminal operations: refresh task, resize, shell restart, and title.
//!
//! Split from `pane/types/pane.rs` to bring that file under 500 lines.
//! Contains the `Pane` impl blocks for long-running async operations and
//! terminal management helpers that don't fit in the struct definition file.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::runtime::Runtime;

use crate::config::Config;
use crate::tab::build_shell_env;

use super::pane::Pane;

impl Pane {
    /// Respawn the shell in this pane
    ///
    /// This resets the terminal state and spawns a new shell process.
    /// Used when shell_exit_action is one of the restart variants.
    pub fn respawn_shell(&mut self, config: &Config) -> anyhow::Result<()> {
        // Clear restart state
        self.restart_state = None;
        self.exit_notified = false;

        // Determine the shell command to use
        #[allow(unused_mut)]
        let (shell_cmd, mut shell_args) = if let Some(ref custom) = config.custom_shell {
            (custom.clone(), config.shell_args.clone())
        } else {
            #[cfg(target_os = "windows")]
            {
                ("powershell.exe".to_string(), None)
            }
            #[cfg(not(target_os = "windows"))]
            {
                (
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
                    None,
                )
            }
        };

        // On Unix-like systems, spawn as login shell if configured
        #[cfg(not(target_os = "windows"))]
        if config.login_shell {
            let args = shell_args.get_or_insert_with(Vec::new);
            if !args.iter().any(|a| a == "-l" || a == "--login") {
                args.insert(0, "-l".to_string());
            }
        }

        // Determine working directory (use current CWD if available, else config)
        let work_dir = self
            .get_cwd()
            .or_else(|| self.working_directory.clone())
            .or_else(|| config.working_directory.clone());

        let shell_args_deref = shell_args.as_deref();
        let shell_env = build_shell_env(config.shell_env.as_ref());

        // Respawn the shell
        if let Ok(mut term) = self.terminal.try_write() {
            // Clear the screen before respawning (using VT escape sequence)
            // This clears screen and moves cursor to home position
            term.process_data(b"\x1b[2J\x1b[H");

            // Spawn new shell
            term.spawn_custom_shell_with_dir(
                &shell_cmd,
                shell_args_deref,
                work_dir.as_deref(),
                shell_env.as_ref(),
            )?;

            log::info!("Respawned shell in pane {}", self.id);
        }

        Ok(())
    }

    /// Write a restart prompt message to the terminal
    pub fn write_restart_prompt(&self) {
        if let Ok(term) = self.terminal.try_write() {
            // Write the prompt message directly to terminal display
            let message = "\r\n[Process exited. Press Enter to restart...]\r\n";
            term.process_data(message.as_bytes());
        }
    }

    /// Get the title for this pane (from OSC or CWD)
    pub fn get_title(&self) -> String {
        if let Ok(term) = self.terminal.try_write() {
            let osc_title = term.get_title();
            if !osc_title.is_empty() {
                return osc_title;
            }
            if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                // Use just the last component for brevity
                if let Some(last) = abbreviated.rsplit('/').next()
                    && !last.is_empty()
                {
                    return last.to_string();
                }
                return abbreviated;
            }
        }
        format!("Pane {}", self.id)
    }

    /// Start the refresh polling task for this pane
    pub fn start_refresh_task(
        &mut self,
        runtime: Arc<Runtime>,
        window: Arc<winit::window::Window>,
        active_fps: u32,
        inactive_fps: u32,
    ) {
        let terminal_clone = Arc::clone(&self.terminal);
        let is_active = Arc::clone(&self.is_active);
        let active_interval_ms = (1000 / active_fps.max(1)) as u64;
        let inactive_interval_ms = (1000 / inactive_fps.max(1)) as u64;

        let handle = runtime.spawn(async move {
            let mut last_gen = 0u64;
            let mut idle_streak = 0u32;
            const MAX_INACTIVE_IDLE_INTERVAL_MS: u64 = 250;

            loop {
                let is_active_now = is_active.load(Ordering::Relaxed);
                // Keep the active tab responsive: only apply backoff to inactive tabs.
                let interval_ms = if is_active_now {
                    active_interval_ms
                } else if idle_streak > 0 {
                    (inactive_interval_ms << idle_streak.min(4)).min(MAX_INACTIVE_IDLE_INTERVAL_MS)
                } else {
                    inactive_interval_ms
                };
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;

                let should_redraw = if let Ok(term) = terminal_clone.try_read() {
                    let current_gen = term.update_generation();
                    if current_gen > last_gen {
                        last_gen = current_gen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                if should_redraw {
                    idle_streak = 0;
                    window.request_redraw();
                } else if is_active_now {
                    idle_streak = 0;
                } else {
                    idle_streak = idle_streak.saturating_add(1);
                }
            }
        });

        self.refresh_task = Some(handle);
    }

    /// Stop the refresh polling task
    pub fn stop_refresh_task(&mut self) {
        if let Some(handle) = self.refresh_task.take() {
            handle.abort();
        }
    }

    /// Resize the terminal to match the pane bounds
    pub fn resize_terminal(&self, cols: usize, rows: usize) {
        if let Ok(mut term) = self.terminal.try_write()
            && term.dimensions() != (cols, rows)
        {
            let _ = term.resize(cols, rows);
        }
    }

    /// Resize the terminal and update cell pixel dimensions.
    ///
    /// Unlike `resize_terminal`, this also calls `set_cell_dimensions` so that
    /// the core library tracks `scroll_offset_rows` in display-cell units rather
    /// than its internal default (2 px per row).  Must be called whenever the
    /// display cell size is known (e.g., on every layout pass).
    pub fn resize_terminal_with_cell_dims(
        &self,
        cols: usize,
        rows: usize,
        cell_width: u32,
        cell_height: u32,
    ) {
        if let Ok(mut term) = self.terminal.try_write() {
            term.set_cell_dimensions(cell_width, cell_height);
            if term.dimensions() != (cols, rows) {
                let _ = term.resize(cols, rows);
            }
        }
    }
}
