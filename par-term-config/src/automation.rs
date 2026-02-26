//! Configuration types for triggers and coprocesses.

use serde::{Deserialize, Serialize};

/// Scope for a prettify trigger action.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrettifyScope {
    /// Apply to the matched line only.
    Line,
    /// Apply to a delimited block (start pattern → block_end pattern).
    Block,
    /// Apply to the entire command output containing the match.
    #[default]
    CommandOutput,
}

/// Payload packed into a Notify relay for prettify trigger actions.
///
/// When the core trigger system fires, the frontend intercepts Notify actions
/// with the `__prettify__` title prefix and deserializes this from the message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrettifyRelayPayload {
    pub format: String,
    pub scope: PrettifyScope,
    #[serde(default)]
    pub block_end: Option<String>,
    #[serde(default)]
    pub sub_format: Option<String>,
    #[serde(default)]
    pub command_filter: Option<String>,
}

/// Magic label prefix used to relay prettify actions through the core MarkLine system.
/// Using MarkLine (instead of Notify) because it carries the matched `row`.
pub const PRETTIFY_RELAY_PREFIX: &str = "__prettify__";

/// A trigger definition that matches terminal output and fires actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerConfig {
    pub name: String,
    pub pattern: String,
    #[serde(default = "crate::defaults::bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub actions: Vec<TriggerActionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerActionConfig {
    Highlight {
        #[serde(default)]
        fg: Option<[u8; 3]>,
        #[serde(default)]
        bg: Option<[u8; 3]>,
        #[serde(default = "default_highlight_duration")]
        duration_ms: u64,
    },
    Notify {
        title: String,
        message: String,
    },
    MarkLine {
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        color: Option<[u8; 3]>,
    },
    SetVariable {
        name: String,
        value: String,
    },
    RunCommand {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    PlaySound {
        #[serde(default)]
        sound_id: String,
        #[serde(default = "default_volume")]
        volume: u8,
    },
    SendText {
        text: String,
        #[serde(default)]
        delay_ms: u64,
    },
    /// Invoke a specific prettifier renderer on matched content.
    Prettify {
        /// Which renderer to invoke (e.g., "json", "markdown", "none").
        format: String,
        /// What scope to apply the renderer to.
        #[serde(default)]
        scope: PrettifyScope,
        /// Optional regex for block end (for block-scoped rendering).
        #[serde(default)]
        block_end: Option<String>,
        /// Optional sub-format (e.g., "plantuml" for diagrams).
        #[serde(default)]
        sub_format: Option<String>,
        /// Optional regex to filter by preceding command.
        #[serde(default)]
        command_filter: Option<String>,
    },
}

/// Policy for restarting a coprocess when it exits
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    /// Never restart (default)
    #[default]
    Never,
    /// Always restart regardless of exit code
    Always,
    /// Restart only on non-zero exit code
    OnFailure,
}

impl RestartPolicy {
    /// All available restart policies for UI dropdowns
    pub fn all() -> &'static [RestartPolicy] {
        &[Self::Never, Self::Always, Self::OnFailure]
    }

    /// Human-readable display name
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::Always => "Always",
            Self::OnFailure => "On Failure",
        }
    }

    /// Convert to core library RestartPolicy
    pub fn to_core(self) -> par_term_emu_core_rust::coprocess::RestartPolicy {
        match self {
            Self::Never => par_term_emu_core_rust::coprocess::RestartPolicy::Never,
            Self::Always => par_term_emu_core_rust::coprocess::RestartPolicy::Always,
            Self::OnFailure => par_term_emu_core_rust::coprocess::RestartPolicy::OnFailure,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoprocessDefConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_terminal_output: bool,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub restart_delay_ms: u64,
}

fn default_highlight_duration() -> u64 {
    5000
}

fn default_volume() -> u8 {
    50
}

impl TriggerActionConfig {
    /// Convert to core library TriggerAction
    pub fn to_core_action(&self) -> par_term_emu_core_rust::terminal::TriggerAction {
        use par_term_emu_core_rust::terminal::TriggerAction;
        match self.clone() {
            Self::Highlight {
                fg,
                bg,
                duration_ms,
            } => TriggerAction::Highlight {
                fg: fg.map(|c| (c[0], c[1], c[2])),
                bg: bg.map(|c| (c[0], c[1], c[2])),
                duration_ms,
            },
            Self::Notify { title, message } => TriggerAction::Notify { title, message },
            Self::MarkLine { label, color } => TriggerAction::MarkLine {
                label,
                color: color.map(|c| (c[0], c[1], c[2])),
            },
            Self::SetVariable { name, value } => TriggerAction::SetVariable { name, value },
            Self::RunCommand { command, args } => TriggerAction::RunCommand { command, args },
            Self::PlaySound { sound_id, volume } => TriggerAction::PlaySound { sound_id, volume },
            Self::SendText { text, delay_ms } => TriggerAction::SendText { text, delay_ms },
            Self::Prettify {
                format,
                scope,
                block_end,
                sub_format,
                command_filter,
            } => {
                // Relay through the core MarkLine mechanism. MarkLine carries the
                // matched `row`, which we need for scope handling. The frontend
                // intercepts ActionResult::MarkLine with the __prettify__ label
                // prefix and dispatches to the PrettifierPipeline.
                let payload = PrettifyRelayPayload {
                    format,
                    scope,
                    block_end,
                    sub_format,
                    command_filter,
                };
                TriggerAction::MarkLine {
                    label: Some(format!(
                        "{}{}",
                        PRETTIFY_RELAY_PREFIX,
                        serde_json::to_string(&payload).unwrap_or_default()
                    )),
                    color: None,
                }
            }
        }
    }
}
