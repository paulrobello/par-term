//! Platform abstraction layer for par-term.
//!
//! This module centralises the platform-specific behaviour that would otherwise be
//! scattered across the codebase as inline `#[cfg(target_os = ...)]` blocks.
//!
//! # Conventions
//!
//! - Every public function in this module has a single, consistent cross-platform
//!   signature.  The platform branching is contained *inside* each function body.
//! - Consumers import `crate::platform` and call the function directly — no
//!   `#[cfg]` attributes are required at the call site.
//!
//! # Contents
//!
//! | Function | Description |
//! |---|---|
//! | [`deliver_desktop_notification`] | Send a native desktop notification |
//! | [`deliver_desktop_notification_request`] | Send a notification with identity/click-token support |
//! | [`drain_notification_clicks`] | Non-blocking drain of notification click tokens |
//! | [`primary_modifier`] | Whether the platform's "primary" modifier key is held |

mod modifiers;
mod notify;
#[cfg(target_os = "macos")]
mod notify_macos;

pub use modifiers::{primary_modifier, primary_modifier_with_shift};
pub use notify::{
    NotificationRequest, NotificationUrgency, deliver_desktop_notification,
    deliver_desktop_notification_request, drain_notification_clicks, escape_for_applescript,
};
