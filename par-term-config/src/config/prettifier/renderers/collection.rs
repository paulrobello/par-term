//! Aggregate renderer config collections — `RenderersConfig` and its profile override.

use serde::{Deserialize, Serialize};

use super::{DiagramRendererConfig, DiffRendererConfig, RendererToggle, RendererToggleOverride};

/// Per-renderer enable/disable and priority settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RenderersConfig {
    #[serde(default)]
    pub markdown: RendererToggle,
    #[serde(default)]
    pub json: RendererToggle,
    #[serde(default)]
    pub yaml: RendererToggle,
    #[serde(default)]
    pub toml: RendererToggle,
    #[serde(default)]
    pub xml: RendererToggle,
    #[serde(default)]
    pub csv: RendererToggle,
    #[serde(default)]
    pub diff: DiffRendererConfig,
    #[serde(default)]
    pub log: RendererToggle,
    #[serde(default)]
    pub diagrams: DiagramRendererConfig,
    #[serde(default)]
    pub sql_results: RendererToggle,
    #[serde(default)]
    pub stack_trace: RendererToggle,
}

/// Profile-level override for per-renderer settings.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RenderersConfigOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagrams: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_results: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<RendererToggleOverride>,
}
