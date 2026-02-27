//! Self-update functionality for par-term.
//!
//! This module handles downloading and installing updates in-place,
//! detecting the current installation method, and performing
//! platform-specific binary replacement.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// How par-term was installed — determines update strategy
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
    /// Human-readable description of the installation type
    pub fn description(&self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::CargoInstall => "cargo install",
            Self::MacOSBundle => "macOS app bundle",
            Self::StandaloneBinary => "standalone binary",
        }
    }
}

/// Result of a successful self-update
#[derive(Debug, Clone)]
pub struct UpdateResult {
    /// Version before the update
    pub old_version: String,
    /// Version after the update
    pub new_version: String,
    /// Path where the binary was installed
    pub install_path: PathBuf,
    /// Whether a restart is needed to use the new version
    pub needs_restart: bool,
}

/// Clean up leftover `.old` binary from a previous self-update.
///
/// On Windows, the running exe cannot be deleted or overwritten, so during
/// self-update we rename it to `.old`. This function removes that stale
/// file on the next startup. It is safe to call on all platforms — on
/// non-Windows it is a no-op.
pub fn cleanup_old_binary() {
    #[cfg(windows)]
    {
        if let Ok(current_exe) = std::env::current_exe() {
            let old_path = current_exe.with_extension("old");
            if old_path.exists() {
                match std::fs::remove_file(&old_path) {
                    Ok(()) => {
                        log::info!(
                            "Cleaned up old binary from previous update: {}",
                            old_path.display()
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to clean up old binary {}: {}",
                            old_path.display(),
                            e
                        );
                    }
                }
            }
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
fn detect_installation_from_path(path: &str) -> InstallationType {
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

/// Get the platform-specific asset name for the current OS/architecture.
pub fn get_asset_name() -> Result<&'static str, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") => Ok("par-term-macos-aarch64.zip"),
        ("macos", "x86_64") => Ok("par-term-macos-x86_64.zip"),
        ("linux", "aarch64") => Ok("par-term-linux-aarch64"),
        ("linux", "x86_64") => Ok("par-term-linux-x86_64"),
        ("windows", "x86_64") => Ok("par-term-windows-x86_64.exe"),
        _ => Err(format!(
            "Unsupported platform: {} {}. \
             Please download manually from GitHub releases.",
            os, arch
        )),
    }
}

/// Get the checksum asset name for the current platform.
///
/// Returns the expected `.sha256` filename, e.g. `par-term-macos-aarch64.zip.sha256`.
pub fn get_checksum_asset_name() -> Result<String, String> {
    let asset_name = get_asset_name()?;
    Ok(format!("{}.sha256", asset_name))
}

/// Compute SHA256 hash of in-memory data, returning the lowercase hex string.
pub fn compute_data_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Download URLs for the binary and optional checksum from a GitHub release.
pub struct DownloadUrls {
    /// URL for the platform binary/archive asset
    pub binary_url: String,
    /// URL for the `.sha256` checksum file, if present in the release
    pub checksum_url: Option<String>,
}

/// Get the download URLs for the platform binary and checksum from the release API response.
pub fn get_download_urls(api_url: &str) -> Result<DownloadUrls, String> {
    let asset_name = get_asset_name()?;
    let checksum_name = get_checksum_asset_name()?;

    // Validate the API URL before making the request.
    crate::http::validate_update_url(api_url)?;

    let mut body = crate::http::agent()
        .get(api_url)
        .header("User-Agent", "par-term")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| {
            format!(
                "Failed to fetch release info from '{}': {}. \
                 Check your internet connection and try again.",
                api_url, e
            )
        })?
        .into_body();

    let body_str = body
        .with_config()
        .limit(crate::http::MAX_API_RESPONSE_SIZE)
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Parse JSON and extract browser_download_url values from assets array
    let json: serde_json::Value =
        serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let mut binary_url: Option<String> = None;
    let mut checksum_url: Option<String> = None;

    if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                if url.ends_with(&checksum_name) {
                    // Validate each download URL extracted from the release JSON
                    // before storing it — a compromised release payload could
                    // otherwise inject a URL pointing to an attacker-controlled host.
                    crate::http::validate_update_url(url).map_err(|e| {
                        format!(
                            "Checksum asset URL from GitHub release failed validation: {}",
                            e
                        )
                    })?;
                    checksum_url = Some(url.to_string());
                } else if url.ends_with(asset_name) {
                    crate::http::validate_update_url(url).map_err(|e| {
                        format!(
                            "Binary asset URL from GitHub release failed validation: {}",
                            e
                        )
                    })?;
                    binary_url = Some(url.to_string());
                }
            }
        }
    }

    match binary_url {
        Some(url) => Ok(DownloadUrls {
            binary_url: url,
            checksum_url,
        }),
        None => Err(format!(
            "Could not find asset '{}' in the latest GitHub release.\n\
             This platform ({} {}) may not yet have a prebuilt binary for this release.\n\
             Please download manually from https://github.com/paulrobello/par-term/releases",
            asset_name,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )),
    }
}

