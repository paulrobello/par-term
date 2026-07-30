//! One-time best-effort migration of user data from a legacy config directory
//! into the canonical XDG location (`Config::config_dir()`).
//!
//! Before the config-dir fix, several storage sites resolved their path with
//! `dirs::config_dir().join("par-term")`. On macOS that is
//! `~/Library/Application Support/par-term/`, while the main config and all docs
//! use `Config::config_dir()` = `~/.config/par-term/`. The split could leave
//! profiles, command history, arrangements, session state, the dynamic-profile
//! cache, custom trigger sounds, and discovered ACP agent definitions in a
//! different directory than `config.yaml`.
//!
//! [`migrate_legacy_config_dir`] runs once at startup and moves any such legacy
//! entries into the canonical location. It is a no-op when the two locations
//! already coincide (plain Linux/Windows) and a no-op after the first successful
//! run (the legacy directory is then empty).

use std::path::{Path, PathBuf};

use par_term_config::Config;

/// Best-effort one-time migration of user data into the canonical config dir.
///
/// Computes the legacy location (`dirs::config_dir().join("par-term")`) and, when
/// it differs from `Config::config_dir()`, moves any entries present there but
/// absent in the canonical location. Existing canonical files are never
/// overwritten and failures never abort startup; see [`migrate_between`].
pub fn migrate_legacy_config_dir() {
    let canonical = Config::config_dir();
    for legacy in legacy_config_dirs(&canonical) {
        let moved = migrate_between(&legacy, &canonical);
        if moved > 0 {
            log::info!(
                "Migrated {moved} legacy config file(s)/dir(s) from {legacy:?} to {canonical:?}"
            );
        }
        // Clean up the now-empty legacy dir; silently ignored if entries remain.
        let _ = std::fs::remove_dir(&legacy);
    }
}

