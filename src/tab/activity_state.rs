//! Activity tracking state for a terminal tab.
//!
//! Groups all fields related to activity detection, anti-idle keep-alive,
//! silence notifications, and exit tracking.

/// Activity tracking state for a terminal tab.
pub(crate) struct TabActivityState {
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

impl Default for TabActivityState {
    fn default() -> Self {
        Self {
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
        }
    }
}
