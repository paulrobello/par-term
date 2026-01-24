use super::TerminalManager;
use anyhow::Result;

impl TerminalManager {
    /// Spawn a shell in the terminal
    #[allow(dead_code)]
    pub fn spawn_shell(&mut self) -> Result<()> {
        log::info!("Spawning shell in PTY");
        let mut pty = self.pty_session.lock();
        pty.spawn_shell()
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell command in the terminal
    ///
    /// # Arguments
    /// * `command` - The shell command to execute (e.g., "/bin/zsh", "fish")
    #[allow(dead_code)]
    pub fn spawn_custom_shell(&mut self, command: &str) -> Result<()> {
        log::info!("Spawning custom shell: {}", command);
        let mut pty = self.pty_session.lock();
        let args: Vec<&str> = Vec::new();
        pty.spawn(command, &args)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell with arguments
    ///
    /// # Arguments
    /// * `command` - The shell command to execute
    /// * `args` - Arguments to pass to the shell
    #[allow(dead_code)]
    pub fn spawn_custom_shell_with_args(&mut self, command: &str, args: &[String]) -> Result<()> {
        log::info!("Spawning custom shell: {} with args: {:?}", command, args);
        let mut pty = self.pty_session.lock();
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        pty.spawn(command, &args_refs)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn shell with optional working directory and environment variables
    ///
    /// # Arguments
    /// * `working_dir` - Optional working directory path
    /// * `env_vars` - Optional environment variables to set
    #[allow(dead_code)]
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
    ///
    /// # Arguments
    /// * `command` - The shell command to execute
    /// * `args` - Arguments to pass to the shell
    /// * `working_dir` - Optional working directory path
    /// * `env_vars` - Optional environment variables to set
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
