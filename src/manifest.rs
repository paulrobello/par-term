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
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let manifest_path = dir.join("manifest.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        fs::write(&manifest_path, content).map_err(|e| format!("Failed to write manifest: {}", e))
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
