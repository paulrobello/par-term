//! Shared shader installation logic.
//!
//! Used by both the CLI (`install-shaders` command) and the UI (shader install dialog).
//!
//! # Error Handling Convention
//!
//! Functions here return `Result<T, String>` (string errors for UI display)
//! so callers can surface messages to the user without a conversion step.
//! New functions added to this module should follow the same pattern.

use crate::config::Config;
use crate::manifest::{self, FileStatus, Manifest};
use std::io::{Cursor, Read};
use std::path::Path;

/// Result of shader installation
#[derive(Debug, Default)]
pub struct InstallResult {
    /// Number of files installed
    pub installed: usize,
    /// Number of files skipped (unchanged)
    pub skipped: usize,
    /// Files that need user confirmation (modified bundled files)
    pub needs_confirmation: Vec<String>,
    /// Files that were removed (no longer in bundle)
    pub removed: usize,
}

/// Result of shader uninstallation
#[derive(Debug, Default)]
pub struct UninstallResult {
    /// Number of files removed
    pub removed: usize,
    /// Number of files kept (user-created or modified)
    pub kept: usize,
    /// Files that need user confirmation
    pub needs_confirmation: Vec<String>,
}

/// Install shaders from GitHub release
/// Returns the number of shaders installed
pub fn install_shaders() -> Result<usize, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Fetch latest release info
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let (download_url, checksum_url) = get_shaders_download_url(&api_url, REPO)?;

    // Download the zip file with optional SHA256 verification.
    let zip_data = download_and_verify(&download_url, checksum_url.as_deref())?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    // Extract shaders
    extract_shaders(&zip_data, &shaders_dir)?;

    // Count installed shaders
    let count = count_shader_files(&shaders_dir);

    Ok(count)
}

/// Get the download URL (and optional `.sha256` URL) for shaders.zip from the latest release.
///
/// Returns `(zip_url, Option<sha256_url>)`. The SHA256 URL is present only when a
/// `shaders.zip.sha256` asset exists in the release.
pub fn get_shaders_download_url(
    api_url: &str,
    repo: &str,
) -> Result<(String, Option<String>), String> {
    let mut body = crate::http::agent()
        .get(api_url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Parse JSON to find shaders.zip and optional shaders.zip.sha256 browser_download_url.
    let search_pattern = "\"browser_download_url\":\"";
    let target_file = "shaders.zip";
    let checksum_file = "shaders.zip.sha256";

    let mut zip_url: Option<String> = None;
    let mut sha256_url: Option<String> = None;

    for (i, _) in body_str.match_indices(search_pattern) {
        let url_start = i + search_pattern.len();
        if let Some(url_end) = body_str[url_start..].find('"') {
            let url = &body_str[url_start..url_start + url_end];
            if url.ends_with(checksum_file) {
                sha256_url = Some(url.to_string());
            } else if url.ends_with(target_file) {
                zip_url = Some(url.to_string());
            }
        }
    }

    match zip_url {
        Some(url) => Ok((url, sha256_url)),
        None => Err(format!(
            "Could not find shaders.zip in the latest release.\n\
             Please check https://github.com/{}/releases",
            repo
        )),
    }
}

/// Download `zip_url` and, when `checksum_url` is provided, fetch and verify
/// its SHA256 checksum. Returns the raw zip bytes on success.
///
/// # Security
///
/// When `checksum_url` is `None` the download proceeds without checksum
/// verification (pre-checksum releases). A warning is logged to make this
/// visible in the debug log.
pub fn download_and_verify(zip_url: &str, checksum_url: Option<&str>) -> Result<Vec<u8>, String> {
    let zip_data = download_file(zip_url)?;

    match checksum_url {
        Some(csum_url) => {
            // Fetch the expected checksum (plain text: "<hex_digest>  shaders.zip\n")
            let checksum_body = download_file(csum_url)?;
            let checksum_str = String::from_utf8_lossy(&checksum_body);
            // The file may be "<hex>  filename" or just "<hex>\n"
            let expected_hex = checksum_str
                .split_whitespace()
                .next()
                .ok_or_else(|| "Checksum file is empty or malformed".to_string())?;
            verify_sha256(&zip_data, expected_hex)?;
            crate::debug_info!(
                "SHADER_INSTALL",
                "SHA256 checksum verified for shaders.zip: {}",
                expected_hex
            );
        }
        None => {
            // SEC-004: No checksum asset found. Reject the installation rather than
            // proceeding without integrity verification. A missing checksum means
            // the download cannot be validated and MITM injection is possible.
            //
            // Older releases that predate checksum assets are no longer installable
            // via this path. Users requiring older shaders must install manually.
            log::error!(
                "par-term shader install: no shaders.zip.sha256 asset found in release. \
                 Installation aborted — cannot verify download integrity. \
                 Ensure the release includes a shaders.zip.sha256 asset."
            );
            return Err(
                "Shader installation requires a shaders.zip.sha256 checksum asset in the \
                 GitHub release. No checksum asset was found for this release. \
                 Installation cannot proceed without integrity verification. \
                 Please report this to the par-term maintainers."
                    .to_string(),
            );
        }
    }

    Ok(zip_data)
}

/// Maximum download size for shader zip files (50 MB).
/// Prevents memory exhaustion or disk-fill from an oversized or malicious response.
const MAX_SHADER_DOWNLOAD_SIZE: u64 = 50 * 1024 * 1024;

/// Download a file from URL and return its contents.
///
/// # Security
///
/// - Enforces a 50 MB size limit to prevent memory exhaustion.
/// - Callers that require integrity checking should use [`download_file_with_checksum`].
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let mut body = crate::http::agent()
        .get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body();

    body.with_config()
        .limit(MAX_SHADER_DOWNLOAD_SIZE)
        .read_to_vec()
        .map_err(|e| {
            format!(
                "Failed to read download (file may exceed 50 MB limit): {}",
                e
            )
        })
}

