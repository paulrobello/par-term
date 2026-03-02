//! Binary path resolution for the ACP harness.
//!
//! Resolves the path to the `par-term` binary used to host the MCP server.
//! Checks the explicit CLI override first, then the directory of the current
//! executable, and finally the system PATH.

use std::path::{Path, PathBuf};

use par_term_acp::agents::resolve_binary_in_path;

/// Resolve the `par-term` binary to use for hosting the MCP server.
///
/// Resolution order:
/// 1. `explicit` — if provided, return it immediately (no existence check).
/// 2. Same directory as the current executable — used in bundled/installed layouts.
/// 3. System `PATH` lookup via `resolve_binary_in_path`.
///
/// Returns an error if none of the above succeed.
pub fn resolve_par_term_binary(
    explicit: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    if let Ok(current) = std::env::current_exe()
        && let Some(dir) = current.parent()
    {
        let candidate = dir.join(if cfg!(windows) {
            "par-term.exe"
        } else {
            "par-term"
        });
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if let Some(path) = resolve_binary_in_path("par-term") {
        return Ok(path);
    }

    Err("Could not find `par-term` binary. Pass --par-term-bin /path/to/par-term".into())
}
