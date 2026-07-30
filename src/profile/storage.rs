//! Storage utilities for profile persistence
//!
//! Profiles are stored in `~/.config/par-term/profiles.yaml`

use anyhow::{Context, Result};
use par_term_config::{Config, Profile, ProfileManager};
use std::path::PathBuf;

/// Get the default profiles file path
pub fn profiles_path() -> PathBuf {
    Config::config_dir().join("profiles.yaml")
}

/// Load profiles from the default location
pub fn load_profiles() -> Result<ProfileManager> {
    load_profiles_from(profiles_path())
}

/// Load profiles from a specific file
pub fn load_profiles_from(path: PathBuf) -> Result<ProfileManager> {
    crate::debug_info!("PROFILE", "Loading profiles from {:?}", path);
    if !path.exists() {
        crate::debug_info!(
            "PROFILE",
            "No profiles file found at {:?}, starting with empty profiles",
            path
        );
        return Ok(ProfileManager::new());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read profiles from {:?}", path))?;

    crate::debug_info!(
        "PROFILE",
        "Read {} bytes from profiles file",
        contents.len()
    );

    if contents.trim().is_empty() {
        crate::debug_info!(
            "PROFILE",
            "Profiles file is empty, starting with empty profiles"
        );
        return Ok(ProfileManager::new());
    }

    let profiles: Vec<Profile> = serde_yaml_ng::from_str(&contents)
        .with_context(|| format!("Failed to parse profiles from {:?}", path))?;

    crate::debug_info!(
        "PROFILE",
        "Parsed {} profiles from {:?}",
        profiles.len(),
        path
    );
    for p in &profiles {
        crate::debug_info!("PROFILE", "  - {}: {}", p.id, p.name);
    }
    Ok(ProfileManager::from_profiles(profiles))
}

/// Save profiles to the default location
pub fn save_profiles(manager: &ProfileManager) -> Result<()> {
    save_profiles_to(manager, profiles_path())
}

/// Save profiles to a specific file
pub fn save_profiles_to(manager: &ProfileManager, path: PathBuf) -> Result<()> {
    let profiles = manager.to_vec();

    // QA-010/SEC-021: profiles carry commands, ssh arguments and environment
    // overrides, and a truncated file reads back as "no profiles" rather than as
    // an error. Stage in a sibling temp file at mode 0600, fsync, then rename.
    crate::atomic_save::save_yaml_atomic(&path, &profiles)
        .with_context(|| format!("Failed to write profiles to {:?}", path))?;

    log::info!("Saved {} profiles to {:?}", profiles.len(), path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_nonexistent_file() {
        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("nonexistent.yaml");

        let manager = load_profiles_from(path)
            .expect("loading from nonexistent path should return empty manager");
        assert!(manager.is_empty());
    }

    #[test]
    fn test_load_empty_file() {
        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("empty.yaml");
        std::fs::write(&path, "").expect("failed to write empty file");

        let manager =
            load_profiles_from(path).expect("loading empty file should return empty manager");
        assert!(manager.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("profiles.yaml");

        let mut manager = ProfileManager::new();
        manager.add(
            Profile::new("Test Profile 1")
                .working_directory("/home/user")
                .command("bash")
                .tab_name("Test Tab")
                .icon("🔧"),
        );
        manager.add(
            Profile::new("Test Profile 2")
                .command("ssh")
                .command_args(vec![
                    "user@server".to_string(),
                    "-p".to_string(),
                    "22".to_string(),
                ]),
        );

        save_profiles_to(&manager, path.clone()).expect("failed to save profiles");

        let loaded = load_profiles_from(path).expect("failed to load saved profiles");
        assert_eq!(loaded.len(), 2);

        let profiles: Vec<_> = loaded.profiles_ordered().into_iter().collect();
        assert_eq!(profiles[0].name, "Test Profile 1");
        assert_eq!(profiles[0].working_directory.as_deref(), Some("/home/user"));
        assert_eq!(profiles[0].command.as_deref(), Some("bash"));
        assert_eq!(profiles[0].tab_name.as_deref(), Some("Test Tab"));
        assert_eq!(profiles[0].icon.as_deref(), Some("🔧"));

        assert_eq!(profiles[1].name, "Test Profile 2");
        assert_eq!(profiles[1].command.as_deref(), Some("ssh"));
        assert_eq!(
            profiles[1].command_args,
            Some(vec![
                "user@server".to_string(),
                "-p".to_string(),
                "22".to_string()
            ])
        );
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("nested").join("dir").join("profiles.yaml");

        let manager = ProfileManager::new();
        save_profiles_to(&manager, path.clone()).expect("failed to save profiles to nested dir");

        assert!(path.exists());
    }

    /// SEC-021: profiles carry commands and ssh arguments; the atomic-save
    /// helper must land them at mode 0600, not at the process umask default.
    #[cfg(unix)]
    #[test]
    fn test_saved_profiles_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("profiles.yaml");

        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Test Profile"));
        save_profiles_to(&manager, path.clone()).expect("failed to save profiles");

        let mode = std::fs::metadata(&path)
            .expect("failed to stat saved profiles")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    /// QA-010: a save that cannot complete must report the error and clean up its
    /// staging file, so the next save is not blocked by the debris. (That the
    /// previous *contents* survive is covered generically by
    /// `crate::atomic_save::tests::failed_save_leaves_the_previous_file_intact`.)
    #[test]
    fn test_failed_save_reports_error_and_leaves_no_staging_file() {
        let temp = tempdir().expect("failed to create temp dir");

        // A directory where the file belongs makes the rename fail after the
        // payload has already been written and fsynced.
        let path = temp.path().join("profiles.yaml");
        std::fs::create_dir(&path).expect("failed to create blocking directory");

        let manager = ProfileManager::new();
        assert!(save_profiles_to(&manager, path).is_err());

        let leftovers: Vec<String> = std::fs::read_dir(temp.path())
            .expect("failed to read temp dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "staging files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn test_load_corrupt_file_returns_error() {
        let temp = tempdir().expect("failed to create temp dir");
        let path = temp.path().join("corrupt.yaml");
        std::fs::write(&path, "not: valid: yaml: [[[").expect("failed to write corrupt file");

        let result = load_profiles_from(path);
        assert!(result.is_err());
    }
}
