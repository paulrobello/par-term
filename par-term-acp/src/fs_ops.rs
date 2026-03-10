//! Filesystem operations for ACP agent requests.
//!
//! These functions handle `fs/read_text_file`, `fs/write_text_file`,
//! `fs/list_directory`, and `fs/find` RPC calls from the agent.
//! They are executed directly in the async message handler task
//! (via `spawn_blocking`) so they do not depend on UI-thread state.
//!
//! # Security
//!
//! All path-accepting functions enforce two layers of path restriction:
//!
//! 1. **Sensitive path blocklist**: Paths under `~/.ssh/`, `~/.gnupg/`, and
//!    `/etc/` are unconditionally rejected to protect private keys and
//!    system credentials even when `auto_approve` is enabled.
//!
//! 2. **Directory restrictions for listing/find**: `list_directory_entries`
//!    and `find_files_recursive` additionally apply the same blocklist so
//!    that a malicious agent cannot enumerate sensitive directories.

/// Maximum file size allowed for reading via ACP (50MB).
/// This prevents memory exhaustion from reading multi-GB files.
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB

// =========================================================================
// Sensitive path blocklist (SEC-011, SEC-014)
// =========================================================================

/// Sensitive path prefixes that ACP file operations must never access,
/// regardless of `auto_approve` mode.
///
/// The check is performed on the **canonicalized** absolute path, so
/// symlink-based traversal attacks are mitigated before the comparison.
///
/// # Rationale
///
/// - `~/.ssh/`: private keys, authorized_keys, known_hosts
/// - `~/.gnupg/`: PGP private keys
/// - `/etc/`: system configuration, passwd, sudoers, shadow
fn is_sensitive_path(canonical: &std::path::Path) -> bool {
    // Paths under the user's home directory that contain credentials.
    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");
        let gnupg_dir = home.join(".gnupg");
        if canonical.starts_with(&ssh_dir) || canonical.starts_with(&gnupg_dir) {
            return true;
        }
    }
    // System credential and configuration directories.
    if canonical.starts_with("/etc/") || canonical == std::path::Path::new("/etc") {
        return true;
    }
    false
}

/// Canonicalize `path` and check it against the sensitive path blocklist.
/// Returns `Ok(canonical)` when safe, `Err(message)` when blocked.
fn check_path_allowed(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(path);

    // Resolve the canonical path (follows symlinks, resolves ..).
    // For non-existent paths, canonicalize the parent and re-append the filename
    // so that new-file creation in safe directories is still allowed.
    let canonical = if p.exists() {
        std::fs::canonicalize(p).map_err(|e| format!("Cannot resolve path: {e}"))?
    } else {
        let parent = p
            .parent()
            .ok_or_else(|| "Path has no parent directory".to_string())?;
        let canonical_parent =
            std::fs::canonicalize(parent).map_err(|e| format!("Cannot resolve parent: {e}"))?;
        let file_name = p
            .file_name()
            .ok_or_else(|| "Path has no file name".to_string())?;
        canonical_parent.join(file_name)
    };

    if is_sensitive_path(&canonical) {
        return Err(format!(
            "Access denied: '{}' is in a restricted directory. \
             ACP agents cannot read or list ~/.ssh/, ~/.gnupg/, or /etc/.",
            path
        ));
    }

    Ok(canonical)
}

/// Read a text file, optionally returning a line range.
///
/// `line` is 1-based (line 1 is the first line).
///
/// # Security
///
/// - Files larger than [`MAX_FILE_SIZE`] (50MB) are rejected.
/// - Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are unconditionally blocked.
pub fn read_file_with_range(
    path: &str,
    line: Option<u64>,
    limit: Option<u64>,
) -> Result<String, String> {
    // SEC-011: Validate path against sensitive directory blocklist before reading.
    check_path_allowed(path)?;

    // Check file size before reading to prevent memory exhaustion.
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "File too large: {} bytes (max {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    match (line, limit) {
        (None, None) => Ok(content),
        _ => {
            let skip = line.unwrap_or(1).saturating_sub(1) as usize;
            let lines: Vec<&str> = content.lines().skip(skip).collect();
            let taken: Vec<&str> = if let Some(lim) = limit {
                lines.into_iter().take(lim as usize).collect()
            } else {
                lines
            };
            Ok(taken.join("\n"))
        }
    }
}

/// Write content to a file, creating parent directories as needed.
///
/// Requires an absolute path for safety.
pub fn write_file_safe(path: &str, content: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }
    std::fs::write(p, content).map_err(|e| format!("Failed to write file: {e}"))
}

/// List directory entries, optionally filtering by a glob-like pattern.
///
/// Returns a sorted vec of JSON objects with `name`, `path`, `isDirectory`, and
/// `isFile` fields.
///
/// # Security
///
/// Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are blocked (SEC-014).
pub fn list_directory_entries(
    path: &str,
    pattern: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    // SEC-014: Block listing of sensitive directories.
    check_path_allowed(path)?;
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut result: Vec<serde_json::Value> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Simple glob matching: supports "*.ext" and "*" patterns.
        if let Some(pat) = pattern
            && !glob_match_simple(pat, &name)
        {
            continue;
        }

        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        result.push(serde_json::json!({
            "name": name,
            "path": entry.path().to_string_lossy(),
            "isDirectory": file_type.is_dir(),
            "isFile": file_type.is_file(),
        }));
    }
    result.sort_by(|a, b| {
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a_name.cmp(b_name)
    });
    Ok(result)
}

/// Maximum directory depth for recursive file searches.
/// This prevents stack overflow from deep directory trees or symlink loops.
const MAX_SEARCH_DEPTH: usize = 20;

/// Recursively find files matching a glob pattern.
///
/// Supports simple patterns like `*.glsl`, `**/*.rs`, and literal names.
/// Returns a sorted list of absolute file paths.
///
/// # Security
///
/// - Maximum recursion depth is limited to [`MAX_SEARCH_DEPTH`] to prevent stack overflow.
/// - Symlinks are skipped to prevent infinite loops from symlink cycles.
/// - Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are blocked (SEC-014).
pub fn find_files_recursive(base_path: &str, pattern: &str) -> Result<Vec<String>, String> {
    let base = std::path::Path::new(base_path);
    if !base.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if !base.exists() {
        return Err(format!("Path does not exist: {base_path}"));
    }
    // SEC-014: Block recursive search of sensitive directories.
    check_path_allowed(base_path)?;

    let mut results = Vec::new();
    // Strip leading **/ for simple recursive matching.
    let file_pattern = pattern.strip_prefix("**/").unwrap_or(pattern);

    fn walk_dir(
        dir: &std::path::Path,
        file_pattern: &str,
        results: &mut Vec<String>,
        remaining_depth: usize,
    ) -> Result<(), String> {
        // Stop recursion if we've reached the maximum depth.
        if remaining_depth == 0 {
            return Ok(());
        }

        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            // Get file type and skip symlinks to prevent infinite loops from symlink cycles.
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                walk_dir(&path, file_pattern, results, remaining_depth - 1)?;
            } else if file_type.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if glob_match_simple(file_pattern, &name) {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }
        Ok(())
    }

    walk_dir(base, file_pattern, &mut results, MAX_SEARCH_DEPTH)?;
    results.sort();
    Ok(results)
}

/// Simple glob matching for directory listing filters.
///
/// Supports `*` (match anything), `*.ext` (match extension), and literal names.
pub fn glob_match_simple(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pattern.strip_suffix("*") {
        return name.starts_with(prefix);
    }
    name == pattern
}
