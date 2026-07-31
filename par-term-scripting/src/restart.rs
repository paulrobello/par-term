//! Restart supervision for script subprocesses.
//!
//! `ScriptConfig::restart_policy` / `restart_delay_ms` are accepted by the
//! config layer but the actual supervision happens in the frontend event loop,
//! which is the only place that can re-register a terminal observer and re-spawn
//! against a config index. This module owns the *decision* — a pure state
//! machine with no I/O — so it stays deterministic and unit-testable.
//!
//! The orchestrator drives one [`ScriptRestartState`] per script slot:
//!
//! 1. On a fresh process start, call [`ScriptRestartState::on_started`].
//! 2. Each frame, poll the process; if it has exited, call [`ScriptRestartState::on_exit`]
//!    (only when not already pending) to get the first reaction.
//! 3. While a restart is pending, call [`ScriptRestartState::poll`] to learn when
//!    the delay has elapsed.

use std::time::{Duration, Instant};

use par_term_config::RestartPolicy;

/// What the supervisor wants the orchestrator to do this frame for one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartAction {
    /// No transition observed and nothing pending — leave the slot alone.
    Idle,
    /// Tear the slot down: the policy says stop, or the attempt cap was hit.
    Stop,
    /// A pending restart's delay has elapsed — re-spawn now.
    Restart,
    /// A restart is scheduled but the delay has not yet elapsed.
    Wait,
}

/// Maximum number of consecutive restart attempts before the supervisor gives up.
///
/// Consecutive failures are counted only within the [`RESTART_GRACE`] window: a
/// process that runs past grace resets the counter, so a slow leak (e.g. crashes
/// once an hour) never exhausts the budget. Only a tight crash-loop does.
pub const MAX_RESTART_ATTEMPTS: u32 = 5;

/// A process that survives this long before exiting resets the failure counter.
pub const RESTART_GRACE: Duration = Duration::from_secs(5);

/// Pure per-slot restart supervisor.
#[derive(Debug, Clone)]
pub struct ScriptRestartState {
    policy: RestartPolicy,
    delay: Duration,
    /// When the current process was (re)started; `None` until the first start.
    started_at: Option<Instant>,
    /// When a pending restart should fire; `None` when the slot is idle.
    deadline: Option<Instant>,
    /// Consecutive restarts that re-exited inside the grace window.
    consecutive_failures: u32,
}

impl ScriptRestartState {
    /// Build a supervisor for a slot configured with `policy` and `delay_ms`.
    pub fn new(policy: RestartPolicy, delay_ms: u64) -> Self {
        Self {
            policy,
            delay: Duration::from_millis(delay_ms),
            started_at: None,
            deadline: None,
            consecutive_failures: 0,
        }
    }

    /// Apply a config edit without losing pending timing beyond the current cycle.
    pub fn reconfigure(&mut self, policy: RestartPolicy, delay_ms: u64) {
        self.policy = policy;
        self.delay = Duration::from_millis(delay_ms);
    }

    /// Record that a (re)start just happened. Begins a fresh grace window.
    pub fn on_started(&mut self, now: Instant) {
        self.started_at = Some(now);
        self.deadline = None;
    }

    /// React to a process exit. Only call when [`Self::pending`] is false, so
    /// this fires once per exit rather than every sticky `Exited` poll.
    pub fn on_exit(&mut self, now: Instant, success: bool) -> RestartAction {
        // Reset the failure counter if the process ran past the grace window —
        // a slow leak must not exhaust the crash-loop budget.
        if let Some(start) = self.started_at
            && now.duration_since(start) >= RESTART_GRACE
        {
            self.consecutive_failures = 0;
        }
        let should_restart = match self.policy {
            RestartPolicy::Never => false,
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure => !success,
        };
        if !should_restart {
            self.deadline = None;
            return RestartAction::Stop;
        }
        self.schedule(now)
    }

    /// Check whether a pending restart's deadline has fired. Call each frame
    /// while [`Self::pending`].
    pub fn poll(&mut self, now: Instant) -> RestartAction {
        match self.deadline {
            Some(deadline) if now >= deadline => {
                self.deadline = None;
                RestartAction::Restart
            }
            Some(_) => RestartAction::Wait,
            None => RestartAction::Idle,
        }
    }

