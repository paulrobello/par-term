# Integrations Install System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add shell integration and shader manifest systems with unified install/uninstall UI

**Architecture:** Modular installer modules (shell_integration_installer.rs, updated shader_installer.rs) with manifest-based tracking, unified integrations_ui.rs for welcome dialog, and new Integrations settings tab. Config tracks versions for "once per version" prompting.

**Tech Stack:** Rust, egui, serde_json (manifests), sha2 (hashing), include_str! (embedded scripts)

---

## Phase 1: Config Schema & Types

### Task 1: Add Integration Version Types

**Files:**
- Modify: `src/config/types.rs`

**Step 1: Add new types after ShaderInstallPrompt (around line 296)**

```rust
/// State of an integration's install prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallPromptState {
    /// Prompt user when appropriate (default)
    #[default]
    Ask,
    /// User said "never ask again"
    Never,
    /// Currently installed
    Installed,
}

impl InstallPromptState {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Never => "Never",
            Self::Installed => "Installed",
        }
    }
}

/// Tracks installed and prompted versions for integrations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationVersions {
    /// Version when shaders were installed
    pub shaders_installed_version: Option<String>,
    /// Version when user was last prompted about shaders
    pub shaders_prompted_version: Option<String>,
    /// Version when shell integration was installed
    pub shell_integration_installed_version: Option<String>,
    /// Version when user was last prompted about shell integration
    pub shell_integration_prompted_version: Option<String>,
}

/// Detected shell type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    Unknown,
}

impl ShellType {
    /// Detect shell from $SHELL environment variable
    pub fn detect() -> Self {
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                Self::Zsh
            } else if shell.contains("bash") {
                Self::Bash
            } else if shell.contains("fish") {
                Self::Fish
            } else {
                Self::Unknown
            }
        } else {
            Self::Unknown
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::Unknown => "Unknown",
        }
    }

    /// File extension for integration script
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Unknown => "sh",
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/config/types.rs
git commit -m "feat(config): add integration version tracking types"
```

---

### Task 2: Update Config Struct with Integration Fields

**Files:**
- Modify: `src/config/mod.rs`

**Step 1: Add re-exports in the pub use types block (around line 25)**

Add to the `pub use types::{...}` block:
```rust
    InstallPromptState, IntegrationVersions, ShellType,
```

**Step 2: Add new fields to Config struct (after shader_install_prompt, around line 998)**

```rust
    /// Shell integration install state
    #[serde(default)]
    pub shell_integration_state: InstallPromptState,

    /// Version tracking for integrations
    #[serde(default)]
    pub integration_versions: IntegrationVersions,
```

**Step 3: Update Default impl for Config (around line 1259)**

Add after `shader_install_prompt`:
```rust
            shell_integration_state: InstallPromptState::default(),
            integration_versions: IntegrationVersions::default(),
```

**Step 4: Add helper methods to Config impl**

Add these methods to the Config impl block:

```rust
    /// Get the shell integration directory
    pub fn shell_integration_dir() -> PathBuf {
        Self::config_dir()
    }

    /// Check if shell integration should be prompted
    pub fn should_prompt_shell_integration(&self) -> bool {
        if self.shell_integration_state != InstallPromptState::Ask {
            return false;
        }

        let current_version = env!("CARGO_PKG_VERSION");

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shell_integration_prompted_version {
            if prompted == current_version {
                return false;
            }
        }

        // Check if installed and up to date
        if let Some(ref installed) = self.integration_versions.shell_integration_installed_version {
            if installed == current_version {
                return false;
            }
        }

        true
    }

    /// Check if shaders should be prompted (updated version-aware logic)
    pub fn should_prompt_shader_install_versioned(&self) -> bool {
        if self.shader_install_prompt != ShaderInstallPrompt::Ask {
            return false;
        }

        let current_version = env!("CARGO_PKG_VERSION");

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shaders_prompted_version {
            if prompted == current_version {
                return false;
            }
        }

        // Check if installed and up to date
        if let Some(ref installed) = self.integration_versions.shaders_installed_version {
            if installed == current_version {
                return false;
            }
        }

        // Also check if shaders folder exists and has files
        let shaders_dir = Self::shaders_dir();
        !shaders_dir.exists() || !crate::shader_installer::has_shader_files(&shaders_dir)
    }

    /// Check if either integration should be prompted
    pub fn should_prompt_integrations(&self) -> bool {
        self.should_prompt_shader_install_versioned() || self.should_prompt_shell_integration()
    }
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add shell integration state and version tracking"
```

---

## Phase 2: Shader Manifest System

### Task 3: Create Manifest Types Module

**Files:**
- Create: `src/manifest.rs`

**Step 1: Write manifest types**

```rust
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
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse manifest: {}", e))
    }

    /// Save manifest to a directory
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let manifest_path = dir.join("manifest.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        fs::write(&manifest_path, content)
            .map_err(|e| format!("Failed to write manifest: {}", e))
    }

    /// Build a lookup map from path to file entry
    pub fn file_map(&self) -> HashMap<&str, &ManifestFile> {
        self.files.iter().map(|f| (f.path.as_str(), f)).collect()
    }
}

/// Compute SHA256 hash of a file
pub fn compute_file_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)
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
pub fn check_file_status(
    file_path: &Path,
    relative_path: &str,
    manifest: &Manifest,
) -> FileStatus {
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
    } else {
        if file_path.exists() {
            FileStatus::UserCreated
        } else {
            FileStatus::Missing
        }
    }
}
```

**Step 2: Add to lib.rs**

Add to `src/lib.rs`:
```rust
pub mod manifest;
```

**Step 3: Add sha2 dependency**

Run: `cargo add sha2`

**Step 4: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add src/manifest.rs src/lib.rs Cargo.toml Cargo.lock
git commit -m "feat: add manifest system for tracking bundled files"
```

---

### Task 4: Create Manifest Generation Script

**Files:**
- Create: `scripts/generate_manifest.py`

**Step 1: Write the script**

```python
#!/usr/bin/env python3
"""Generate manifest.json for shader bundle.

Usage:
    python scripts/generate_manifest.py shaders/

This scans the shaders directory and generates a manifest.json
with SHA256 hashes for all files.
"""

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path


def get_file_type(path: Path) -> str:
    """Determine file type from path."""
    name = path.name.lower()
    suffix = path.suffix.lower()

    if suffix == ".glsl":
        if name.startswith("cursor_"):
            return "cursor_shader"
        return "shader"
    elif suffix in (".png", ".jpg", ".jpeg", ".webp", ".gif"):
        return "texture"
    elif suffix in (".md", ".txt", ".rst"):
        return "doc"
    else:
        return "other"


def get_category(path: Path, file_type: str) -> str | None:
    """Determine category from file path/name."""
    name = path.stem.lower()

    if file_type == "cursor_shader":
        return "cursor"
    elif file_type == "texture":
        return "texture"

    # Categorize background shaders by name patterns
    retro_keywords = ["crt", "scanline", "vhs", "retro", "8bit", "pixel"]
    space_keywords = ["star", "galaxy", "nebula", "space", "cosmic"]
    nature_keywords = ["fire", "water", "cloud", "rain", "snow", "ocean", "wave"]
    abstract_keywords = ["plasma", "fractal", "noise", "pattern", "warp"]
    matrix_keywords = ["matrix", "digital", "cyber", "code", "rain"]

    for kw in retro_keywords:
        if kw in name:
            return "retro"
    for kw in space_keywords:
        if kw in name:
            return "space"
    for kw in nature_keywords:
        if kw in name:
            return "nature"
    for kw in matrix_keywords:
        if kw in name:
            return "matrix"
    for kw in abstract_keywords:
        if kw in name:
            return "abstract"

    return "effects"


