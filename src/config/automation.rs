//! Configuration types for triggers and coprocesses.

use serde::{Deserialize, Serialize};

/// A trigger definition that matches terminal output and fires actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerConfig {
    pub name: String,
    pub pattern: String,
    #[serde(default = "super::defaults::bool_true")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoprocessDefConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "super::defaults::bool_true")]
    pub copy_terminal_output: bool,
}

fn default_highlight_duration() -> u64 {
    5000
}

fn default_volume() -> u8 {
    50
}

impl TriggerActionConfig {
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
            Self::MarkLine { label } => TriggerAction::MarkLine { label },
            Self::SetVariable { name, value } => TriggerAction::SetVariable { name, value },
            Self::RunCommand { command, args } => TriggerAction::RunCommand { command, args },
            Self::PlaySound { sound_id, volume } => TriggerAction::PlaySound { sound_id, volume },
            Self::SendText { text, delay_ms } => TriggerAction::SendText { text, delay_ms },
        }
    }
}
