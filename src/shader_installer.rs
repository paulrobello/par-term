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
            // No checksum asset in this release. Warn but allow installation so
            // that older releases remain installable. Future releases SHOULD
            // include a shaders.zip.sha256 asset.
            log::warn!(
                "par-term shader install: no shaders.zip.sha256 asset found in release. \
                 Proceeding without integrity verification. \
                 MITM injection of shaders is possible."
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
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = {
        // Use a simple implementation to avoid adding a heavy dependency.
        // sha2 is already transitively available via the update crate.
        compute_sha256(data)
    };
    let mut hex = String::with_capacity(64);
    for byte in &digest {
        write!(hex, "{:02x}", byte).unwrap_or_default();
    }
    hex
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

/// Minimal SHA-256 implementation to avoid adding a new crate dependency.
///
/// Follows FIPS 180-4. This is used only for integrity verification of
/// downloaded release assets; it is not used for any cryptographic secret.
fn compute_sha256(data: &[u8]) -> [u8; 32] {
    // Round constants (first 32 bits of cube roots of first 64 primes)
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    // Initial hash values (first 32 bits of square roots of first 8 primes)
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Pre-processing: pad the message
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process 512-bit chunks
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, b) in chunk.chunks_exact(4).enumerate().take(16) {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut digest = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
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
