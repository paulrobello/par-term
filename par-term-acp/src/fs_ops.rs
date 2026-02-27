//! Filesystem operations for ACP agent requests.
//!
//! These functions handle `fs/read_text_file`, `fs/write_text_file`,
//! `fs/list_directory`, and `fs/find` RPC calls from the agent.
//! They are executed directly in the async message handler task
//! (via `spawn_blocking`) so they do not depend on UI-thread state.

/// Maximum file size allowed for reading via ACP (50MB).
/// This prevents memory exhaustion from reading multi-GB files.
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB

/// Read a text file, optionally returning a line range.
///
/// `line` is 1-based (line 1 is the first line).
///
/// # Security
///
/// Files larger than [`MAX_FILE_SIZE`] (50MB) are rejected to prevent
/// memory exhaustion from reading multi-GB files.
pub fn read_file_with_range(
    path: &str,
    line: Option<u64>,
    limit: Option<u64>,
) -> Result<String, String> {
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
pub fn list_directory_entries(
    path: &str,
    pattern: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
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
pub fn find_files_recursive(base_path: &str, pattern: &str) -> Result<Vec<String>, String> {
    let base = std::path::Path::new(base_path);
    if !base.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if !base.exists() {
        return Err(format!("Path does not exist: {base_path}"));
    }

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
