//! Status bar format string parser and renderer.
//!
//! Parses and expands format strings like `[{session}] {windows}` into
//! actual status bar content based on the current tmux session state.
//!
//! ## Supported Variables
//!
//! - `{session}` - Session name
//! - `{windows}` - Window list with active marker (*)
//! - `{pane}` - Focused pane ID (e.g., "%0")
//! - `{time:FORMAT}` - Current time with strftime format (e.g., `{time:%H:%M}`)
//! - `{hostname}` - Machine hostname
//! - `{user}` - Current username
//!
//! ## Native tmux Format Support
//!
//! When `tmux_status_bar_use_native_format` is enabled, the status bar queries
//! tmux directly using `display-message -p '#{T:status-left}'` for the actual
//! expanded tmux format strings.

use crate::TmuxSession;
use chrono::Local;

/// Context for format string expansion.
/// Contains all the data needed to expand format variables.
pub struct FormatContext<'a> {
    /// The tmux session (optional - may not be connected)
    pub session: Option<&'a TmuxSession>,
    /// Session name (from notification, may differ from session.session_name())
    pub session_name: Option<&'a str>,
    /// Hostname (cached at startup)
    pub hostname: String,
    /// Username (cached at startup)
    pub username: String,
}

impl<'a> FormatContext<'a> {
    /// Create a new format context.
    pub fn new(session: Option<&'a TmuxSession>, session_name: Option<&'a str>) -> Self {
        Self {
            session,
            session_name,
            hostname: get_hostname(),
            username: get_username(),
        }
    }
}

/// Parse and expand a format string using the given context.
///
/// # Arguments
/// * `format` - The format string to expand (e.g., `[{session}] {windows}`)
/// * `ctx` - The context containing data for variable expansion
///
/// # Returns
/// The expanded string with all variables replaced.
///
/// # Example
/// ```ignore
/// let ctx = FormatContext::new(Some(&session), Some("dev"));
/// let result = expand_format("[{session}] {time:%H:%M}", &ctx);
/// // Result: "[dev] 14:30"
/// ```
pub fn expand_format(format: &str, ctx: &FormatContext) -> String {
    let mut result = String::with_capacity(format.len() * 2);
    let mut chars = format.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            // Start of a variable
            let mut var_name = String::new();
            let mut found_closing = false;

            // Collect characters until we find '}'
            while let Some(&next_c) = chars.peek() {
                if next_c == '}' {
                    chars.next(); // consume '}'
                    found_closing = true;
                    break;
                }
                var_name.push(chars.next().expect("peek confirmed a next char exists"));
            }

            if found_closing {
                // Expand the variable
                let expanded = expand_variable(&var_name, ctx);
                result.push_str(&expanded);
            } else {
                // No closing brace found - treat as literal
                result.push('{');
                result.push_str(&var_name);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Expand a single variable name to its value.
fn expand_variable(var_name: &str, ctx: &FormatContext) -> String {
    // Check for time with format specifier: time:FORMAT
    if let Some(time_format) = var_name.strip_prefix("time:") {
        return expand_time(time_format);
    }

    match var_name {
        "session" => expand_session(ctx),
        "windows" => expand_windows(ctx),
        "pane" => expand_pane(ctx),
        "time" => expand_time("%H:%M"), // Default time format
        "hostname" => ctx.hostname.clone(),
        "user" => ctx.username.clone(),
        _ => format!("{{{}}}", var_name), // Unknown variable - keep as-is
    }
}

/// Expand the {session} variable.
fn expand_session(ctx: &FormatContext) -> String {
    // First try the session_name from context (from notification)
    if let Some(name) = ctx.session_name {
        return name.to_string();
    }

    // Fall back to session's stored name
    if let Some(session) = ctx.session
        && let Some(name) = session.session_name()
    {
        return name.to_string();
    }

    // No session name available
    "tmux".to_string()
}

/// Expand the {windows} variable to show window list.
fn expand_windows(ctx: &FormatContext) -> String {
    let session = match ctx.session {
        Some(s) => s,
        None => return String::new(),
    };

    let windows = session.windows();
    if windows.is_empty() {
        return String::new();
    }

    // Sort windows by index
    let mut window_list: Vec<_> = windows.values().collect();
    window_list.sort_by_key(|w| w.index);

    // Format each window
    let parts: Vec<String> = window_list
        .iter()
        .map(|window| {
            let marker = if window.active { "*" } else { "" };
            format!("{}:{}{}", window.index, window.name, marker)
        })
        .collect();

    parts.join(" ")
}

/// Expand the {pane} variable to show focused pane ID.
fn expand_pane(ctx: &FormatContext) -> String {
    if let Some(session) = ctx.session
        && let Some(pane_id) = session.focused_pane()
    {
        return format!("%{}", pane_id);
    }
    String::new()
}

/// Expand the {time:FORMAT} variable.
fn expand_time(format: &str) -> String {
    let now = Local::now();
    now.format(format).to_string()
}

/// Get the system hostname.
fn get_hostname() -> String {
    hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "localhost".to_string())
}

/// Get the current username.
fn get_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string())
}

