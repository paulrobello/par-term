//! Anti-idle helper utilities.
//!
//! Provides simple time-based helpers used by the anti-idle keep-alive logic.

use std::time::{Duration, Instant};

/// Determine whether a keep-alive should be sent given the last activity timestamp.
pub(crate) fn should_send_keep_alive(
    last_activity: Instant,
    now: Instant,
    idle_threshold: Duration,
) -> bool {
    now.duration_since(last_activity) >= idle_threshold
}

#[cfg(test)]
mod tests {
    use super::should_send_keep_alive;
    use std::time::{Duration, Instant};

    #[test]
    fn test_should_send_keep_alive_after_threshold() {
        let now = Instant::now();
        let past = now - Duration::from_secs(61);
        assert!(should_send_keep_alive(past, now, Duration::from_secs(60)));
    }

    #[test]
    fn test_should_not_send_before_threshold() {
        let now = Instant::now();
        let recent = now - Duration::from_secs(30);
        assert!(!should_send_keep_alive(
            recent,
            now,
            Duration::from_secs(60)
        ));
    }

    #[test]
    fn test_boundary_condition_triggers() {
        let now = Instant::now();
        let at_threshold = now - Duration::from_secs(60);
        assert!(should_send_keep_alive(
            at_threshold,
            now,
            Duration::from_secs(60)
        ));
    }
}
