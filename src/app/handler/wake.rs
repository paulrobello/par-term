//! One event-loop wake cadence for every window.
//!
//! `ControlFlow` is a property of the event loop, not of a window, but the state
//! that decides it — cursor blink, animated shaders, a live ACP agent, a running
//! script, an in-flight file transfer — is per window. Each window therefore
//! reports a [`WakeRequest`] and sets nothing; [`reduce_wake_requests`] folds
//! them into the single [`WakeDecision`] the loop actually applies.
//!
//! Three rules, each of which was a bug when every window set the control flow
//! for itself and the last one iterated won:
//!
//! 1. **The wake deadline is the minimum.** A window that needs to be woken in
//!    100ms must not have that discarded by a window iterated after it that only
//!    needs waking in a second.
//! 2. **`Poll` wins over any deadline.** A file transfer needs `Poll` on macOS
//!    for `RedrawRequested` to be delivered at all, so one window needing it
//!    settles the question.
//! 3. **The loop sleeps once, and only when every window is idle.** The explicit
//!    idle sleep exists because `WaitUntil` does not reliably stop the loop from
//!    spinning on macOS. Run per window it stacked — N windows slept N times per
//!    iteration — and an idle window's sleep throttled a busy one that had just
//!    asked for `Poll`.

use std::time::Instant;

use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// What one window wants from the event loop for the coming iteration.
///
/// Produced by `WindowState::about_to_wait`, which no longer touches the event
/// loop itself.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WakeRequest {
    /// This window needs `ControlFlow::Poll` rather than a timed wait.
    pub(crate) needs_poll: bool,
    /// The earliest instant this window must be woken.
    pub(crate) wake_at: Instant,
    /// The latest instant this window is willing to be slept through, or `None`
    /// when it has a frame in flight and must not be slept through at all.
    pub(crate) idle_sleep_cap: Option<Instant>,
}

/// The single control-flow decision for one loop iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WakeDecision {
    /// The deadline to pass to `ControlFlow::WaitUntil`, or `None` for `Poll`.
    pub(crate) wake_at: Option<Instant>,
    /// How long to sleep before returning to the loop, if at all.
    pub(crate) sleep_until: Option<Instant>,
}

/// Fold every window's request into the one decision the event loop applies.
///
/// Returns `None` when no window reported — an empty window list, or every
/// window shutting down — in which case the existing control flow is left
/// alone, exactly as it was when a skipped window set nothing.
pub(crate) fn reduce_wake_requests(requests: &[WakeRequest]) -> Option<WakeDecision> {
    let first = requests.first()?;

    if requests.iter().any(|r| r.needs_poll) {
        // Polling is already the busiest cadence there is; sleeping on top of
        // it would defeat the reason a window asked for it.
        return Some(WakeDecision {
            wake_at: None,
            sleep_until: None,
        });
    }

    let wake_at = requests
        .iter()
        .map(|r| r.wake_at)
        .fold(first.wake_at, Instant::min);

    // One window with a frame in flight keeps the whole loop awake: the sleep
    // blocks the thread that would service it.
    let sleep_until = requests.iter().try_fold(wake_at, |earliest, r| {
        r.idle_sleep_cap.map(|cap| earliest.min(cap))
    });

    Some(WakeDecision {
        wake_at: Some(wake_at),
        sleep_until,
    })
}

/// Apply a reduced decision to the event loop.
///
/// The sleep is measured against the current instant rather than the one each
/// window captured, so work done between collecting the requests and arriving
/// here (script servicing, tab moves, window closes) shortens the sleep instead
/// of being added to it.
pub(crate) fn apply_wake_decision(event_loop: &ActiveEventLoop, decision: WakeDecision) {
    if let Some(sleep_until) = decision.sleep_until {
        let duration = sleep_until.saturating_duration_since(Instant::now());
        if duration > std::time::Duration::from_millis(1) {
            std::thread::sleep(duration);
        }
    }

    match decision.wake_at {
        Some(wake_at) => event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at)),
        None => event_loop.set_control_flow(ControlFlow::Poll),
    }
}

