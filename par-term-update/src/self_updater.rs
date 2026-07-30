//! Self-update orchestration for par-term.
//!
//! This module is the entry point for the self-update workflow. It delegates
//! to the focused sub-modules:
//! - [`crate::install_methods`] — installation type detection and binary replacement
//! - [`crate::binary_ops`] — asset name resolution, checksum verification, download URLs

// Re-export the public API so callers can continue to use `self_updater::*`.
pub use crate::binary_ops::{
    DownloadUrls, cleanup_old_binary, compute_data_hash, get_asset_name, get_binary_download_url,
    get_checksum_asset_name, get_download_urls,
};
pub use crate::install_methods::{InstallationType, detect_installation};

use crate::binary_ops::{fetch_expected_hash, fetch_signature_text, verify_hash};
use crate::install_methods::{install_macos_bundle, install_standalone};
use std::path::{Path, PathBuf};

/// Result of a successful self-update.
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

/// Perform the self-update: download, verify, replace binary, report result.
///
/// # Arguments
/// * `new_version` - The version being updated to
/// * `old_version` - The current application version (from root crate's `VERSION` constant)
pub fn perform_update(new_version: &str, old_version: &str) -> Result<UpdateResult, String> {
    let installation = detect_installation()?;

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

    // Refuse before spending bandwidth on a download this build could never
    // accept. `verify_and_install` re-checks the same condition, so this is a
    // better error message rather than the gate itself.
    if !crate::signature::signing_key_configured() {
        crate::signature::verify_detached(&[], "")?;
    }

    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to determine current exe: {}", e))?;

    // Fetch release API and get download URLs (binary + checksum + signature)
    let api_url = "https://api.github.com/repos/paulrobello/par-term/releases/latest";
    let urls = get_download_urls(api_url)?;

    // Download the binary/archive
    let data = crate::http::download_file(&urls.binary_url)?;

    // Sanity-check the content type before verifying the checksum.
    // This catches obviously wrong responses (e.g., HTML error pages) early,
    // giving a clearer error message than a checksum mismatch would.
    crate::http::validate_binary_content(&data)?;

    let expected_hash = fetch_expected_hash(urls.checksum_url.as_deref())?;
    let signature_text = fetch_signature_text(urls.signature_url.as_deref())?;

    let install_path = verify_and_install(
        &installation,
        &current_exe,
        &data,
        &expected_hash,
        &signature_text,
    )?;

    Ok(UpdateResult {
        old_version: old_version.to_string(),
        new_version: new_version.to_string(),
        install_path,
        needs_restart: true,
    })
}

/// Run every verification gate, then install — in that order.
///
/// Split out from [`perform_update`] so the ordering guarantee can be tested
/// without a live release: both arguments are already-fetched bytes, so this
/// function performs no network I/O.
///
/// # Ordering
///
/// 1. **SHA256** against the release checksum — integrity.
/// 2. **Detached minisign signature** against the pinned public key —
///    authenticity. The checksum cannot cover this: it ships as an asset of the
///    same release as the binary, so whoever can replace one can replace both.
/// 3. Only then, platform-specific installation.
///
/// Both gates precede every write, and on macOS the install itself stages and
/// verifies before swapping, so a failure at any point leaves the existing
/// installation untouched.
pub(crate) fn verify_and_install(
    installation: &InstallationType,
    current_exe: &Path,
    data: &[u8],
    expected_hash: &str,
    signature_text: &str,
) -> Result<PathBuf, String> {
    verify_hash(data, expected_hash)?;
    crate::signature::verify_detached(data, signature_text)?;

    match installation {
        InstallationType::MacOSBundle => install_macos_bundle(current_exe, data),
        InstallationType::StandaloneBinary => install_standalone(current_exe, data),
        InstallationType::Homebrew | InstallationType::CargoInstall => {
            Err("Managed installations cannot be updated in place".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary_ops::compute_data_hash;

    /// A well-formed-looking `.minisig` that signs nothing.
    const WRONG_SIGNATURE: &str = "untrusted comment: signature from minisign secret key\n\
         RUQf6LRCGA9i57777777777777777777777777777777777777777777777777777777777777777\n\
         trusted comment: timestamp:1700000000\tfile:par-term\n\
         77777777777777777777777777777777777777777777777777777777777777777777777777==\n";

    /// SEC-005's acceptance test: a bad signature must refuse the install *and*
    /// leave the binary that is already on disk byte-for-byte unchanged.
    ///
    /// The checksum is deliberately correct, so the only gate that can reject
    /// this payload is the signature gate — if a refactor ever lets the install
    /// run before signature verification, this test fails.
    #[test]
    fn wrong_signature_refuses_and_leaves_the_existing_binary_untouched() {
        let dir = tempfile::tempdir().expect("tempdir");
        let existing = dir.path().join("par-term");
        let original = b"ORIGINAL BINARY CONTENTS";
        std::fs::write(&existing, original).expect("seed the existing binary");

        let update = b"REPLACEMENT BINARY CONTENTS";
        let result = verify_and_install(
            &InstallationType::StandaloneBinary,
            &existing,
            update,
            &compute_data_hash(update),
            WRONG_SIGNATURE,
        );

        let err = result.expect_err("a bad signature must refuse the install");
        assert!(
            err.to_lowercase().contains("signature"),
            "the refusal must come from the signature gate, got: {err}"
        );

        assert_eq!(
            std::fs::read(&existing).expect("read the existing binary"),
            original,
            "the existing binary must be untouched after a refused update"
        );
        assert!(
            !existing.with_extension("new").exists(),
            "no staged binary should be left behind after a refused update"
        );
    }

    /// The checksum gate still runs first and still precedes any write.
    #[test]
    fn wrong_checksum_refuses_and_leaves_the_existing_binary_untouched() {
        let dir = tempfile::tempdir().expect("tempdir");
        let existing = dir.path().join("par-term");
        let original = b"ORIGINAL BINARY CONTENTS";
        std::fs::write(&existing, original).expect("seed the existing binary");

        let result = verify_and_install(
            &InstallationType::StandaloneBinary,
            &existing,
            b"REPLACEMENT BINARY CONTENTS",
            "0000000000000000000000000000000000000000000000000000000000000000",
            WRONG_SIGNATURE,
        );

        let err = result.expect_err("a bad checksum must refuse the install");
        assert!(
            err.contains("Checksum verification failed"),
            "the refusal must come from the checksum gate, got: {err}"
        );
        assert_eq!(
            std::fs::read(&existing).expect("read the existing binary"),
            original
        );
    }

    #[test]
    fn managed_installations_are_not_installable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let existing = dir.path().join("par-term");
        std::fs::write(&existing, b"x").expect("seed");

        for installation in [InstallationType::Homebrew, InstallationType::CargoInstall] {
            assert!(
                verify_and_install(&installation, &existing, b"x", &compute_data_hash(b"x"), "")
                    .is_err()
            );
        }
    }
}