/// Sanitize output from native tmux format queries.
///
/// Removes tmux-specific markers and ANSI color codes that shouldn't
/// be displayed in par-term's UI.
///
/// Based on iTerm2's approach in `iTermTmuxStatusBarMonitor.m`.
pub fn sanitize_tmux_output(output: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    // Patterns to remove:
    // 1. <'...' not ready> markers (tmux pending command output)
    // 2. #[...] ANSI style codes (tmux color/attribute codes)
    static NOT_READY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<'[^']*' not ready>").expect("NOT_READY_RE regex pattern is valid"));
    static STYLE_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\[[^\]]*\]").expect("STYLE_CODE_RE regex pattern is valid"));

    let result = NOT_READY_RE.replace_all(output, "");
    let result = STYLE_CODE_RE.replace_all(&result, "");
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_simple_session() {
        let ctx = FormatContext {
            session: None,
            session_name: Some("dev"),
            hostname: "myhost".to_string(),
            username: "alice".to_string(),
        };

        let result = expand_format("[{session}]", &ctx);
        assert_eq!(result, "[dev]");
    }

    #[test]
    fn test_expand_hostname_and_user() {
        let ctx = FormatContext {
            session: None,
            session_name: None,
            hostname: "myhost".to_string(),
            username: "alice".to_string(),
        };

        let result = expand_format("{user}@{hostname}", &ctx);
        assert_eq!(result, "alice@myhost");
    }

    #[test]
    fn test_expand_time_default() {
        let ctx = FormatContext {
            session: None,
            session_name: None,
            hostname: "host".to_string(),
            username: "user".to_string(),
        };

        // Just check that it produces something that looks like a time
        let result = expand_format("{time}", &ctx);
        assert!(
            result.contains(':'),
            "Expected time format HH:MM, got: {}",
            result
        );
    }

    #[test]
    fn test_expand_time_custom_format() {
        let ctx = FormatContext {
            session: None,
            session_name: None,
            hostname: "host".to_string(),
            username: "user".to_string(),
        };

        // Test with a custom format that includes year
        let result = expand_format("{time:%Y}", &ctx);
        // Should be a 4-digit year
        assert_eq!(result.len(), 4);
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_expand_unknown_variable() {
        let ctx = FormatContext {
            session: None,
            session_name: None,
            hostname: "host".to_string(),
            username: "user".to_string(),
        };

        let result = expand_format("{unknown}", &ctx);
        assert_eq!(result, "{unknown}");
    }

    #[test]
    fn test_expand_mixed_content() {
        let ctx = FormatContext {
            session: None,
            session_name: Some("work"),
            hostname: "laptop".to_string(),
            username: "bob".to_string(),
        };

        let result = expand_format("[{session}] {user}@{hostname}", &ctx);
        assert_eq!(result, "[work] bob@laptop");
    }

    #[test]
    fn test_expand_unclosed_brace() {
        let ctx = FormatContext {
            session: None,
            session_name: None,
            hostname: "host".to_string(),
            username: "user".to_string(),
        };

        // Unclosed brace should be treated as literal
        let result = expand_format("test {session", &ctx);
        assert_eq!(result, "test {session");
    }

    #[test]
    fn test_sanitize_not_ready() {
        let input = "left <'command' not ready> right";
        let result = sanitize_tmux_output(input);
        assert_eq!(result, "left  right");
    }

    #[test]
    fn test_sanitize_style_codes() {
        let input = "text #[fg=red]colored#[default] more";
        let result = sanitize_tmux_output(input);
        assert_eq!(result, "text colored more");
    }

    #[test]
    fn test_sanitize_combined() {
        let input = "#[bold]session#[default] <'cmd' not ready>";
        let result = sanitize_tmux_output(input);
        assert_eq!(result, "session");
    }
}
