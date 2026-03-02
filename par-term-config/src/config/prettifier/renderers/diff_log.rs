//! Diff renderer configuration.

use serde::{Deserialize, Serialize};

use super::{default_priority, default_true};

/// Diff renderer with side-by-side option.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffRendererConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Display mode: "unified" or "side_by_side".
    #[serde(default)]
    pub display_mode: Option<String>,
}

impl Default for DiffRendererConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_priority(),
            display_mode: None,
        }
    }
}
