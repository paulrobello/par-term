//! File system operation parameter types for the ACP protocol.
//!
//! Covers `fs/readTextFile`, `fs/writeTextFile`, `fs/listDirectory`, and `fs/find`
//! RPC calls from the agent to the host.

use serde::{Deserialize, Serialize};

/// Parameters for the `fs/readTextFile` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadParams {
    #[serde(default)]
    pub session_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

/// Parameters for the `fs/writeTextFile` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteParams {
    #[serde(default)]
    pub session_id: String,
    pub path: String,
    pub content: String,
}

/// Parameters for the `fs/listDirectory` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsListDirectoryParams {
    #[serde(default)]
    pub session_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Parameters for the `fs/find` RPC call from agent to host.
///
/// This is a par-term extension (not part of the core ACP spec) that provides
/// recursive glob-based file search, similar to Claude Code's built-in Glob tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsFindParams {
    #[serde(default)]
    pub session_id: String,
    pub path: String,
    pub pattern: String,
}
