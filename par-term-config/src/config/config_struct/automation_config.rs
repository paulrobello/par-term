//! Trigger, coprocess and observer-script definitions.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Regex triggers, coprocess definitions and external observer scripts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutomationConfig {
    /// Regex trigger definitions that match terminal output and fire actions
    #[serde(default)]
    pub triggers: Vec<crate::automation::TriggerConfig>,

    /// Coprocess definitions for piped subprocess management
    #[serde(default)]
    pub coprocesses: Vec<crate::automation::CoprocessDefConfig>,

    /// External observer script definitions
    #[serde(default)]
    pub scripts: Vec<crate::scripting::ScriptConfig>,

    /// Plugin state entries (O2 Phase 3); empty until the user enables one.
    #[serde(default)]
    pub plugins: Vec<PluginStateConfig>,
}

/// Per-plugin persisted state (one `plugins:` list entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateConfig {
    /// Manifest id; must match a discovered plugin.
    pub id: String,
    /// Whether the host may run this plugin. Defaults to false: plugins are
    /// land-disabled, so a dropped-in plugin directory does nothing until
    /// the Settings layer flips this bit.
    #[serde(default = "default_plugin_disabled")]
    pub enabled: bool,
    /// Persisted settings values; validated against the manifest schema at
    /// apply time, not parse time.
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
    /// Status-bar section placement override; the manifest default is used
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<crate::status_bar::StatusBarSection>,
}

/// `PluginStateConfig::enabled` serde seed: plugins land disabled.
fn default_plugin_disabled() -> bool {
    false
}

impl Default for PluginStateConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: default_plugin_disabled(),
            settings: HashMap::new(),
            section: None,
        }
    }
}
