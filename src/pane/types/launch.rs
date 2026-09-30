//! The process a pane was started with, kept so a restart (UX.md A9) and
//! a split of a profile tab (D5) run the same program again.

/// A resolved program and arguments, with any environment the launcher
/// added on top of `shell_env` (a profile's `SHELL`, for example).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchCommand {
    /// Program to execute.
    pub program: String,
    /// Arguments, already including any login-shell flag.
    pub args: Option<Vec<String>>,
    /// Environment set on top of the configured shell environment.
    pub extra_env: Vec<(String, String)>,
}

impl LaunchCommand {
    /// A program with arguments and no extra environment.
    pub fn new(program: String, args: Vec<String>) -> Self {
        Self {
            program,
            args: Some(args),
            extra_env: Vec::new(),
        }
    }

    /// The configured shell environment plus this launch's additions.
    pub(crate) fn env(
        &self,
        config: &crate::config::Config,
    ) -> Option<std::collections::HashMap<String, String>> {
        let mut env = crate::tab::build_shell_env(config.shell.shell_env.as_ref());
        if let Some(env) = env.as_mut() {
            env.extend(self.extra_env.iter().cloned());
        }
        env
    }
}
