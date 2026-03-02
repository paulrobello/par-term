//! Diagram renderer configuration.

use serde::{Deserialize, Serialize};

use super::{default_diagrams_priority, default_true};

/// Diagram renderer with engine selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagramRendererConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_diagrams_priority")]
    pub priority: i32,

    /// Rendering engine: "auto" (default — tries native → local → kroki),
    /// "native" (pure-Rust mermaid only), "local" (CLI tools), "kroki" (API),
    /// or "text_fallback" (source display only).
    #[serde(default)]
    pub engine: Option<String>,

    /// Kroki server URL (only used when engine = "kroki").
    #[serde(default)]
    pub kroki_server: Option<String>,
}

impl Default for DiagramRendererConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_diagrams_priority(),
            engine: None,
            kroki_server: None,
        }
    }
}
