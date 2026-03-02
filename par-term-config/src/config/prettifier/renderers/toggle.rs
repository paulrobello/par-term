//! Basic renderer toggle types used by multiple renderers.

use serde::{Deserialize, Serialize};

use super::{default_priority, default_true};

/// Enable/disable and priority for a renderer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RendererToggle {
    /// Whether this renderer is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Priority (higher = checked first in detection).
    #[serde(default = "default_priority")]
    pub priority: i32,
}

impl Default for RendererToggle {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_priority(),
        }
    }
}

/// Profile-level override for a single renderer's toggle.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RendererToggleOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}
