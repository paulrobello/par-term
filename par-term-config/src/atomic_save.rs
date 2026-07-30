//! Crash-safe atomic file writes for user data that cannot be reconstructed.
//!
//! Sessions, profiles, the dynamic-profile cache, `config.yaml`, the assistant
//! history and the user's shell rc files are written through
//! [`save_bytes_atomic`] and its wrappers. Each save serializes into a temporary
//! file **in the same directory** as the target, fsyncs it, then renames it over
//! the target. A crash or a full disk therefore leaves either the complete
//! previous file or the complete new one — never a truncated mix of the two.
//!
//! This matters more than the usual "corrupt file" argument because of the
//! recovery behaviour of the callers: a truncated session or profile file is
//! read back as *empty*, which every loader treats as "no saved data" rather
//! than as an error. The user loses the data with no diagnostic. Session save
//! also runs at shutdown, when an abrupt kill is most likely. For a shell rc
//! file the failure is worse still: the content is user-authored and
//! unreconstructable, and a truncated `.zshrc` breaks their login shell.
//!
//! # Crate placement
//!
//! This lives in `par-term-config` (Layer 1, no internal dependencies) rather
//! than in the root crate, because `par-term-config`, `par-term-settings-ui`
//! and `par-term-update` all need it and none of them can depend on the root
//! crate. The root crate re-exports it from `crate::atomic_save`.
//!
//! # Permissions (SEC-021)
//!
//! Two policies, chosen per call site:
//!
//! - [`save_bytes_atomic`] and its wrappers force mode `0o600`. Use these for
//!   par-term's own state in par-term's own directories (`config.yaml`,
//!   `state.yaml`, `arrangements.yaml`, `command_history.yaml`, the assistant
//!   history and prompt store, the bundle manifest). They may hold secrets, and
//!   nothing outside par-term reads them.
//!
//! - [`save_bytes_atomic_preserving_mode`] and its wrapper keep whatever mode
//!   the target already has. Use these for files that belong to the *user*
//!   rather than to par-term — shell rc files, GLSL shader sources, exports to
//!   a path chosen in a save dialog. Forcing `0o600` on those is wrong: an rc
//!   file may legitimately be group-readable, and a bundled `0o644` shader must
//!   not be silently tightened just because the user edited it.
//!
//! In both cases the staging file is created `0o600` **before any bytes are
//! written**, so contents are never briefly world-readable; the
//! mode-preserving variant widens it back to the target's mode only once the
//! payload is on disk. When the target does not exist there is no mode to
//! preserve and `0o600` is kept: an rc file, a shader and a snippet export are
//! all read only by the owning user, so `0o600` is never the unsafe direction.
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

/// What permissions the replaced file should end up with.
#[derive(Clone, Copy)]
enum ModePolicy {
    /// Always `0o600`: par-term's own state, which may hold secrets.
    Private,
    /// Keep the target's current mode; `0o600` when the target is new.
    PreserveTarget,
}

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

/// The mode to give the staged file just before it is renamed over `target`.
///
/// `None` means "leave it at the `0o600` it was staged with".
#[cfg(unix)]
fn final_mode(target: &Path, policy: ModePolicy) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;

    match policy {
        ModePolicy::Private => None,
        // `symlink_metadata` deliberately: if the target is a symlink the
        // rename replaces the link itself, so the link's own mode is what the
        // caller had, not the mode of whatever it pointed at.
        ModePolicy::PreserveTarget => fs::symlink_metadata(target)
            .ok()
            .map(|m| m.permissions().mode() & 0o7777)
            .filter(|mode| *mode != 0o600),
    }
}

/// Permissions are a Unix concept; on other platforms the staged file is
/// renamed as-is under either policy.
#[cfg(not(unix))]
fn final_mode(_target: &Path, policy: ModePolicy) -> Option<u32> {
    match policy {
        ModePolicy::Private | ModePolicy::PreserveTarget => None,
    }
}

/// Write `bytes` into the staging file and flush them all the way to disk.
///
/// `final_mode` is applied after the payload is written and before the caller
/// renames, so the file is never more permissive than `0o600` while it is being
/// filled in.
#[cfg_attr(not(unix), allow(unused_variables))]
fn write_and_sync(temp_path: &Path, bytes: &[u8], final_mode: Option<u32>) -> Result<()> {
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

    #[cfg(unix)]
    if let Some(mode) = final_mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .with_context(|| format!("Failed to restore permissions on {temp_path:?}"))?;
    }

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

fn save_bytes_with_policy(path: &Path, bytes: &[u8], policy: ModePolicy) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {parent:?}"))?;
    }

    let mode = final_mode(path, policy);
    let temp_path = temp_path_for(path);

    if let Err(e) = write_and_sync(&temp_path, bytes, mode) {
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

/// Atomically replace `path` with `bytes`, forcing mode `0o600`.
///
/// Creates the parent directory if needed, stages the write in a sibling
/// temporary file (mode `0o600` on Unix), fsyncs it, and renames it over the
/// target. On any failure the staging file is removed and the previous
/// contents of `path` are left untouched.
///
/// For files that belong to the user rather than to par-term, use
/// [`save_bytes_atomic_preserving_mode`] instead.
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created, if the staging
/// file cannot be written, fsynced or permission-restricted, or if the rename
/// fails.
pub fn save_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    save_bytes_with_policy(path, bytes, ModePolicy::Private)
}

