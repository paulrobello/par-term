//! Typed error type for the MCP server's file-based IPC layer.
//!
//! Every fallible helper in [`crate::ipc`] returns [`IpcError`], so callers can
//! match on the failure category (directory creation, temp-file write, rename,
//! response read, response parse, …) instead of parsing a `String`. Tool
//! handlers convert these to MCP tool-error results at the JSON-RPC boundary
//! via their `Display` text.

use std::path::PathBuf;
use thiserror::Error;

/// Failure of an IPC file operation between the MCP server and the par-term
/// GUI app (config-update, screenshot, and shader-diagnostics handshakes).
///
/// Path-carrying variants hold the path the operation targeted so the message
/// shown to the agent names the file that failed.
#[derive(Debug, Error)]
pub enum IpcError {
    /// The parent directory of an IPC file could not be created.
    #[error("Failed to create directory {}: {source}", path.display())]
    CreateDir {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An IPC payload could not be serialized to JSON.
    #[error("Failed to serialize IPC payload: {0}")]
    Serialize(#[source] serde_json::Error),

    /// The temporary file of an atomic write could not be created or written.
    #[error("Failed to write temp file {}: {source}", path.display())]
    WriteTemp {
        /// Temporary file path (`<target>.json.tmp`).
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The temporary file could not be renamed over the target path. The
    /// temporary file has already been removed when this is returned.
    #[error("Failed to rename temp file to {}: {source}", path.display())]
    Rename {
        /// Final target path of the atomic write.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Owner-only permissions could not be applied to an IPC file (Unix only).
    #[error("Failed to set permissions on {}: {source}", path.display())]
    SetPermissions {
        /// File whose permissions could not be changed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An IPC response file exists but could not be read.
    #[error("Failed to read IPC response {}: {source}", path.display())]
    ReadResponse {
        /// Response file path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An IPC response file was read but did not contain the expected JSON.
    #[error("Failed to parse IPC response {}: {source}", path.display())]
    ParseResponse {
        /// Response file path.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },

    /// The screenshot fallback path (`PAR_TERM_SCREENSHOT_FALLBACK_PATH`)
    /// could not be canonicalized — typically because it does not exist.
    #[error("Fallback screenshot path {} cannot be canonicalized: {source}", path.display())]
    FallbackPathUnresolved {
        /// Path as supplied in the environment variable.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The screenshot fallback path resolves outside the allowed roots (the
    /// system temp directory and the par-term data directory), so reading it
    /// could exfiltrate an arbitrary file (SEC-003).
    #[error(
        "Fallback screenshot path {} is outside the allowed roots (system temp or par-term \
         data dir). Refusing to read arbitrary files.",
        path.display()
    )]
    FallbackPathOutsideRoots {
        /// Canonicalized path that was refused.
        path: PathBuf,
    },
}
