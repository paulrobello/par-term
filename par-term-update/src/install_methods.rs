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
///
/// Returns `Err` when the executable path cannot be determined, rather than
/// classifying a placeholder path. Classification is purely substring-based, so any
/// path that matches nothing — including the empty one — yields
/// [`InstallationType::StandaloneBinary`], which would let a self-update overwrite a
/// package-managed binary.
pub fn detect_installation() -> Result<InstallationType, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Failed to determine current executable path: {}", e))?;
    Ok(detect_installation_from_path(
        exe.to_string_lossy().as_ref(),
    ))
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

/// Code requirement the downloaded bundle must satisfy (SEC-005).
///
/// `anchor apple generic` requires the certificate chain to terminate at Apple's
/// root; `subject.OU` pins the leaf to par-term's Apple Team ID. Together they
/// reject a bundle signed by any other developer, and reject an ad-hoc signature
/// outright.
///
/// Passed to `codesign` as `-R=<requirement>`. The `=` prefix is what makes
/// `codesign` read the argument as requirement *text*; without it the argument
/// is interpreted as a path to a requirements file and verification fails with
/// "No such file or directory" on a perfectly valid bundle.
#[cfg(target_os = "macos")]
const REQUIRED_CODE_REQUIREMENT: &str =
    "anchor apple generic and certificate leaf[subject.OU] = \"QMLVG482FY\"";

/// Install update for macOS .app bundle.
///
/// The archive is extracted into a **staging directory beside the live bundle**,
/// verified there, and only swapped into place once every gate has passed. The
/// previous behaviour extracted straight over the running bundle and verified
/// afterwards, so a failed signature check left a half-replaced application on
/// disk and returned an error — the user lost the working copy either way.
///
/// The staging directory is a sibling of the live bundle so the final `rename`
/// stays within one filesystem and is therefore atomic.
pub(crate) fn install_macos_bundle(
    current_exe: &std::path::Path,
    zip_data: &[u8],
) -> Result<PathBuf, String> {
    // Derive .app root: go up 3 levels from Contents/MacOS/par-term
    let app_root = current_exe
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .and_then(|p| p.parent()) // .app/
        .ok_or_else(|| "Could not determine .app bundle root".to_string())?;

    let install_dir = app_root.parent().ok_or_else(|| {
        "Could not determine the directory containing the .app bundle".to_string()
    })?;
    let app_name = app_root
        .file_name()
        .ok_or_else(|| "Could not determine the .app bundle name".to_string())?;

    let staging_dir = install_dir.join(format!(".par-term-update-{}", std::process::id()));
    // A previous run killed mid-update could have left this behind.
    let _ = std::fs::remove_dir_all(&staging_dir);
    std::fs::create_dir_all(&staging_dir).map_err(|e| {
        format!(
            "Failed to create staging directory {}: {}. \
             The update was not installed and the current version is untouched.",
            staging_dir.display(),
            e
        )
    })?;

    let staged_app = staging_dir.join(app_name);

    // Everything from here until the swap happens inside the staging directory,
    // so any failure leaves the live bundle exactly as it was.
    let staged =
        extract_bundle(&staged_app, zip_data).and_then(|()| verify_staged_bundle(&staged_app));

    if let Err(e) = staged {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(e);
    }

    swap_staged_bundle(&staged_app, app_root).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&staging_dir);
    })?;

    // The staging directory is empty once the staged bundle has been renamed out
    // of it; leaving it behind would only accumulate clutter.
    let _ = std::fs::remove_dir_all(&staging_dir);

    Ok(app_root.to_path_buf())
}

/// Extract the release archive into `staged_app`, inside the staging directory.
fn extract_bundle(staged_app: &std::path::Path, zip_data: &[u8]) -> Result<(), String> {
    use std::io::Cursor;
    use zip::ZipArchive;

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

        let final_path = staged_app.join(&relative_path);

        // Zip-slip protection: ensure the final path stays within the staged
        // bundle. A crafted zip could contain paths like
        // "../../etc/cron.d/malware" that escape the target after joining.
        // The anchor is the staged bundle rather than the live one — the live
        // bundle is not written to at all during extraction.
        if !final_path.starts_with(staged_app) {
            log::warn!(
                "Skipping zip entry outside the staged bundle: {} resolves to {}",
                relative_path.display(),
                final_path.display()
            );
            continue;
        }

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

    if !staged_app.exists() {
        return Err(format!(
            "The release archive did not produce a bundle at {}. \
             Update aborted; the current version is untouched.",
            staged_app.display()
        ));
    }

    Ok(())
}

