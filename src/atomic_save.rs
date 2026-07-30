//! Crash-safe atomic file writes for user data that cannot be reconstructed.
//!
//! Sessions, profiles and the dynamic-profile cache are written through
//! [`save_bytes_atomic`] and its wrappers. Each save serializes into a temporary
//! file **in the same directory** as the target, fsyncs it, then renames it over
//! the target. A crash or a full disk therefore leaves either the complete
//! previous file or the complete new one — never a truncated mix of the two.
//!
//! This matters more than the usual "corrupt file" argument because of the
//! recovery behaviour of the callers: a truncated session or profile file is
//! read back as *empty*, which every loader treats as "no saved data" rather
//! than as an error. The user loses the data with no diagnostic. Session save
//! also runs at shutdown, when an abrupt kill is most likely.
//!
//! # Permissions (SEC-021)
//!
//! On Unix the temporary file is created with mode `0o600` **before any bytes
//! are written**, so the contents are never briefly world-readable, and
//! `rename` carries that mode onto the target. Callers therefore do not need
//! their own `set_permissions` step.
//!
//! # Windows
//!
//! `std::fs::rename` maps to `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, so
//! replacing an existing target is supported — but it fails with a sharing
//! violation while another process holds the target open (antivirus and search
//! indexers do this routinely). The rename is retried with a short backoff; if
//! it still fails the temporary file is removed and the error is returned, so
//! the previous file survives and the failure is loud rather than lossy.

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes concurrent saves to the same target within one process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Build the temporary path used to stage a write to `path`.
///
/// Always a sibling of `path`: a cross-filesystem rename is a copy, which is
/// not atomic, so the staging file must live in the target's own directory.
fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "par-term-save".to_string());
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!("{file_name}.tmp.{}.{unique}", std::process::id()))
}

/// Write `bytes` into the staging file and flush them all the way to disk.
fn write_and_sync(temp_path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(temp_path)
        .with_context(|| format!("Failed to create temporary file {temp_path:?}"))?;

    // SEC-021: `mode()` above only applies when the open *creates* the file, so a
    // stale staging file left by a crashed process would keep its old mode. Set
    // it explicitly while the file is still empty — before the first byte is
    // written, never after.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to restrict permissions on {temp_path:?}"))?;
    }

    file.write_all(bytes)
        .with_context(|| format!("Failed to write temporary file {temp_path:?}"))?;
    file.sync_all()
        .with_context(|| format!("Failed to flush temporary file {temp_path:?} to disk"))?;
    Ok(())
}

#[cfg(not(windows))]
fn rename_into_place(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).with_context(|| format!("Failed to rename {from:?} to {to:?}"))
}

#[cfg(windows)]
fn rename_into_place(from: &Path, to: &Path) -> Result<()> {
    const ATTEMPTS: u32 = 5;

    let mut last_err = None;
    for attempt in 0..ATTEMPTS {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(
                        50 * u64::from(attempt + 1),
                    ));
                }
            }
        }
    }

    Err(last_err.expect("loop body runs at least once")).with_context(|| {
        format!(
            "Failed to rename {from:?} to {to:?} after {ATTEMPTS} attempts; \
             the target may be held open by another process"
        )
    })
}

/// Best-effort fsync of the parent directory so the rename itself is durable.
#[cfg(unix)]
fn sync_parent_dir(path: &Path) {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && let Ok(dir) = fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) {}

/// Atomically replace `path` with `bytes`.
///
/// Creates the parent directory if needed, stages the write in a sibling
/// temporary file (mode `0o600` on Unix), fsyncs it, and renames it over the
/// target. On any failure the staging file is removed and the previous
/// contents of `path` are left untouched.
pub fn save_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {parent:?}"))?;
    }

    let temp_path = temp_path_for(path);

    if let Err(e) = write_and_sync(&temp_path, bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(e.context(format!(
            "Failed to stage write for {path:?}; the previous file was left unchanged"
        )));
    }

    if let Err(e) = rename_into_place(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(e.context(format!(
            "Failed to replace {path:?}; the previous file was left unchanged"
        )));
    }

    sync_parent_dir(path);
    Ok(())
}

/// Atomically replace `path` with `contents`. See [`save_bytes_atomic`].
pub fn save_string_atomic(path: &Path, contents: &str) -> Result<()> {
    save_bytes_atomic(path, contents.as_bytes())
}

