//! Manifest system for tracking bundled files.
//!
//! Used by shader and shell integration installers to track which files
//! are part of the bundle vs user-created, and detect modifications.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Manifest tracking bundled files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Version of par-term that created this manifest
    pub version: String,
    /// ISO 8601 timestamp when manifest was generated
    pub generated: String,
    /// List of bundled files
    pub files: Vec<ManifestFile>,
}

/// A file entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Relative path from install directory
    pub path: String,
    /// SHA256 hash of file contents
    pub sha256: String,
    /// File type for categorization
    #[serde(rename = "type")]
    pub file_type: FileType,
    /// Optional category for organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Type of file in the manifest
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    /// Background shader (.glsl)
    Shader,
    /// Cursor effect shader
    CursorShader,
    /// Texture/image used by shaders
    Texture,
    /// Documentation file
    Doc,
    /// Other file type
    Other,
}

impl Manifest {
    /// Load manifest from a directory
    pub fn load(dir: &Path) -> Result<Self, String> {
        let manifest_path = dir.join("manifest.json");
        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse manifest: {}", e))
    }

    /// Save manifest to a directory
    ///
    /// Uses atomic write pattern: writes to a temp file first, then renames to final path.
    /// This ensures the manifest is never left in a corrupted state if writing fails.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let manifest_path = dir.join("manifest.json");
        let temp_path = dir.join("manifest.json.tmp");

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;

        // Write to temp file first
        fs::write(&temp_path, content)
            .map_err(|e| format!("Failed to write manifest temp file: {}", e))?;

        // Atomically rename to final path
        fs::rename(&temp_path, &manifest_path)
            .map_err(|e| format!("Failed to rename manifest temp file: {}", e))?;

        Ok(())
    }

    /// Build a lookup map from path to file entry
    pub fn file_map(&self) -> HashMap<&str, &ManifestFile> {
        self.files.iter().map(|f| (f.path.as_str(), f)).collect()
    }
}

/// Compute SHA256 hash of a file
pub fn compute_file_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Result of comparing a file against the manifest
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    /// File matches manifest (unchanged bundled file)
    Unchanged,
    /// File exists but hash differs from manifest (modified bundled file)
    Modified,
    /// File not in manifest (user-created)
    UserCreated,
    /// File in manifest but doesn't exist on disk
    Missing,
}

/// Check the status of a file against the manifest
pub fn check_file_status(file_path: &Path, relative_path: &str, manifest: &Manifest) -> FileStatus {
    let file_map = manifest.file_map();

    if let Some(manifest_entry) = file_map.get(relative_path) {
        if !file_path.exists() {
            FileStatus::Missing
        } else if let Ok(hash) = compute_file_hash(file_path) {
            if hash == manifest_entry.sha256 {
                FileStatus::Unchanged
            } else {
                FileStatus::Modified
            }
        } else {
            FileStatus::Modified // Can't read = treat as modified
        }
    } else if file_path.exists() {
        FileStatus::UserCreated
    } else {
        FileStatus::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_manifest() -> Manifest {
        Manifest {
            version: "0.2.0".to_string(),
            generated: "2026-02-02T12:00:00Z".to_string(),
            files: vec![
                ManifestFile {
                    path: "test.glsl".to_string(),
                    sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                        .to_string(), // SHA256 of empty file
                    file_type: FileType::Shader,
                    category: Some("test".to_string()),
                },
                ManifestFile {
                    path: "cursor_glow.glsl".to_string(),
                    sha256: "abc123".to_string(),
                    file_type: FileType::CursorShader,
                    category: None,
                },
            ],
        }
    }

    #[test]
    fn test_manifest_file_map() {
        let manifest = create_test_manifest();
        let map = manifest.file_map();

        assert_eq!(map.len(), 2);
        assert!(map.contains_key("test.glsl"));
        assert!(map.contains_key("cursor_glow.glsl"));
        assert!(!map.contains_key("nonexistent.glsl"));
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = create_test_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();

        assert!(json.contains("\"version\": \"0.2.0\""));
        assert!(json.contains("\"test.glsl\""));
        assert!(json.contains("\"shader\""));
        assert!(json.contains("\"cursor_shader\""));
    }

    #[test]
    fn test_manifest_deserialization() {
        let json = r#"{
            "version": "0.2.0",
            "generated": "2026-02-02T12:00:00Z",
            "files": [
                {
                    "path": "example.glsl",
                    "sha256": "abc123",
                    "type": "shader",
                    "category": "effects"
                }
            ]
        }"#;

        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.version, "0.2.0");
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "example.glsl");
        assert_eq!(manifest.files[0].file_type, FileType::Shader);
        assert_eq!(manifest.files[0].category, Some("effects".to_string()));
    }

    #[test]
    fn test_compute_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Create file with known content
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"hello world").unwrap();

        let hash = compute_file_hash(&test_file).unwrap();
        // SHA256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_file_hash_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.txt");

        fs::File::create(&test_file).unwrap();

        let hash = compute_file_hash(&test_file).unwrap();
        // SHA256 of empty file
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_file_status_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.glsl");

        // Create empty file (matches manifest hash)
        fs::File::create(&test_file).unwrap();

        let manifest = create_test_manifest();
        let status = check_file_status(&test_file, "test.glsl", &manifest);

        assert_eq!(status, FileStatus::Unchanged);
    }

    #[test]
    fn test_file_status_modified() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.glsl");

        // Create file with different content
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"modified content").unwrap();

        let manifest = create_test_manifest();
        let status = check_file_status(&test_file, "test.glsl", &manifest);

        assert_eq!(status, FileStatus::Modified);
    }

    #[test]
    fn test_file_status_missing() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("nonexistent.glsl");

        let manifest = create_test_manifest();
        let status = check_file_status(&test_file, "test.glsl", &manifest);

        assert_eq!(status, FileStatus::Missing);
    }

    #[test]
    fn test_file_status_user_created() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("user_shader.glsl");

        // Create file not in manifest
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"user shader content").unwrap();

        let manifest = create_test_manifest();
        let status = check_file_status(&test_file, "user_shader.glsl", &manifest);

        assert_eq!(status, FileStatus::UserCreated);
    }

    #[test]
    fn test_manifest_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manifest = create_test_manifest();

        // Save manifest
        manifest.save(temp_dir.path()).unwrap();

        // Verify file was created
        let manifest_path = temp_dir.path().join("manifest.json");
        assert!(manifest_path.exists());

        // Load and verify
        let loaded = Manifest::load(temp_dir.path()).unwrap();
        assert_eq!(loaded.version, manifest.version);
        assert_eq!(loaded.files.len(), manifest.files.len());
        assert_eq!(loaded.files[0].path, manifest.files[0].path);
    }

    #[test]
    fn test_file_type_serialization() {
        assert_eq!(
            serde_json::to_string(&FileType::Shader).unwrap(),
            "\"shader\""
        );
        assert_eq!(
            serde_json::to_string(&FileType::CursorShader).unwrap(),
            "\"cursor_shader\""
        );
        assert_eq!(
            serde_json::to_string(&FileType::Texture).unwrap(),
            "\"texture\""
        );
        assert_eq!(serde_json::to_string(&FileType::Doc).unwrap(), "\"doc\"");
        assert_eq!(
            serde_json::to_string(&FileType::Other).unwrap(),
            "\"other\""
        );
    }
}
