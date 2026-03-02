//! Activity monitoring state for a terminal tab.
//!
//! Groups all fields related to the tab activity indicator (tab bar dot),
//! activity detection, anti-idle keep-alive, silence notifications, and exit tracking.

/// Activity monitoring state for a terminal tab.
///
/// Centralises the `has_activity` indicator (shown as a dot in the tab bar),
/// timing data for silence/exit notifications, and anti-idle keep-alive bookkeeping.
/// Extracted from `Tab` to keep the activity concern in one place (R-11).
pub(crate) struct TabActivityMonitor {
    /// Whether this tab has unread activity since it was last viewed (shown in tab bar)
    pub(crate) has_activity: bool,
    /// Last time terminal output (activity) was detected
    pub(crate) last_activity_time: std::time::Instant,
    /// Last terminal update generation seen (to detect new output)
    pub(crate) last_seen_generation: u64,
    /// Last activity time for anti-idle keep-alive
    pub(crate) anti_idle_last_activity: std::time::Instant,
    /// Last terminal generation recorded for anti-idle tracking
    pub(crate) anti_idle_last_generation: u64,
    /// Whether silence notification has been sent for current idle period
    pub(crate) silence_notified: bool,
    /// Whether exit notification has been sent for this tab
    pub(crate) exit_notified: bool,
}

impl Default for TabActivityMonitor {
    fn default() -> Self {
        Self {
            has_activity: false,
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
        }
    }
}

/// Type alias for backwards compatibility during the transition period.
/// New code should use `TabActivityMonitor` directly.
#[allow(dead_code)]
pub(crate) type TabActivityState = TabActivityMonitor;