/// Get the download URL for the platform binary from the release API response.
///
/// This is a convenience wrapper around [`get_download_urls`] that returns only
/// the binary URL, for callers that don't need checksum verification.
pub fn get_binary_download_url(api_url: &str) -> Result<String, String> {
    get_download_urls(api_url).map(|urls| urls.binary_url)
}

/// Parse expected hash from a `.sha256` checksum file.
///
/// Supports two common formats:
/// - Plain hash: `abcdef1234...`
/// - BSD/GNU style: `abcdef1234...  filename`
fn parse_checksum_file(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Checksum file is empty".to_string());
    }

    // Take the first whitespace-delimited token as the hex hash
    let hash = trimmed
        .split_whitespace()
        .next()
        .ok_or_else(|| "Checksum file is empty".to_string())?
        .to_lowercase();

    // Validate it looks like a SHA256 hex string (64 hex chars)
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Checksum file does not contain a valid SHA256 hash (got '{}')",
            hash
        ));
    }

    Ok(hash)
}

/// Verify the downloaded data against a SHA256 checksum from the release.
///
/// Returns `Ok(())` if verification passes or no checksum is available
/// (with a warning log for older releases).
/// Returns `Err` if:
/// - A checksum URL exists but the download fails (security: abort unverified updates)
/// - The checksum does not match (binary may be corrupted or tampered with)
fn verify_download(data: &[u8], checksum_url: Option<&str>) -> Result<(), String> {
    let checksum_url = match checksum_url {
        Some(url) => url,
        None => {
            // No checksum available for this release (older releases)
            log::warn!(
                "No .sha256 checksum file found in release — \
                 skipping integrity verification. \
                 This is expected for older releases."
            );
            return Ok(());
        }
    };

    // Download the checksum file
    // SECURITY: If a checksum URL exists but download fails, we MUST abort the update.
    // Returning Ok(()) here would allow a MITM attacker to block the checksum URL
    // while allowing the binary URL through, resulting in an unverified install.
    let checksum_data = crate::http::download_file(checksum_url).map_err(|e| {
        format!(
            "Failed to download checksum file from {}: {}\n\
             Update aborted for security — cannot verify binary integrity without checksum.\n\
             This may indicate a network issue or a targeted attack blocking checksum verification.\n\
             If the problem persists, please download manually from:\n\
             https://github.com/paulrobello/par-term/releases",
            checksum_url, e
        )
    })?;

    let checksum_content = String::from_utf8(checksum_data)
        .map_err(|_| "Checksum file contains invalid UTF-8".to_string())?;

    let expected_hash = parse_checksum_file(&checksum_content)?;
    let actual_hash = compute_data_hash(data);

    if actual_hash != expected_hash {
        return Err(format!(
            "Checksum verification failed!\n\
             Expected: {}\n\
             Actual:   {}\n\
             The downloaded binary may be corrupted or tampered with. \
             Update aborted for safety.",
            expected_hash, actual_hash
        ));
    }

    log::info!("SHA256 checksum verified successfully");
    Ok(())
}

