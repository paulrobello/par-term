//! Command-line interface for par-term.
//!
//! This module handles CLI argument parsing and subcommands like shader installation.

use crate::shader_installer;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;

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
        None => {
            // Extract runtime options from CLI flags
            let options = RuntimeOptions {
                shader: cli.shader,
                exit_after: cli.exit_after,
                screenshot: cli.screenshot,
                command_to_send: cli.command_to_send,
                log_session: cli.log_session,
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
