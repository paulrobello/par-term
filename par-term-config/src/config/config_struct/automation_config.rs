//! Trigger, coprocess and observer-script definitions.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

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
}
