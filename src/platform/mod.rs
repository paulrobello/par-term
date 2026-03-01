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
//! | [`primary_modifier`] | Whether the platform's "primary" modifier key is held |

mod modifiers;
mod notify;

pub use modifiers::{primary_modifier, primary_modifier_with_shift};
pub use notify::{deliver_desktop_notification, escape_for_applescript};
