//! [`AiInspectorConfig`]: AI Inspector panel settings.

use crate::config::acp::CustomAcpAgentConfig;
use serde::{Deserialize, Serialize};

/// Configuration for the AI Inspector side panel.
///
/// Extracted from the monolithic `Config` struct via `#[serde(flatten)]`.
/// All fields that were previously `ai_inspector_*` on `Config` are now
/// grouped here, keeping the YAML format fully backward-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiInspectorConfig {
    /// Enable AI Inspector side panel
    #[serde(default = "default_ai_inspector_enabled")]
    pub ai_inspector_enabled: bool,

    /// Open the AI Inspector panel automatically on startup
    #[serde(default = "default_ai_inspector_open_on_startup")]
    pub ai_inspector_open_on_startup: bool,

    /// Width of the AI Inspector panel in pixels
    #[serde(default = "default_ai_inspector_width")]
    pub ai_inspector_width: f32,

    /// Default capture scope: "visible", "scrollback", or "selection"
    #[serde(default = "default_ai_inspector_default_scope")]
    pub ai_inspector_default_scope: String,

    /// View mode for inspector results: "cards" or "raw"
    #[serde(default = "default_ai_inspector_view_mode")]
    pub ai_inspector_view_mode: String,

    /// Automatically refresh inspector when terminal content changes
    #[serde(default = "default_ai_inspector_live_update")]
    pub ai_inspector_live_update: bool,

    /// Show semantic zone overlays on terminal content
    #[serde(default = "default_ai_inspector_show_zones")]
    pub ai_inspector_show_zones: bool,

    /// AI agent identifier for inspector queries
    #[serde(default = "default_ai_inspector_agent")]
    pub ai_inspector_agent: String,

    /// Automatically launch AI agent when inspector opens
    #[serde(default = "default_ai_inspector_auto_launch")]
    pub ai_inspector_auto_launch: bool,

    /// Automatically include terminal context with AI queries
    #[serde(default = "default_ai_inspector_auto_context")]
    pub ai_inspector_auto_context: bool,

    /// Maximum number of terminal lines to include as AI context
    #[serde(default = "default_ai_inspector_context_max_lines")]
    pub ai_inspector_context_max_lines: usize,

    /// Automatically approve AI-suggested actions without confirmation
    #[serde(default = "default_ai_inspector_auto_approve")]
    pub ai_inspector_auto_approve: bool,

    /// Allow the AI agent to write input to the terminal (drive terminal)
    #[serde(default = "default_ai_inspector_agent_terminal_access")]
    pub ai_inspector_agent_terminal_access: bool,

    /// Allow the AI agent to request terminal screenshots (permission-gated per request)
    #[serde(default = "default_ai_inspector_agent_screenshot_access")]
    pub ai_inspector_agent_screenshot_access: bool,

    /// Additional ACP agents defined directly in `config.yaml`.
    ///
    /// Entries here are merged into discovered agents and override agents with
    /// the same `identity`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ai_inspector_custom_agents: Vec<CustomAcpAgentConfig>,
}

// ── Default value functions ────────────────────────────────────────────────

fn default_ai_inspector_enabled() -> bool {
    true
}

fn default_ai_inspector_open_on_startup() -> bool {
    false
}

fn default_ai_inspector_width() -> f32 {
    300.0
}

fn default_ai_inspector_default_scope() -> String {
    "visible".to_string()
}

fn default_ai_inspector_view_mode() -> String {
    "tree".to_string()
}

fn default_ai_inspector_live_update() -> bool {
    false
}

fn default_ai_inspector_show_zones() -> bool {
    true
}

fn default_ai_inspector_agent() -> String {
    "claude.com".to_string()
}

fn default_ai_inspector_auto_launch() -> bool {
    false
}

fn default_ai_inspector_auto_context() -> bool {
    false
}

fn default_ai_inspector_context_max_lines() -> usize {
    200
}

fn default_ai_inspector_auto_approve() -> bool {
    false
}

fn default_ai_inspector_agent_terminal_access() -> bool {
    false
}

fn default_ai_inspector_agent_screenshot_access() -> bool {
    true
}

impl Default for AiInspectorConfig {
    fn default() -> Self {
        Self {
            ai_inspector_enabled: default_ai_inspector_enabled(),
            ai_inspector_open_on_startup: default_ai_inspector_open_on_startup(),
            ai_inspector_width: default_ai_inspector_width(),
            ai_inspector_default_scope: default_ai_inspector_default_scope(),
            ai_inspector_view_mode: default_ai_inspector_view_mode(),
            ai_inspector_live_update: default_ai_inspector_live_update(),
            ai_inspector_show_zones: default_ai_inspector_show_zones(),
            ai_inspector_agent: default_ai_inspector_agent(),
            ai_inspector_auto_launch: default_ai_inspector_auto_launch(),
            ai_inspector_auto_context: default_ai_inspector_auto_context(),
            ai_inspector_context_max_lines: default_ai_inspector_context_max_lines(),
            ai_inspector_auto_approve: default_ai_inspector_auto_approve(),
            ai_inspector_agent_terminal_access: default_ai_inspector_agent_terminal_access(),
            ai_inspector_agent_screenshot_access: default_ai_inspector_agent_screenshot_access(),
            ai_inspector_custom_agents: Vec::new(),
        }
    }
}
