//! Shell integration installation logic.
//!
//! This module handles installing and uninstalling shell integration scripts for
//! bash, zsh, and fish shells. It:
//! - Embeds shell scripts via `include_str!`
//! - Detects the current shell from $SHELL
//! - Writes scripts to `~/.config/par-term/shell_integration.{bash,zsh,fish}`
//! - Adds marker-wrapped source lines to RC files
//! - Supports clean uninstall that safely removes the marker blocks

use crate::config::{Config, ShellType};
use std::fs;
use std::path::{Path, PathBuf};

// Embedded shell integration scripts
const BASH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.bash");
const ZSH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.zsh");
const FISH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.fish");

/// Marker comments for identifying our additions to RC files
const MARKER_START: &str = "# >>> par-term shell integration >>>";
const MARKER_END: &str = "# <<< par-term shell integration <<<";

/// Result of installation
#[derive(Debug)]
pub struct InstallResult {
    /// Shell type that was installed
    pub shell: ShellType,
    /// Path where the integration script was written
    pub script_path: PathBuf,
    /// Path to the RC file that was modified
    pub rc_file: PathBuf,
    /// Whether a shell restart is needed to activate
    pub needs_restart: bool,
}

/// Result of uninstallation
#[derive(Debug, Default)]
pub struct UninstallResult {
    /// RC files that were successfully cleaned
    pub cleaned: Vec<PathBuf>,
    /// RC files that need manual cleanup (markers found but couldn't remove)
    pub needs_manual: Vec<PathBuf>,
    /// Integration script files that were removed
    pub scripts_removed: Vec<PathBuf>,
}

/// Install shell integration for detected or specified shell
///
/// # Arguments
/// * `shell` - Optional shell type override. If None, detects from $SHELL
///
/// # Returns
/// * `Ok(InstallResult)` - Installation succeeded
/// * `Err(String)` - Installation failed with error message
pub fn install(shell: Option<ShellType>) -> Result<InstallResult, String> {
    let shell = shell.unwrap_or_else(detected_shell);

    if shell == ShellType::Unknown {
        return Err(
            "Could not detect shell type. Please specify shell manually (bash, zsh, or fish)."
                .to_string(),
        );
    }

    // Get the script content for this shell
    let script_content = get_script_content(shell);

    // Get the integration directory
    let integration_dir = Config::shell_integration_dir();

    // Create the directory if it doesn't exist
    fs::create_dir_all(&integration_dir)
        .map_err(|e| format!("Failed to create directory {:?}: {}", integration_dir, e))?;

    // Write the script file
    let script_filename = format!("shell_integration.{}", shell.extension());
    let script_path = integration_dir.join(&script_filename);

    fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to write script to {:?}: {}", script_path, e))?;

    // Get the RC file path
    let rc_file = get_rc_file(shell)?;

    // Add source line to RC file
    add_to_rc_file(&rc_file, shell)?;

    Ok(InstallResult {
        shell,
        script_path,
        rc_file,
        needs_restart: true,
    })
}

/// Uninstall shell integration for all supported shells
///
/// Removes integration scripts and cleans up RC files for bash, zsh, and fish.
///
/// # Returns
/// * `Ok(UninstallResult)` - Uninstallation completed (may have partial success)
/// * `Err(String)` - Critical error during uninstallation
pub fn uninstall() -> Result<UninstallResult, String> {
    let mut result = UninstallResult::default();

    // Clean up RC files for all shell types
    for shell in [ShellType::Bash, ShellType::Zsh, ShellType::Fish] {
        if let Ok(rc_file) = get_rc_file(shell)
            && rc_file.exists()
        {
            match remove_from_rc_file(&rc_file) {
                Ok(true) => result.cleaned.push(rc_file),
                Ok(false) => { /* No markers found, nothing to do */ }
                Err(_) => result.needs_manual.push(rc_file),
            }
        }
    }

    // Remove integration script files
    let integration_dir = Config::shell_integration_dir();
    for shell in [ShellType::Bash, ShellType::Zsh, ShellType::Fish] {
        let script_filename = format!("shell_integration.{}", shell.extension());
        let script_path = integration_dir.join(&script_filename);

        if script_path.exists() && fs::remove_file(&script_path).is_ok() {
            result.scripts_removed.push(script_path);
        }
    }

    Ok(result)
}