def compute_sha256(path: Path) -> str:
    """Compute SHA256 hash of file."""
    hasher = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main():
    if len(sys.argv) < 2:
        print("Usage: python scripts/generate_manifest.py <shaders_dir>")
        sys.exit(1)

    shaders_dir = Path(sys.argv[1])
    if not shaders_dir.is_dir():
        print(f"Error: {shaders_dir} is not a directory")
        sys.exit(1)

    # Get version from Cargo.toml
    cargo_toml = Path("Cargo.toml")
    version = "0.0.0"
    if cargo_toml.exists():
        for line in cargo_toml.read_text().splitlines():
            if line.startswith("version = "):
                version = line.split('"')[1]
                break

    files = []
    for path in sorted(shaders_dir.rglob("*")):
        if path.is_file() and path.name != "manifest.json" and not path.name.startswith("."):
            relative = path.relative_to(shaders_dir)
            file_type = get_file_type(path)
            category = get_category(path, file_type)

            entry = {
                "path": str(relative),
                "sha256": compute_sha256(path),
                "type": file_type,
            }
            if category:
                entry["category"] = category

            files.append(entry)

    manifest = {
        "version": version,
        "generated": datetime.now(timezone.utc).isoformat(),
        "files": files,
    }

    output_path = shaders_dir / "manifest.json"
    with open(output_path, "w") as f:
        json.dump(manifest, f, indent=2)

    print(f"Generated {output_path} with {len(files)} files")


if __name__ == "__main__":
    main()
```

**Step 2: Make executable and test**

Run: `chmod +x scripts/generate_manifest.py && python scripts/generate_manifest.py shaders/`
Expected: "Generated shaders/manifest.json with NN files"

**Step 3: Verify manifest**

Run: `head -30 shaders/manifest.json`
Expected: Valid JSON with version, generated timestamp, and files array

**Step 4: Commit**

```bash
git add scripts/generate_manifest.py shaders/manifest.json
git commit -m "feat: add manifest generation script and initial shader manifest"
```

---

### Task 5: Update Shader Installer with Manifest Support

**Files:**
- Modify: `src/shader_installer.rs`

**Step 1: Add manifest-aware installation logic**

Replace entire file with updated version that supports manifests:

```rust
//! Shader installation logic with manifest support.
//!
//! Used by both the CLI (`install-shaders` command) and the UI (shader install dialog).

use crate::config::Config;
use crate::manifest::{self, FileStatus, Manifest};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Result of shader installation
#[derive(Debug, Default)]
pub struct InstallResult {
    /// Number of files installed
    pub installed: usize,
    /// Number of files skipped (unchanged)
    pub skipped: usize,
    /// Files that need user confirmation (modified bundled files)
    pub needs_confirmation: Vec<String>,
    /// Files that were removed (no longer in bundle)
    pub removed: usize,
}

/// Result of shader uninstallation
#[derive(Debug, Default)]
pub struct UninstallResult {
    /// Number of files removed
    pub removed: usize,
    /// Number of files kept (user-created or modified)
    pub kept: usize,
    /// Files that need user confirmation
    pub needs_confirmation: Vec<String>,
}

/// Install shaders from GitHub release
/// Returns the number of shaders installed
pub fn install_shaders() -> Result<usize, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Fetch latest release info
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let download_url = get_shaders_download_url(&api_url, REPO)?;

    // Download the zip file
    let zip_data = download_file(&download_url)?;

    // Create shaders directory if it doesn't exist
    fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    // Extract shaders
    extract_shaders(&zip_data, &shaders_dir)?;

    // Count installed shaders
    let count = count_shader_files(&shaders_dir);

    Ok(count)
}

/// Install shaders with manifest awareness
/// Returns detailed result with counts and files needing confirmation
pub fn install_shaders_with_manifest(
    force_overwrite: bool,
) -> Result<InstallResult, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Load existing manifest if present
    let old_manifest = Manifest::load(&shaders_dir).ok();

    // Download and extract to temp location first
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let download_url = get_shaders_download_url(&api_url, REPO)?;
    let zip_data = download_file(&download_url)?;

    // Parse the new manifest from the zip
    let new_manifest = extract_manifest_from_zip(&zip_data)?;

    // Create shaders directory
    fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    let mut result = InstallResult::default();
    let new_file_map = new_manifest.file_map();

    // Check each file in new manifest
    for file_entry in &new_manifest.files {
        let file_path = shaders_dir.join(&file_entry.path);
        let status = manifest::check_file_status(&file_path, &file_entry.path, &new_manifest);

        match status {
            FileStatus::Missing => {
                // New file, install it
                result.installed += 1;
            }
            FileStatus::Unchanged => {
                // Same as bundle, skip
                result.skipped += 1;
            }
            FileStatus::Modified if !force_overwrite => {
                // Modified by user, needs confirmation
                result.needs_confirmation.push(file_entry.path.clone());
            }
            FileStatus::Modified => {
                // Force overwrite
                result.installed += 1;
            }
            FileStatus::UserCreated => {
                // This shouldn't happen for files in manifest
                result.skipped += 1;
            }
        }
    }

    // Check for files in old manifest but not in new (removed from bundle)
    if let Some(ref old) = old_manifest {
        for old_file in &old.files {
            if !new_file_map.contains_key(old_file.path.as_str()) {
                let file_path = shaders_dir.join(&old_file.path);
                if file_path.exists() {
                    let status = manifest::check_file_status(&file_path, &old_file.path, old);
                    match status {
                        FileStatus::Unchanged => {
                            // Unmodified, safe to remove
                            fs::remove_file(&file_path).ok();
                            result.removed += 1;
                        }
                        FileStatus::Modified if !force_overwrite => {
                            // Modified, needs confirmation
                            result.needs_confirmation.push(old_file.path.clone());
                        }
                        FileStatus::Modified => {
                            fs::remove_file(&file_path).ok();
                            result.removed += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // If no confirmations needed or force, extract all
    if result.needs_confirmation.is_empty() || force_overwrite {
        extract_shaders(&zip_data, &shaders_dir)?;
    }

    Ok(result)
}

/// Uninstall bundled shaders while preserving user files
pub fn uninstall_shaders(force: bool) -> Result<UninstallResult, String> {
    let shaders_dir = Config::shaders_dir();

    if !shaders_dir.exists() {
        return Ok(UninstallResult::default());
    }

    let manifest = Manifest::load(&shaders_dir)
        .map_err(|_| "No manifest found - cannot safely uninstall".to_string())?;

    let mut result = UninstallResult::default();

    for file_entry in &manifest.files {
        let file_path = shaders_dir.join(&file_entry.path);
        if !file_path.exists() {
            continue;
        }

        let status = manifest::check_file_status(&file_path, &file_entry.path, &manifest);

        match status {
            FileStatus::Unchanged => {
                // Unmodified bundled file, safe to remove
                fs::remove_file(&file_path).ok();
                result.removed += 1;
            }
            FileStatus::Modified if !force => {
                // Modified, needs confirmation
                result.needs_confirmation.push(file_entry.path.clone());
            }
            FileStatus::Modified => {
                // Force remove
                fs::remove_file(&file_path).ok();
                result.removed += 1;
            }
            FileStatus::UserCreated | FileStatus::Missing => {
                result.kept += 1;
            }
        }
    }

    // Remove manifest if all bundled files were removed
    if result.needs_confirmation.is_empty() {
        let manifest_path = shaders_dir.join("manifest.json");
        fs::remove_file(manifest_path).ok();

        // Remove empty subdirectories
        remove_empty_dirs(&shaders_dir);
    }

    Ok(result)
}

/// Extract manifest.json from zip data
fn extract_manifest_from_zip(zip_data: &[u8]) -> Result<Manifest, String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        if let Some(name) = file.enclosed_name() {
            if name.ends_with("manifest.json") {
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .map_err(|e| format!("Failed to read manifest: {}", e))?;
                return serde_json::from_str(&content)
                    .map_err(|e| format!("Failed to parse manifest: {}", e));
            }
        }
    }

    Err("No manifest.json found in archive".to_string())
}

/// Remove empty directories recursively
fn remove_empty_dirs(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_empty_dirs(&path);
                // Try to remove if empty
                fs::remove_dir(&path).ok();
            }
        }
    }
}

