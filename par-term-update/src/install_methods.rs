//! Installation type detection and platform-specific binary installation strategies.
//!
//! This module identifies how par-term was installed (Homebrew, cargo, macOS
//! bundle, or standalone binary) and provides the in-place replacement logic
//! for the installation methods that support self-update.

use std::path::PathBuf;

/// How par-term was installed — determines update strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationType {
    /// Installed via Homebrew (path contains "homebrew" or "Cellar")
    Homebrew,
    /// Installed via `cargo install` (path contains ".cargo/bin")
    CargoInstall,
    /// Running from a macOS .app bundle (path contains ".app/Contents/MacOS")
    MacOSBundle,
    /// Standalone binary (Linux, Windows, or custom location)
    StandaloneBinary,
}

impl InstallationType {
    /// Human-readable description of the installation type.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::CargoInstall => "cargo install",
            Self::MacOSBundle => "macOS app bundle",
            Self::StandaloneBinary => "standalone binary",
        }
    }
}

/// Detect the installation method based on the current executable path.
pub fn detect_installation() -> InstallationType {
    detect_installation_from_path(
        std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    )
}

/// Detect installation type from a given path string (testable).
pub(crate) fn detect_installation_from_path(path: &str) -> InstallationType {
    let path_lower = path.to_lowercase();

    if path_lower.contains("/homebrew/") || path_lower.contains("/cellar/") {
        InstallationType::Homebrew
    } else if path_lower.contains("/.cargo/bin/") {
        InstallationType::CargoInstall
    } else if path_lower.contains(".app/contents/macos/") {
        InstallationType::MacOSBundle
    } else {
        InstallationType::StandaloneBinary
    }
}

/// Install update for macOS .app bundle by extracting the zip.
pub(crate) fn install_macos_bundle(
    current_exe: &std::path::Path,
    zip_data: &[u8],
) -> Result<PathBuf, String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    // Derive .app root: go up 3 levels from Contents/MacOS/par-term
    let app_root = current_exe
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .and_then(|p| p.parent()) // .app/
        .ok_or_else(|| "Could not determine .app bundle root".to_string())?;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    // Find the top-level .app directory name in the archive
    let app_prefix = find_app_prefix(&mut archive)?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Strip the top-level .app directory from the zip path
        let relative_path = match outpath.strip_prefix(&app_prefix) {
            Ok(p) => p.to_owned(),
            Err(_) => continue,
        };

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = app_root.join(&relative_path);

        if file.is_dir() {
            std::fs::create_dir_all(&final_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            continue;
        }

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file {}: {}", final_path.display(), e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        // Set executable permission on macOS/Linux
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(mode))
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }
        }
    }

    // Remove macOS quarantine attribute from downloaded files.
    // Files downloaded from the internet get com.apple.quarantine set,
    // which causes Gatekeeper to block the app on next launch.
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("xattr")
            .args(["-cr", &app_root.to_string_lossy()])
            .status();
        match status {
            Ok(s) if s.success() => {
                log::info!("Removed quarantine attributes from {}", app_root.display());
            }
            Ok(s) => {
                log::warn!(
                    "xattr -cr exited with status {} for {}",
                    s,
                    app_root.display()
                );
            }
            Err(e) => {
                log::warn!("Failed to run xattr -cr on {}: {}", app_root.display(), e);
            }
        }
    }

    Ok(app_root.to_path_buf())
}

/// Find the top-level .app directory name in the zip archive.
fn find_app_prefix(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<String, String> {
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;
        let name = file.name().to_string();
        // Look for paths like "par-term.app/" or "par-term.app/Contents/..."
        if let Some(app_end) = name.find(".app/") {
            let prefix = &name[..app_end + 5]; // includes ".app/"
            return Ok(prefix.to_string());
        }
    }
    Err("Could not find .app bundle in zip archive".to_string())
}

/// Install update for standalone binary (Linux/Windows).
pub(crate) fn install_standalone(
    current_exe: &std::path::Path,
    data: &[u8],
) -> Result<PathBuf, String> {
    let new_path = current_exe.with_extension("new");

    // Write the new binary to a temp file
    std::fs::write(&new_path, data).map_err(|e| format!("Failed to write new binary: {}", e))?;

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Platform-specific replacement
    #[cfg(unix)]
    {
        // On Unix, rename is atomic if on the same filesystem.
        // A running binary's inode stays valid even after rename.
        std::fs::rename(&new_path, current_exe)
            .map_err(|e| format!("Failed to replace binary: {}", e))?;
    }

    #[cfg(windows)]
    {
        // On Windows, rename current exe to .old, then rename new to current
        let old_path = current_exe.with_extension("old");
        // Clean up previous .old file if it exists
        let _ = std::fs::remove_file(&old_path);
        std::fs::rename(current_exe, &old_path)
            .map_err(|e| format!("Failed to rename current binary: {}", e))?;
        std::fs::rename(&new_path, current_exe)
            .map_err(|e| format!("Failed to rename new binary: {}", e))?;
    }

    Ok(current_exe.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_installation_standalone() {
        assert_eq!(
            detect_installation_from_path("/usr/local/bin/par-term"),
            InstallationType::StandaloneBinary
        );
        assert_eq!(
            detect_installation_from_path("/home/user/bin/par-term"),
            InstallationType::StandaloneBinary
        );
    }

    #[test]
    fn test_detect_installation_homebrew() {
        assert_eq!(
            detect_installation_from_path("/opt/homebrew/bin/par-term"),
            InstallationType::Homebrew
        );
        assert_eq!(
            detect_installation_from_path("/usr/local/Cellar/par-term/0.12.0/bin/par-term"),
            InstallationType::Homebrew
        );
    }

    #[test]
    fn test_detect_installation_cargo() {
        assert_eq!(
            detect_installation_from_path("/home/user/.cargo/bin/par-term"),
            InstallationType::CargoInstall
        );
    }

    #[test]
    fn test_detect_installation_macos_bundle() {
        assert_eq!(
            detect_installation_from_path("/Applications/par-term.app/Contents/MacOS/par-term"),
            InstallationType::MacOSBundle
        );
    }

    #[test]
    fn test_installation_type_description() {
        assert_eq!(InstallationType::Homebrew.description(), "Homebrew");
        assert_eq!(
            InstallationType::CargoInstall.description(),
            "cargo install"
        );
        assert_eq!(
            InstallationType::MacOSBundle.description(),
            "macOS app bundle"
        );
        assert_eq!(
            InstallationType::StandaloneBinary.description(),
            "standalone binary"
        );
    }
}
