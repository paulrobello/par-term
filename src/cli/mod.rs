//! Command-line interface for par-term.
//!
//! This module handles CLI argument parsing and subcommands like shader installation.
//! Install/uninstall procedure implementations live in the [`install`] submodule;
//! plugin add/update/remove in [`plugin`].

pub mod install;
pub mod plugin;

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

    /// Run a scripted UI test (JSON script; see
    /// docs/guides/AGENT_UI_VERIFICATION.md), then write a report and exit
    #[arg(long, value_name = "SCRIPT")]
    pub ui_test: Option<PathBuf>,

    /// Where the --ui-test JSON report is written (default: beside the script)
    #[arg(long, value_name = "PATH", requires = "ui_test")]
    pub ui_test_report: Option<PathBuf>,

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

    /// Install par-mux agent-state extensions for pi and omp
    InstallMuxExtensions {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Uninstall par-mux agent-state extensions
    UninstallMuxExtensions,

    /// Install par-mux session hooks for config-entry agents (claude)
    InstallMuxHooks {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Uninstall par-mux session hooks
    UninstallMuxHooks,

    /// Lint a custom background shader file
    ShaderLint {
        /// Path to the .glsl shader file to lint
        path: PathBuf,

        /// Include source-level readability scoring and suggested readability defaults
        #[arg(long)]
        readability: bool,

        /// Apply suggested readability defaults to the shader metadata without prompting
        #[arg(long)]
        apply: bool,

        /// Do not prompt to apply suggested readability defaults
        #[arg(long)]
        no_prompt: bool,
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

    /// Manage plugins: install from a git URL, update, remove
    Plugin {
        #[command(subcommand)]
        command: plugin::PluginCommands,
    },

    /// Run as an MCP server (used by ACP agents for config updates)
    McpServer,

    /// Agent/user command fallthrough: `par-term <id> [args...]` runs the
    /// command file `<config_dir>/commands/<id>.yaml` when `<id>` matches no
    /// built-in subcommand above. Real subcommands always win (clap-native).
    #[command(external_subcommand)]
    External(Vec<String>),
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
    /// Scripted UI test to run (see docs/guides/AGENT_UI_VERIFICATION.md)
    pub ui_test: Option<PathBuf>,
    /// Where the --ui-test JSON report is written
    pub ui_test_report: Option<PathBuf>,
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
        install_integrations_cli, install_mux_extensions_cli, install_mux_hooks_cli,
        install_shaders_cli, install_shell_integration_cli, self_update_cli,
        uninstall_mux_extensions_cli, uninstall_mux_hooks_cli, uninstall_shaders_cli,
        uninstall_shell_integration_cli,
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
        Some(Commands::InstallMuxExtensions { yes }) => {
            let result = install_mux_extensions_cli(yes);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallMuxExtensions) => {
            let result = uninstall_mux_extensions_cli();
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::InstallMuxHooks { yes }) => {
            let result = install_mux_hooks_cli(yes);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallMuxHooks) => {
            let result = uninstall_mux_hooks_cli();
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::ShaderLint {
            path,
            readability,
            apply,
            no_prompt,
        }) => {
            let result = crate::shader_lint::shader_lint_cli(&path, readability, apply, !no_prompt);
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
        Some(Commands::Plugin { command }) => {
            use plugin::PluginCommands;
            let result = match command {
                PluginCommands::Add { url, yes } => plugin::plugin_add_cli(&url, yes),
                PluginCommands::Update { id, yes } => plugin::plugin_update_cli(id.as_deref(), yes),
                PluginCommands::Remove { id, yes } => plugin::plugin_remove_cli(&id, yes),
            };
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::McpServer) => {
            crate::mcp_server::set_app_version(crate::VERSION);
            crate::mcp_server::run_mcp_server();
            CliResult::Exit(0)
        }
        Some(Commands::External(parts)) => {
            // External-subcommand fallthrough: an id matching no built-in
            // subcommand resolves against the commands directory
            // (design 2026-09-24, CLI section).
            run_external_command(&parts)
        }
        None => {
            // Extract runtime options from CLI flags
            let options = RuntimeOptions {
                shader: cli.shader,
                exit_after: cli.exit_after,
                screenshot: cli.screenshot,
                command_to_send: cli.command_to_send,
                ui_test: cli.ui_test,
                ui_test_report: cli.ui_test_report,
                log_session: cli.log_session,
                log_level: cli.log_level.map(|l| l.to_level_filter()),
            };
            CliResult::Continue(options)
        }
    }
}

/// Run an external-subcommand fallthrough: `par-term <id> [args...]`.
///
/// `<id>` resolves against the commands directory. Script commands run as a
/// foreground subprocess (extra args appended after the stored ones,
/// inheriting stdio so the user sees live output) with the same
/// first-run-confirmation rule as the palette: an unconfirmed body prints
/// itself and exits non-zero unless `--yes` is the first extra arg. Macro
/// commands cannot run without the app's window and are refused with a
/// pointer at the palette.
fn run_external_command(parts: &[String]) -> CliResult {
    use par_term_config::CustomActionConfig;
    use par_term_config::agent_commands as cmd_fmt;

    let id = &parts[0];
    let mut extra: Vec<String> = parts[1..].to_vec();
    let mut assume_yes = false;
    if extra.first().is_some_and(|a| a == "--yes") {
        assume_yes = true;
        extra.remove(0);
    }

    let dir = cmd_fmt::commands_dir();
    let path = dir.join(format!("{id}.yaml"));
    if !path.exists() {
        eprintln!(
            "Unknown subcommand or command id {id:?}. No such file: {}. Run \
             `par-term --help` for built-in subcommands.",
            path.display()
        );
        return CliResult::Exit(1);
    }

    let file = match cmd_fmt::load_command_file(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Command {id:?} is invalid: {e:#}");
            return CliResult::Exit(1);
        }
    };

    match file.action.clone() {
        CustomActionConfig::ShellCommand {
            command,
            mut args,
            timeout_secs,
            ..
        } => {
            let mut ledger = cmd_fmt::load_confirmation_ledger(&dir);
            let hash = file.body_hash();
            if ledger.get(file.id()) != Some(&hash) {
                if !assume_yes {
                    println!(
                        "Unconfirmed agent script command {}. Run `par-term {id} --yes` \
                         to execute it, or approve it once in the app's palette dialog.",
                        file.title()
                    );
                    println!("--- body ---");
                    println!("{command} {}", args.join(" "));
                    return CliResult::Exit(2);
                }
                // `--yes` is an explicit approval of this exact body —
                // persist it so later runs need neither the flag nor the
                // prompt, matching the palette dialog's Run.
                ledger.insert(file.id().to_string(), hash);
                if let Err(e) = cmd_fmt::save_confirmation_ledger(&ledger, &dir) {
                    eprintln!("Warning: could not persist confirmation ({e:#})");
                }
            }
            args.extend(extra);
            let env = [
                ("PAR_TERM_COMMAND_ID", Some(file.id().to_string())),
                ("PAR_TERM_COMMAND_SOURCE_AGENT", file.source_agent.clone()),
            ];
            let mut cmd = std::process::Command::new(&command);
            cmd.args(&args);
            for (k, v) in env.iter().filter_map(|(k, v)| v.clone().map(|v| (k, v))) {
                cmd.env(k, v);
            }
            // Spawn + poll so stdio stays inherited (live output) while the
            // stored timeout_secs is still honored.
            let status = match cmd.spawn() {
                Ok(mut child) => {
                    let deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
                    loop {
                        match child.try_wait() {
                            Ok(Some(s)) => break Some(Ok(s)),
                            Ok(None) => {
                                if std::time::Instant::now() > deadline {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    break None;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            Err(e) => break Some(Err(e)),
                        }
                    }
                }
                Err(e) => Some(Err(e)),
            };
            match status {
                Some(Ok(s)) => CliResult::Exit(s.code().unwrap_or(1)),
                Some(Err(e)) => {
                    eprintln!("Failed to run {command:?}: {e}");
                    CliResult::Exit(1)
                }
                None => {
                    eprintln!("Command {id:?} timed out after {timeout_secs}s");
                    CliResult::Exit(1)
                }
            }
        }
        macro_action => {
            let _ = &macro_action;
            eprintln!(
                "Command {id:?} is a macro ({}); macros drive the par-term UI \
                 and run from the command palette (open it and search \"{}\"), \
                 not from the CLI.",
                file.title(),
                file.title()
            );
            CliResult::Exit(1)
        }
    }
}