/// Check if shell integration is installed for the detected shell
///
/// Returns true if:
/// - The integration script file exists
/// - The RC file contains our marker block
pub fn is_installed() -> bool {
    let shell = detected_shell();
    if shell == ShellType::Unknown {
        return false;
    }

    // Check if script file exists
    let integration_dir = Config::shell_integration_dir();
    let script_filename = format!("shell_integration.{}", shell.extension());
    let script_path = integration_dir.join(&script_filename);

    if !script_path.exists() {
        return false;
    }

    // Check if RC file has our markers
    if let Ok(rc_file) = get_rc_file(shell)
        && let Ok(content) = fs::read_to_string(&rc_file)
    {
        return content.contains(MARKER_START) && content.contains(MARKER_END);
    }

    false
}

/// Detect shell type from $SHELL environment variable
pub fn detected_shell() -> ShellType {
    ShellType::detect()
}

/// Get the script content for a given shell type
fn get_script_content(shell: ShellType) -> &'static str {
    match shell {
        ShellType::Bash => BASH_SCRIPT,
        ShellType::Zsh => ZSH_SCRIPT,
        ShellType::Fish => FISH_SCRIPT,
        ShellType::Unknown => BASH_SCRIPT, // Fallback to bash
    }
}

/// Get the RC file path for a given shell type
fn get_rc_file(shell: ShellType) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;

    let rc_file = match shell {
        ShellType::Bash => {
            // Prefer .bashrc if it exists, otherwise .bash_profile
            let bashrc = home.join(".bashrc");
            let bash_profile = home.join(".bash_profile");
            if bashrc.exists() {
                bashrc
            } else {
                bash_profile
            }
        }
        ShellType::Zsh => home.join(".zshrc"),
        ShellType::Fish => {
            // Fish config is at ~/.config/fish/config.fish
            let xdg_config = std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".config"));
            xdg_config.join("fish").join("config.fish")
        }
        ShellType::Unknown => return Err("Unknown shell type".to_string()),
    };

    Ok(rc_file)
}

/// Add the source line to the RC file, wrapped in markers
fn add_to_rc_file(rc_file: &Path, shell: ShellType) -> Result<(), String> {
    // Read existing content (or empty string if file doesn't exist)
    let existing_content = if rc_file.exists() {
        fs::read_to_string(rc_file).map_err(|e| format!("Failed to read {:?}: {}", rc_file, e))?
    } else {
        // Create parent directories if needed
        if let Some(parent) = rc_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
        }
        String::new()
    };

    // Check if our markers already exist
    if existing_content.contains(MARKER_START) {
        // Remove existing block and add fresh one
        let cleaned = remove_marker_block(&existing_content);
        let new_content = format!("{}\n{}", cleaned.trim_end(), generate_source_block(shell));
        fs::write(rc_file, new_content)
            .map_err(|e| format!("Failed to write {:?}: {}", rc_file, e))?;
    } else {
        // Append our block to the file
        let new_content = if existing_content.is_empty() {
            generate_source_block(shell)
        } else if existing_content.ends_with('\n') {
            format!("{}\n{}", existing_content, generate_source_block(shell))
        } else {
            format!("{}\n\n{}", existing_content, generate_source_block(shell))
        };
        fs::write(rc_file, new_content)
            .map_err(|e| format!("Failed to write {:?}: {}", rc_file, e))?;
    }

    Ok(())
}

