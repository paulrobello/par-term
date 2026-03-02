//! IPC file path resolution and file helpers.
//!
//! Resolves platform-appropriate paths for config-update and screenshot IPC
//! files, and provides atomic write / restricted-permission helpers used by
//! both the MCP tool handlers and external consumers (e.g. the ACP harness).

use crate::{
    CONFIG_UPDATE_FILENAME, CONFIG_UPDATE_PATH_ENV, SCREENSHOT_REQUEST_FILENAME,
    SCREENSHOT_REQUEST_PATH_ENV, SCREENSHOT_RESPONSE_FILENAME, SCREENSHOT_RESPONSE_PATH_ENV,
};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Platform-aware restricted file creation
// ---------------------------------------------------------------------------

/// Open (or create/truncate) a file for writing with owner-only permissions
/// (0o600) on Unix, or default permissions on other platforms.
pub fn open_restricted_write(path: &Path) -> Result<std::fs::File, std::io::Error> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

// ---------------------------------------------------------------------------
// IPC file permission helpers
// ---------------------------------------------------------------------------

/// Set restrictive permissions (owner read/write only) on an IPC file.
///
/// On Unix systems this sets mode 0o600 so that only the file owner can
/// read or write. On non-Unix platforms this is a no-op.
///
/// Prefer `open_restricted_write` for new files to avoid a world-readable
/// race between creation and permission fixup. This helper is retained for
/// fixing permissions on pre-existing files.
#[allow(dead_code)]
pub fn set_ipc_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set permissions on {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path; // suppress unused warning
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// IPC path resolution
// ---------------------------------------------------------------------------

/// Resolve the path where config updates should be written.
///
/// Checks `PAR_TERM_CONFIG_UPDATE_PATH` env var first, then falls back to
/// `~/.config/par-term/.config-update.json`.
pub fn config_update_path() -> PathBuf {
    resolve_ipc_path(CONFIG_UPDATE_PATH_ENV, CONFIG_UPDATE_FILENAME)
}

/// Resolve the path where screenshot requests should be written.
pub fn screenshot_request_path() -> PathBuf {
    resolve_ipc_path(SCREENSHOT_REQUEST_PATH_ENV, SCREENSHOT_REQUEST_FILENAME)
}

/// Resolve the path where screenshot responses should be written.
pub fn screenshot_response_path() -> PathBuf {
    resolve_ipc_path(SCREENSHOT_RESPONSE_PATH_ENV, SCREENSHOT_RESPONSE_FILENAME)
}

/// Resolve a path from env var or default filename under `~/.config/par-term`.
pub fn resolve_ipc_path(env_var: &str, default_filename: &str) -> PathBuf {
    if let Ok(path) = std::env::var(env_var) {
        return PathBuf::from(path);
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| {
            // Last resort: ~/.config
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("par-term");

    config_dir.join(default_filename)
}

// ---------------------------------------------------------------------------
// Atomic write helper
// ---------------------------------------------------------------------------

/// Atomically write a JSON payload to a path.
///
/// Creates parent directories if needed, writes to a `.json.tmp` temp file
/// with restricted permissions, then renames into place.
pub fn write_json_atomic<T: Serialize>(payload: &T, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(format!(
            "Failed to create parent directory {}: {e}",
            parent.display()
        ));
    }

    let temp_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(payload).map_err(|e| e.to_string())?;
    // Create temp file with restricted permissions from creation (0o600 on Unix)
    open_restricted_write(&temp_path)
        .and_then(|mut f| f.write_all(&bytes))
        .map_err(|e| {
            format!(
                "Failed to write temp file {}: {e}",
                temp_path.to_string_lossy()
            )
        })?;
    std::fs::rename(&temp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Failed to rename temp file to {}: {e}",
            path.to_string_lossy()
        )
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Screenshot response reader
// ---------------------------------------------------------------------------

/// Read and parse a screenshot response file, returning `None` for empty files.
pub fn try_read_screenshot_response(
    path: &Path,
) -> Result<Option<crate::TerminalScreenshotResponse>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if content.trim().is_empty() {
        return Ok(None);
    }
    let resp = serde_json::from_str::<crate::TerminalScreenshotResponse>(&content)
        .map_err(|e| e.to_string())?;
    Ok(Some(resp))
}
