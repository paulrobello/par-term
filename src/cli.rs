//! Command-line interface for par-term.
//!
//! This module handles CLI argument parsing and subcommands like shader installation.

use crate::config::ShellType;
use crate::shader_installer;
use crate::shell_integration_installer;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
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

/// Install shaders from the latest GitHub release (CLI version with prompts and output)
fn install_shaders_cli(skip_prompt: bool) -> anyhow::Result<()> {
    let shaders_dir = crate::config::Config::shaders_dir();

    println!("=============================================");
    println!("  par-term Shader Installer");
    println!("=============================================");
    println!();
    println!("Target directory: {}", shaders_dir.display());
    println!();

    // Check if directory has existing shaders
    if shaders_dir.exists() && shader_installer::has_shader_files(&shaders_dir) && !skip_prompt {
        println!("WARNING: This will overwrite existing shaders in:");
        println!("  {}", shaders_dir.display());
        println!();
        print!("Do you want to continue? [y/N] ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if response != "y" && response != "yes" {
            println!("Installation cancelled.");
            return Ok(());
        }
        println!();
    }

    // Fetch latest release info
    println!("Fetching latest release information...");

    const REPO: &str = "paulrobello/par-term";
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let download_url = shader_installer::get_shaders_download_url(&api_url, REPO)
        .map_err(|e| anyhow::anyhow!(e))?;

    println!("Downloading shaders from: {}", download_url);
    println!();

    // Download the zip file
    let zip_data =
        shader_installer::download_file(&download_url).map_err(|e| anyhow::anyhow!(e))?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)?;

    // Extract shaders
    println!("Extracting shaders to {}...", shaders_dir.display());
    shader_installer::extract_shaders(&zip_data, &shaders_dir).map_err(|e| anyhow::anyhow!(e))?;

    // Count installed shaders
    let shader_count = shader_installer::count_shader_files(&shaders_dir);

    println!();
    println!("=============================================");
    println!("  Installation complete!");
    println!("=============================================");
    println!();
    println!("Installed {} shaders to:", shader_count);
    println!("  {}", shaders_dir.display());
    println!();
    println!("To use a shader, add to your config.yaml:");
    println!("  custom_shader: \"shader_name.glsl\"");
    println!("  custom_shader_enabled: true");
    println!();
    println!("For cursor shaders:");
    println!("  cursor_shader: \"cursor_glow.glsl\"");
    println!("  cursor_shader_enabled: true");
    println!();
    println!("See docs/SHADERS.md for the full shader gallery.");

    Ok(())
}

