//! Configuration types for triggers, coprocesses, and trigger security.

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
    /// When true (the default), dangerous actions (`RunCommand`, `SendText`)
    /// are suppressed when triggered solely by passive terminal output.
    /// This prevents malicious terminal output (e.g., `cat malicious_file`)
    /// from executing arbitrary commands via pattern matching.
    ///
    /// Safe actions (`Highlight`, `Notify`, `MarkLine`, `SetVariable`,
    /// `PlaySound`, `Prettify`) always fire regardless of this flag.
    #[serde(default = "crate::defaults::bool_true")]
    pub require_user_action: bool,
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
    /// Returns true if this action is considered dangerous when triggered by
    /// passive terminal output (i.e., without explicit user interaction).
    ///
    /// Dangerous actions: `RunCommand`, `SendText`
    /// Safe actions: `Highlight`, `Notify`, `MarkLine`, `SetVariable`, `PlaySound`, `Prettify`
    pub fn is_dangerous(&self) -> bool {
        matches!(self, Self::RunCommand { .. } | Self::SendText { .. })
    }

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

// ============================================================================
// Trigger Security — command denylist and rate limiting
// ============================================================================

/// Denied command patterns for `RunCommand` trigger actions.
///
/// These patterns are checked against the command string (command + args joined).
/// A match means the command is blocked from execution.
/// Simple substring-matched denied patterns.
const DENIED_COMMAND_PATTERNS: &[&str] = &[
    // Destructive file operations
    "rm -rf /",
    "rm -rf ~",
    "rm -rf .",
    "mkfs.",
    "dd if=",
    // Shell evaluation
    "eval ",
    "exec ",
    // Credential/key exfiltration
    "ssh-add",
    ".ssh/id_",
    ".ssh/authorized_keys",
    ".gnupg/",
    // System manipulation
    "chmod 777",
    "chown root",
    "passwd",
    "sudoers",
];

/// Pipe-to-shell patterns checked with word-boundary awareness.
/// These match `| bash`, `|bash`, `| sh`, `|sh` only when `bash`/`sh`
/// appear as whole words (not as part of longer words like "polish").
const PIPE_SHELL_TARGETS: &[&str] = &["bash", "sh"];

/// Check if a command string matches any denied pattern.
///
/// The check is case-insensitive and looks for substring matches.
/// Checks both the full joined command and each individual argument
/// (since shell evaluation like `bash -c "curl ... | bash"` puts the
/// dangerous content in a single arg).
///
/// Returns `Some(pattern)` if denied, `None` if allowed.
pub fn check_command_denylist(command: &str, args: &[String]) -> Option<&'static str> {
    // Build full command string for pattern matching
    let full_command = if args.is_empty() {
        command.to_lowercase()
    } else {
        format!("{} {}", command, args.join(" ")).to_lowercase()
    };

    // Collect all strings to check: the full command and each individual arg.
    // Individual arg checking catches `bash -c "curl ... | bash"` where the
    // pipe-to-shell pattern is within a single argument.
    let mut check_strings = vec![full_command];
    for arg in args {
        let lowered = arg.to_lowercase();
        if !lowered.is_empty() {
            check_strings.push(lowered);
        }
    }

    // Check simple substring patterns
    for pattern in DENIED_COMMAND_PATTERNS {
        let normalized_pattern = pattern.to_lowercase();
        for check_str in &check_strings {
            if check_str.contains(&normalized_pattern) {
                return Some(pattern);
            }
        }
    }

    // Check pipe-to-shell patterns with word boundary awareness.
    // We look for `|<shell>` or `| <shell>` where <shell> is followed by
    // end-of-string or a non-alphanumeric character (word boundary).
    for check_str in &check_strings {
        for &shell in PIPE_SHELL_TARGETS {
            if check_pipe_to_shell(check_str, shell) {
                // Return a static description (we can't construct dynamic strings here)
                return match shell {
                    "bash" => Some("| bash"),
                    "sh" => Some("| sh"),
                    _ => Some("| <shell>"),
                };
            }
        }
    }

    None
}

/// Check if a string contains a pipe-to-shell pattern like `|bash` or `| sh`
/// with word boundary awareness to avoid false positives (e.g., "polish").
fn check_pipe_to_shell(s: &str, shell: &str) -> bool {
    // Check both `|<shell>` and `| <shell>` patterns
    for sep in &["|", "| "] {
        let pattern = format!("{}{}", sep, shell);
        if let Some(pos) = s.find(&pattern) {
            let end_pos = pos + pattern.len();
            // Check that `shell` is at a word boundary (end of string or followed by non-alphanumeric)
            if end_pos >= s.len() || !s.as_bytes()[end_pos].is_ascii_alphanumeric() {
                return true;
            }
        }
    }
    false
}

/// Rate limiter for output-triggered actions.
///
/// Tracks when actions last fired per trigger_id and enforces a minimum
/// interval between firings to prevent malicious output flooding.
pub struct TriggerRateLimiter {
    /// Map of trigger_id -> last fire time
    last_fire: std::collections::HashMap<u64, std::time::Instant>,
    /// Minimum interval between trigger firings (in milliseconds)
    min_interval_ms: u64,
}

/// Default minimum interval between output trigger firings (1 second).
const DEFAULT_TRIGGER_RATE_LIMIT_MS: u64 = 1000;

impl Default for TriggerRateLimiter {
    fn default() -> Self {
        Self {
            last_fire: std::collections::HashMap::new(),
            min_interval_ms: DEFAULT_TRIGGER_RATE_LIMIT_MS,
        }
    }
}

impl TriggerRateLimiter {
    /// Create a new rate limiter with a custom minimum interval.
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            last_fire: std::collections::HashMap::new(),
            min_interval_ms,
        }
    }

    /// Check if a trigger is allowed to fire. Returns true if allowed,
    /// false if rate-limited. Updates the last fire time on success.
    pub fn check_and_update(&mut self, trigger_id: u64) -> bool {
        let now = std::time::Instant::now();
        if let Some(last) = self.last_fire.get(&trigger_id) {
            let elapsed = now.duration_since(*last).as_millis() as u64;
            if elapsed < self.min_interval_ms {
                return false;
            }
        }
        self.last_fire.insert(trigger_id, now);
        true
    }

    /// Remove stale entries for triggers that haven't fired recently.
    /// Call periodically to prevent unbounded growth.
    pub fn cleanup(&mut self, max_age_secs: u64) {
        let now = std::time::Instant::now();
        let max_age = std::time::Duration::from_secs(max_age_secs);
        self.last_fire
            .retain(|_, last| now.duration_since(*last) < max_age);
    }
}
