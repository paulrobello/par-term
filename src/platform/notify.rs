//! Cross-platform desktop notification delivery.
//!
//! Abstracts over:
//! - **macOS**: native `UNUserNotificationCenter` (bundled/signed apps), falling back to
//!   the `osascript` AppleScript `display notification` command for bare `cargo run`
//!   binaries or if the native path errors at runtime. See [`crate::platform::notify_macos`].
//! - **Linux/BSD**: the `notify_rust` crate (XDG desktop notifications over D-Bus).
//! - **Windows**: the `notify_rust` crate (no urgency/identity/click support there).
//!
//! All callers should use [`deliver_desktop_notification`] or
//! [`deliver_desktop_notification_request`] rather than inline
//! `#[cfg(target_os = ...)]` blocks, so platform differences live only here.
//!
//! ## Identity and click support
//!
//! [`NotificationRequest::identity`] and [`NotificationRequest::click_token`] are
//! opt-in, cross-platform-safe extensions to the original notification API:
//!
//! | Platform | Identity (replace-in-place) | Click callback |
//! |---|---|---|
//! | Linux/BSD (XDG) | Yes — via a hashed numeric id (`.id(u32)`) | Yes — via `wait_for_action` on a detached thread |
//! | macOS (native, bundled app) | Yes — via `UNNotificationRequest` identifier | Yes — via `UNUserNotificationCenterDelegate` |
//! | macOS (osascript fallback) | No (ignored) | No (ignored) |
//! | Windows | No (ignored) | No (ignored) |
//!
//! Click tokens are delivered asynchronously on a channel; call
//! [`drain_notification_clicks`] once per frame from the event loop to collect them.

#[cfg(target_os = "macos")]
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock, mpsc};

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

/// Notification urgency level, local to the platform layer so this module has
/// no dependency on any particular terminal library's urgency type. Callers
/// map their own urgency representation onto this at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationUrgency {
    /// Low urgency — e.g. shown briefly, no interruption.
    Low,
    /// Normal urgency — the default.
    #[default]
    Normal,
    /// Critical urgency — e.g. kept on-screen / given an audible cue.
    Critical,
}

/// Parameters for delivering a desktop notification.
///
/// See the module docs for the per-platform identity/click support matrix.
pub struct NotificationRequest<'a> {
    /// Notification title / summary line (may be empty).
    pub title: &'a str,
    /// Notification body text.
    pub message: &'a str,
    /// How long the notification should be displayed on platforms that honor
    /// an explicit timeout (macOS ignores this value; the OS controls duration).
    pub timeout_ms: u32,
    /// Notification urgency; affects timeout/presentation.
    pub urgency: NotificationUrgency,
    /// Stable identity: redelivering with the same identity REPLACES the
    /// previous notification where the platform supports it (`None` disables
    /// replacement — every call creates a distinct notification).
    pub identity: Option<&'a str>,
    /// When `Some`, a user click on the notification emits this token on the
    /// click channel (drained via [`drain_notification_clicks`]). Platforms
    /// without click support ignore it.
    pub click_token: Option<u64>,
}

/// Deliver a native desktop notification.
///
/// Thin wrapper over [`deliver_desktop_notification_request`] with no
/// identity/click-token association, kept so existing call sites compile
/// unchanged.
///
/// # Arguments
/// * `title`   – Notification title / summary line (may be empty).
/// * `message` – Notification body text.
/// * `timeout_ms` – How long the notification should be displayed on non-macOS
///   platforms (macOS ignores this value; the OS controls notification duration).
/// * `urgency` – Notification urgency; affects timeout/presentation (see platform blocks below).
pub fn deliver_desktop_notification(
    title: &str,
    message: &str,
    timeout_ms: u32,
    urgency: NotificationUrgency,
) {
    deliver_desktop_notification_request(&NotificationRequest {
        title,
        message,
        timeout_ms,
        urgency,
        identity: None,
        click_token: None,
    });
}

