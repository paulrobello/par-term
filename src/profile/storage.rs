//! Storage utilities for profile persistence
//!
//! Profiles are stored in `~/.config/par-term/profiles.yaml`

use super::types::{Profile, ProfileManager};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the default profiles file path
pub fn profiles_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("par-term")
        .join("profiles.yaml")
}

/// Load profiles from the default location
pub fn load_profiles() -> Result<ProfileManager> {
    load_profiles_from(profiles_path())
}

/// Load profiles from a specific file
pub fn load_profiles_from(path: PathBuf) -> Result<ProfileManager> {
    crate::debug_info!("PROFILE", "Loading profiles from {:?}", path);
    if !path.exists() {
        crate::debug_info!("PROFILE", "No profiles file found at {:?}, starting with empty profiles", path);
        return Ok(ProfileManager::new());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read profiles from {:?}", path))?;

    crate::debug_info!("PROFILE", "Read {} bytes from profiles file", contents.len());

    if contents.trim().is_empty() {
        crate::debug_info!("PROFILE", "Profiles file is empty, starting with empty profiles");
        return Ok(ProfileManager::new());
    }

    let profiles: Vec<Profile> = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse profiles from {:?}", path))?;

    crate::debug_info!("PROFILE", "Parsed {} profiles from {:?}", profiles.len(), path);
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
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {:?}", parent))?;
    }

    let profiles = manager.to_vec();
    let contents = serde_yaml::to_string(&profiles)
        .context("Failed to serialize profiles")?;

    std::fs::write(&path, contents)
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
        let temp = tempdir().unwrap();
        let path = temp.path().join("nonexistent.yaml");

        let manager = load_profiles_from(path).unwrap();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_load_empty_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("empty.yaml");
        std::fs::write(&path, "").unwrap();

        let manager = load_profiles_from(path).unwrap();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempdir().unwrap();
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
                .command_args(vec!["user@server".to_string(), "-p".to_string(), "22".to_string()]),
        );

        save_profiles_to(&manager, path.clone()).unwrap();

        let loaded = load_profiles_from(path).unwrap();
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
            Some(vec!["user@server".to_string(), "-p".to_string(), "22".to_string()])
        );
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nested").join("dir").join("profiles.yaml");

        let manager = ProfileManager::new();
        save_profiles_to(&manager, path.clone()).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_load_corrupt_file_returns_error() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("corrupt.yaml");
        std::fs::write(&path, "not: valid: yaml: [[[").unwrap();

        let result = load_profiles_from(path);
        assert!(result.is_err());
    }
}