/// Get the download URL for shaders.zip from the latest release
pub fn get_shaders_download_url(api_url: &str, repo: &str) -> Result<String, String> {
    let mut body = ureq::get(api_url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    let search_pattern = "\"browser_download_url\":\"";
    let target_file = "shaders.zip";

    for (i, _) in body_str.match_indices(search_pattern) {
        let url_start = i + search_pattern.len();
        if let Some(url_end) = body_str[url_start..].find('"') {
            let url = &body_str[url_start..url_start + url_end];
            if url.ends_with(target_file) {
                return Ok(url.to_string());
            }
        }
    }

    Err(format!(
        "Could not find shaders.zip in the latest release.\n\
         Please check https://github.com/{}/releases",
        repo
    ))
}

/// Download a file from URL and return its contents
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let mut body = ureq::get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body();

    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download: {}", e))?;

    Ok(bytes)
}

/// Extract shaders from zip data to target directory
pub fn extract_shaders(zip_data: &[u8], target_dir: &Path) -> Result<(), String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    Ok(())
}

/// Count .glsl files in directory
pub fn count_shader_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                count += 1;
            }
        }
    }
    count
}

/// Count all files tracked by manifest
pub fn count_manifest_files(dir: &Path) -> usize {
    if let Ok(manifest) = Manifest::load(dir) {
        manifest.files.len()
    } else {
        0
    }
}

/// Check if directory contains any .glsl files
pub fn has_shader_files(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                return true;
            }
        }
    }
    false
}

/// Get installed shader version from manifest
pub fn get_installed_version(dir: &Path) -> Option<String> {
    Manifest::load(dir).ok().map(|m| m.version)
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/shader_installer.rs
git commit -m "feat(shader_installer): add manifest-aware install/uninstall"
```

---

## Phase 3: Shell Integration Scripts

### Task 6: Create Shell Integration Directory and Scripts

**Files:**
- Create: `shell_integration/par_term_shell_integration.bash`
- Create: `shell_integration/par_term_shell_integration.zsh`
- Create: `shell_integration/par_term_shell_integration.fish`
- Create: `shell_integration/README.md`

**Step 1: Copy and rebrand bash script**

Copy from `/Users/probello/Repos/par-term-emu-core-rust/shell_integration/par_term_emu_core_rust_shell_integration.bash` to `shell_integration/par_term_shell_integration.bash` and replace all instances of:
- `par_term_emu_core_rust` → `par_term`
- `par-term-emu-core-rust` → `par-term`
- `PAR_TERM_EMU_` → `PAR_TERM_`

**Step 2: Copy and rebrand zsh script**

Copy and rebrand similarly for zsh.

**Step 3: Copy and rebrand fish script**

Copy and rebrand similarly for fish.

**Step 4: Create README.md**

```markdown
# par-term Shell Integration

Shell integration provides enhanced terminal features by embedding semantic markers
in your shell's prompt and command output.

## Features

- **Prompt Navigation**: Jump between command prompts
- **Command Status Tracking**: Visual indicators for successful/failed commands
- **Working Directory Tracking**: Terminal knows current directory
- **Command Duration**: Measure how long commands take

## Installation

### From par-term

Open Settings (F12) → Integrations → Install Shell Integration

### Manual (curl)

```bash
curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash
```

### Manual (copy)

Copy the appropriate script to your config directory and source it:

```bash
# Bash
mkdir -p ~/.config/par-term
cp par_term_shell_integration.bash ~/.config/par-term/shell_integration.bash
echo 'source ~/.config/par-term/shell_integration.bash' >> ~/.bashrc

# Zsh
mkdir -p ~/.config/par-term
cp par_term_shell_integration.zsh ~/.config/par-term/shell_integration.zsh
echo 'source ~/.config/par-term/shell_integration.zsh' >> ~/.zshrc

# Fish
mkdir -p ~/.config/par-term
cp par_term_shell_integration.fish ~/.config/par-term/shell_integration.fish
echo 'source ~/.config/par-term/shell_integration.fish' >> ~/.config/fish/config.fish
```

## Technical Details

Uses OSC 133 protocol (also used by iTerm2, VSCode, WezTerm).
```

**Step 5: Commit**

```bash
git add shell_integration/
git commit -m "feat: add rebranded shell integration scripts"
```

---

### Task 7: Create Shell Integration Installer Module

**Files:**
- Create: `src/shell_integration_installer.rs`

**Step 1: Write the installer module**

```rust
//! Shell integration installation logic.
//!
//! Handles installing, updating, and uninstalling shell integration scripts
//! for bash, zsh, and fish shells.

use crate::config::{Config, ShellType};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

// Embedded shell integration scripts
const BASH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.bash");
const ZSH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.zsh");
const FISH_SCRIPT: &str = include_str!("../shell_integration/par_term_shell_integration.fish");

// Marker comments for RC files
const MARKER_START: &str = "# >>> par-term shell integration >>>";
const MARKER_END: &str = "# <<< par-term shell integration <<<";

/// Result of shell integration installation
#[derive(Debug)]
pub struct InstallResult {
    /// Shell type that was configured
    pub shell: ShellType,
    /// Path to installed script
    pub script_path: PathBuf,
    /// Path to RC file that was modified
    pub rc_file: PathBuf,
    /// Whether shell restart is needed
    pub needs_restart: bool,
}

/// Result of shell integration uninstallation
#[derive(Debug, Default)]
pub struct UninstallResult {
    /// RC files that were cleaned
    pub cleaned: Vec<PathBuf>,
    /// RC files that need manual cleanup
    pub needs_manual: Vec<PathBuf>,
    /// Script files that were removed
    pub scripts_removed: Vec<PathBuf>,
}

/// Install shell integration for the detected or specified shell
pub fn install(shell: Option<ShellType>) -> Result<InstallResult, String> {
    let shell = shell.unwrap_or_else(ShellType::detect);

    if shell == ShellType::Unknown {
        return Err("Could not detect shell type. Please specify --shell bash|zsh|fish".into());
    }

    let config_dir = Config::shell_integration_dir();
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    // Write the integration script
    let script_name = format!("shell_integration.{}", shell.extension());
    let script_path = config_dir.join(&script_name);
    let script_content = get_script_content(shell);

    fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to write integration script: {}", e))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)
            .map_err(|e| format!("Failed to get permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Add to RC file
    let rc_file = get_rc_file(shell)?;
    add_to_rc_file(&rc_file, shell)?;

    Ok(InstallResult {
        shell,
        script_path,
        rc_file,
        needs_restart: true,
    })
}

/// Uninstall shell integration
pub fn uninstall() -> Result<UninstallResult, String> {
    let config_dir = Config::shell_integration_dir();
    let mut result = UninstallResult::default();

    // Remove script files
    for ext in &["bash", "zsh", "fish"] {
        let script_path = config_dir.join(format!("shell_integration.{}", ext));
        if script_path.exists() {
            if fs::remove_file(&script_path).is_ok() {
                result.scripts_removed.push(script_path);
            }
        }
    }

    // Clean RC files
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let rc_files = vec![
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".zshrc"),
        home.join(".config/fish/config.fish"),
    ];

    for rc_file in rc_files {
        if rc_file.exists() {
            match remove_from_rc_file(&rc_file) {
                Ok(true) => result.cleaned.push(rc_file),
                Ok(false) => {} // No changes needed
                Err(_) => result.needs_manual.push(rc_file),
            }
        }
    }

    Ok(result)
}

/// Check if shell integration is installed
pub fn is_installed() -> bool {
    let config_dir = Config::shell_integration_dir();
    let shell = ShellType::detect();

    if shell == ShellType::Unknown {
        return false;
    }

    let script_path = config_dir.join(format!("shell_integration.{}", shell.extension()));
    script_path.exists()
}

/// Get the detected shell type
pub fn detected_shell() -> ShellType {
    ShellType::detect()
}

fn get_script_content(shell: ShellType) -> &'static str {
    match shell {
        ShellType::Bash => BASH_SCRIPT,
        ShellType::Zsh => ZSH_SCRIPT,
        ShellType::Fish => FISH_SCRIPT,
        ShellType::Unknown => "",
    }
}

fn get_rc_file(shell: ShellType) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;

    match shell {
        ShellType::Bash => {
            // Prefer .bashrc, fall back to .bash_profile
            let bashrc = home.join(".bashrc");
            if bashrc.exists() {
                Ok(bashrc)
            } else {
                Ok(home.join(".bash_profile"))
            }
        }
        ShellType::Zsh => {
            // Check ZDOTDIR first
            if let Ok(zdotdir) = std::env::var("ZDOTDIR") {
                Ok(PathBuf::from(zdotdir).join(".zshrc"))
            } else {
                Ok(home.join(".zshrc"))
            }
        }
        ShellType::Fish => Ok(home.join(".config/fish/config.fish")),
        ShellType::Unknown => Err("Unknown shell type".into()),
    }
}

