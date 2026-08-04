//! Binary download, hash verification, and cleanup operations for self-update.
//!
//! This module is responsible for:
//! - Determining the platform-specific asset name and download URLs
//! - Computing and verifying SHA256 checksums
//! - Cleaning up leftover `.old` binaries from previous updates

use sha2::{Digest, Sha256};

/// Ordered asset-name candidates for the current platform (preferred first).
///
/// macOS prefers the Universal build (`par-term-macos-universal.zip`) so an
/// Apple-Silicon host never installs the Intel-only slice (and an Intel host
/// never installs the ARM-only slice) — the Rosetta "Support ending for
/// Intel-based apps" path. The per-arch asset is kept as a fallback so the
/// updater still resolves against a release that predates the Universal build,
/// or one where the Universal job failed but the per-arch job succeeded. Other
/// platforms resolve to a single candidate.
fn asset_name_candidates() -> Result<Vec<&'static str>, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("macos", "aarch64") => Ok(vec![
            "par-term-macos-universal.zip",
            "par-term-macos-aarch64.zip",
        ]),
        ("macos", "x86_64") => Ok(vec![
            "par-term-macos-universal.zip",
            "par-term-macos-x86_64.zip",
        ]),
        ("linux", "aarch64") => Ok(vec!["par-term-linux-aarch64"]),
        ("linux", "x86_64") => Ok(vec!["par-term-linux-x86_64"]),
        ("windows", "x86_64") => Ok(vec!["par-term-windows-x86_64.exe"]),
        _ => Err(format!(
            "Unsupported platform: {} {}. \
             Please download manually from GitHub releases.",
            os, arch
        )),
    }
}

/// Get the preferred platform asset name for the current OS/architecture.
///
/// This is the first (preferred) entry from [`asset_name_candidates`] — the
/// Universal build on macOS. [`get_download_urls`] falls back through the rest
/// of the list when the preferred asset is absent from a release.
pub fn get_asset_name() -> Result<&'static str, String> {
    asset_name_candidates()?
        .into_iter()
        .next()
        .ok_or_else(|| "No asset name candidate resolved for this platform".to_string())
}

/// Get the checksum asset name for the current platform.
///
/// Returns the expected `.sha256` filename, e.g. `par-term-macos-universal.zip.sha256`.
pub fn get_checksum_asset_name() -> Result<String, String> {
    let asset_name = get_asset_name()?;
    Ok(format!("{}.sha256", asset_name))
}

/// Get the detached-signature asset name for the current platform.
///
/// Returns the expected `.minisig` filename, e.g.
/// `par-term-macos-universal.zip.minisig` — the name `minisign -S -m <asset>`
/// produces by default.
pub fn get_signature_asset_name() -> Result<String, String> {
    let asset_name = get_asset_name()?;
    Ok(format!("{}.minisig", asset_name))
}

/// Compute SHA256 hash of in-memory data, returning the lowercase hex string.
pub fn compute_data_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// Download URLs for the binary and its verification assets from a GitHub release.
pub struct DownloadUrls {
    /// URL for the platform binary/archive asset
    pub binary_url: String,
    /// URL for the `.sha256` checksum file, if present in the release
    pub checksum_url: Option<String>,
    /// URL for the `.minisig` detached signature, if present in the release
    pub signature_url: Option<String>,
}

