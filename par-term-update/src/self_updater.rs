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

use crate::binary_ops::verify_download;
use crate::install_methods::{install_macos_bundle, install_standalone};
use std::path::PathBuf;

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