/// Download a file and verify its SHA256 checksum.
///
/// `expected_hex` is the lowercase hex-encoded expected SHA256 digest.
/// Returns an error if the digest does not match, preventing installation of
/// tampered or corrupted downloads.
///
/// # Security
///
/// This is the preferred download function for release assets. Checksum
/// verification ensures MITM-injected or corrupted payloads are rejected
/// before extraction.
pub fn download_file_with_checksum(url: &str, expected_hex: &str) -> Result<Vec<u8>, String> {
    let bytes = download_file(url)?;
    verify_sha256(&bytes, expected_hex)?;
    Ok(bytes)
}

/// Compute the lowercase hex SHA256 digest of `data`.
///
/// Uses the `sha2` crate (already a workspace dependency) rather than a
/// hand-rolled implementation, following SEC-008.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    // Format each byte as two lowercase hex digits
    digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write as _;
        write!(acc, "{:02x}", b).unwrap_or_default();
        acc
    })
}

/// Verify that `data` has the given lowercase hex SHA256 digest.
/// Returns `Ok(())` on match, `Err(...)` on mismatch or parse failure.
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> Result<(), String> {
    let actual = sha256_hex(data);
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(format!(
            "SHA256 checksum mismatch — download may be corrupt or tampered.\n\
             Expected: {}\n\
             Actual:   {}",
            expected_hex, actual
        ))
    }
}


/// Extract shaders from zip data to target directory
pub fn extract_shaders(zip_data: &[u8], target_dir: &Path) -> Result<(), String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    Ok(())
}

/// Count .glsl files in directory
pub fn count_shader_files(dir: &Path) -> usize {
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

/// Check if directory contains any .glsl files
pub fn has_shader_files(dir: &Path) -> bool {
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

/// Extract manifest from zip data
pub fn extract_manifest_from_zip(zip_data: &[u8]) -> Result<Manifest, String> {
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    // Look for manifest.json in the zip (may be at root or in shaders/ prefix)
    let manifest_names = ["manifest.json", "shaders/manifest.json"];

    for name in &manifest_names {
        if let Ok(mut file) = archive.by_name(name) {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read manifest: {}", e))?;
            return serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse manifest: {}", e));
        }
    }

    Err("No manifest.json found in zip".to_string())
}

/// Count files tracked by manifest in directory
pub fn count_manifest_files(dir: &Path) -> usize {
    if let Ok(manifest) = Manifest::load(dir) {
        manifest.files.len()
    } else {
        0
    }
}

/// Get version from installed manifest
pub fn get_installed_version(dir: &Path) -> Option<String> {
    Manifest::load(dir).ok().map(|m| m.version)
}

/// Detect bundled shader files that have been modified by the user.
///
/// Returns a list of relative paths that differ from the recorded manifest
/// hashes. If no manifest is present, an empty vector is returned.
pub fn detect_modified_bundled_shaders() -> Result<Vec<String>, String> {
    let shaders_dir = Config::shaders_dir();

    let manifest = match Manifest::load(&shaders_dir) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(Vec::new()),
    };

    let mut modified = Vec::new();

    for file in &manifest.files {
        let path = shaders_dir.join(&file.path);
        let status = manifest::check_file_status(&path, &file.path, &manifest);
        if status == FileStatus::Modified {
            modified.push(file.path.clone());
        }
    }

    Ok(modified)
}

/// Install shaders with manifest support
///
/// Downloads shaders from GitHub and installs them using manifest tracking.
/// - Compares new manifest against existing files
/// - Skips unchanged files
/// - Tracks modified files that need user confirmation
/// - Returns detailed installation result
pub fn install_shaders_with_manifest(force_overwrite: bool) -> Result<InstallResult, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Fetch latest release info
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let (download_url, checksum_url) = get_shaders_download_url(&api_url, REPO)?;

    // Download the zip file with optional SHA256 verification.
    let zip_data = download_and_verify(&download_url, checksum_url.as_deref())?;

    // Extract manifest from the new zip
    let new_manifest = extract_manifest_from_zip(&zip_data)?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    // Load existing manifest if present
    let old_manifest = Manifest::load(&shaders_dir).ok();

    let mut result = InstallResult::default();

    // Build map of new files
    let new_file_map = new_manifest.file_map();

    // Check each file in new manifest
    for new_file in &new_manifest.files {
        let file_path = shaders_dir.join(&new_file.path);
        let status = manifest::check_file_status(&file_path, &new_file.path, &new_manifest);

        match status {
            FileStatus::Missing => {
                // File doesn't exist, will be installed
            }
            FileStatus::Unchanged => {
                // File exists and matches - skip
                result.skipped += 1;
                continue;
            }
            FileStatus::Modified => {
                if !force_overwrite {
                    // User has modified this file - needs confirmation
                    result.needs_confirmation.push(new_file.path.clone());
                    result.skipped += 1;
                    continue;
                }
                // force_overwrite is true, will be installed
            }
            FileStatus::UserCreated => {
                // This shouldn't happen for files in new manifest, but skip anyway
                result.skipped += 1;
                continue;
            }
        }
    }

    // Now actually extract the files
    extract_shaders_with_manifest(&zip_data, &shaders_dir, &new_manifest, force_overwrite)?;

    // Count installed files (all files in manifest minus skipped)
    result.installed = new_manifest.files.len() - result.skipped;

    // Check for removed files (in old manifest but not in new)
    if let Some(old_manifest) = old_manifest {
        for old_file in &old_manifest.files {
            if !new_file_map.contains_key(old_file.path.as_str()) {
                let old_path = shaders_dir.join(&old_file.path);
                if old_path.exists() {
                    // Check if file matches old manifest (unmodified bundled file)
                    let status =
                        manifest::check_file_status(&old_path, &old_file.path, &old_manifest);
                    if status == FileStatus::Unchanged || force_overwrite {
                        // Safe to remove - it's an unmodified bundled file
                        if std::fs::remove_file(&old_path).is_ok() {
                            result.removed += 1;
                        }
                    }
                }
            }
        }
    }

    // Save the new manifest
    new_manifest.save(&shaders_dir)?;

    Ok(result)
}

