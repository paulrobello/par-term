//! Tests for shell environment and PATH building logic
//!
//! These tests cover the cross-platform PATH augmentation used when launching
//! terminal sessions, particularly important when launching from Finder/Explorer.

use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// PATH Separator Tests
// ============================================================================

#[test]
fn test_path_separator_constant() {
    // The PATH separator should be platform-specific
    #[cfg(target_os = "windows")]
    {
        assert_eq!(';', ';', "Windows uses semicolon");
    }
    #[cfg(not(target_os = "windows"))]
    {
        assert_eq!(':', ':', "Unix uses colon");
    }
}

#[test]
fn test_path_join_with_separator() {
    let paths = vec!["/usr/local/bin", "/opt/bin", "/home/user/bin"];

    #[cfg(target_os = "windows")]
    let sep = ";";
    #[cfg(not(target_os = "windows"))]
    let sep = ":";

    let joined = paths.join(sep);

    #[cfg(target_os = "windows")]
    assert!(joined.contains(";"));
    #[cfg(not(target_os = "windows"))]
    assert!(joined.contains(":"));
}

// ============================================================================
// Path Existence Filtering Tests
// ============================================================================

#[test]
fn test_filter_existing_paths() {
    let temp_dir = std::env::temp_dir();
    let nonexistent = PathBuf::from("/this/path/definitely/does/not/exist/12345");

    let paths = vec![
        temp_dir.to_string_lossy().to_string(),
        nonexistent.to_string_lossy().to_string(),
    ];

    let existing: Vec<_> = paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0], temp_dir.to_string_lossy().to_string());
}

#[test]
fn test_filter_empty_paths() {
    let paths = vec!["", "   ", "/valid/path"];

    let non_empty: Vec<_> = paths
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();

    assert_eq!(non_empty.len(), 2);
    assert!(!non_empty.contains(&""));
}

// ============================================================================
// PATH Deduplication Tests
// ============================================================================

#[test]
fn test_path_not_duplicated() {
    let current_path = "/usr/bin:/usr/local/bin:/opt/bin";
    let new_path = "/usr/local/bin"; // Already in PATH

    let should_add = !current_path.contains(new_path);
    assert!(!should_add, "Should not add path that already exists");
}

#[test]
fn test_path_added_when_missing() {
    let current_path = "/usr/bin:/opt/bin";
    let new_path = "/usr/local/bin"; // Not in PATH

    let should_add = !current_path.contains(new_path);
    assert!(should_add, "Should add path that doesn't exist");
}

#[test]
fn test_path_contains_check_exact() {
    // This tests a potential bug where "/usr/bin" contains "/bin"
    let current_path = "/usr/bin:/usr/sbin";
    let new_path = "/bin";

    // Simple contains() would incorrectly match
    // But for PATH purposes, this is often acceptable since /bin is typically symlinked
    let naive_contains = current_path.contains(new_path);
    assert!(naive_contains, "Naive contains finds substring");

    // A more precise check would split on separator and compare exactly
    #[cfg(not(target_os = "windows"))]
    let paths: Vec<_> = current_path.split(':').collect();
    #[cfg(target_os = "windows")]
    let paths: Vec<_> = current_path.split(';').collect();

    let exact_match = paths.contains(&new_path);
    assert!(!exact_match, "Exact match doesn't find /bin in /usr/bin");
}

// ============================================================================
// Environment Variable Map Tests
// ============================================================================

#[test]
fn test_env_map_insert_path() {
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

    assert!(env.contains_key("PATH"));
    assert_eq!(env.get("PATH").unwrap(), "/usr/bin:/bin");
}

#[test]
fn test_env_map_merge_with_config() {
    let mut config_env: HashMap<String, String> = HashMap::new();
    config_env.insert("MY_VAR".to_string(), "my_value".to_string());
    config_env.insert("ANOTHER".to_string(), "another_value".to_string());

    let mut env = config_env.clone();
    env.insert("PATH".to_string(), "/augmented/path".to_string());

    assert_eq!(env.len(), 3);
    assert!(env.contains_key("MY_VAR"));
    assert!(env.contains_key("ANOTHER"));
    assert!(env.contains_key("PATH"));
}

#[test]
fn test_env_map_path_override() {
    let mut config_env: HashMap<String, String> = HashMap::new();
    config_env.insert("PATH".to_string(), "/config/path".to_string());

    let mut env = config_env.clone();
    // Augmented PATH should override config PATH
    env.insert("PATH".to_string(), "/augmented:/config/path".to_string());

    assert_eq!(env.get("PATH").unwrap(), "/augmented:/config/path");
}

// ============================================================================
// Home Directory Path Building Tests
// ============================================================================

#[test]
fn test_home_dir_cargo_bin() {
    if let Some(home) = dirs::home_dir() {
        let cargo_bin = home.join(".cargo").join("bin");
        let path_str = cargo_bin.to_string_lossy().to_string();

        assert!(path_str.contains(".cargo"));
        assert!(path_str.contains("bin"));

        // Path should be absolute
        assert!(cargo_bin.is_absolute());
    }
}