fn add_to_rc_file(rc_file: &Path, shell: ShellType) -> Result<(), String> {
    // Check if already installed
    if let Ok(content) = fs::read_to_string(rc_file) {
        if content.contains(MARKER_START) {
            // Already installed, update it
            return update_rc_file(rc_file, shell);
        }
    }

    // Create RC file if it doesn't exist
    if !rc_file.exists() {
        if let Some(parent) = rc_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    // Append the source block
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_file)
        .map_err(|e| format!("Failed to open RC file: {}", e))?;

    let source_block = generate_source_block(shell);
    writeln!(file, "\n{}", source_block)
        .map_err(|e| format!("Failed to write to RC file: {}", e))?;

    Ok(())
}

fn update_rc_file(rc_file: &Path, shell: ShellType) -> Result<(), String> {
    let content = fs::read_to_string(rc_file)
        .map_err(|e| format!("Failed to read RC file: {}", e))?;

    let new_content = replace_marker_block(&content, shell);

    fs::write(rc_file, new_content)
        .map_err(|e| format!("Failed to write RC file: {}", e))?;

    Ok(())
}

fn remove_from_rc_file(rc_file: &Path) -> Result<bool, String> {
    let content = fs::read_to_string(rc_file)
        .map_err(|e| format!("Failed to read RC file: {}", e))?;

    if !content.contains(MARKER_START) {
        return Ok(false); // Nothing to remove
    }

    // Check if markers are intact
    if !content.contains(MARKER_END) {
        return Err("Marker block was modified".into());
    }

    let new_content = remove_marker_block(&content);

    fs::write(rc_file, new_content)
        .map_err(|e| format!("Failed to write RC file: {}", e))?;

    Ok(true)
}

fn generate_source_block(shell: ShellType) -> String {
    let config_path = "${XDG_CONFIG_HOME:-$HOME/.config}/par-term";

    match shell {
        ShellType::Bash => format!(
            r#"{MARKER_START}
if [ -f "{config_path}/shell_integration.bash" ]; then
  source "{config_path}/shell_integration.bash"
fi
{MARKER_END}"#
        ),
        ShellType::Zsh => format!(
            r#"{MARKER_START}
if [ -f "{config_path}/shell_integration.zsh" ]; then
  source "{config_path}/shell_integration.zsh"
fi
{MARKER_END}"#
        ),
        ShellType::Fish => format!(
            r#"{MARKER_START}
set -l __par_term_config_dir (begin
    if set -q XDG_CONFIG_HOME
        echo $XDG_CONFIG_HOME
    else
        echo $HOME/.config
    end
end)/par-term
if test -f "$__par_term_config_dir/shell_integration.fish"
    source "$__par_term_config_dir/shell_integration.fish"
end
{MARKER_END}"#
        ),
        ShellType::Unknown => String::new(),
    }
}

fn replace_marker_block(content: &str, shell: ShellType) -> String {
    let new_block = generate_source_block(shell);

    if let (Some(start), Some(end)) = (content.find(MARKER_START), content.find(MARKER_END)) {
        let end = end + MARKER_END.len();
        let mut result = content[..start].to_string();
        result.push_str(&new_block);
        result.push_str(&content[end..]);
        result
    } else {
        content.to_string()
    }
}

