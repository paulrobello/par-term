//! Tests for automation runtime behavior — rate limiter functionality.

use par_term::config::TriggerRateLimiter;

// ============================================================================
// Rate Limiter Tests
// ============================================================================

#[test]
fn test_rate_limiter_allows_first_call() {
    let mut limiter = TriggerRateLimiter::default();
    assert!(limiter.check_and_update(1), "First call should be allowed");
}

#[test]
fn test_rate_limiter_blocks_immediate_second_call() {
    let mut limiter = TriggerRateLimiter::default();
    limiter.check_and_update(1);
    assert!(
        !limiter.check_and_update(1),
        "Immediate second call should be blocked"
    );
}

#[test]
fn test_rate_limiter_allows_different_trigger_ids() {
    let mut limiter = TriggerRateLimiter::default();
    assert!(limiter.check_and_update(1), "Trigger 1 first call");
    assert!(
        limiter.check_and_update(2),
        "Trigger 2 should be independent"
    );
}

#[test]
fn test_rate_limiter_custom_interval() {
    // Use a very short interval for testing
    let mut limiter = TriggerRateLimiter::new(1);
    limiter.check_and_update(1);
    // Sleep just past the interval
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(
        limiter.check_and_update(1),
        "Should be allowed after interval passes"
    );
}

#[test]
fn test_rate_limiter_cleanup() {
    let mut limiter = TriggerRateLimiter::new(1);
    limiter.check_and_update(1);
    limiter.check_and_update(2);

    // Wait a bit, then cleanup with a very short max_age
    std::thread::sleep(std::time::Duration::from_millis(5));
    limiter.cleanup(0); // max_age_secs = 0 should clear everything

    // After cleanup, both should be allowed again
    assert!(
        limiter.check_and_update(1),
        "Should be allowed after cleanup"
    );
    assert!(
        limiter.check_and_update(2),
        "Should be allowed after cleanup"
    );
}
