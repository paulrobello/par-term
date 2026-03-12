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
    /// When true (default), dangerous actions show a confirmation dialog before executing.
    /// When false, they execute automatically (with rate-limit + denylist guards still applied).
    ///
    /// Previously named `require_user_action`. The old name is accepted as an alias for
    /// backward compatibility with existing config files.
    #[serde(
        default = "crate::defaults::bool_true",
        alias = "require_user_action"
    )]
    pub prompt_before_run: bool,
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
    /// Open a new pane (horizontal or vertical split) and optionally run a command in it.
    SplitPane {
        direction: TriggerSplitDirection,
        #[serde(default)]
        command: Option<SplitPaneCommand>,
        #[serde(default = "crate::defaults::bool_true")]
        focus_new_pane: bool,
        #[serde(default)]
        target: TriggerSplitTarget,
    },
}

/// Split orientation for a new pane created by a trigger action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSplitDirection {
    Horizontal, // new pane below (panes stacked vertically)
    Vertical,   // new pane to the right (side by side)
}

/// Which pane to split when a SplitPane trigger fires.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSplitTarget {
    #[default]
    Active,  // split the currently focused pane
    Source,  // split the pane whose PTY output matched (degrades to Active for now)
}

/// How to run a command in the newly created pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SplitPaneCommand {
    /// Send text to the shell with a trailing newline. Best-effort; shell must be running.
    SendText {
        text: String,
        #[serde(default = "default_split_send_delay")]
        delay_ms: u64,
    },
    /// Launch the pane with this command instead of the login shell.
    InitialCommand {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

fn default_split_send_delay() -> u64 {
    200
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
    /// Dangerous actions: `RunCommand`, `SendText`, `SplitPane`
    /// Safe actions: `Highlight`, `Notify`, `MarkLine`, `SetVariable`, `PlaySound`, `Prettify`
    pub fn is_dangerous(&self) -> bool {
        matches!(
            self,
            Self::RunCommand { .. } | Self::SendText { .. } | Self::SplitPane { .. }
        )
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
            Self::SplitPane {
                direction,
                command,
                focus_new_pane,
                target,
            } => {
                // Use fully-qualified core paths to avoid shadowing the config-side types.
                let core_direction = match direction {
                    crate::automation::TriggerSplitDirection::Horizontal => {
                        par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal
                    }
                    crate::automation::TriggerSplitDirection::Vertical => {
                        par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical
                    }
                };
                let core_command = command.map(|c| match c {
                    crate::automation::SplitPaneCommand::SendText { text, delay_ms } => {
                        par_term_emu_core_rust::terminal::TriggerSplitCommand::SendText {
                            text,
                            delay_ms,
                        }
                    }
                    crate::automation::SplitPaneCommand::InitialCommand { command, args } => {
                        par_term_emu_core_rust::terminal::TriggerSplitCommand::InitialCommand {
                            command,
                            args,
                        }
                    }
                });
                let core_target = match target {
                    crate::automation::TriggerSplitTarget::Active => {
                        par_term_emu_core_rust::terminal::TriggerSplitTarget::Active
                    }
                    crate::automation::TriggerSplitTarget::Source => {
                        par_term_emu_core_rust::terminal::TriggerSplitTarget::Source
                    }
                };
                TriggerAction::SplitPane {
                    direction: core_direction,
                    command: core_command,
                    focus_new_pane,
                    target: core_target,
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
///
/// # Known Bypass Vectors
///
/// This denylist uses **substring matching only** and is **not a security boundary**.
/// It is a best-effort heuristic to catch obviously dangerous patterns, not a
/// comprehensive security solution. Known bypass techniques include (but are not
/// limited to):
///
/// - Encoding/obfuscation: `$'\x72\x6d' -rf /`
/// - Variable indirection: `CMD=rm; $CMD -rf /`
/// - Path variations: `/usr/bin/rm -rf /` vs `rm -rf /`
/// - Argument reordering: `rm / -rf`
///
/// **The recommended security setting is `prompt_before_run: true` (the default).**
/// The denylist is a secondary defense layer for triggers that opt in to
/// `prompt_before_run: false`, not a substitute for user confirmation.
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

/// Command wrapper prefixes that are used to bypass simple command-name checks.
///
/// Patterns like `/usr/bin/env rm -rf /` or `sh -c "rm -rf /"` can bypass a
/// check that only looks at the first word of the command. These prefixes are
/// detected in [`check_command_denylist`] so that the full reconstructed command
/// string (after stripping the wrapper) is re-checked against
/// [`DENIED_COMMAND_PATTERNS`].
///
/// # Limitations
///
/// Detecting wrappers via substring matching is still bypassable (e.g. through
/// quoting, encoding, or unusual shell invocations). This is a best-effort
/// heuristic only. **Use `prompt_before_run: true` for real protection.**
const BYPASS_WRAPPER_PATTERNS: &[&str] = &[
    "env ",
    "/usr/bin/env ",
    "/bin/env ",
    "sh -c ",
    "bash -c ",
    "zsh -c ",
    "fish -c ",
    "dash -c ",
    "ksh -c ",
    "csh -c ",
    "tcsh -c ",
];

/// Pipe-to-shell patterns checked with word-boundary awareness.
/// These match `| bash`, `|bash`, `| sh`, `|sh` only when `bash`/`sh`
/// appear as whole words (not as part of longer words like "polish").
const PIPE_SHELL_TARGETS: &[&str] = &["bash", "sh", "zsh", "fish", "dash", "ksh"];

/// Check if a command string matches any denied pattern.
///
/// The check is case-insensitive and looks for substring matches.
/// Checks both the full joined command and each individual argument
/// (since shell evaluation like `bash -c "curl ... | bash"` puts the
/// dangerous content in a single arg).
///
/// # SECURITY WARNING
///
/// This function implements a **best-effort heuristic denylist**, not a security
/// boundary. The following bypass techniques are known to exist and are **not**
/// fully mitigated by this check:
///
/// - **Shell wrapper bypass**: `sh -c "rm -rf /"`, `bash -c "..."`, `zsh -c "..."`
///   (partially mitigated by [`BYPASS_WRAPPER_PATTERNS`])
/// - **env wrapper bypass**: `/usr/bin/env rm -rf /` (partially mitigated)
/// - **Encoding/obfuscation**: `$'\x72\x6d' -rf /` — the raw bytes bypass substring matching
/// - **Variable indirection**: `CMD=rm; $CMD -rf /` — shell variables are opaque
/// - **Path variations**: `/bin/rm -rf /` vs `rm -rf /` (full absolute paths may bypass)
/// - **Argument reordering**: `rm / -rf` — patterns that depend on argument order
/// - **Commands not on the list**: Anything not explicitly enumerated is allowed
///
/// **The recommended and default setting is `prompt_before_run: true`.**
/// When `prompt_before_run` is `false`, the denylist is the only automated
/// protection against malicious terminal output triggering dangerous commands.
/// For any trigger that uses `prompt_before_run: false`, users should
/// carefully audit the command and args to ensure they cannot be exploited.
///
/// # Why Not Shell Parsing?
///
/// A truly robust solution would require **full shell parsing** of the command string:
/// expanding variables, resolving aliases, decoding escape sequences, and evaluating
/// subshells before checking against any policy. Implementing a complete POSIX shell
/// parser is a significant undertaking and would itself introduce a large attack surface.
/// This function intentionally does not attempt shell parsing and instead relies on
/// `prompt_before_run: true` as the primary security control. The denylist exists
/// only as a best-effort secondary guard.
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
    let mut check_strings = vec![full_command.clone()];
    for arg in args {
        let lowered = arg.to_lowercase();
        if !lowered.is_empty() {
            check_strings.push(lowered);
        }
    }

    // Check for bypass wrapper patterns (env, sh -c, bash -c, etc.).
    // When a wrapper is detected, also add the de-wrapped remainder to the
    // check list so that patterns like `/usr/bin/env rm -rf /` are caught
    // by the standard DENIED_COMMAND_PATTERNS check below.
    //
    // NOTE: This is a best-effort mitigation only. Sufficiently obfuscated
    // wrappers (e.g. using variable indirection, encoding, or unusual quoting)
    // will still bypass this check. See the SECURITY WARNING on this function.
    for wrapper in BYPASS_WRAPPER_PATTERNS {
        let normalized_wrapper = wrapper.to_lowercase();
        if full_command.starts_with(&normalized_wrapper) {
            let remainder = full_command[normalized_wrapper.len()..].trim().to_string();
            if !remainder.is_empty() {
                check_strings.push(remainder);
            }
            // Also flag any `sh -c`, `bash -c`, etc. usage directly as denied,
            // since shell invocation with `-c` allows arbitrary code execution
            // and cannot be safely filtered by substring matching alone.
            if normalized_wrapper.contains(" -c ") || normalized_wrapper.ends_with(" -c") {
                return Some("shell -c wrapper");
            }
        }
        // Also check each individual arg for wrapper patterns, since the
        // wrapper might appear in an argument (e.g., `sudo bash -c "..."`)
        for arg in args {
            let lowered_arg = arg.to_lowercase();
            if lowered_arg.starts_with(&normalized_wrapper)
                && (normalized_wrapper.contains(" -c ") || normalized_wrapper.ends_with(" -c"))
            {
                return Some("shell -c wrapper");
            }
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
                    "zsh" => Some("| zsh"),
                    "fish" => Some("| fish"),
                    "dash" => Some("| dash"),
                    "ksh" => Some("| ksh"),
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

/// Emit a security warning when a trigger is configured with `prompt_before_run: false`.
///
/// Called during config load for any trigger with `prompt_before_run: false` that contains
/// dangerous actions. With `prompt_before_run: false`, dangerous actions execute automatically
/// without user confirmation; only the rate-limiter and denylist provide protection.
pub fn warn_prompt_before_run_false(trigger_name: &str) {
    eprintln!(
        "[par-term SECURITY WARNING] Trigger '{trigger_name}' has `prompt_before_run: false`.\n\
         This allows terminal output to directly trigger RunCommand/SendText/SplitPane actions\n\
         without confirmation. The command denylist provides only limited protection.\n\
         Only use this setting if you fully trust the configured commands and environment.\n\
         Recommendation: set `prompt_before_run: true` (the default) to require confirmation."
    );
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

#[cfg(test)]
mod split_pane_tests {
    use super::*;

    #[test]
    fn test_split_pane_config_deserialize_send_text() {
        let yaml = r#"
type: split_pane
direction: horizontal
command:
  type: send_text
  text: "tail -f build.log"
  delay_ms: 300
focus_new_pane: true
target: active
"#;
        let action: TriggerActionConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(matches!(action, TriggerActionConfig::SplitPane { .. }));
        assert!(action.is_dangerous());
    }

    #[test]
    fn test_split_pane_config_deserialize_initial_command() {
        let yaml = r#"
type: split_pane
direction: vertical
command:
  type: initial_command
  command: htop
  args: []
focus_new_pane: false
target: source
"#;
        let action: TriggerActionConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(matches!(
            action,
            TriggerActionConfig::SplitPane {
                direction: TriggerSplitDirection::Vertical,
                focus_new_pane: false,
                target: TriggerSplitTarget::Source,
                ..
            }
        ));
    }

    #[test]
    fn test_split_pane_defaults() {
        let yaml = r#"
type: split_pane
direction: horizontal
"#;
        let action: TriggerActionConfig = serde_yaml_ng::from_str(yaml).unwrap();
        if let TriggerActionConfig::SplitPane {
            command,
            focus_new_pane,
            target,
            ..
        } = action
        {
            assert!(command.is_none());
            assert!(focus_new_pane); // defaults true
            assert_eq!(target, TriggerSplitTarget::Active); // defaults Active
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_send_text_default_delay() {
        let yaml = r#"type: send_text
text: "hello"
"#;
        let cmd: SplitPaneCommand = serde_yaml_ng::from_str(yaml).unwrap();
        if let SplitPaneCommand::SendText { delay_ms, .. } = cmd {
            assert_eq!(delay_ms, 200);
        }
    }
}