fn remove_marker_block(content: &str) -> String {
    if let (Some(start), Some(end)) = (content.find(MARKER_START), content.find(MARKER_END)) {
        let end = end + MARKER_END.len();
        // Also remove trailing newline if present
        let end = if content[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        // Also remove leading newline if present
        let start = if start > 0 && content[..start].ends_with('\n') {
            start - 1
        } else {
            start
        };

        let mut result = content[..start].to_string();
        result.push_str(&content[end..]);
        result
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_source_block_bash() {
        let block = generate_source_block(ShellType::Bash);
        assert!(block.contains(MARKER_START));
        assert!(block.contains(MARKER_END));
        assert!(block.contains("shell_integration.bash"));
    }

    #[test]
    fn test_remove_marker_block() {
        let content = "before\n# >>> par-term shell integration >>>\nstuff\n# <<< par-term shell integration <<<\nafter";
        let result = remove_marker_block(content);
        assert_eq!(result, "before\nafter");
    }
}
```

**Step 2: Add to lib.rs**

Add to `src/lib.rs`:
```rust
pub mod shell_integration_installer;
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/shell_integration_installer.rs src/lib.rs
git commit -m "feat: add shell integration installer module"
```

---

## Phase 4: CLI Commands

### Task 8: Add Shell Integration CLI Commands

**Files:**
- Modify: `src/cli.rs`

**Step 1: Add new commands to Commands enum**

Add after `InstallShaders`:

```rust
    /// Install shell integration for your shell
    InstallShellIntegration {
        /// Specify shell type (auto-detected if not provided)
        #[arg(long, value_enum)]
        shell: Option<ShellTypeArg>,
    },

    /// Uninstall shell integration
    UninstallShellIntegration,

    /// Uninstall shaders (removes bundled files, keeps user files)
    UninstallShaders {
        /// Force removal without prompting
        #[arg(short, long)]
        force: bool,
    },

    /// Install both shaders and shell integration
    InstallIntegrations {
        /// Skip confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
    },
```

**Step 2: Add ShellTypeArg enum for CLI**

Add near the top of the file:

```rust
use crate::config::ShellType;

/// Shell type argument for CLI
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShellTypeArg {
    Bash,
    Zsh,
    Fish,
}

impl From<ShellTypeArg> for ShellType {
    fn from(arg: ShellTypeArg) -> Self {
        match arg {
            ShellTypeArg::Bash => ShellType::Bash,
            ShellTypeArg::Zsh => ShellType::Zsh,
            ShellTypeArg::Fish => ShellType::Fish,
        }
    }
}
```

**Step 3: Add command handlers in process_cli**

Update the match in `process_cli`:

```rust
        Some(Commands::InstallShellIntegration { shell }) => {
            let result = install_shell_integration_cli(shell.map(Into::into));
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallShellIntegration) => {
            let result = uninstall_shell_integration_cli();
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::UninstallShaders { force }) => {
            let result = uninstall_shaders_cli(force);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
        Some(Commands::InstallIntegrations { yes }) => {
            let result = install_integrations_cli(yes);
            CliResult::Exit(if result.is_ok() { 0 } else { 1 })
        }
```

**Step 4: Add CLI handler functions**

Add these functions:

```rust
use crate::shell_integration_installer;

fn install_shell_integration_cli(shell: Option<ShellType>) -> anyhow::Result<()> {
    println!("=============================================");
    println!("  par-term Shell Integration Installer");
    println!("=============================================");
    println!();

    let shell = shell.unwrap_or_else(ShellType::detect);
    println!("Detected shell: {}", shell.display_name());

    if shell == ShellType::Unknown {
        println!("Error: Could not detect shell type.");
        println!("Please specify with: par-term install-shell-integration --shell bash|zsh|fish");
        return Err(anyhow::anyhow!("Unknown shell"));
    }

    println!();
    println!("Installing shell integration...");

    match shell_integration_installer::install(Some(shell)) {
        Ok(result) => {
            println!();
            println!("=============================================");
            println!("  Installation complete!");
            println!("=============================================");
            println!();
            println!("Installed to: {}", result.script_path.display());
            println!("Modified: {}", result.rc_file.display());
            println!();
            println!("To activate now, run:");
            println!("  source {}", result.script_path.display());
            println!();
            println!("Or restart your shell.");
            Ok(())
        }
        Err(e) => {
            println!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

fn uninstall_shell_integration_cli() -> anyhow::Result<()> {
    println!("Uninstalling shell integration...");
    println!();

    match shell_integration_installer::uninstall() {
        Ok(result) => {
            if !result.scripts_removed.is_empty() {
                println!("Removed scripts:");
                for path in &result.scripts_removed {
                    println!("  {}", path.display());
                }
            }
            if !result.cleaned.is_empty() {
                println!("Cleaned RC files:");
                for path in &result.cleaned {
                    println!("  {}", path.display());
                }
            }
            if !result.needs_manual.is_empty() {
                println!();
                println!("Manual cleanup needed for:");
                for path in &result.needs_manual {
                    println!("  {}", path.display());
                }
                println!();
                println!("Remove lines containing 'par-term shell integration' from these files.");
            }
            println!();
            println!("Uninstallation complete!");
            Ok(())
        }
        Err(e) => {
            println!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

fn uninstall_shaders_cli(force: bool) -> anyhow::Result<()> {
    println!("Uninstalling shaders...");
    println!();

    match shader_installer::uninstall_shaders(force) {
        Ok(result) => {
            println!("Removed {} bundled files", result.removed);
            if result.kept > 0 {
                println!("Kept {} user files", result.kept);
            }
            if !result.needs_confirmation.is_empty() {
                println!();
                println!("Modified bundled files (use --force to remove):");
                for path in &result.needs_confirmation {
                    println!("  {}", path);
                }
            }
            println!();
            println!("Uninstallation complete!");
            Ok(())
        }
        Err(e) => {
            println!("Error: {}", e);
            Err(anyhow::anyhow!(e))
        }
    }
}

fn install_integrations_cli(skip_prompt: bool) -> anyhow::Result<()> {
    println!("=============================================");
    println!("  par-term Integrations Installer");
    println!("=============================================");
    println!();

    // Install shaders
    println!("Installing shaders...");
    if let Err(e) = install_shaders_cli(skip_prompt) {
        println!("Warning: Shader installation failed: {}", e);
    }

    println!();

    // Install shell integration
    println!("Installing shell integration...");
    if let Err(e) = install_shell_integration_cli(None) {
        println!("Warning: Shell integration installation failed: {}", e);
    }

    println!();
    println!("=============================================");
    println!("  All integrations installed!");
    println!("=============================================");

    Ok(())
}
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 6: Test CLI commands**

Run: `cargo run -- install-shell-integration --help`
Expected: Shows help for the command

**Step 7: Commit**

```bash
git add src/cli.rs
git commit -m "feat(cli): add shell integration and uninstall commands"
```

---

## Phase 5: Settings UI - Integrations Tab

### Task 9: Create Integrations Tab

**Files:**
- Create: `src/settings_ui/integrations_tab.rs`
- Modify: `src/settings_ui/mod.rs`
- Modify: `src/settings_ui/sidebar.rs`

**Step 1: Create integrations_tab.rs**

```rust
//! Integrations tab for settings UI.
//!
//! Provides install/reinstall/uninstall controls for shell integration and shaders.

use crate::config::{Config, InstallPromptState, ShellType};
use crate::shader_installer;
use crate::shell_integration_installer;
use egui::{Color32, RichText, Ui};

use super::section::collapsing_section;
use super::SettingsUI;

/// Status of an integration
#[derive(Debug, Clone)]
pub struct IntegrationStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub details: String,
}

impl SettingsUI {
    /// Render the integrations tab
    pub fn show_integrations_tab(&mut self, ui: &mut Ui, changes_this_frame: &mut bool) {
        ui.heading("Integrations");
        ui.add_space(8.0);

        ui.label("Optional enhancements that can be installed separately.");
        ui.add_space(16.0);

        // Shell Integration Section
        collapsing_section(ui, "Shell Integration", true, |ui| {
            self.show_shell_integration_section(ui, changes_this_frame);
        });

        ui.add_space(16.0);

        // Shaders Section
        collapsing_section(ui, "Custom Shaders", true, |ui| {
            self.show_shaders_section(ui, changes_this_frame);
        });
    }

    fn show_shell_integration_section(&mut self, ui: &mut Ui, _changes_this_frame: &mut bool) {
        let shell = ShellType::detect();
        let installed = shell_integration_installer::is_installed();

        // Status indicator
        ui.horizontal(|ui| {
            ui.label("Status:");
            if installed {
                ui.colored_label(Color32::from_rgb(100, 200, 100), "● Installed");
                if let Some(ref version) = self.config.integration_versions.shell_integration_installed_version {
                    ui.label(format!("(v{})", version));
                }
                ui.label(format!("for {}", shell.display_name()));
            } else {
                ui.colored_label(Color32::from_rgb(150, 150, 150), "○ Not installed");
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Detected shell:");
            ui.strong(shell.display_name());
        });

        ui.add_space(8.0);

        // Description
        ui.label("Shell integration provides:");
        ui.indent("shell_features", |ui| {
            ui.label("• Working directory tracking in tab titles");
            ui.label("• Command exit status indicators");
            ui.label("• Prompt navigation between commands");
        });

        ui.add_space(12.0);

        // Buttons
        ui.horizontal(|ui| {
            if installed {
                if ui.button("Reinstall").clicked() {
                    self.shell_integration_action = Some(ShellIntegrationAction::Install);
                }
                if ui.button("Uninstall").clicked() {
                    self.shell_integration_action = Some(ShellIntegrationAction::Uninstall);
                }
            } else if ui.button("Install").clicked() {
                self.shell_integration_action = Some(ShellIntegrationAction::Install);
            }
        });

        ui.add_space(12.0);

        // Manual install command
        ui.horizontal(|ui| {
            ui.label("Manual install:");
        });
        let cmd = "curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash";
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut cmd.to_string())
                .font(egui::TextStyle::Monospace)
                .desired_width(400.0)
                .interactive(false));
            if ui.button("📋").on_hover_text("Copy to clipboard").clicked() {
                ui.ctx().copy_text(cmd.to_string());
            }
        });
    }

    fn show_shaders_section(&mut self, ui: &mut Ui, _changes_this_frame: &mut bool) {
        let shaders_dir = Config::shaders_dir();
        let installed = shader_installer::has_shader_files(&shaders_dir);
        let file_count = shader_installer::count_manifest_files(&shaders_dir);
        let shader_count = shader_installer::count_shader_files(&shaders_dir);

        // Status indicator
        ui.horizontal(|ui| {
            ui.label("Status:");
            if installed {
                ui.colored_label(Color32::from_rgb(100, 200, 100), "● Installed");
                if let Some(ref version) = self.config.integration_versions.shaders_installed_version {
                    ui.label(format!("(v{})", version));
                }
                ui.label(format!("- {} shaders, {} total files", shader_count, file_count));
            } else {
                ui.colored_label(Color32::from_rgb(150, 150, 150), "○ Not installed");
            }
        });

        ui.add_space(8.0);

        // Description
        ui.label("Custom shaders include:");
        ui.indent("shader_features", |ui| {
            ui.label("• 49+ background effects (CRT, matrix, plasma, etc.)");
            ui.label("• 12 cursor effect shaders");
            ui.label("• Textures and supporting files");
        });

        ui.add_space(12.0);

        // Buttons
        ui.horizontal(|ui| {
            if installed {
                if ui.button("Reinstall").clicked() {
                    self.shader_action = Some(ShaderAction::Install);
                }
                if ui.button("Uninstall").clicked() {
                    self.shader_action = Some(ShaderAction::Uninstall);
                }
            } else if ui.button("Install").clicked() {
                self.shader_action = Some(ShaderAction::Install);
            }
        });

        ui.add_space(12.0);

        // Manual install command
        ui.horizontal(|ui| {
            ui.label("Manual install:");
        });
        let cmd = "curl -fsSL https://paulrobello.github.io/par-term/install-shaders.sh | bash";
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut cmd.to_string())
                .font(egui::TextStyle::Monospace)
                .desired_width(400.0)
                .interactive(false));
            if ui.button("📋").on_hover_text("Copy to clipboard").clicked() {
                ui.ctx().copy_text(cmd.to_string());
            }
        });
    }
}

/// Actions for shell integration
#[derive(Debug, Clone, Copy)]
pub enum ShellIntegrationAction {
    Install,
    Uninstall,
}

/// Actions for shaders
#[derive(Debug, Clone, Copy)]
pub enum ShaderAction {
    Install,
    Uninstall,
}
```

**Step 2: Update sidebar.rs to add Integrations tab**

Add `Integrations` to the `SettingsTab` enum:

```rust
pub enum SettingsTab {
    #[default]
    Appearance,
    Window,
    Input,
    Terminal,
    Effects,
    Notifications,
    Integrations,  // NEW
    Advanced,
}
```

Update `display_name`, `icon`, and `all` methods:

```rust
    pub fn display_name(&self) -> &'static str {
        match self {
            // ... existing ...
            Self::Integrations => "Integrations",
            Self::Advanced => "Advanced",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            // ... existing ...
            Self::Integrations => "🔌",
            Self::Advanced => "⚙",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Appearance,
            Self::Window,
            Self::Input,
            Self::Terminal,
            Self::Effects,
            Self::Notifications,
            Self::Integrations,  // NEW
            Self::Advanced,
        ]
    }
```

**Step 3: Update mod.rs to include integrations_tab**

Add:
```rust
pub mod integrations_tab;
```

And add action fields to SettingsUI struct:
```rust
    /// Pending shell integration action
    pub(crate) shell_integration_action: Option<integrations_tab::ShellIntegrationAction>,
    /// Pending shader action
    pub(crate) shader_action: Option<integrations_tab::ShaderAction>,
```

Initialize in `new()`:
```rust
            shell_integration_action: None,
            shader_action: None,
```

**Step 4: Update the tab rendering in mod.rs**

In the `show` method, add the Integrations case:
```rust
                SettingsTab::Integrations => {
                    self.show_integrations_tab(ui, &mut changes_this_frame);
                }
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add src/settings_ui/integrations_tab.rs src/settings_ui/mod.rs src/settings_ui/sidebar.rs
git commit -m "feat(settings): add Integrations tab with install/uninstall controls"
```

---

## Phase 6: Welcome Dialog

### Task 10: Create Combined Integrations Welcome Dialog

**Files:**
- Create: `src/integrations_ui.rs`
- Modify: `src/lib.rs`

**Step 1: Create integrations_ui.rs**

```rust
//! Combined integrations welcome dialog.
//!
//! Shows on first run when integrations are not installed,
//! offering to install shaders and/or shell integration.

use crate::config::ShellType;
use egui::{Align2, Color32, Context, Frame, RichText, Window, epaint::Shadow};

/// User's response to the integrations dialog
#[derive(Debug, Clone, Default)]
pub struct IntegrationsResponse {
    /// User wants to install shaders
    pub install_shaders: bool,
    /// User wants to install shell integration
    pub install_shell_integration: bool,
    /// User clicked Skip (dismiss for this session)
    pub skipped: bool,
    /// User clicked Never Ask
    pub never_ask: bool,
    /// Dialog was closed
    pub closed: bool,
}

/// Combined integrations welcome dialog
pub struct IntegrationsUI {
    /// Whether the dialog is visible
    pub visible: bool,
    /// Checkbox state for shaders
    pub shaders_checked: bool,
    /// Checkbox state for shell integration
    pub shell_integration_checked: bool,
    /// Detected shell type
    pub detected_shell: ShellType,
    /// Installation in progress
    pub installing: bool,
    /// Progress message
    pub progress_message: Option<String>,
    /// Error message
    pub error_message: Option<String>,
    /// Success message
    pub success_message: Option<String>,
}

impl IntegrationsUI {
    pub fn new() -> Self {
        Self {
            visible: false,
            shaders_checked: true,
            shell_integration_checked: true,
            detected_shell: ShellType::detect(),
            installing: false,
            progress_message: None,
            error_message: None,
            success_message: None,
        }
    }

    /// Show the dialog
    pub fn show_dialog(&mut self) {
        self.visible = true;
        self.installing = false;
        self.progress_message = None;
        self.error_message = None;
        self.success_message = None;
        self.detected_shell = ShellType::detect();
    }

    /// Hide the dialog
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Render the dialog and return user response
    pub fn show(&mut self, ctx: &Context) -> IntegrationsResponse {
        if !self.visible {
            return IntegrationsResponse::default();
        }

        let mut response = IntegrationsResponse::default();

        // Style for opaque dialog
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(32, 32, 32, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        ctx.set_style(style);

        let viewport = ctx.input(|i| i.viewport_rect());
        let version = env!("CARGO_PKG_VERSION");

        Window::new(format!("Welcome to par-term v{}", version))
            .resizable(false)
            .collapsible(false)
            .default_width(500.0)
            .default_pos(viewport.center())
            .pivot(Align2::CENTER_CENTER)
            .frame(
                Frame::window(&ctx.style())
                    .fill(solid_bg)
                    .inner_margin(24.0)
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(80)))
                    .shadow(Shadow {
                        offset: [4, 4],
                        blur: 16,
                        spread: 4,
                        color: Color32::from_black_alpha(180),
                    }),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Optional Enhancements Available")
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(16.0);
                });

                // Show progress/error/success
                if self.installing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            self.progress_message
                                .as_deref()
                                .unwrap_or("Installing..."),
                        );
                    });
                    return;
                }

                if let Some(error) = &self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), error);
                    ui.add_space(8.0);
                }

                if let Some(success) = &self.success_message {
                    ui.colored_label(Color32::from_rgb(100, 255, 100), success);
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        if ui.add_sized([120.0, 32.0], egui::Button::new("OK")).clicked() {
                            response.closed = true;
                            self.visible = false;
                        }
                    });
                    return;
                }

                // Checkboxes
                ui.add_space(8.0);

                // Shaders checkbox
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.shaders_checked, "");
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Custom Shaders").strong());
                        ui.label("49+ background effects and 12 cursor shaders");
                        ui.label(
                            RichText::new("CRT, matrix rain, plasma, starfields, and more")
                                .weak()
                                .small(),
                        );
                    });
                });

                ui.add_space(12.0);

                // Shell integration checkbox
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.shell_integration_checked, "");
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Shell Integration").strong());
                        ui.label("Directory tracking, command status, prompt navigation");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Detected shell:").weak().small());
                            ui.label(
                                RichText::new(self.detected_shell.display_name())
                                    .weak()
                                    .small(),
                            );
                        });
                    });
                });

                ui.add_space(20.0);

                // Buttons
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        let any_selected =
                            self.shaders_checked || self.shell_integration_checked;

                        if ui
                            .add_enabled(
                                any_selected,
                                egui::Button::new("Install Selected").min_size([140.0, 32.0].into()),
                            )
                            .clicked()
                        {
                            response.install_shaders = self.shaders_checked;
                            response.install_shell_integration = self.shell_integration_checked;
                        }

                        ui.add_space(8.0);

                        if ui
                            .add_sized([100.0, 32.0], egui::Button::new("Skip"))
                            .on_hover_text("Ask again next version")
                            .clicked()
                        {
                            response.skipped = true;
                            self.visible = false;
                        }

                        ui.add_space(8.0);

                        if ui
                            .add_sized([100.0, 32.0], egui::Button::new("Never Ask"))
                            .on_hover_text("Don't ask again")
                            .clicked()
                        {
                            response.never_ask = true;
                            self.visible = false;
                        }
                    });
                });

                ui.add_space(12.0);

                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("You can install these later from Settings (F12) → Integrations")
                            .weak()
                            .small(),
                    );
                });
            });

        response
    }

    /// Set installing state
    pub fn set_installing(&mut self, message: &str) {
        self.installing = true;
        self.progress_message = Some(message.to_string());
    }

    /// Set error state
    pub fn set_error(&mut self, error: &str) {
        self.installing = false;
        self.error_message = Some(error.to_string());
    }

    /// Set success state
    pub fn set_success(&mut self, message: &str) {
        self.installing = false;
        self.success_message = Some(message.to_string());
    }
}

impl Default for IntegrationsUI {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Add to lib.rs**

```rust
pub mod integrations_ui;
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/integrations_ui.rs src/lib.rs
git commit -m "feat: add combined integrations welcome dialog"
```

---

## Phase 7: GitHub Pages Setup

### Task 11: Restructure to gh-pages Directory

**Files:**
- Rename: `shader-gallery/` → `gh-pages/gallery/`
- Create: `gh-pages/index.html`
- Create: `gh-pages/install-shaders.sh`
- Create: `gh-pages/install-shell-integration.sh`
- Modify: `.github/workflows/pages.yml`

**Step 1: Create gh-pages structure**

```bash
mkdir -p gh-pages/gallery
mv shader-gallery/* gh-pages/gallery/
rmdir shader-gallery
```

**Step 2: Create gh-pages/index.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>par-term</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 2rem;
            background: #1a1a2e;
            color: #eee;
        }
        h1 { color: #00d9ff; }
        a { color: #00d9ff; }
        code {
            background: #16213e;
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
            font-family: 'JetBrains Mono', monospace;
        }
        pre {
            background: #16213e;
            padding: 1rem;
            border-radius: 8px;
            overflow-x: auto;
        }
    </style>
</head>
<body>
    <h1>par-term</h1>
    <p>A GPU-accelerated terminal emulator with custom shaders and shell integration.</p>

    <h2>Quick Install</h2>

    <h3>Shaders</h3>
    <pre><code>curl -fsSL https://paulrobello.github.io/par-term/install-shaders.sh | bash</code></pre>

    <h3>Shell Integration</h3>
    <pre><code>curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash</code></pre>

    <h2>Resources</h2>
    <ul>
        <li><a href="gallery/">Shader Gallery</a></li>
        <li><a href="https://github.com/paulrobello/par-term">GitHub Repository</a></li>
    </ul>
</body>
</html>
```

**Step 3: Copy and adapt install-shaders.sh for gh-pages**

Copy `install_shaders.sh` to `gh-pages/install-shaders.sh` (same content works).

**Step 4: Create gh-pages/install-shell-integration.sh**

```bash
#!/bin/sh
# install-shell-integration.sh - Install par-term shell integration
#
# Usage: curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash

set -e

REPO="paulrobello/par-term"
BRANCH="main"

echo "============================================="
echo "  par-term Shell Integration Installer"
echo "============================================="
echo ""

# Detect shell
detect_shell() {
    case "$(basename "$SHELL")" in
        bash) echo "bash" ;;
        zsh)  echo "zsh" ;;
        fish) echo "fish" ;;
        *)    echo "unknown" ;;
    esac
}

SHELL_TYPE=$(detect_shell)
echo "Detected shell: $SHELL_TYPE"

if [ "$SHELL_TYPE" = "unknown" ]; then
    echo "Error: Could not detect shell type."
    echo "Supported shells: bash, zsh, fish"
    exit 1
fi

# Set paths
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/par-term"
SCRIPT_NAME="shell_integration.$SHELL_TYPE"
SCRIPT_URL="https://raw.githubusercontent.com/$REPO/$BRANCH/shell_integration/par_term_shell_integration.$SHELL_TYPE"

# Determine RC file
case "$SHELL_TYPE" in
    bash)
        if [ -f "$HOME/.bashrc" ]; then
            RC_FILE="$HOME/.bashrc"
        else
            RC_FILE="$HOME/.bash_profile"
        fi
        ;;
    zsh)
        RC_FILE="${ZDOTDIR:-$HOME}/.zshrc"
        ;;
    fish)
        RC_FILE="$HOME/.config/fish/config.fish"
        ;;
esac

echo "Config directory: $CONFIG_DIR"
echo "RC file: $RC_FILE"
echo ""

# Check if already installed
MARKER="# >>> par-term shell integration >>>"
if grep -q "$MARKER" "$RC_FILE" 2>/dev/null; then
    echo "Shell integration already installed in $RC_FILE"
    printf "Do you want to reinstall? [y/N] "
    read -r response
    case "$response" in
        [yY][eE][sS]|[yY]) ;;
        *) echo "Cancelled."; exit 0 ;;
    esac
fi

# Download script
echo "Downloading shell integration script..."
mkdir -p "$CONFIG_DIR"

if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$SCRIPT_URL" -o "$CONFIG_DIR/$SCRIPT_NAME"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$SCRIPT_URL" -O "$CONFIG_DIR/$SCRIPT_NAME"
else
    echo "Error: curl or wget required"
    exit 1
fi

chmod +x "$CONFIG_DIR/$SCRIPT_NAME"

# Add to RC file if not present
if ! grep -q "$MARKER" "$RC_FILE" 2>/dev/null; then
    echo "Adding source line to $RC_FILE..."

    # Create RC file directory if needed
    mkdir -p "$(dirname "$RC_FILE")"

    case "$SHELL_TYPE" in
        bash|zsh)
            cat >> "$RC_FILE" << 'RCEOF'

# >>> par-term shell integration >>>
if [ -f "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.SHELL_TYPE" ]; then
  source "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.SHELL_TYPE"
fi
# <<< par-term shell integration <<<
RCEOF
            # Replace SHELL_TYPE placeholder
            sed -i.bak "s/SHELL_TYPE/$SHELL_TYPE/g" "$RC_FILE"
            rm -f "$RC_FILE.bak"
            ;;
        fish)
            cat >> "$RC_FILE" << 'RCEOF'

# >>> par-term shell integration >>>
set -l __par_term_config_dir (begin
    if set -q XDG_CONFIG_HOME
        echo $XDG_CONFIG_HOME
    else
        echo $HOME/.config
    end
end)/par-term
if test -f "$__par_term_config_dir/shell_integration.fish"
    source "$__par_term_config_dir/shell_integration.fish"
end
# <<< par-term shell integration <<<
RCEOF
            ;;
    esac
fi

echo ""
echo "============================================="
echo "  Installation complete!"
echo "============================================="
echo ""
echo "Installed to: $CONFIG_DIR/$SCRIPT_NAME"
echo ""
echo "To activate now, run:"
echo "  source $CONFIG_DIR/$SCRIPT_NAME"
echo ""
echo "Or restart your shell."
```

**Step 5: Update pages.yml workflow**

```yaml
# Deploy to GitHub Pages
name: Deploy to GitHub Pages

on:
  push:
    branches: ["main"]
    paths:
      - "gh-pages/**"
      - "shell_integration/**"
      - ".github/workflows/pages.yml"

  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Pages
        uses: actions/configure-pages@v5

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: "gh-pages"

      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

**Step 6: Commit**

```bash
git add gh-pages/ .github/workflows/pages.yml
git rm -r shader-gallery/
git commit -m "refactor: restructure to gh-pages with install scripts"
```

---

## Phase 8: Integration & Testing

### Task 12: Wire Up Dialog to App Handler

**Files:**
- Modify: `src/app/window_state.rs` (add integrations_ui field)
- Modify: `src/app/handler.rs` (check and show dialog)

**Step 1: Add IntegrationsUI to WindowState**

In `src/app/window_state.rs`, add:
```rust
use crate::integrations_ui::IntegrationsUI;

// In WindowState struct:
    /// Integrations welcome dialog
    pub integrations_ui: IntegrationsUI,
```

Initialize in WindowState::new():
```rust
            integrations_ui: IntegrationsUI::new(),
```

**Step 2: Add check in handler.rs after window creation**

In the appropriate initialization section, add:
```rust
        // Check if we should show integrations dialog
        if self.config.should_prompt_integrations() {
            self.integrations_ui.show_dialog();
        }
```

**Step 3: Handle IntegrationsUI in render loop**

In the egui rendering section:
```rust
        // Show integrations dialog if visible
        let integrations_response = self.integrations_ui.show(ctx);

        // Handle integrations response
        if integrations_response.install_shaders || integrations_response.install_shell_integration {
            // Trigger installation
            self.integrations_ui.set_installing("Installing...");
            // ... actual installation logic
        }

        if integrations_response.skipped {
            // Update prompted version
            let version = env!("CARGO_PKG_VERSION").to_string();
            self.config.integration_versions.shaders_prompted_version = Some(version.clone());
            self.config.integration_versions.shell_integration_prompted_version = Some(version);
            self.config.save().ok();
        }

        if integrations_response.never_ask {
            self.config.shader_install_prompt = ShaderInstallPrompt::Never;
            self.config.shell_integration_state = InstallPromptState::Never;
            self.config.save().ok();
        }
```

**Step 4: Verify compilation and test**

Run: `cargo build`
Run: `cargo run` - verify dialog appears on fresh config

**Step 5: Commit**

```bash
git add src/app/window_state.rs src/app/handler.rs
git commit -m "feat: wire up integrations dialog to app startup"
```

---

### Task 13: Add Integration Tests

**Files:**
- Create: `tests/integration_installer_tests.rs`

**Step 1: Write tests**

```rust
//! Integration tests for shell integration and shader installers.

use par_term::config::ShellType;
use par_term::manifest::{compute_file_hash, FileStatus, Manifest};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_shell_type_detection() {
    // This test depends on the environment
    let shell = ShellType::detect();
    // Should be one of the known types or Unknown
    assert!(matches!(
        shell,
        ShellType::Bash | ShellType::Zsh | ShellType::Fish | ShellType::Unknown
    ));
}

#[test]
fn test_manifest_load_save() {
    let temp = TempDir::new().unwrap();

    let manifest = Manifest {
        version: "0.1.0".to_string(),
        generated: "2024-01-01T00:00:00Z".to_string(),
        files: vec![],
    };

    manifest.save(temp.path()).unwrap();

    let loaded = Manifest::load(temp.path()).unwrap();
    assert_eq!(loaded.version, "0.1.0");
}

#[test]
fn test_file_hash() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let hash = compute_file_hash(&file_path).unwrap();
    // SHA256 of "hello world"
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}
```

**Step 2: Run tests**

Run: `cargo test integration_installer`
Expected: All tests pass

**Step 3: Commit**

```bash
git add tests/integration_installer_tests.rs
git commit -m "test: add integration installer tests"
```

---

### Task 14: Final Documentation Update

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Step 1: Update README.md**

Add section about integrations:

```markdown
## Optional Integrations

### Shell Integration

Enables directory tracking, command status indicators, and prompt navigation.

**Install from par-term:**
Settings (F12) → Integrations → Install

**Install via curl:**
```bash
curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash
```

### Custom Shaders

49+ background effects and 12 cursor shaders.

**Install from par-term:**
Settings (F12) → Integrations → Install

**Install via curl:**
```bash
curl -fsSL https://paulrobello.github.io/par-term/install-shaders.sh | bash
```
```

**Step 2: Update CHANGELOG.md**

Add entry for new version:

```markdown
## [Unreleased]

### Added
- Shell integration scripts for bash, zsh, and fish
- Combined integrations welcome dialog on first run
- Integrations tab in Settings UI for install/reinstall/uninstall
- Manifest system for tracking bundled shader files
- `install-shell-integration` and `uninstall-shaders` CLI commands
- GitHub Pages hosting for curl-installable scripts
- Version tracking for "once per version" install prompts

### Changed
- Restructured GitHub Pages from shader-gallery to gh-pages
- Shader installer now uses manifest for smart updates
```

**Step 3: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: update README and CHANGELOG for integrations system"
```

---

## Summary

**Total Tasks:** 14
**Estimated Commits:** 14+

**Key Deliverables:**
1. Config schema with version tracking
2. Manifest system for bundled files
3. Shell integration scripts (bash, zsh, fish)
4. Shell integration installer module
5. Updated shader installer with manifest support
6. CLI commands for all install/uninstall operations
7. Integrations settings tab
8. Combined welcome dialog
9. GitHub Pages with curl installers
10. Tests and documentation

**Testing Checklist:**
- [ ] `cargo test` passes
- [ ] `cargo run -- install-shell-integration` works
- [ ] `cargo run -- install-shaders` works with manifest
- [ ] `cargo run -- uninstall-shaders` preserves user files
- [ ] Settings UI → Integrations tab shows correct status
- [ ] Welcome dialog appears on fresh config
- [ ] curl installers work from GitHub Pages
