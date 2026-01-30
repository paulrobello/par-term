//! Command-line interface for par-term.
//!
//! This module handles CLI argument parsing and subcommands like shader installation.

use clap::{Parser, Subcommand};
use std::io::{self, Read, Write};
use std::path::Path;

/// par-term - A GPU-accelerated terminal emulator
#[derive(Parser)]
#[command(name = "par-term")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
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

/// Result of CLI processing
pub enum CliResult {
    /// Continue with normal application startup
    Continue,
    /// Exit with the given code (subcommand completed)
    Exit(i32),
}

/// Process CLI arguments and handle subcommands
pub fn process_cli() -> CliResult {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::InstallShaders { yes, force }) => {
            let result = install_shaders(yes || force);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        None => CliResult::Continue,
    }
}

/// Install shaders from the latest GitHub release
fn install_shaders(skip_prompt: bool) -> anyhow::Result<()> {
    const REPO: &str = "paulrobello/par-term";

    let shaders_dir = crate::config::Config::shaders_dir();

    println!("=============================================");
    println!("  par-term Shader Installer");
    println!("=============================================");
    println!();
    println!("Target directory: {}", shaders_dir.display());
    println!();

    // Check if directory has existing shaders
    if shaders_dir.exists() && has_shader_files(&shaders_dir) && !skip_prompt {
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

    let download_url = get_shaders_download_url(REPO)?;
    println!("Downloading shaders from: {}", download_url);
    println!();

    // Download the zip file
    let zip_data = download_file(&download_url)?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)?;

    // Extract shaders
    println!("Extracting shaders to {}...", shaders_dir.display());
    extract_shaders(&zip_data, &shaders_dir)?;

    // Count installed shaders
    let shader_count = count_shader_files(&shaders_dir);

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

/// Check if directory contains any .glsl files
fn has_shader_files(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                return true;
            }
        }
    }
    false
}

/// Count .glsl files in directory
fn count_shader_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                count += 1;
            }
        }
    }
    count
}

/// Get the download URL for shaders.zip from the latest release
fn get_shaders_download_url(repo: &str) -> anyhow::Result<String> {
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let mut body = ureq::get(&api_url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| anyhow::anyhow!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

    // Parse JSON to find shaders.zip URL
    // Look for "browser_download_url": "...shaders.zip"
    if let Some(pos) = body_str.find("shaders.zip") {
        // Search backwards for the URL start
        let search_start = pos.saturating_sub(200);
        let search_str = &body_str[search_start..pos + 11];
        if let Some(url_start) = search_str.rfind("https://") {
            let url_slice = &search_str[url_start..];
            if let Some(url_end) = url_slice.find('"') {
                return Ok(url_slice[..url_end].to_string());
            }
        }
    }

    anyhow::bail!(
        "Could not find shaders.zip in the latest release.\n\
         Please check https://github.com/{}/releases",
        repo
    )
}

/// Download a file from URL and return its contents
fn download_file(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut body = ureq::get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| anyhow::anyhow!("Failed to download file: {}", e))?
        .into_body();

    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| anyhow::anyhow!("Failed to read download: {}", e))?;

    Ok(bytes)
}

/// Extract shaders from zip data to target directory
fn extract_shaders(zip_data: &[u8], target_dir: &Path) -> anyhow::Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Skip directories and non-glsl files, but keep textures folder
        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);

        // Skip if empty path
        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }

    Ok(())
}