#[cfg(test)]
mod tests {
    use super::{WakeRequest, reduce_wake_requests};
    use std::time::{Duration, Instant};

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    /// An idle window: willing to be slept through up to `cap_ms`.
    fn idle(base: Instant, wake_ms: u64, cap_ms: u64) -> WakeRequest {
        WakeRequest {
            needs_poll: false,
            wake_at: at(base, wake_ms),
            idle_sleep_cap: Some(at(base, cap_ms)),
        }
    }

    /// A window with a frame in flight: must not be slept through.
    fn busy(base: Instant, wake_ms: u64) -> WakeRequest {
        WakeRequest {
            needs_poll: false,
            wake_at: at(base, wake_ms),
            idle_sleep_cap: None,
        }
    }

    #[test]
    fn no_windows_leaves_the_control_flow_alone() {
        assert!(reduce_wake_requests(&[]).is_none());
    }

    #[test]
    fn the_deadline_is_the_minimum_not_the_last_window() {
        // The regression this whole module exists for: a background window that
        // needs waking in 100ms must not inherit an idle window's 1s deadline
        // just because the idle one was iterated last.
        let base = Instant::now();
        let decision = reduce_wake_requests(&[
            idle(base, 100, 100),
            idle(base, 1000, 1000),
            idle(base, 500, 500),
        ])
        .expect("a decision for three windows");

        assert_eq!(decision.wake_at, Some(at(base, 100)));
    }

    #[test]
    fn the_minimum_holds_whatever_the_iteration_order() {
        let base = Instant::now();
        let soonest = reduce_wake_requests(&[idle(base, 30, 900), idle(base, 900, 900)])
            .expect("decision")
            .wake_at;
        let reversed = reduce_wake_requests(&[idle(base, 900, 900), idle(base, 30, 900)])
            .expect("decision")
            .wake_at;

        assert_eq!(soonest, reversed);
        assert_eq!(soonest, Some(at(base, 30)));
    }

    #[test]
    fn one_window_needing_poll_settles_it_for_all() {
        let base = Instant::now();
        let polling = WakeRequest {
            needs_poll: true,
            wake_at: at(base, 1000),
            idle_sleep_cap: None,
        };

        for requests in [
            vec![polling, idle(base, 50, 50)],
            vec![idle(base, 50, 50), polling],
        ] {
            let decision = reduce_wake_requests(&requests).expect("decision");
            assert_eq!(decision.wake_at, None, "Poll must win over any deadline");
            // An idle sibling's sleep previously throttled the polling window.
            assert_eq!(decision.sleep_until, None, "Poll must not be slept through");
        }
    }

    #[test]
    fn the_loop_sleeps_only_when_every_window_is_idle() {
        let base = Instant::now();

        let all_idle =
            reduce_wake_requests(&[idle(base, 900, 50), idle(base, 900, 100)]).expect("decision");
        assert!(all_idle.sleep_until.is_some());

        let one_busy =
            reduce_wake_requests(&[idle(base, 900, 50), busy(base, 900)]).expect("decision");
        assert_eq!(
            one_busy.sleep_until, None,
            "a window with a frame in flight must not be slept through"
        );
    }

    #[test]
    fn the_sleep_stops_at_the_earliest_cap_or_deadline() {
        let base = Instant::now();

        // Tightest cap wins: a focused window caps at 50ms where an unfocused
        // one would tolerate 100ms.
        let capped =
            reduce_wake_requests(&[idle(base, 900, 100), idle(base, 900, 50)]).expect("decision");
        assert_eq!(capped.sleep_until, Some(at(base, 50)));

        // A wake deadline sooner than every cap wins instead.
        let deadline =
            reduce_wake_requests(&[idle(base, 20, 100), idle(base, 900, 50)]).expect("decision");
        assert_eq!(deadline.sleep_until, Some(at(base, 20)));
        assert_eq!(deadline.wake_at, Some(at(base, 20)));
    }

    #[test]
    fn a_single_window_is_unchanged_by_the_reduction() {
        // The one-window case must behave exactly as the per-window code did:
        // sleep until min(next_wake, cap), then WaitUntil(next_wake).
        let base = Instant::now();
        let decision = reduce_wake_requests(&[idle(base, 1000, 50)]).expect("decision");

        assert_eq!(decision.wake_at, Some(at(base, 1000)));
        assert_eq!(decision.sleep_until, Some(at(base, 50)));
    }
}
