//! Cross-platform desktop notification delivery.
//!
//! Abstracts over:
//! - **macOS**: `osascript` AppleScript `display notification` command
//! - **Windows / Linux**: the `notify_rust` crate
//!
//! All callers should use [`deliver_desktop_notification`] rather than inline
//! `#[cfg(target_os = ...)]` blocks, so platform differences live only here.

/// Escape a string for safe embedding inside an AppleScript double-quoted string.
///
/// AppleScript requires that backslashes, double-quotes, and newlines are escaped.
/// The order of replacements matters: backslashes must be escaped *first* so that
/// the subsequent replacements do not accidentally double-escape them.
pub fn escape_for_applescript(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Deliver a native desktop notification.
///
/// On macOS the notification is sent via `osascript`; on all other platforms
/// the `notify_rust` crate is used.  Both paths are fire-and-forget: failures
/// are logged as warnings and the function always returns normally.
///
/// # Arguments
/// * `title`   – Notification title / summary line (may be empty).
/// * `message` – Notification body text.
/// * `timeout_ms` – How long the notification should be displayed on non-macOS
///   platforms (macOS ignores this value; the OS controls notification duration).
pub fn deliver_desktop_notification(title: &str, message: &str, timeout_ms: u32) {
    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;
        let notification_title = if !title.is_empty() {
            title
        } else {
            "Terminal Notification"
        };
        if let Err(e) = Notification::new()
            .summary(notification_title)
            .body(message)
            .timeout(notify_rust::Timeout::Milliseconds(timeout_ms))
            .show()
        {
            log::warn!("Failed to send desktop notification: {}", e);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = timeout_ms; // macOS duration is controlled by the OS
        let notification_title = if !title.is_empty() {
            title
        } else {
            "Terminal Notification"
        };
        let escaped_title = escape_for_applescript(notification_title);
        let escaped_message = escape_for_applescript(message);
        let script = format!(
            r#"display notification "{}" with title "{}""#,
            escaped_message, escaped_title,
        );
        if let Err(e) = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
        {
            log::warn!("Failed to send macOS desktop notification: {}", e);
        }
    }
}