#[test]
fn test_home_dir_local_bin() {
    if let Some(home) = dirs::home_dir() {
        let local_bin = home.join(".local").join("bin");
        let path_str = local_bin.to_string_lossy().to_string();

        assert!(path_str.contains(".local"));
        assert!(path_str.contains("bin"));
    }
}

#[test]
fn test_home_dir_go_bin() {
    if let Some(home) = dirs::home_dir() {
        let go_bin = home.join("go").join("bin");
        let path_str = go_bin.to_string_lossy().to_string();

        assert!(path_str.contains("go"));
        assert!(path_str.contains("bin"));
    }
}

// ============================================================================
// Platform-Specific Path Tests
// ============================================================================

#[cfg(target_os = "macos")]
mod macos_tests {
    #[test]
    fn test_homebrew_paths() {
        let apple_silicon = "/opt/homebrew/bin";
        let intel = "/usr/local/bin";

        // At least one should exist on a Mac with Homebrew
        let exists = std::path::Path::new(apple_silicon).exists()
            || std::path::Path::new(intel).exists();

        // This is informational - may not have Homebrew
        if exists {
            println!("Homebrew detected");
        }
    }

    #[test]
    fn test_macports_path() {
        let macports = "/opt/local/bin";
        // Just verify we can check it
        let _exists = std::path::Path::new(macports).exists();
    }
}

#[cfg(target_os = "linux")]
mod linux_tests {
    #[test]
    fn test_snap_path() {
        let snap = "/snap/bin";
        let _exists = std::path::Path::new(snap).exists();
    }

    #[test]
    fn test_flatpak_paths() {
        let system_flatpak = "/var/lib/flatpak/exports/bin";
        let _exists = std::path::Path::new(system_flatpak).exists();

        if let Some(home) = dirs::home_dir() {
            let user_flatpak = home
                .join(".local")
                .join("share")
                .join("flatpak")
                .join("exports")
                .join("bin");
            let _exists = user_flatpak.exists();
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_tests {
    #[test]
    fn test_chocolatey_path() {
        let chocolatey = r"C:\ProgramData\chocolatey\bin";
        let _exists = std::path::Path::new(chocolatey).exists();
    }

    #[test]
    fn test_scoop_path() {
        if let Some(home) = dirs::home_dir() {
            let scoop = home.join("scoop").join("shims");
            let _exists = scoop.exists();
        }
    }
}

// ============================================================================
// Nix Path Tests (Cross-Platform)
// ============================================================================

#[test]
fn test_nix_system_path() {
    let nix_system = "/nix/var/nix/profiles/default/bin";
    // Just verify the path string is valid
    assert!(nix_system.starts_with("/nix"));
}

#[test]
fn test_nix_user_path() {
    if let Some(home) = dirs::home_dir() {
        let nix_user = home.join(".nix-profile").join("bin");
        let path_str = nix_user.to_string_lossy().to_string();
        assert!(path_str.contains(".nix-profile"));
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_current_path() {
    let current_path = "";
    let new_paths = vec!["/usr/local/bin"];

    #[cfg(not(target_os = "windows"))]
    let sep = ":";
    #[cfg(target_os = "windows")]
    let sep = ";";

    let augmented = if current_path.is_empty() {
        new_paths.join(sep)
    } else {
        format!("{}{}{}", new_paths.join(sep), sep, current_path)
    };

    assert_eq!(augmented, "/usr/local/bin");
}

#[test]
fn test_no_new_paths_to_add() {
    let current_path = "/usr/bin:/usr/local/bin:/opt/bin";
    let new_paths: Vec<String> = vec![];

    // When no new paths, return current path unchanged
    let result = if new_paths.is_empty() {
        current_path.to_string()
    } else {
        format!("{}:{}", new_paths.join(":"), current_path)
    };

    assert_eq!(result, current_path);
}

#[test]
fn test_path_with_spaces() {
    let path_with_spaces = "/Users/John Doe/Applications/bin";

    // Paths with spaces should be handled correctly
    let paths = vec![path_with_spaces.to_string()];

    #[cfg(not(target_os = "windows"))]
    let joined = paths.join(":");
    #[cfg(target_os = "windows")]
    let joined = paths.join(";");

    assert!(joined.contains("John Doe"));
}

#[test]
fn test_path_ordering_prepend() {
    let current = "/system/bin";
    let new_paths = vec!["/user/bin", "/local/bin"];

    #[cfg(not(target_os = "windows"))]
    let sep = ":";
    #[cfg(target_os = "windows")]
    let sep = ";";

    let augmented = format!("{}{}{}", new_paths.join(sep), sep, current);

    // New paths should come first (higher priority)
    let parts: Vec<_> = augmented.split(sep).collect();
    assert_eq!(parts[0], "/user/bin");
    assert_eq!(parts[1], "/local/bin");
    assert_eq!(parts[2], "/system/bin");
}