/// Atomically replace `path` with `contents`, forcing mode `0o600`.
///
/// See [`save_bytes_atomic`].
///
/// # Errors
///
/// As [`save_bytes_atomic`].
pub fn save_string_atomic(path: &Path, contents: &str) -> Result<()> {
    save_bytes_atomic(path, contents.as_bytes())
}

/// Serialize `value` as YAML and atomically replace `path` with it, forcing
/// mode `0o600`.
///
/// Serialization happens before the target is touched, so a serialization
/// failure cannot damage the existing file.
///
/// # Errors
///
/// Returns an error if `value` cannot be serialized to YAML, plus everything
/// [`save_bytes_atomic`] can fail with.
pub fn save_yaml_atomic<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let yaml = serde_yaml_ng::to_string(value)
        .with_context(|| format!("Failed to serialize data for {path:?}"))?;
    save_string_atomic(path, &yaml)
}

/// Atomically replace `path` with `bytes`, keeping the target's current mode.
///
/// Identical to [`save_bytes_atomic`] except for permissions: the staged file
/// is still created `0o600`, but immediately before the rename it is set back
/// to whatever mode `path` already had. A target that does not exist yet is
/// created `0o600`.
///
/// Use this for files par-term does not own: the user's shell rc files, GLSL
/// shader sources, and exports written to a path chosen in a save dialog.
/// Forcing `0o600` on those would silently change permissions the user chose.
///
/// # Errors
///
/// As [`save_bytes_atomic`]. Reading the target's current mode is best-effort:
/// if it cannot be stat'ed the `0o600` staging mode is kept rather than failing
/// the save.
pub fn save_bytes_atomic_preserving_mode(path: &Path, bytes: &[u8]) -> Result<()> {
    save_bytes_with_policy(path, bytes, ModePolicy::PreserveTarget)
}

/// Atomically replace `path` with `contents`, keeping the target's current mode.
///
/// See [`save_bytes_atomic_preserving_mode`].
///
/// # Errors
///
/// As [`save_bytes_atomic_preserving_mode`].
pub fn save_string_atomic_preserving_mode(path: &Path, contents: &str) -> Result<()> {
    save_bytes_atomic_preserving_mode(path, contents.as_bytes())
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

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
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
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        save_string_atomic(&path, "secret").expect("save");

        assert_eq!(mode_of(&path), 0o600);
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

        assert_eq!(mode_of(&path), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn staging_file_is_never_world_readable() {
        let temp = tempdir().expect("tempdir");
        let staging = temp.path().join("staged");

        write_and_sync(&staging, b"secret", None).expect("stage");

        assert_eq!(mode_of(&staging), 0o600);
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

        write_and_sync(&staging, b"secret", None).expect("stage");

        assert_eq!(mode_of(&staging), 0o600);
        assert_eq!(fs::read_to_string(&staging).expect("read"), "secret");
    }

    // ---- mode-preserving variant -------------------------------------------

    #[cfg(unix)]
    #[test]
    fn preserving_save_keeps_a_group_readable_rc_file_group_readable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(".zshrc");

        fs::write(&path, "export PATH=/bin\n").expect("seed");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");

        save_string_atomic_preserving_mode(&path, "export PATH=/bin\nnew line\n").expect("save");

        assert_eq!(mode_of(&path), 0o644, "the user's own mode must survive");
        assert_eq!(
            fs::read_to_string(&path).expect("read"),
            "export PATH=/bin\nnew line\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn preserving_save_keeps_an_executable_bit() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("script.sh");

        fs::write(&path, "#!/bin/sh\n").expect("seed");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");

        save_string_atomic_preserving_mode(&path, "#!/bin/sh\necho hi\n").expect("save");

        assert_eq!(mode_of(&path), 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn preserving_save_creates_a_new_file_private() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("fresh.glsl");

        save_string_atomic_preserving_mode(&path, "void main() {}").expect("save");

        assert_eq!(
            mode_of(&path),
            0o600,
            "with no target to preserve, 0600 is the safe default"
        );
    }

    #[test]
    fn preserving_save_is_still_atomic_and_leaves_no_temp_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(".bashrc");

        save_string_atomic_preserving_mode(&path, "first").expect("first save");
        save_string_atomic_preserving_mode(&path, "second").expect("second save");

        assert_eq!(fs::read_to_string(&path).expect("read"), "second");
        assert_eq!(dir_entries(temp.path()), vec![".bashrc".to_string()]);
    }

    #[test]
    fn preserving_save_failure_leaves_the_previous_rc_file_intact() {
        let temp = tempdir().expect("tempdir");
        // A directory where the file belongs makes the rename fail after the
        // payload is already written and fsynced.
        let path = temp.path().join("blocked");
        fs::create_dir(&path).expect("blocking directory");

        let err = save_string_atomic_preserving_mode(&path, "payload")
            .expect_err("renaming over a directory must fail");
        assert!(
            format!("{err:#}").contains("left unchanged"),
            "unexpected error: {err:#}"
        );

        let leftovers: Vec<String> = dir_entries(temp.path())
            .into_iter()
            .filter(|n| n.contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "staging files left: {leftovers:?}");
    }
}