/// Install shell integration for the specified or detected shell (CLI version)
fn install_shell_integration_cli(shell: Option<ShellType>) -> anyhow::Result<()> {
    let detected = shell_integration_installer::detected_shell();
    let target_shell = shell.unwrap_or(detected);

    println!("=============================================");
    println!("  par-term Shell Integration Installer");
    println!("=============================================");
    println!();

    if target_shell == ShellType::Unknown {
        eprintln!("Error: Could not detect shell type.");
        eprintln!("Please specify your shell with --shell bash|zsh|fish");
        return Err(anyhow::anyhow!("Unknown shell type"));
    }

    println!("Detected shell: {:?}", target_shell);

    // Check if already installed
    if shell_integration_installer::is_installed() {
        println!("Shell integration is already installed.");
        print!("Do you want to reinstall? [y/N] ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if response != "y" && response != "yes" {
            println!("Installation cancelled.");
            return Ok(());
        }
        println!();
    }

    println!("Installing shell integration...");

    match shell_integration_installer::install(Some(target_shell)) {
        Ok(result) => {
            println!();
            println!("=============================================");
            println!("  Installation complete!");
            println!("=============================================");
            println!();
            println!("Script installed to:");
            println!("  {}", result.script_path.display());
            println!();
            println!("Added source line to:");
            println!("  {}", result.rc_file.display());
            println!();
            if result.needs_restart {
                println!("Please restart your shell or run:");
                println!("  source {}", result.rc_file.display());
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

/// Uninstall shell integration (CLI version)
fn uninstall_shell_integration_cli() -> anyhow::Result<()> {
    println!("=============================================");
    println!("  par-term Shell Integration Uninstaller");
    println!("=============================================");
    println!();

    if !shell_integration_installer::is_installed() {
        println!("Shell integration is not installed.");
        return Ok(());
    }

    println!("Uninstalling shell integration...");

    match shell_integration_installer::uninstall() {
        Ok(result) => {
            println!();
            println!("=============================================");
            println!("  Uninstallation complete!");
            println!("=============================================");
            println!();

            if !result.cleaned.is_empty() {
                println!("Cleaned RC files:");
                for path in &result.cleaned {
                    println!("  {}", path.display());
                }
                println!();
            }

            if !result.scripts_removed.is_empty() {
                println!("Removed integration scripts:");
                for path in &result.scripts_removed {
                    println!("  {}", path.display());
                }
                println!();
            }

            if !result.needs_manual.is_empty() {
                println!("WARNING: Some files need manual cleanup:");
                for path in &result.needs_manual {
                    println!("  {}", path.display());
                }
                println!();
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

/// Uninstall shaders using manifest (CLI version)
fn uninstall_shaders_cli(force: bool) -> anyhow::Result<()> {
    let shaders_dir = crate::config::Config::shaders_dir();

    println!("=============================================");
    println!("  par-term Shader Uninstaller");
    println!("=============================================");
    println!();
    println!("Shaders directory: {}", shaders_dir.display());
    println!();

    if !shaders_dir.exists() {
        println!("No shaders installed.");
        return Ok(());
    }

    // Check for manifest
    let manifest_path = shaders_dir.join("manifest.json");
    if !manifest_path.exists() {
        println!("No manifest.json found. Cannot determine which files are bundled.");
        println!("Only files installed with the installer can be safely uninstalled.");
        return Err(anyhow::anyhow!("No manifest found"));
    }

    if !force {
        println!("This will remove bundled shader files.");
        println!("User-created and modified files will be preserved.");
        println!();
        print!("Do you want to continue? [y/N] ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if response != "y" && response != "yes" {
            println!("Uninstallation cancelled.");
            return Ok(());
        }
        println!();
    }

    println!("Uninstalling shaders...");

    match shader_installer::uninstall_shaders(force) {
        Ok(result) => {
            println!();
            println!("=============================================");
            println!("  Uninstallation complete!");
            println!("=============================================");
            println!();
            println!("Removed {} bundled files.", result.removed);

            if result.kept > 0 {
                println!("Preserved {} user files.", result.kept);
            }

            if !result.needs_confirmation.is_empty() {
                println!();
                println!("Modified files that were preserved:");
                for path in &result.needs_confirmation {
                    println!("  {}", path);
                }
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

/// Self-update par-term to the latest version (CLI version)
fn self_update_cli(skip_prompt: bool) -> anyhow::Result<()> {
    use crate::self_updater;
    use crate::update_checker;

    println!("=============================================");
    println!("  par-term Self-Updater");
    println!("=============================================");
    println!();

    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current_version);

    // Detect installation type
    let installation = self_updater::detect_installation();
    println!("Installation type: {}", installation.description());
    println!();

    // Check for managed installations early
    match &installation {
        self_updater::InstallationType::Homebrew => {
            println!("par-term is installed via Homebrew.");
            println!("Please update with:");
            println!("  brew upgrade --cask par-term");
            return Err(anyhow::anyhow!("Cannot self-update Homebrew installation"));
        }
        self_updater::InstallationType::CargoInstall => {
            println!("par-term is installed via cargo.");
            println!("Please update with:");
            println!("  cargo install par-term");
            return Err(anyhow::anyhow!("Cannot self-update cargo installation"));
        }
        _ => {}
    }

    // Check for updates
    println!("Checking for updates...");
    let release_info = update_checker::fetch_latest_release().map_err(|e| anyhow::anyhow!(e))?;

    let latest_version = release_info
        .version
        .strip_prefix('v')
        .unwrap_or(&release_info.version);

    let current = semver::Version::parse(current_version)?;
    let latest = semver::Version::parse(latest_version)?;

    if latest <= current {
        println!();
        println!(
            "You are already running the latest version ({}).",
            current_version
        );
        return Ok(());
    }

    println!();
    println!(
        "New version available: {} -> {}",
        current_version, latest_version
    );
    if let Some(ref notes) = release_info.release_notes {
        println!();
        println!("Release notes:");
        // Show first few lines of release notes
        for line in notes.lines().take(10) {
            println!("  {}", line);
        }
        if notes.lines().count() > 10 {
            println!("  ...");
        }
    }
    println!();

    // Confirm unless --yes
    if !skip_prompt {
        print!("Do you want to update? [y/N] ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if response != "y" && response != "yes" {
            println!("Update cancelled.");
            return Ok(());
        }
        println!();
    }

    println!("Downloading and installing update...");

    match self_updater::perform_update(latest_version) {
        Ok(result) => {
            println!();
            println!("=============================================");
            println!("  Update complete!");
            println!("=============================================");
            println!();
            println!("Updated: {} -> {}", result.old_version, result.new_version);
            println!("Location: {}", result.install_path.display());
            if result.needs_restart {
                println!();
                println!("Please restart par-term to use the new version.");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Update failed: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

/// Install both shaders and shell integration (CLI version)
fn install_integrations_cli(skip_prompt: bool) -> anyhow::Result<()> {
    println!("=============================================");
    println!("  par-term Integrations Installer");
    println!("=============================================");
    println!();
    println!("This will install:");
    println!("  1. Shader collection from latest release");
    println!("  2. Shell integration for your current shell");
    println!();

    if !skip_prompt {
        print!("Do you want to continue? [y/N] ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        let response = response.trim().to_lowercase();

        if response != "y" && response != "yes" {
            println!("Installation cancelled.");
            return Ok(());
        }
        println!();
    }

    // Install shaders
    println!("Step 1: Installing shaders...");
    println!("---------------------------------------------");

    let shader_result = install_shaders_cli(true);
    if shader_result.is_err() {
        println!();
        println!("WARNING: Shader installation failed.");
        println!("Continuing with shell integration...");
    }

    println!();
    println!("Step 2: Installing shell integration...");
    println!("---------------------------------------------");

    let shell_result = install_shell_integration_cli(None);

    println!();
    println!("=============================================");
    println!("  Integrations Installation Summary");
    println!("=============================================");
    println!();

    match (&shader_result, &shell_result) {
        (Ok(()), Ok(())) => {
            println!("All integrations installed successfully!");
        }
        (Err(_), Ok(())) => {
            println!("Shell integration: INSTALLED");
            println!("Shaders: FAILED (see above for errors)");
        }
        (Ok(()), Err(_)) => {
            println!("Shaders: INSTALLED");
            println!("Shell integration: FAILED (see above for errors)");
        }
        (Err(_), Err(_)) => {
            println!("Both installations failed. See above for errors.");
        }
    }

    // Return success if at least one succeeded
    if shader_result.is_ok() || shell_result.is_ok() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Both installations failed"))
    }
}