/// Extract shaders from zip with manifest awareness
fn extract_shaders_with_manifest(
    zip_data: &[u8],
    target_dir: &Path,
    manifest: &Manifest,
    force_overwrite: bool,
) -> Result<(), String> {
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    // Build set of files to install from manifest
    let manifest_files: std::collections::HashSet<&str> =
        manifest.files.iter().map(|f| f.path.as_str()).collect();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);
        let relative_path_str = relative_path.to_string_lossy();

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        // Always extract manifest.json
        let is_manifest = relative_path_str == "manifest.json";

        // Skip files not in manifest (except manifest.json itself)
        if !is_manifest && !manifest_files.contains(&relative_path_str.as_ref()) {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Check if file exists and is modified (skip unless force)
        if !is_manifest && final_path.exists() && !force_overwrite {
            let status = manifest::check_file_status(&final_path, &relative_path_str, manifest);
            if status == FileStatus::Modified {
                continue; // Skip modified files unless force
            }
        }

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    Ok(())
}

/// Uninstall bundled shaders
///
/// Only removes files that are tracked by the manifest.
/// - Preserves user-created files
/// - Optionally preserves modified bundled files
pub fn uninstall_shaders(force: bool) -> Result<UninstallResult, String> {
    let shaders_dir = Config::shaders_dir();

    // Load manifest
    let manifest = Manifest::load(&shaders_dir)
        .map_err(|_| "No manifest found - cannot determine which files are bundled".to_string())?;

    let mut result = UninstallResult::default();

    // Process each file in manifest
    for manifest_file in &manifest.files {
        let file_path = shaders_dir.join(&manifest_file.path);

        if !file_path.exists() {
            continue;
        }

        let status = manifest::check_file_status(&file_path, &manifest_file.path, &manifest);

        match status {
            FileStatus::Unchanged => {
                // Unmodified bundled file - safe to remove
                if std::fs::remove_file(&file_path).is_ok() {
                    result.removed += 1;
                }
            }
            FileStatus::Modified => {
                if force {
                    // Force removal of modified files
                    if std::fs::remove_file(&file_path).is_ok() {
                        result.removed += 1;
                    }
                } else {
                    // Needs user confirmation
                    result.needs_confirmation.push(manifest_file.path.clone());
                    result.kept += 1;
                }
            }
            FileStatus::UserCreated | FileStatus::Missing => {
                // Not a bundled file or already gone
                result.kept += 1;
            }
        }
    }

    // Remove manifest file itself
    let manifest_path = shaders_dir.join("manifest.json");
    if manifest_path.exists() && std::fs::remove_file(&manifest_path).is_ok() {
        result.removed += 1;
    }

    // Try to remove empty directories
    cleanup_empty_dirs(&shaders_dir);

    Ok(result)
}

/// Remove empty directories recursively
fn cleanup_empty_dirs(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                cleanup_empty_dirs(&path);
                // Try to remove if empty (will fail if not empty, which is fine)
                let _ = std::fs::remove_dir(&path);
            }
        }
    }
}
