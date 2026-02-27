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
    /// When true (the default and **recommended** setting), dangerous actions
    /// (`RunCommand`, `SendText`) are suppressed when triggered solely by
    /// passive terminal output. This prevents malicious terminal output
    /// (e.g., `cat malicious_file`) from executing arbitrary commands via
    /// pattern matching.
    ///
    /// Safe actions (`Highlight`, `Notify`, `MarkLine`, `SetVariable`,
    /// `PlaySound`, `Prettify`) always fire regardless of this flag.
    ///
    /// # SECURITY WARNING
    ///
    /// Setting this to `false` allows terminal output to directly trigger
    /// `RunCommand` and `SendText` actions. When `false`, the only automated
    /// protection is the command denylist (`check_command_denylist`), which
    /// uses **substring matching only** and can be bypassed by:
    ///
    /// - Shell wrappers: `sh -c "..."`, `bash -c "..."` (partially mitigated)
    /// - Environment wrappers: `/usr/bin/env <cmd>` (partially mitigated)
    /// - Encoding/obfuscation, variable indirection, path variations, etc.
    ///
    /// **Only set `require_user_action: false` if you fully trust the commands
    /// configured and the environment in which the trigger will fire.**
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
/// **The recommended security setting is `require_user_action: true` (the default).**
/// The denylist is a secondary defense layer for triggers that opt in to
/// `require_user_action: false`, not a substitute for user confirmation.
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
/// heuristic only. **Use `require_user_action: true` for real protection.**
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
/// **The recommended and default setting is `require_user_action: true`.**
/// When `require_user_action` is `false`, the denylist is the only automated
/// protection against malicious terminal output triggering dangerous commands.
/// For any trigger that uses `require_user_action: false`, users should
/// carefully audit the command and args to ensure they cannot be exploited.
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

/// Emit a security warning when a trigger is configured with `require_user_action: false`.
///
/// This function should be called during config load or validation for any trigger
/// that has `require_user_action: false` **and** contains dangerous actions
/// (`RunCommand` or `SendText`). It writes a prominent warning to stderr so that
/// users are aware of the security implications.
///
/// # Security Model
///
/// When `require_user_action` is `false`, terminal output pattern matches can
/// directly execute commands or send text to the PTY. The only remaining automated
/// protection is the command denylist, which is a best-effort heuristic and can
/// be bypassed. Users should treat `require_user_action: false` as an advanced
/// opt-in feature and audit all associated commands carefully.
///
/// **Recommendation**: Keep `require_user_action: true` (the default) unless you
/// have a specific use case that requires output-driven automation and you fully
/// understand and accept the security trade-offs.
pub fn warn_require_user_action_false(trigger_name: &str) {
    eprintln!(
        "[par-term SECURITY WARNING] Trigger '{trigger_name}' has `require_user_action: false`.\n\
         This allows terminal output to directly trigger RunCommand/SendText actions.\n\
         The command denylist provides only limited protection and can be bypassed.\n\
         Only use this setting if you fully trust the configured commands and environment.\n\
         Recommendation: set `require_user_action: true` (the default) for safety."
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
