//! Command-line interface for par-term.
//!
//! This module handles CLI argument parsing and subcommands like shader installation.
//! Install/uninstall procedure implementations live in the [`install`] submodule.

pub mod install;

use crate::config::ShellType;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Shell type argument for CLI
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShellTypeArg {
    Bash,
    Zsh,
    Fish,
}

impl From<ShellTypeArg> for ShellType {
    fn from(arg: ShellTypeArg) -> Self {
        match arg {
            ShellTypeArg::Bash => ShellType::Bash,
            ShellTypeArg::Zsh => ShellType::Zsh,
            ShellTypeArg::Fish => ShellType::Fish,
        }
    }
}

/// par-term - A GPU-accelerated terminal emulator
#[derive(Parser)]
#[command(name = "par-term")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Background shader to use (filename from shaders directory)
    #[arg(long, value_name = "SHADER")]
    pub shader: Option<String>,

    /// Exit after the specified number of seconds
    #[arg(long, value_name = "SECONDS")]
    pub exit_after: Option<f64>,

    /// Take a screenshot and save to the specified path (default: timestamped PNG in current dir)
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "")]
    pub screenshot: Option<PathBuf>,

    /// Send a command to the shell after 1 second delay
    #[arg(long, value_name = "COMMAND")]
    pub command_to_send: Option<String>,

    /// Enable session logging (overrides config setting)
    #[arg(long)]
    pub log_session: bool,

    /// Set debug log level (overrides config and RUST_LOG)
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub log_level: Option<LogLevelArg>,
}

/// Log level argument for CLI
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum LogLevelArg {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevelArg {
    /// Convert to `log::LevelFilter`
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            LogLevelArg::Off => log::LevelFilter::Off,
            LogLevelArg::Error => log::LevelFilter::Error,
            LogLevelArg::Warn => log::LevelFilter::Warn,
            LogLevelArg::Info => log::LevelFilter::Info,
            LogLevelArg::Debug => log::LevelFilter::Debug,
            LogLevelArg::Trace => log::LevelFilter::Trace,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install shaders from the latest GitHub release
    InstallShaders {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,

        /// Force overwrite without prompting
        #[arg(short, long)]
        force: bool,
    },

    /// Install shell integration for your shell
    InstallShellIntegration {
        /// Specify shell type (auto-detected if not provided)
        #[arg(long, value_enum)]
        shell: Option<ShellTypeArg>,
    },

    /// Uninstall shell integration
    UninstallShellIntegration,

    /// Uninstall shaders (removes bundled files, keeps user files)
    UninstallShaders {
        /// Force removal without prompting
        #[arg(short, long)]
        force: bool,
    },

    /// Install both shaders and shell integration
    InstallIntegrations {
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Update par-term to the latest version
    SelfUpdate {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Run as an MCP server (used by ACP agents for config updates)
    McpServer,
}

/// Runtime options passed from CLI to the application
#[derive(Clone, Debug, Default)]
pub struct RuntimeOptions {
    /// Background shader to use
    pub shader: Option<String>,
    /// Exit after this many seconds
    pub exit_after: Option<f64>,
    /// Take a screenshot (Some(empty path) = auto-name, Some(path) = specific path, None = no screenshot)
    pub screenshot: Option<PathBuf>,
    /// Command to send to shell after delay
    pub command_to_send: Option<String>,
    /// Enable session logging (overrides config)
    pub log_session: bool,
    /// Log level override from CLI
    pub log_level: Option<log::LevelFilter>,
}

/// Result of CLI processing
pub enum CliResult {
    /// Continue with normal application startup, with optional runtime options
    Continue(RuntimeOptions),
    /// Exit with the given code (subcommand completed)
    Exit(i32),
}

/// Process CLI arguments and handle subcommands
pub fn process_cli() -> CliResult {
    use install::{
        install_integrations_cli, install_shaders_cli, install_shell_integration_cli,
        self_update_cli, uninstall_shaders_cli, uninstall_shell_integration_cli,
    };

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::InstallShaders { yes, force }) => {
            let result = install_shaders_cli(yes || force);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::InstallShellIntegration { shell }) => {
            let result = install_shell_integration_cli(shell.map(Into::into));
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallShellIntegration) => {
            let result = uninstall_shell_integration_cli();
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallShaders { force }) => {
            let result = uninstall_shaders_cli(force);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::InstallIntegrations { yes }) => {
            let result = install_integrations_cli(yes);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::SelfUpdate { yes }) => {
            let result = self_update_cli(yes);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::McpServer) => {
            crate::mcp_server::set_app_version(crate::VERSION);
            crate::mcp_server::run_mcp_server();
            CliResult::Exit(0)
        }
        None => {
            // Extract runtime options from CLI flags
            let options = RuntimeOptions {
                shader: cli.shader,
                exit_after: cli.exit_after,
                screenshot: cli.screenshot,
                command_to_send: cli.command_to_send,
                log_session: cli.log_session,
                log_level: cli.log_level.map(|l| l.to_level_filter()),
            };
            CliResult::Continue(options)
        }
    }
}
