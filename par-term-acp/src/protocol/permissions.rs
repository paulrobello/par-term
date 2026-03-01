//! Permission request/response types for the ACP protocol.
//!
//! Covers `session/request_permission` RPC calls from agent to host.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for the `session/request_permission` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    #[serde(default)]
    pub session_id: String,
    pub tool_call: Value,
    pub options: Vec<PermissionOption>,
}

/// A single permission option the host can choose from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// The host's response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionResponse {
    pub outcome: PermissionOutcome,
}

/// The chosen outcome of a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOutcome {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
}
