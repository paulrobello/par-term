//! Recovery state and permission-option selection logic for the ACP harness.
//!
//! Tracks per-session event flags that drive the auto-recovery loop and
//! provides a helper for selecting the best permission option when
//! auto-approval is enabled.

use crate::PermissionOption;

// ---------------------------------------------------------------------------
// Event flags
// ---------------------------------------------------------------------------

/// Tracks notable agent events observed during a harness session.
///
/// These flags are consumed by the main event loop to decide whether and how
/// to issue automatic recovery follow-up prompts.
#[derive(Default)]
pub struct HarnessEventFlags {
    /// A tool call failed since the last prompt was sent.
    pub saw_failed_tool_since_prompt: bool,
    /// At least one tool call failed during the session.
    pub saw_any_failed_tool: bool,
    /// A `config_update` tool call succeeded during the session.
    pub saw_config_update: bool,
}

// ---------------------------------------------------------------------------
// Permission option selection
// ---------------------------------------------------------------------------

/// Choose the best permission option for auto-approval.
///
/// Returns `Some((option_id, label))` when `auto_approve` is `true` and at
/// least one option is available, or `None` to cancel.
pub fn choose_permission_option(
    options: &[PermissionOption],
    auto_approve: bool,
) -> Option<(&str, &str)> {
    if !auto_approve {
        return None;
    }

    let preferred = options.iter().find(|o| {
        matches!(
            o.kind.as_deref(),
            Some("allow") | Some("approve") | Some("accept")
        ) || o.name.to_ascii_lowercase().contains("allow")
            || o.name.to_ascii_lowercase().contains("approve")
            || o.name.to_ascii_lowercase().contains("accept")
    });

    let option = preferred.or_else(|| options.first())?;
    Some((option.option_id.as_str(), option.name.as_str()))
}
