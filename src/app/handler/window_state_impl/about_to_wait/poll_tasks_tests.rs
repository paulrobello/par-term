use super::poll_tasks::*;
use std::time::{Duration, Instant};

fn never(_: &mut crate::app::window_state::WindowState, _: &DrainOutcome) -> bool {
    false
}

fn task(redraw: Redraw, wake: Wake) -> PollTask {
    PollTask {
        name: "test",
        gate: never,
        redraw,
        wake,
        needs_poll: false,
        bypass_render_delay: false,
    }
}

#[test]
fn deadlines_only_move_earlier_and_redraw_obeys_the_frame_budget() {
    let now = Instant::now();
    let mut schedule = WakeSchedule::new(now);
    let mut effects = TaskEffects::default();
    let budget = FrameBudget {
        frame_interval: Duration::from_millis(16),
        can_render: false,
    };
    apply_task(
        &task(Redraw::IfCanRender, Wake::IdleSpin),
        budget,
        &mut schedule,
        &mut effects,
    );
    assert!(!effects.needs_redraw);
    assert_eq!(schedule.next_wake, now + Duration::from_millis(100));
    apply_task(
        &task(Redraw::Always, Wake::After(Duration::from_secs(1))),
        budget,
        &mut schedule,
        &mut effects,
    );
    assert!(effects.needs_redraw);
    assert_eq!(schedule.next_wake, now + Duration::from_millis(100));
    apply_task(
        &task(Redraw::Never, Wake::FrameInterval),
        budget,
        &mut schedule,
        &mut effects,
    );
    assert!(effects.needs_redraw);
    assert_eq!(schedule.next_wake, now + Duration::from_millis(16));
}

#[test]
fn task_effects_are_order_independent_and_poll_survives_other_rows() {
    let now = Instant::now();
    let budget = FrameBudget {
        frame_interval: Duration::from_millis(16),
        can_render: true,
    };
    let mut transfer = task(Redraw::Always, Wake::After(Duration::from_secs(1)));
    transfer.needs_poll = true;
    transfer.bypass_render_delay = true;
    let agent = task(Redraw::Never, Wake::IdleSpin);
    let frame = task(Redraw::IfCanRender, Wake::FrameInterval);
    let fold = |tasks: [&PollTask; 3]| {
        let mut schedule = WakeSchedule::new(now);
        let mut effects = TaskEffects::default();
        for t in tasks {
            apply_task(t, budget, &mut schedule, &mut effects);
        }
        (effects, schedule.next_wake)
    };
    let forward = fold([&transfer, &agent, &frame]);
    assert_eq!(forward, fold([&frame, &agent, &transfer]));
    assert_eq!(
        forward.0,
        TaskEffects {
            needs_redraw: true,
            needs_poll: true,
            bypass_render_delay: true
        }
    );
    assert_eq!(forward.1, now + budget.frame_interval);
}