/// Get the download URLs for the platform binary, checksum and signature from
/// the release API response.
///
/// macOS prefers the Universal asset and falls back to the per-arch asset; see
/// [`asset_name_candidates`]. The first candidate whose binary is present in the
/// release is selected, and its `.sha256` / `.minisig` URLs are resolved
/// alongside it.
pub fn get_download_urls(api_url: &str) -> Result<DownloadUrls, String> {
    let candidates = asset_name_candidates()?;

    let body_str = crate::http::get_validated(api_url, Some("application/vnd.github+json"))?
        .into_body()
        .with_config()
        .limit(crate::http::MAX_API_RESPONSE_SIZE)
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Collect every asset download URL declared by the release so candidate
    // selection can scan them without re-walking the JSON.
    let asset_urls: Vec<String> = json
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| {
                    asset
                        .get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();

    // Pick the first candidate (preferred order) whose binary asset is present.
    // macOS resolves to the Universal build when available and the per-arch
    // build otherwise, so an Apple-Silicon host never installs the Intel-only
    // slice even when updating against an older release that predates Universal.
    let asset_name = candidates
        .iter()
        .find(|candidate| asset_urls.iter().any(|url| url.ends_with(*candidate)))
        .copied()
        .ok_or_else(|| {
            format!(
                "Could not find asset '{}' in the latest GitHub release.\n\
                 This platform ({} {}) may not yet have a prebuilt binary for this release.\n\
                 Please download manually from https://github.com/paulrobello/par-term/releases",
                get_asset_name().unwrap_or("(unknown platform)"),
                std::env::consts::OS,
                std::env::consts::ARCH,
            )
        })?;

    let checksum_name = format!("{}.sha256", asset_name);
    let signature_name = format!("{}.minisig", asset_name);

    let mut binary_url: Option<String> = None;
    let mut checksum_url: Option<String> = None;
    let mut signature_url: Option<String> = None;

    if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                // Suffix order matters: `<asset>.sha256` and `<asset>.minisig`
                // both contain `<asset>`, so the derived names are tested first.
                let slot = if url.ends_with(&signature_name) {
                    ("Signature", &mut signature_url)
                } else if url.ends_with(&checksum_name) {
                    ("Checksum", &mut checksum_url)
                } else if url.ends_with(asset_name) {
                    ("Binary", &mut binary_url)
                } else {
                    continue;
                };

                // Validate each download URL extracted from the release JSON
                // before storing it — a compromised release payload could
                // otherwise inject a URL pointing to an attacker-controlled host.
                let (kind, target) = slot;
                crate::http::validate_update_url(url).map_err(|e| {
                    format!(
                        "{} asset URL from GitHub release failed validation: {}",
                        kind, e
                    )
                })?;
                *target = Some(url.to_string());
            }
        }
    }

    match binary_url {
        Some(url) => Ok(DownloadUrls {
            binary_url: url,
            checksum_url,
            signature_url,
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
pub(crate) fn parse_checksum_file(content: &str) -> Result<String, String> {
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

/// Fetch and parse the expected SHA256 hash for this release.
///
/// Returns `Err` if:
/// - No checksum URL is available for the release (hard-fail — refuse to
///   install an unverified binary; matches the shader installer's policy)
/// - A checksum URL exists but the download fails (security: abort unverified updates)
///
/// Note that the checksum defends against *corruption*, not against a
/// compromised release: the `.sha256` is an asset of the same release as the
/// binary. [`crate::signature`] is the gate that covers compromise.
pub(crate) fn fetch_expected_hash(checksum_url: Option<&str>) -> Result<String, String> {
    let checksum_url = match checksum_url {
        Some(url) => url,
        None => {
            // A missing .sha256 checksum file is a hard integrity failure, NOT
            // a warning. Returning Ok here would let a MITM attacker (or a
            // compromised release) ship an unverified binary.
            return Err("No .sha256 checksum file found in release — \
                 refusing to install an unverified binary.\n\
                 Update aborted for safety. This release may predate checksum \
                support, or the checksum file was stripped/blocked.\n\
                 Please download manually from:\n\
                 https://github.com/paulrobello/par-term/releases"
                .to_string());
        }
    };

    // SECURITY: If a checksum URL exists but download fails, we MUST abort the update.
    // Succeeding here would allow a MITM attacker to block the checksum URL
    // while allowing the binary URL through, resulting in an unverified install.
    let checksum_data = crate::http::download_file(checksum_url).map_err(|e| {
        format!(
            "Failed to download the release checksum file: {}\n\
             Update aborted for security — cannot verify binary integrity without checksum.\n\
             This may indicate a network issue or a targeted attack blocking checksum verification.\n\
             If the problem persists, please download manually from:\n\
             https://github.com/paulrobello/par-term/releases",
            e
        )
    })?;

    let checksum_content = String::from_utf8(checksum_data)
        .map_err(|_| "Checksum file contains invalid UTF-8".to_string())?;

    parse_checksum_file(&checksum_content)
}

/// Fetch the detached `.minisig` signature text for this release.
///
/// A missing signature asset is a hard failure for the same reason a missing
/// checksum is: silently skipping the gate is indistinguishable from an attacker
/// stripping the asset.
pub(crate) fn fetch_signature_text(signature_url: Option<&str>) -> Result<String, String> {
    let signature_url = match signature_url {
        Some(url) => url,
        None => {
            return Err("No .minisig signature file found in release — \
                 refusing to install an unsigned binary.\n\
                 The SHA256 checksum alone cannot detect a compromised release, \
                 because the checksum is published as an asset of that same release.\n\
                 Update aborted for safety. Please download manually from:\n\
                 https://github.com/paulrobello/par-term/releases"
                .to_string());
        }
    };

    let signature_data = crate::http::download_file(signature_url).map_err(|e| {
        format!(
            "Failed to download the release signature file: {}\n\
             Update aborted for security — cannot verify the binary's authenticity \
             without its signature.\n\
             If the problem persists, please download manually from:\n\
             https://github.com/paulrobello/par-term/releases",
            e
        )
    })?;

    String::from_utf8(signature_data)
        .map_err(|_| "Signature file contains invalid UTF-8; it is not a minisign .minisig".into())
}

/// Compare `data`'s SHA256 against the expected hash from the release.
///
/// Pure — no network access — so the verification ordering can be tested
/// without a live release.
pub(crate) fn verify_hash(data: &[u8], expected_hash: &str) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the current platform has a published release artifact.
    ///
    /// Mirrors the match in [`get_asset_name`]. Notably `windows` + `aarch64`
    /// is absent: release.yml builds only `x86_64-pc-windows-msvc`, so ARM64
    /// Windows genuinely has no artifact and an error there is correct
    /// behaviour, not a failure.
    fn platform_has_release_artifact() -> bool {
        matches!(
            (std::env::consts::OS, std::env::consts::ARCH),
            ("macos", "aarch64")
                | ("macos", "x86_64")
                | ("linux", "aarch64")
                | ("linux", "x86_64")
                | ("windows", "x86_64")
        )
    }

    #[test]
    fn test_get_asset_name() {
        let result = get_asset_name();
        if platform_has_release_artifact() {
            let name = result.expect("a platform with a release artifact must resolve a name");
            assert!(
                name.starts_with("par-term-"),
                "Asset name should start with 'par-term-'"
            );
        } else {
            let err = result.expect_err("a platform with no release artifact must report one");
            assert!(
                err.contains("Unsupported platform"),
                "error should explain the platform is unsupported, got '{}'",
                err
            );
        }
    }

    #[test]
    fn test_macos_prefers_universal_build() {
        // On macOS the Universal build must be the first candidate so the
        // updater never installs an Intel-only binary on Apple Silicon (or
        // vice-versa), with the per-arch asset as a fallback for older releases.
        if std::env::consts::OS != "macos" {
            return;
        }
        let candidates = asset_name_candidates().expect("macos resolves candidates");
        assert_eq!(
            candidates[0], "par-term-macos-universal.zip",
            "macOS must prefer the Universal build"
        );
        assert!(
            candidates
                .iter()
                .any(|c| c.ends_with(&format!("-{}.zip", std::env::consts::ARCH))),
            "expected a per-arch fallback candidate for {}",
            std::env::consts::ARCH
        );
    }

    #[test]
    fn test_get_checksum_asset_name() {
        if !platform_has_release_artifact() {
            assert!(get_checksum_asset_name().is_err());
            return;
        }
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
    fn test_fetch_expected_hash_no_checksum_url() {
        // A missing checksum URL must abort the update (not silently pass with
        // a warning), so a compromised release cannot ship an unverified binary.
        let result = fetch_expected_hash(None);
        assert!(result.is_err(), "expected hard-fail on missing checksum");
        assert!(
            result.unwrap_err().contains("refusing to install"),
            "expected hard-fail message referencing the abort policy"
        );
    }

    #[test]
    fn test_fetch_signature_text_no_signature_url() {
        // SEC-005: same policy for the signature. A release that ships no
        // .minisig cannot be installed.
        let result = fetch_signature_text(None);
        assert!(result.is_err(), "expected hard-fail on missing signature");
        let err = result.unwrap_err();
        assert!(
            err.contains("refusing to install an unsigned binary"),
            "expected hard-fail message referencing the abort policy: {err}"
        );
    }

    #[test]
    fn test_get_signature_asset_name() {
        if !platform_has_release_artifact() {
            assert!(get_signature_asset_name().is_err());
            return;
        }
        let name = get_signature_asset_name().expect("signature asset name");
        assert!(
            name.ends_with(".minisig"),
            "signature asset name should end with .minisig, got '{}'",
            name
        );
    }

    #[test]
    fn test_verify_hash_matches() {
        let data = b"hello world";
        assert!(verify_hash(data, &compute_data_hash(data)).is_ok());
    }

    #[test]
    fn test_verify_hash_mismatch_is_rejected() {
        let result = verify_hash(
            b"tampered payload",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Checksum verification failed"));
    }
}
