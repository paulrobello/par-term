use super::TerminalManager;
use anyhow::Result;

/// Resolve the user's login shell PATH and return environment variables for coprocess spawning.
///
/// On macOS (and other Unix), app bundles have a minimal PATH that doesn't include
/// user-installed paths like `/opt/homebrew/bin`, `/usr/local/bin`, etc.
/// This function runs the user's login shell once to resolve the full PATH,
/// caches the result, and returns it as a HashMap suitable for `CoprocessConfig.env`.
pub fn coprocess_env() -> std::collections::HashMap<String, String> {
    use std::sync::OnceLock;
    static CACHED_PATH: OnceLock<Option<String>> = OnceLock::new();

    let resolved_path = CACHED_PATH.get_or_init(|| {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            match std::process::Command::new(&shell)
                .args(["-lc", "printf '%s' \"$PATH\""])
                .output()
            {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout).to_string();
                    if !path.is_empty() {
                        log::debug!("Resolved login shell PATH: {}", path);
                        Some(path)
                    } else {
                        log::warn!("Login shell returned empty PATH");
                        None
                    }
                }
                Ok(output) => {
                    log::warn!(
                        "Login shell PATH resolution failed (exit={})",
                        output.status
                    );
                    None
                }
                Err(e) => {
                    log::warn!("Failed to run login shell for PATH resolution: {}", e);
                    None
                }
            }
        }
        #[cfg(not(unix))]
        {
            None
        }
    });

    let mut env = std::collections::HashMap::new();
    if let Some(path) = resolved_path {
        env.insert("PATH".to_string(), path.clone());
    }
    env
}

// ========================================================================
// Shell spawn methods
// ========================================================================

impl TerminalManager {
    /// Spawn a shell in the terminal
    pub fn spawn_shell(&mut self) -> Result<()> {
        log::info!("Spawning shell in PTY");
        let mut pty = self.pty_session.lock();
        pty.spawn_shell()
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell command in the terminal
    pub fn spawn_custom_shell(&mut self, command: &str) -> Result<()> {
        log::info!("Spawning custom shell: {}", command);
        let mut pty = self.pty_session.lock();
        let args: Vec<&str> = Vec::new();
        pty.spawn(command, &args)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell with arguments
    pub fn spawn_custom_shell_with_args(&mut self, command: &str, args: &[String]) -> Result<()> {
        log::info!("Spawning custom shell: {} with args: {:?}", command, args);
        let mut pty = self.pty_session.lock();
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        pty.spawn(command, &args_refs)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn shell with optional working directory and environment variables
    pub fn spawn_shell_with_dir(
        &mut self,
        working_dir: Option<&str>,
        env_vars: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<()> {
        log::info!(
            "Spawning shell with dir: {:?}, env: {:?}",
            working_dir,
            env_vars
        );
        let mut pty = self.pty_session.lock();
        pty.spawn_shell_with_env(env_vars, working_dir)
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell with env: {}", e))
    }

    /// Spawn custom shell with args, optional working directory, and environment variables
    pub fn spawn_custom_shell_with_dir(
        &mut self,
        command: &str,
        args: Option<&[String]>,
        working_dir: Option<&str>,
        env_vars: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<()> {
        log::info!(
            "Spawning custom shell: {} with dir: {:?}, env: {:?}",
            command,
            working_dir,
            env_vars
        );

        let args_refs: Vec<&str> = args
            .map(|a| a.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        let mut pty = self.pty_session.lock();
        pty.spawn_with_env(command, &args_refs, env_vars, working_dir)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell with env: {}", e))
    }
}

// ========================================================================
// PTY I/O methods
// ========================================================================

impl TerminalManager {
    /// Write data to the PTY (send user input to shell)
    pub fn write(&self, data: &[u8]) -> Result<()> {
        if !data.is_empty() {
            log::debug!(
                "Writing to PTY: {:?} (bytes: {:?})",
                String::from_utf8_lossy(data),
                data
            );
        }
        let mut pty = self.pty_session.lock();
        pty.write(data)
            .map_err(|e| anyhow::anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Write string to the PTY
    pub fn write_str(&self, data: &str) -> Result<()> {
        let mut pty = self.pty_session.lock();
        pty.write_str(data)
            .map_err(|e| anyhow::anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Process raw data through the terminal emulator (for tmux output routing).
    pub fn process_data(&self, data: &[u8]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.process(data);
    }

    /// Paste text to the terminal with proper bracketed paste handling.
    pub fn paste(&self, content: &str) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let content = content.replace('\n', "\r");

        log::debug!("Pasting {} chars (bracketed paste check)", content.len());

        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

        let mut pty = self.pty_session.lock();
        if !start.is_empty() {
            log::debug!("Sending bracketed paste start sequence");
            pty.write(&start)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste start: {}", e))?;
        }
        pty.write(content.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to write paste content: {}", e))?;
        if !end.is_empty() {
            log::debug!("Sending bracketed paste end sequence");
            pty.write(&end)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste end: {}", e))?;
        }

        Ok(())
    }

    /// Paste text with a delay between lines.
    pub async fn paste_with_delay(&self, content: &str, delay_ms: u64) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

        if !start.is_empty() {
            let mut pty = self.pty_session.lock();
            pty.write(&start)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste start: {}", e))?;
        }

        let lines: Vec<&str> = content.split('\n').collect();
        let delay = tokio::time::Duration::from_millis(delay_ms);

        for (i, line) in lines.iter().enumerate() {
            let mut line_data = line.replace('\n', "\r");
            if i < lines.len() - 1 {
                line_data.push('\r');
            }

            {
                let mut pty = self.pty_session.lock();
                pty.write(line_data.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to write paste line: {}", e))?;
            }

            if i < lines.len() - 1 {
                tokio::time::sleep(delay).await;
            }
        }

        if !end.is_empty() {
            let mut pty = self.pty_session.lock();
            pty.write(&end)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste end: {}", e))?;
        }

        log::debug!(
            "Pasted {} lines with {}ms delay ({} chars total)",
            lines.len(),
            delay_ms,
            content.len()
        );

        Ok(())
    }
}

// ========================================================================
// Coprocess Management Methods
// ========================================================================

impl TerminalManager {
    pub fn start_coprocess(
        &self,
        config: par_term_emu_core_rust::coprocess::CoprocessConfig,
    ) -> std::result::Result<par_term_emu_core_rust::coprocess::CoprocessId, String> {
        let pty = self.pty_session.lock();
        pty.start_coprocess(config)
    }

    pub fn stop_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<(), String> {
        let pty = self.pty_session.lock();
        pty.stop_coprocess(id)
    }

    pub fn coprocess_status(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> Option<bool> {
        let pty = self.pty_session.lock();
        pty.coprocess_status(id)
    }

    pub fn read_from_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<Vec<String>, String> {
        let pty = self.pty_session.lock();
        pty.read_from_coprocess(id)
    }

    pub fn list_coprocesses(&self) -> Vec<par_term_emu_core_rust::coprocess::CoprocessId> {
        let pty = self.pty_session.lock();
        pty.list_coprocesses()
    }

    pub fn read_coprocess_errors(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<Vec<String>, String> {
        let pty = self.pty_session.lock();
        pty.read_coprocess_errors(id)
    }
}
