//! Notification and bell sections (Advanced › Notifications & Bell) and
//! anti-idle (Advanced › Terminal Emulation). Placement is set in
//! [`crate::layout`].
//!
//! - `bell`: Visual bell, audio bell volume, and desktop notifications
//! - `activity`: Activity, silence, and session notification settings
//! - `alert_sounds`: Per-event sound configuration
//! - `behavior`: Suppress-when-focused, buffer size, and test notification
//! - `anti_idle`: Anti-idle keep-alive settings

pub(crate) mod activity;
pub(crate) mod alert_sounds;
pub(crate) mod anti_idle;
pub(crate) mod behavior;
pub(crate) mod bell;