/// Remove our marker block from an RC file
///
/// Returns Ok(true) if markers were found and removed,
/// Ok(false) if no markers were found,
/// Err if file couldn't be read/written
fn remove_from_rc_file(rc_file: &Path) -> Result<bool, String> {
    let content =
        fs::read_to_string(rc_file).map_err(|e| format!("Failed to read {:?}: {}", rc_file, e))?;

    if !content.contains(MARKER_START) {
        return Ok(false);
    }

    let cleaned = remove_marker_block(&content);

    // Only write if content changed
    if cleaned != content {
        fs::write(rc_file, &cleaned)
            .map_err(|e| format!("Failed to write {:?}: {}", rc_file, e))?;
    }

    Ok(true)
}

/// Generate the source block with markers for a given shell
fn generate_source_block(shell: ShellType) -> String {
    let integration_dir = Config::shell_integration_dir();
    let script_filename = format!("shell_integration.{}", shell.extension());
    let script_path = integration_dir.join(&script_filename);

    // Use display() for path - will work on all platforms
    let script_path_str = script_path.display();

    match shell {
        ShellType::Fish => {
            // Fish uses 'source' command with different syntax
            format!(
                "{}\nif test -f \"{}\"\n    source \"{}\"\nend\n{}\n",
                MARKER_START, script_path_str, script_path_str, MARKER_END
            )
        }
        _ => {
            // Bash and Zsh use similar syntax
            format!(
                "{}\nif [ -f \"{}\" ]; then\n    source \"{}\"\nfi\n{}\n",
                MARKER_START, script_path_str, script_path_str, MARKER_END
            )
        }
    }
}

/// Remove the marker block from content, preserving surrounding content
fn remove_marker_block(content: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;
    let mut found_block = false;

    for line in content.lines() {
        if line.trim() == MARKER_START {
            in_block = true;
            found_block = true;
            continue;
        }
        if line.trim() == MARKER_END {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    // If we found and removed a block, clean up extra blank lines
    if found_block {
        // Remove trailing blank lines that may have accumulated
        let trimmed = result.trim_end();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("{}\n", trimmed)
        }
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_marker_block() {
        let content = format!(
            "# existing content\n{}\nsource something\n{}\n# more content\n",
            MARKER_START, MARKER_END
        );
        let result = remove_marker_block(&content);
        assert!(!result.contains(MARKER_START));
        assert!(!result.contains(MARKER_END));
        assert!(result.contains("# existing content"));
        assert!(result.contains("# more content"));
        assert!(!result.contains("source something"));
    }

    #[test]
    fn test_remove_marker_block_no_markers() {
        let content = "# just some content\nno markers here\n";
        let result = remove_marker_block(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_generate_source_block_bash() {
        let block = generate_source_block(ShellType::Bash);
        assert!(block.contains(MARKER_START));
        assert!(block.contains(MARKER_END));
        assert!(block.contains("source"));
        assert!(block.contains(".bash"));
    }

    #[test]
    fn test_generate_source_block_zsh() {
        let block = generate_source_block(ShellType::Zsh);
        assert!(block.contains(MARKER_START));
        assert!(block.contains(MARKER_END));
        assert!(block.contains("source"));
        assert!(block.contains(".zsh"));
    }

    #[test]
    fn test_generate_source_block_fish() {
        let block = generate_source_block(ShellType::Fish);
        assert!(block.contains(MARKER_START));
        assert!(block.contains(MARKER_END));
        assert!(block.contains("source"));
        assert!(block.contains(".fish"));
        // Fish uses different syntax
        assert!(block.contains("if test -f"));
        assert!(block.contains("end"));
    }

    #[test]
    fn test_get_script_content() {
        // Just verify we get non-empty content
        assert!(!get_script_content(ShellType::Bash).is_empty());
        assert!(!get_script_content(ShellType::Zsh).is_empty());
        assert!(!get_script_content(ShellType::Fish).is_empty());
    }

    #[test]
    fn test_detected_shell() {
        // This will return whatever $SHELL is set to in the test environment
        // We just verify it doesn't panic
        let _shell = detected_shell();
    }
}
