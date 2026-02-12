//! Helpers for restoring session state

use std::path::Path;

/// Validate a working directory path, falling back to $HOME if invalid
pub fn validate_cwd(cwd: &Option<String>) -> Option<String> {
    if let Some(dir) = cwd {
        if Path::new(dir).is_dir() {
            return Some(dir.clone());
        }
        log::warn!(
            "Session restore: directory '{}' no longer exists, falling back to home",
            dir
        );
    }
    // Fall back to home directory
    dirs::home_dir().map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cwd_existing_dir() {
        // /tmp should always exist
        let cwd = Some("/tmp".to_string());
        let result = validate_cwd(&cwd);
        assert_eq!(result, Some("/tmp".to_string()));
    }

    #[test]
    fn test_validate_cwd_missing_dir_falls_back_to_home() {
        let cwd = Some("/nonexistent/path/that/does/not/exist".to_string());
        let result = validate_cwd(&cwd);
        // Should fall back to home directory
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        assert_eq!(result, home);
    }

    #[test]
    fn test_validate_cwd_none_falls_back_to_home() {
        let result = validate_cwd(&None);
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        assert_eq!(result, home);
    }
}