/// Run the Gatekeeper gates against the **staged** bundle.
///
/// Both gates are fatal. `spctl` used to only warn, on the theory that ad-hoc
/// and development builds would trip it — but this code path only ever runs on
/// an artifact just downloaded from the project's GitHub releases, where an
/// unnotarized bundle is a reason to stop, not a reason to shrug. Because the
/// bundle being assessed is the staged copy, a rejection now costs the user
/// nothing: their working installation has not been touched.
#[cfg(target_os = "macos")]
fn verify_staged_bundle(staged_app: &std::path::Path) -> Result<(), String> {
    let path = staged_app.to_string_lossy().to_string();

    // Step 1: signature intact AND issued to par-term's Apple Team ID.
    let codesign_status = std::process::Command::new("/usr/bin/codesign")
        .args([
            "--verify",
            "--deep",
            "--strict",
            &format!("-R={}", REQUIRED_CODE_REQUIREMENT),
            &path,
        ])
        .output();

    match codesign_status {
        Ok(output) if output.status.success() => {
            log::info!("Code signature and Team ID verified for the staged update");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Update rejected: code signature verification failed.\n\
                 The downloaded update is not signed by par-term's Apple Developer \
                 ID, or the signature is damaged.\n\
                 Required: {}\n\
                 codesign output: {}\n\
                 Nothing was installed; your current version is untouched.",
                REQUIRED_CODE_REQUIREMENT,
                stderr.trim()
            ));
        }
        Err(e) => {
            return Err(format!(
                "Update rejected: failed to run codesign verification: {}.\n\
                 Cannot safely proceed without verifying the update's code signature.\n\
                 Nothing was installed; your current version is untouched.",
                e
            ));
        }
    }

    // Step 2: Gatekeeper assessment (notarization check). Fatal, including when
    // spctl itself cannot be run — an unverifiable update is not an installable
    // update.
    let spctl_status = std::process::Command::new("/usr/sbin/spctl")
        .args(["--assess", "--type", "execute", &path])
        .output();

    match spctl_status {
        Ok(output) if output.status.success() => {
            log::info!("Gatekeeper assessment passed for the staged update");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Update rejected: Gatekeeper assessment failed.\n\
                 The downloaded update is not notarized by Apple.\n\
                 spctl output: {}\n\
                 Nothing was installed; your current version is untouched.",
                stderr.trim()
            ));
        }
        Err(e) => {
            return Err(format!(
                "Update rejected: could not run the Gatekeeper assessment: {}.\n\
                 Nothing was installed; your current version is untouched.",
                e
            ));
        }
    }

    // Step 3: only now remove quarantine attributes, and only on the staged
    // copy — the live bundle never spends a moment in a half-verified state.
    let status = std::process::Command::new("xattr")
        .args(["-cr", &path])
        .status();
    match status {
        Ok(s) if s.success() => {
            log::info!("Removed quarantine attributes from the staged update");
        }
        Ok(s) => {
            log::warn!("xattr -cr exited with status {} on the staged update", s);
        }
        Err(e) => {
            log::warn!("Failed to run xattr -cr on the staged update: {}", e);
        }
    }

    Ok(())
}

/// Non-macOS builds have no Gatekeeper gates to run.
#[cfg(not(target_os = "macos"))]
fn verify_staged_bundle(_staged_app: &std::path::Path) -> Result<(), String> {
    Ok(())
}

/// Move the verified staged bundle over the live one.
///
/// Two renames within a single directory: the live bundle is moved aside, then
/// the staged bundle takes its place. If the second rename fails the first is
/// undone, so the outcome is always either the old bundle or the new one — never
/// a missing application.
fn swap_staged_bundle(
    staged_app: &std::path::Path,
    app_root: &std::path::Path,
) -> Result<(), String> {
    let backup = app_root.with_file_name(format!(
        "{}.old-{}",
        app_root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "par-term.app".to_string()),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&backup);

    let had_previous = app_root.exists();
    if had_previous {
        std::fs::rename(app_root, &backup).map_err(|e| {
            format!(
                "Failed to move the current application aside before installing the \
                 update: {}. Nothing was changed.",
                e
            )
        })?;
    }

    if let Err(e) = std::fs::rename(staged_app, app_root) {
        // Put the working installation back before reporting the failure.
        if had_previous {
            let _ = std::fs::rename(&backup, app_root);
        }
        return Err(format!(
            "Failed to move the verified update into place: {}. \
             Your previous version has been restored.",
            e
        ));
    }

    sync_parent_dir(app_root);

    // The swap already succeeded, so failing to delete the backup is untidy
    // rather than incorrect.
    if had_previous && let Err(e) = std::fs::remove_dir_all(&backup) {
        log::warn!(
            "Update installed, but the previous bundle at {} could not be removed: {}",
            backup.display(),
            e
        );
    }

    Ok(())
}