/// Perform the self-update: download, verify, replace binary, report result.
///
/// # Arguments
/// * `new_version` - The version being updated to
/// * `old_version` - The current application version (from root crate's `VERSION` constant)
pub fn perform_update(new_version: &str, old_version: &str) -> Result<UpdateResult, String> {
    let installation = detect_installation();

    // Refuse update for managed installations
    match &installation {
        InstallationType::Homebrew => {
            return Err(
                "par-term is installed via Homebrew. Please update with:\n  \
                 brew upgrade --cask par-term"
                    .to_string(),
            );
        }
        InstallationType::CargoInstall => {
            return Err("par-term is installed via cargo. Please update with:\n  \
                 cargo install par-term"
                .to_string());
        }
        InstallationType::MacOSBundle | InstallationType::StandaloneBinary => {
            // These can be updated in-place
        }
    }

    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to determine current exe: {}", e))?;

    // Fetch release API and get download URLs (binary + optional checksum)
    let api_url = "https://api.github.com/repos/paulrobello/par-term/releases/latest";
    let urls = get_download_urls(api_url)?;

    // Download the binary/archive
    let data = crate::http::download_file(&urls.binary_url)?;

    // Sanity-check the content type before verifying the checksum.
    // This catches obviously wrong responses (e.g., HTML error pages) early,
    // giving a clearer error message than a checksum mismatch would.
    crate::http::validate_binary_content(&data)?;

    // Verify SHA256 checksum (fails on mismatch, warns if no checksum available)
    verify_download(&data, urls.checksum_url.as_deref())?;

    // Perform platform-specific installation
    let install_path = match installation {
        InstallationType::MacOSBundle => install_macos_bundle(&current_exe, &data)?,
        InstallationType::StandaloneBinary => install_standalone(&current_exe, &data)?,
        _ => unreachable!("Managed installations are rejected above"),
    };

    Ok(UpdateResult {
        old_version: old_version.to_string(),
        new_version: new_version.to_string(),
        install_path,
        needs_restart: true,
    })
}

/// Install update for macOS .app bundle by extracting the zip.
fn install_macos_bundle(current_exe: &std::path::Path, zip_data: &[u8]) -> Result<PathBuf, String> {
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
fn install_standalone(current_exe: &std::path::Path, data: &[u8]) -> Result<PathBuf, String> {
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
    fn test_get_asset_name() {
        // Should return a valid asset name for the current platform
        let result = get_asset_name();
        assert!(
            result.is_ok(),
            "get_asset_name() should succeed on supported platforms"
        );
        let name = result.unwrap();
        assert!(
            name.starts_with("par-term-"),
            "Asset name should start with 'par-term-'"
        );
    }

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

    #[test]
    fn test_get_checksum_asset_name() {
        let result = get_checksum_asset_name();
        assert!(result.is_ok());
        let name = result.unwrap();
        assert!(
            name.ends_with(".sha256"),
            "Checksum asset name should end with .sha256, got '{}'",
            name
        );
        assert!(
            name.starts_with("par-term-"),
            "Checksum asset name should start with 'par-term-', got '{}'",
            name
        );
    }

    #[test]
    fn test_compute_data_hash_known_value() {
        // SHA256 of "hello world"
        let hash = compute_data_hash(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_data_hash_empty() {
        let hash = compute_data_hash(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_parse_checksum_file_plain_hash() {
        let content = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9\n";
        let hash = parse_checksum_file(content).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksum_file_with_filename() {
        let content = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9  par-term-linux-x86_64\n";
        let hash = parse_checksum_file(content).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksum_file_uppercase_normalized() {
        let content = "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9\n";
        let hash = parse_checksum_file(content).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_parse_checksum_file_empty() {
        let result = parse_checksum_file("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_checksum_file_invalid_hash() {
        let result = parse_checksum_file("not-a-hash");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("valid SHA256"));
    }

    #[test]
    fn test_parse_checksum_file_wrong_length() {
        // 32 hex chars (MD5 length) instead of 64 (SHA256 length)
        let result = parse_checksum_file("d41d8cd98f00b204e9800998ecf8427e");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("valid SHA256"));
    }

    #[test]
    fn test_verify_download_no_checksum_url() {
        // Should succeed with warning when no checksum URL is available
        let data = b"some binary data";
        let result = verify_download(data, None);
        assert!(result.is_ok());
    }
}