/// Locations that may hold config from an earlier par-term, excluding `canonical`.
///
/// Two roots can be stale, and which ones depends on the platform and on whether
/// `XDG_CONFIG_HOME` is set:
///
/// - `dirs::config_dir()/par-term` — on macOS this is
///   `~/Library/Application Support/par-term`, the pre-`adbf39f6` location. On Linux
///   `dirs` already resolves `XDG_CONFIG_HOME`, so it usually equals `canonical`.
/// - `~/.config/par-term` — the canonical location before par-term honoured
///   `XDG_CONFIG_HOME`. Stale only for users who set that variable to something else.
///
/// Entries equal to `canonical`, and duplicates, are dropped — so on a plain Linux
/// or Windows install this returns nothing and the migration is a no-op.
fn legacy_config_dirs(canonical: &Path) -> Vec<PathBuf> {
    let candidates = [
        dirs::config_dir().map(|dir| dir.join("par-term")),
        dirs::home_dir().map(|home| home.join(".config").join("par-term")),
    ];
    let mut out: Vec<PathBuf> = Vec::new();
    for candidate in candidates.into_iter().flatten() {
        if candidate != canonical && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// Move entries from `legacy` into `canonical` that are not already present there.
///
/// Returns the number of entries moved. Best-effort and non-panicking: a missing
/// or unreadable `legacy` returns `0`, existing `canonical` entries are preserved,
/// and per-entry rename failures (e.g. cross-device) are logged and skipped so the
/// source file is never lost.
fn migrate_between(legacy: &Path, canonical: &Path) -> usize {
    let entries = match std::fs::read_dir(legacy) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let _ = std::fs::create_dir_all(canonical);
    let mut moved = 0;
    for entry in entries.flatten() {
        let dest = canonical.join(entry.file_name());
        if dest.exists() {
            continue; // never clobber existing canonical files
        }
        match std::fs::rename(entry.path(), &dest) {
            Ok(()) => moved += 1,
            Err(e) => log::warn!(
                "Could not migrate legacy config entry {:?} -> {:?}: {e} (left in place)",
                entry.path(),
                dest,
            ),
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn canonical_is_never_a_migration_source() {
        let canonical = Config::config_dir();
        let legacy = legacy_config_dirs(&canonical);
        assert!(
            !legacy.contains(&canonical),
            "migrating a directory into itself would be a no-op at best: {legacy:?}"
        );
        let mut unique = legacy.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            legacy.len(),
            "duplicate legacy roots: {legacy:?}"
        );
    }

    /// The reason `legacy_config_dirs` returns two roots rather than one: a user who
    /// sets `XDG_CONFIG_HOME` moves `canonical` away from `~/.config/par-term`, and
    /// their existing config there must still be picked up.
    #[test]
    fn dot_config_is_a_legacy_source_when_canonical_moves_away() {
        let canonical = PathBuf::from("/an/xdg/root/par-term");
        let legacy = legacy_config_dirs(&canonical);
        if let Some(home) = dirs::home_dir() {
            assert!(
                legacy.contains(&home.join(".config").join("par-term")),
                "~/.config/par-term must be migrated when XDG_CONFIG_HOME points elsewhere: {legacy:?}"
            );
        }
    }

    #[test]
    fn moves_all_when_canonical_empty() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        fs::write(legacy.path().join("profiles.yaml"), "profiles").unwrap();
        fs::write(legacy.path().join("arrangements.yaml"), "arrangements").unwrap();
        fs::create_dir_all(legacy.path().join("sounds").join("sub")).unwrap();
        fs::write(legacy.path().join("sounds").join("a.wav"), "audio").unwrap();

        let moved = migrate_between(legacy.path(), canonical.path());

        assert_eq!(moved, 3); // profiles.yaml, arrangements.yaml, sounds/
        assert!(canonical.path().join("profiles.yaml").exists());
        assert!(canonical.path().join("arrangements.yaml").exists());
        assert!(canonical.path().join("sounds").join("a.wav").exists());
        assert!(canonical.path().join("sounds").join("sub").exists());
        // legacy is emptied of migrated entries
        assert!(legacy.path().read_dir().unwrap().next().is_none());
    }

    #[test]
    fn does_not_overwrite_existing_canonical() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        fs::write(legacy.path().join("profiles.yaml"), "stale-profiles").unwrap();
        fs::create_dir(legacy.path().join("sounds")).unwrap();
        // canonical already has profiles.yaml — must be preserved, not clobbered
        fs::write(canonical.path().join("profiles.yaml"), "current-profiles").unwrap();

        let moved = migrate_between(legacy.path(), canonical.path());

        assert_eq!(moved, 1); // only sounds/ moved; profiles.yaml skipped
        assert_eq!(
            fs::read_to_string(canonical.path().join("profiles.yaml")).unwrap(),
            "current-profiles",
        );
        assert!(canonical.path().join("sounds").exists());
        // the stale profiles.yaml is left in legacy (canonical-wins policy)
        assert!(legacy.path().join("profiles.yaml").exists());
        assert!(!legacy.path().join("sounds").exists());
    }

    #[test]
    fn missing_legacy_is_noop() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        let absent = legacy.path().join("does-not-exist");
        assert!(!absent.exists());
        assert_eq!(migrate_between(&absent, canonical.path()), 0);
    }

    #[test]
    fn empty_legacy_moves_nothing() {
        let legacy = tempdir().unwrap();
        let canonical = tempdir().unwrap();
        assert_eq!(migrate_between(legacy.path(), canonical.path()), 0);
        assert!(canonical.path().read_dir().unwrap().next().is_none());
    }

    #[test]
    fn creates_canonical_dir_if_missing() {
        let legacy = tempdir().unwrap();
        fs::write(legacy.path().join("arrangements.yaml"), "x").unwrap();
        let canonical = legacy.path().join("sibling").join("canonical");
        assert!(!canonical.exists());

        let moved = migrate_between(legacy.path(), &canonical);

        assert_eq!(moved, 1);
        assert!(canonical.join("arrangements.yaml").exists());
    }
}