/// Serialize `value` as YAML and atomically replace `path` with it.
///
/// Serialization happens before the target is touched, so a serialization
/// failure cannot damage the existing file.
pub fn save_yaml_atomic<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let yaml = serde_yaml_ng::to_string(value)
        .with_context(|| format!("Failed to serialize data for {path:?}"))?;
    save_string_atomic(path, &yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A type whose `Serialize` impl always fails, to exercise the
    /// "save failed partway" path without needing a full disk.
    struct AlwaysFailsToSerialize;

    impl Serialize for AlwaysFailsToSerialize {
        fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom(
                "deliberate serialization failure",
            ))
        }
    }

    fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn temp_path_is_a_sibling_of_the_target() {
        let target = Path::new("/some/dir/profiles.yaml");
        let temp = temp_path_for(target);

        // Same directory, so the rename never crosses a filesystem boundary.
        assert_eq!(temp.parent(), target.parent());
        assert_ne!(temp, target);
        assert!(
            temp.file_name()
                .expect("temp has a file name")
                .to_string_lossy()
                .starts_with("profiles.yaml.tmp.")
        );
    }

    #[test]
    fn temp_paths_are_unique_within_a_process() {
        let target = Path::new("/some/dir/profiles.yaml");
        assert_ne!(temp_path_for(target), temp_path_for(target));
    }

    #[test]
    fn save_creates_parent_directory() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("nested").join("dir").join("data.yaml");

        save_string_atomic(&path, "hello").expect("save");
        assert_eq!(fs::read_to_string(&path).expect("read"), "hello");
    }

    #[test]
    fn save_replaces_existing_content_and_leaves_no_temp_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        save_string_atomic(&path, "first").expect("first save");
        save_string_atomic(&path, "second").expect("second save");

        assert_eq!(fs::read_to_string(&path).expect("read"), "second");
        assert_eq!(dir_entries(temp.path()), vec!["data.yaml".to_string()]);
    }

    #[test]
    fn failed_save_leaves_the_previous_file_intact() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        save_yaml_atomic(&path, &"good data".to_string()).expect("initial save");
        let before = fs::read_to_string(&path).expect("read before");

        let err =
            save_yaml_atomic(&path, &AlwaysFailsToSerialize).expect_err("serialization must fail");
        assert!(
            err.to_string().contains("Failed to serialize"),
            "unexpected error: {err:#}"
        );

        // The previous good content survives byte-for-byte, and no staging file
        // is left behind for a later save to trip over.
        assert_eq!(fs::read_to_string(&path).expect("read after"), before);
        assert_eq!(dir_entries(temp.path()), vec!["data.yaml".to_string()]);
    }

    #[test]
    fn stale_temp_file_does_not_break_a_later_save() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        // Simulate a crash that left a staging file behind.
        fs::write(temp.path().join("data.yaml.tmp.999999.0"), "garbage").expect("stale temp");

        save_string_atomic(&path, "fresh").expect("save over a stale temp");
        assert_eq!(fs::read_to_string(&path).expect("read"), "fresh");
    }

    #[test]
    fn yaml_roundtrip() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        let value = vec!["a".to_string(), "b".to_string()];
        save_yaml_atomic(&path, &value).expect("save");

        let loaded: Vec<String> =
            serde_yaml_ng::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
        assert_eq!(loaded, value);
    }

    #[cfg(unix)]
    #[test]
    fn completed_save_has_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        save_string_atomic(&path, "secret").expect("save");

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn save_tightens_a_world_readable_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        fs::write(&path, "old").expect("seed");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");

        save_string_atomic(&path, "new").expect("save");

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn staging_file_is_never_world_readable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let staging = temp.path().join("staged");

        write_and_sync(&staging, b"secret").expect("stage");

        let mode = fs::metadata(&staging)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected 0600 on the temp file, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn staging_reuses_a_stale_temp_file_but_re_restricts_it() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let staging = temp.path().join("staged");

        // A crashed process could leave a staging file with a permissive mode.
        fs::write(&staging, "stale").expect("seed");
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o666)).expect("chmod");

        write_and_sync(&staging, b"secret").expect("stage");

        let mode = fs::metadata(&staging)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
        assert_eq!(fs::read_to_string(&staging).expect("read"), "secret");
    }
}