/// Best-effort fsync of a path's parent directory so a rename is durable.
#[cfg(unix)]
fn sync_parent_dir(path: &std::path::Path) {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && let Ok(dir) = std::fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &std::path::Path) {}

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
///
/// Stages the new binary as a sibling of the target, flushes it to disk, and
/// renames it into place. Any failure removes the staging file and leaves the
/// existing binary exactly as it was.
pub(crate) fn install_standalone(
    current_exe: &std::path::Path,
    data: &[u8],
) -> Result<PathBuf, String> {
    let new_path = current_exe.with_extension("new");

    if let Err(e) = stage_binary(&new_path, data) {
        let _ = std::fs::remove_file(&new_path);
        return Err(e);
    }

    if let Err(e) = replace_binary(&new_path, current_exe) {
        let _ = std::fs::remove_file(&new_path);
        return Err(e);
    }

    sync_parent_dir(current_exe);

    Ok(current_exe.to_path_buf())
}

/// Write the staged binary and flush it all the way to disk.
///
/// The `sync_all` matters here: without it a crash between rename and writeback
/// can leave the target name pointing at a zero-length file, which is an
/// unrunnable application rather than an old one.
fn stage_binary(new_path: &std::path::Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let mut file = std::fs::File::create(new_path)
        .map_err(|e| format!("Failed to create the staged binary: {}", e))?;
    file.write_all(data)
        .map_err(|e| format!("Failed to write new binary: {}", e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to flush the new binary to disk: {}", e))?;
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    Ok(())
}

/// Rename the staged binary over the live one.
#[cfg(unix)]
fn replace_binary(new_path: &std::path::Path, current_exe: &std::path::Path) -> Result<(), String> {
    // On Unix, rename is atomic if on the same filesystem.
    // A running binary's inode stays valid even after rename.
    std::fs::rename(new_path, current_exe).map_err(|e| {
        format!(
            "Failed to replace binary: {}. The existing binary was left in place.",
            e
        )
    })
}

/// Rename the staged binary over the live one.
#[cfg(windows)]
fn replace_binary(new_path: &std::path::Path, current_exe: &std::path::Path) -> Result<(), String> {
    // On Windows the running exe cannot be overwritten, so it is renamed to
    // `.old` first and removed on the next startup by `cleanup_old_binary`.
    let old_path = current_exe.with_extension("old");
    let _ = std::fs::remove_file(&old_path);

    std::fs::rename(current_exe, &old_path).map_err(|e| {
        format!(
            "Failed to rename current binary: {}. The existing binary was left in place.",
            e
        )
    })?;

    if let Err(e) = std::fs::rename(new_path, current_exe) {
        // Put the working binary back rather than leaving nothing at the path.
        let _ = std::fs::rename(&old_path, current_exe);
        return Err(format!(
            "Failed to rename new binary: {}. Your previous version has been restored.",
            e
        ));
    }

    Ok(())
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

    /// Build a directory that stands in for a `.app` bundle, holding one marker
    /// file so the swap can be observed.
    fn make_bundle(path: &std::path::Path, contents: &str) {
        std::fs::create_dir_all(path).expect("create bundle dir");
        std::fs::write(path.join("marker"), contents).expect("write marker");
    }

    fn sibling_names(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn swap_replaces_the_live_bundle_and_removes_the_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staged_app = dir.path().join(".staging").join("par-term.app");
        make_bundle(&staged_app, "NEW");
        let live = dir.path().join("par-term.app");
        make_bundle(&live, "OLD");

        swap_staged_bundle(&staged_app, &live).expect("swap");

        assert_eq!(
            std::fs::read_to_string(live.join("marker")).expect("read marker"),
            "NEW"
        );
        assert!(
            !sibling_names(dir.path())
                .iter()
                .any(|n| n.contains(".old-")),
            "the backup bundle should be removed once the swap succeeds: {:?}",
            sibling_names(dir.path())
        );
    }

    #[test]
    fn swap_works_when_there_is_no_previous_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let staged_app = dir.path().join(".staging").join("par-term.app");
        make_bundle(&staged_app, "NEW");
        let live = dir.path().join("par-term.app");

        swap_staged_bundle(&staged_app, &live).expect("swap with no previous bundle");

        assert_eq!(
            std::fs::read_to_string(live.join("marker")).expect("read marker"),
            "NEW"
        );
    }

    #[test]
    fn standalone_install_replaces_the_binary_and_leaves_no_staging_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe = dir.path().join("par-term");
        std::fs::write(&exe, b"OLD").expect("seed");

        install_standalone(&exe, b"NEW").expect("install");

        assert_eq!(std::fs::read(&exe).expect("read"), b"NEW");
        assert!(!exe.with_extension("new").exists());
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
