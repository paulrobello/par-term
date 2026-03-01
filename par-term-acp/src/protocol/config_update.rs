//! Config update RPC types for the ACP protocol.
//!
//! Covers the `config/update` RPC call from agent to host.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Parameters for the `config/update` RPC call from agent to host.
///
/// Allows the agent to update par-term configuration settings directly,
/// bypassing the config.yaml file to avoid race conditions with par-term's
/// own config saves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateParams {
    #[serde(default)]
    pub session_id: Option<String>,
    /// Map of config key -> value to update.
    /// Keys use snake_case matching config.yaml field names.
    /// Values must be the correct JSON type for the field.
    pub updates: HashMap<String, Value>,
}