    /// A restart is awaiting its delay window.
    pub fn pending(&self) -> bool {
        self.deadline.is_some()
    }

    /// Re-arm after a restart *attempt* failed to spawn (distinct from a real
    /// exit — no process ever ran). Counts toward the attempt cap.
    pub fn reschedule(&mut self, now: Instant) -> RestartAction {
        self.schedule(now)
    }

    /// Consecutive failures accrued inside the grace window (for UI/debug).
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    fn schedule(&mut self, now: Instant) -> RestartAction {
        if self.consecutive_failures >= MAX_RESTART_ATTEMPTS {
            self.deadline = None;
            return RestartAction::Stop;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.deadline = Some(now + self.delay);
        RestartAction::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor() -> Instant {
        Instant::now()
    }

    #[test]
    fn never_policy_stops_on_clean_exit() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Never, 0);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, true), RestartAction::Stop);
        assert!(!s.pending());
    }

    #[test]
    fn never_policy_stops_on_failure_too() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Never, 0);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, false), RestartAction::Stop);
    }

    #[test]
    fn always_policy_waits_for_delay_then_restarts() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 100);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, true), RestartAction::Wait);
        assert!(s.pending());
        assert_eq!(s.poll(t0 + Duration::from_millis(99)), RestartAction::Wait);
        assert_eq!(
            s.poll(t0 + Duration::from_millis(100)),
            RestartAction::Restart
        );
        assert!(!s.pending());
    }

    #[test]
    fn always_policy_restarts_immediately_when_delay_zero() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 0);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, true), RestartAction::Wait);
        assert_eq!(s.poll(t0), RestartAction::Restart);
    }

    #[test]
    fn on_failure_stops_on_clean_exit() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::OnFailure, 100);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, true), RestartAction::Stop);
        assert!(!s.pending());
    }

    #[test]
    fn on_failure_restarts_on_failure_after_delay() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::OnFailure, 50);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, false), RestartAction::Wait);
        assert_eq!(s.poll(t0 + Duration::from_millis(49)), RestartAction::Wait);
        assert_eq!(
            s.poll(t0 + Duration::from_millis(50)),
            RestartAction::Restart
        );
    }

    #[test]
    fn idle_when_nothing_has_happened() {
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 100);
        s.on_started(anchor());
        assert_eq!(s.poll(anchor()), RestartAction::Idle);
        assert!(!s.pending());
    }

    #[test]
    fn gives_up_after_max_attempts_within_grace_window() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 10);
        for _ in 0..MAX_RESTART_ATTEMPTS {
            s.on_started(t0);
            assert_eq!(s.on_exit(t0, false), RestartAction::Wait);
            assert_eq!(
                s.poll(t0 + Duration::from_millis(10)),
                RestartAction::Restart
            );
        }
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, false), RestartAction::Stop);
    }

    #[test]
    fn long_lived_process_resets_failure_counter() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 10);
        s.on_started(t0);
        s.on_exit(t0, false);
        assert_eq!(s.consecutive_failures(), 1);
        s.poll(t0 + Duration::from_millis(10));
        let restart_at = t0 + Duration::from_millis(20);
        s.on_started(restart_at);
        let after_grace = restart_at + RESTART_GRACE + Duration::from_millis(1);
        s.on_exit(after_grace, false);
        assert_eq!(s.consecutive_failures(), 1);
    }

    #[test]
    fn reschedule_after_spawn_failure_counts_toward_cap() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Always, 10);
        s.on_started(t0);
        s.on_exit(t0, false); // failures = 1
        s.poll(t0 + Duration::from_millis(10)); // fire restart
        assert_eq!(
            s.reschedule(t0 + Duration::from_millis(11)),
            RestartAction::Wait
        );
        assert_eq!(s.consecutive_failures(), 2);
    }

    #[test]
    fn reconfigure_swaps_policy_and_delay() {
        let t0 = anchor();
        let mut s = ScriptRestartState::new(RestartPolicy::Never, 0);
        s.reconfigure(RestartPolicy::Always, 200);
        s.on_started(t0);
        assert_eq!(s.on_exit(t0, true), RestartAction::Wait);
        assert_eq!(
            s.poll(t0 + Duration::from_millis(200)),
            RestartAction::Restart
        );
    }
}
