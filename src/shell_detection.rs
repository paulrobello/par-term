//! Platform-aware shell detection for profile shell selection.
//!
//! Discovers available shells on the host OS and caches the results.

use std::path::Path;
use std::sync::OnceLock;

/// Information about a detected shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellInfo {
    /// Human-readable display name (e.g. "zsh", "bash", "PowerShell")
    pub name: String,
    /// Absolute path to the shell binary
    pub path: String,
}

impl ShellInfo {
    /// Create a new `ShellInfo`.
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
        }
    }
}

impl std::fmt::Display for ShellInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.path)
    }
}

/// Cached list of detected shells.
static DETECTED_SHELLS: OnceLock<Vec<ShellInfo>> = OnceLock::new();

/// Get the list of available shells on the current system.
///
/// Results are cached after the first call. The list is sorted with common
/// shells first and always includes shells that actually exist on disk.
pub fn detected_shells() -> &'static [ShellInfo] {
    DETECTED_SHELLS.get_or_init(detect_shells)
}

/// Detect available shells on Unix/macOS by parsing `/etc/shells`.
#[cfg(not(target_os = "windows"))]
fn detect_shells() -> Vec<ShellInfo> {
    let mut shells = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // Parse /etc/shells (standard on Unix/macOS)
    if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
        for line in contents.lines() {
            let line = line.trim();
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if Path::new(line).exists() && seen_paths.insert(line.to_string()) {
                let name = Path::new(line)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| line.to_string());
                shells.push(ShellInfo::new(name, line));
            }
        }
    }

    // Ensure current $SHELL is in the list
    if let Ok(current_shell) = std::env::var("SHELL")
        && !current_shell.is_empty()
        && Path::new(&current_shell).exists()
        && seen_paths.insert(current_shell.clone())
    {
        let name = Path::new(&current_shell)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| current_shell.clone());
        shells.insert(0, ShellInfo::new(name, &current_shell));
    }

    // Check for common shells not listed in /etc/shells
    // (e.g. Homebrew-installed pwsh, fish, nushell)
    let extra_shells: &[(&str, &[&str])] = &[
        (
            "pwsh",
            &[
                "/opt/homebrew/bin/pwsh",
                "/usr/local/bin/pwsh",
                "/usr/bin/pwsh",
            ],
        ),
        (
            "fish",
            &[
                "/opt/homebrew/bin/fish",
                "/usr/local/bin/fish",
                "/usr/bin/fish",
            ],
        ),
        (
            "nu",
            &["/opt/homebrew/bin/nu", "/usr/local/bin/nu", "/usr/bin/nu"],
        ),
        (
            "elvish",
            &[
                "/opt/homebrew/bin/elvish",
                "/usr/local/bin/elvish",
                "/usr/bin/elvish",
            ],
        ),
    ];
    for (name, paths) in extra_shells {
        for path in *paths {
            if Path::new(path).exists() && seen_paths.insert((*path).to_string()) {
                shells.push(ShellInfo::new(*name, *path));
                break; // Only add first found path for each shell
            }
        }
    }

    // If nothing found, provide reasonable fallbacks
    if shells.is_empty() {
        for path in ["/bin/bash", "/bin/sh"] {
            if Path::new(path).exists() {
                let name = Path::new(path)
                    .file_name()
                    .expect("hard-coded paths /bin/bash and /bin/sh always have a file name component")
                    .to_string_lossy()
                    .to_string();
                shells.push(ShellInfo::new(name, path));
            }
        }
    }

    shells
}

/// Detect available shells on Windows.
#[cfg(target_os = "windows")]
fn detect_shells() -> Vec<ShellInfo> {
    let mut shells = Vec::new();

    // PowerShell 7+ (pwsh)
    if let Ok(output) = std::process::Command::new("where").arg("pwsh.exe").output() {
        if output.status.success() {
            if let Ok(path) = String::from_utf8(output.stdout) {
                let path = path.lines().next().unwrap_or("").trim();
                if !path.is_empty() && Path::new(path).exists() {
                    shells.push(ShellInfo::new("PowerShell 7", path));
                }
            }
        }
    }

    // Windows PowerShell (5.1)
    let ps_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if Path::new(ps_path).exists() {
        shells.push(ShellInfo::new("Windows PowerShell", ps_path));
    }

    // Command Prompt
    let cmd_path = r"C:\Windows\System32\cmd.exe";
    if Path::new(cmd_path).exists() {
        shells.push(ShellInfo::new("Command Prompt", cmd_path));
    }

    // Git Bash
    let git_bash_paths = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];
    for path in &git_bash_paths {
        if Path::new(path).exists() {
            shells.push(ShellInfo::new("Git Bash", *path));
            break;
        }
    }

    // WSL (if available)
    let wsl_path = r"C:\Windows\System32\wsl.exe";
    if Path::new(wsl_path).exists() {
        shells.push(ShellInfo::new("WSL", wsl_path));
    }

    // MSYS2
    let msys2_path = r"C:\msys64\usr\bin\bash.exe";
    if Path::new(msys2_path).exists() {
        shells.push(ShellInfo::new("MSYS2 Bash", msys2_path));
    }

    // Cygwin
    let cygwin_path = r"C:\cygwin64\bin\bash.exe";
    if Path::new(cygwin_path).exists() {
        shells.push(ShellInfo::new("Cygwin Bash", cygwin_path));
    }

    if shells.is_empty() {
        // Ultimate fallback
        shells.push(ShellInfo::new("PowerShell", "powershell.exe"));
    }

    shells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detected_shells_not_empty() {
        let shells = detected_shells();
        assert!(
            !shells.is_empty(),
            "Should detect at least one shell on any platform"
        );
    }

    #[test]
    fn test_detected_shells_have_valid_paths() {
        let shells = detected_shells();
        for shell in shells {
            assert!(!shell.name.is_empty(), "Shell name should not be empty");
            assert!(!shell.path.is_empty(), "Shell path should not be empty");
        }
    }

    #[test]
    fn test_detected_shells_cached() {
        let first = detected_shells();
        let second = detected_shells();
        // Same pointer means cached
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn test_shell_info_display() {
        let info = ShellInfo::new("bash", "/bin/bash");
        assert_eq!(info.to_string(), "bash (/bin/bash)");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_shells_exist_on_disk() {
        let shells = detected_shells();
        for shell in shells {
            assert!(
                Path::new(&shell.path).exists(),
                "Shell path should exist: {}",
                shell.path
            );
        }
    }
}