/// Deliver a native desktop notification, with optional identity (replacement)
/// and click-token association. See the module docs for the per-platform
/// support matrix.
///
/// Fire-and-forget on every platform: failures are logged as warnings and the
/// function always returns normally.
pub fn deliver_desktop_notification_request(req: &NotificationRequest<'_>) {
    // Linux/BSD: notify_rust exposes `.urgency()`/`.id()`/`.action()` here, so
    // map urgency to both the notification's urgency hint and its timeout
    // (Critical stays on-screen), derive a stable numeric id from `identity`
    // for XDG same-id replacement, and register a "default" action so a click
    // can be observed via `wait_for_action` when `click_token` is set.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use notify_rust::{Notification, Timeout, Urgency as RustUrgency};
        let notification_title = if !req.title.is_empty() {
            req.title
        } else {
            "Terminal Notification"
        };
        let timeout = match req.urgency {
            NotificationUrgency::Low => Timeout::Milliseconds(1500),
            NotificationUrgency::Normal => Timeout::Milliseconds(req.timeout_ms),
            NotificationUrgency::Critical => Timeout::Never,
        };
        let rust_urgency = match req.urgency {
            NotificationUrgency::Low => RustUrgency::Low,
            NotificationUrgency::Normal => RustUrgency::Normal,
            NotificationUrgency::Critical => RustUrgency::Critical,
        };
        let mut notification = Notification::new();
        notification
            .summary(notification_title)
            .body(req.message)
            .timeout(timeout)
            .urgency(rust_urgency);
        if let Some(identity) = req.identity {
            notification.id(fnv1a_u32(identity));
        }
        if req.click_token.is_some() {
            notification.action("default", "Open");
        }
        match notification.show() {
            Ok(handle) => {
                if let Some(token) = req.click_token {
                    let sender = click_sender();
                    // Only spawn a wait thread when a click callback was
                    // requested — fire-and-forget notifications don't need one.
                    std::thread::spawn(move || {
                        handle.wait_for_action(move |action: &str| {
                            if action == "default" || action == "clicked" {
                                let _ = sender.send(token);
                            }
                        });
                    });
                }
            }
            Err(e) => log::warn!("Failed to send desktop notification: {}", e),
        }
    }

    // Windows: notify_rust's `.urgency()`/`.id()`/`.action()` builders are
    // Linux/BSD-only, so identity and click_token are not surfaced here;
    // timeout behavior is unchanged.
    #[cfg(all(not(unix), not(target_os = "macos")))]
    {
        use notify_rust::Notification;
        let _ = req.urgency;
        let _ = req.identity;
        let _ = req.click_token;
        let notification_title = if !req.title.is_empty() {
            req.title
        } else {
            "Terminal Notification"
        };
        if let Err(e) = Notification::new()
            .summary(notification_title)
            .body(req.message)
            .timeout(notify_rust::Timeout::Milliseconds(req.timeout_ms))
            .show()
        {
            log::warn!("Failed to send desktop notification: {}", e);
        }
    }

    #[cfg(target_os = "macos")]
    {
        crate::platform::notify_macos::deliver(req);
    }
}

/// FNV-1a hash of a UTF-8 string, used on Linux/BSD to derive a stable XDG
/// numeric notification id from an opaque identity string — the freedesktop
/// notification spec replaces an existing notification in place when a new
/// one is shown with the same numeric id.
#[cfg(all(unix, not(target_os = "macos")))]
fn fnv1a_u32(s: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in s.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Global click-token channel shared by all notification backends.
struct ClickChannel {
    sender: mpsc::Sender<u64>,
    receiver: Mutex<mpsc::Receiver<u64>>,
}

static CLICK_CHANNEL: OnceLock<ClickChannel> = OnceLock::new();

fn click_channel() -> &'static ClickChannel {
    CLICK_CHANNEL.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        ClickChannel {
            sender,
            receiver: Mutex::new(receiver),
        }
    })
}

/// Clone a sender that notification backends use to report a click.
#[cfg(target_os = "macos")]
pub(crate) fn click_sender() -> Sender<u64> {
    click_channel().sender.clone()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn click_sender() -> mpsc::Sender<u64> {
    click_channel().sender.clone()
}

/// Non-blocking drain of click tokens emitted by notification backends.
/// Safe to call every frame from the event loop.
pub fn drain_notification_clicks() -> Vec<u64> {
    let receiver = match click_channel().receiver.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut tokens = Vec::new();
    while let Ok(token) = receiver.try_recv() {
        tokens.push(token);
    }
    tokens
}
