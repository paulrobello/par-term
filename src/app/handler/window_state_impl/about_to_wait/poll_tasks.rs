//! The declarative poll-task table for `about_to_wait` (AUDIT.md ARC-002).
//!
//! A poll task is a per-window condition that, while it holds, needs the
//! event loop to come back sooner than the 1 s idle default and/or needs a
//! frame drawn. Each row says three things: the **gate** (is the task live
//! this iteration), the **redraw policy**, and the **wake request**. Adding a
//! poller means adding a row here, not another branch in `about_to_wait`.
//!
//! Rows are order-independent: every effect is monotone (a redraw is only
//! ever requested, never withdrawn, and a wake deadline only ever moves
//! earlier), so the table may be evaluated in any order relative to itself
//! and to the timer phases in `about_to_wait`. Work that is NOT monotone,
//! or whose deadline depends on the last render time, stays a phase
//! function in the parent module: render-delay batching, cursor blink,
//! overlay timers, the animated background shader, and anti-idle.
//!
//! Applying a row ([`apply_task`]) is pure over [`FrameBudget`] and
//! [`WakeSchedule`], so it is unit-tested without a `WindowState`.

use std::time::{Duration, Instant};

use crate::app::window_state::WindowState;

use super::UNFOCUSED_IDLE_SPIN_SLEEP_MS;

/// What the drain phase reports that a gate needs to read.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DrainOutcome {
    /// A background integrations install (shaders / shell integration) is
    /// still running.
    pub(super) integrations_install_running: bool,
}

/// The render cadence for this iteration, computed once.
#[derive(Clone, Copy, Debug)]
pub(super) struct FrameBudget {
    /// The frame interval at the current focus/power state.
    pub(super) frame_interval: Duration,
    /// Enough time has passed since the last render to draw another frame.
    pub(super) can_render: bool,
}

/// The earliest instant this window must be woken, folded by `min`.
#[derive(Clone, Copy, Debug)]
pub(super) struct WakeSchedule {
    pub(super) now: Instant,
    pub(super) next_wake: Instant,
}

impl WakeSchedule {
    /// Idle default: one second of inactivity.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            now,
            next_wake: now + Duration::from_secs(1),
        }
    }

    /// Move the deadline earlier to `at`; a later `at` is ignored.
    pub(super) fn wake_by(&mut self, at: Instant) {
        if at < self.next_wake {
            self.next_wake = at;
        }
    }
}

/// Whether a live task asks for a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Redraw {
    /// Wake only; the task draws nothing itself.
    Never,
    /// Draw when the FPS throttle allows.
    IfCanRender,
    /// Draw regardless of the FPS throttle (interaction and progress UI).
    Always,
}

/// When a live task needs the loop back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wake {
    /// No deadline: the redraw (or the event that produced it) suffices.
    None,
    /// One frame interval from now.
    FrameInterval,
    /// The unfocused idle-spin cadence: cheap enough to poll a queue that
    /// nothing else wakes the loop for.
    IdleSpin,
    /// A fixed span from now.
    After(Duration),
}

/// One row of the table.
pub(super) struct PollTask {
    pub(super) name: &'static str,
    pub(super) gate: fn(&mut WindowState, &DrainOutcome) -> bool,
    pub(super) redraw: Redraw,
    pub(super) wake: Wake,
    /// Ask for `ControlFlow::Poll`: on macOS `WaitUntil` starves
    /// `RedrawRequested` while PTY events keep the loop busy.
    pub(super) needs_poll: bool,
    /// Draw even while flicker/throughput batching is delaying renders.
    pub(super) bypass_render_delay: bool,
}

/// The accumulated effect of every live row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TaskEffects {
    pub(super) needs_redraw: bool,
    pub(super) needs_poll: bool,
    pub(super) bypass_render_delay: bool,
}

/// Fold one live row's effects into `effects` and `schedule`.
pub(super) fn apply_task(
    task: &PollTask,
    budget: FrameBudget,
    schedule: &mut WakeSchedule,
    effects: &mut TaskEffects,
) {
    match task.redraw {
        Redraw::Never => {}
        Redraw::IfCanRender => effects.needs_redraw |= budget.can_render,
        Redraw::Always => effects.needs_redraw = true,
    }
    let now = schedule.now;
    match task.wake {
        Wake::None => {}
        Wake::FrameInterval => schedule.wake_by(now + budget.frame_interval),
        Wake::IdleSpin => {
            schedule.wake_by(now + Duration::from_millis(UNFOCUSED_IDLE_SPIN_SLEEP_MS))
        }
        Wake::After(span) => schedule.wake_by(now + span),
    }
    effects.needs_poll |= task.needs_poll;
    effects.bypass_render_delay |= task.bypass_render_delay;
}

/// Evaluate every row's gate and apply the live ones. The redraw request
/// lands on the window before returning; poll and bypass are returned for
/// the trigger step.
pub(super) fn run_poll_tasks(
    ws: &mut WindowState,
    drain: &DrainOutcome,
    budget: FrameBudget,
    schedule: &mut WakeSchedule,
) -> TaskEffects {
    let mut effects = TaskEffects::default();
    for task in POLL_TASKS {
        if (task.gate)(ws, drain) {
            crate::debug_trace!("EVENT_LOOP", "poll task live: {}", task.name);
            apply_task(task, budget, schedule, &mut effects);
        }
    }
    if effects.needs_redraw {
        ws.focus_state.needs_redraw = true;
    }
    effects
}

/// The table. Rationale for each row lives on its gate.
pub(super) const POLL_TASKS: &[PollTask] = &[
    PollTask {
        name: "smooth-scroll animation",
        gate: scroll_animating,
        redraw: Redraw::IfCanRender,
        wake: Wake::FrameInterval,
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "visual bell fade",
        gate: visual_bell_flashing,
        redraw: Redraw::IfCanRender,
        wake: Wake::FrameInterval,
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "selection / scrollbar drag",
        gate: pointer_dragging,
        redraw: Redraw::Always,
        wake: Wake::None,
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "shader install spinner",
        gate: install_in_progress,
        redraw: Redraw::Always,
        wake: Wake::After(Duration::from_millis(16)),
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "ACP agent queue",
        gate: agent_is_live,
        redraw: Redraw::Never,
        wake: Wake::IdleSpin,
        needs_poll: false,
        bypass_render_delay: false,
    },
    #[cfg(feature = "mux")]
    PollTask {
        name: "par-mux off-loop jobs",
        gate: mux_jobs_in_flight,
        redraw: Redraw::Never,
        wake: Wake::IdleSpin,
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "script command queue",
        gate: script_is_live,
        redraw: Redraw::Never,
        wake: Wake::IdleSpin,
        needs_poll: false,
        bypass_render_delay: false,
    },
    PollTask {
        name: "file transfer progress",
        gate: file_transfers_active,
        redraw: Redraw::Always,
        wake: Wake::After(Duration::from_secs(1)),
        needs_poll: true,
        bypass_render_delay: true,
    },
];

/// A scroll interpolation or terminal animation on the active tab.
fn scroll_animating(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.tab_manager
        .active_tab()
        .is_some_and(|tab| tab.active_scroll_state().animation_start.is_some())
}

/// Hold frame rate through the visual-bell flash fade-out.
fn visual_bell_flashing(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.tab_manager
        .active_tab()
        .is_some_and(|tab| tab.active_bell().visual_flash.is_some())
}

/// Text selection or scrollbar drag with the button held: always draw,
/// even when throttled, for responsiveness.
fn pointer_dragging(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.tab_manager.active_tab().is_some_and(|tab| {
        let sm = tab.selection_mouse();
        (sm.is_selecting || sm.selection.is_some() || tab.active_scroll_state().dragging)
            && tab.active_mouse().button_pressed
    })
}

/// The shader install dialog's spinner, and an install whose completion
/// the drain phase must observe.
fn install_in_progress(ws: &mut WindowState, drain: &DrainOutcome) -> bool {
    ws.overlay_ui.shader_install_ui.visible || drain.integrations_install_running
}

/// The drain phase only runs when the loop wakes, and two ACP message
/// variants are synchronous RPCs the agent is blocked on, so bound that
/// wait while an agent is live. Only the first message after a quiet
/// period pays this latency: each drained message re-arms `needs_redraw`,
/// so a streaming agent already keeps the loop at frame rate. The cadence
/// matches the unfocused idle spin because each wake is a full pass (the
/// request-file checks stat the filesystem).
///
/// `agent_rx` stays `Some` after an agent exits on its own (only an
/// explicit disconnect clears it), so the status gates too — otherwise one
/// Assistant-panel session would hold the faster cadence for the life of
/// the window.
fn agent_is_live(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.agent_state.agent_rx.is_some()
        && !matches!(
            ws.overlay_ui.ai_inspector.agent_status,
            par_term_acp::AgentStatus::Disconnected | par_term_acp::AgentStatus::Error(_)
        )
}

/// par-mux window discovery and screen seeds run on the transport's
/// worker, and nothing wakes the loop when one finishes.
#[cfg(feature = "mux")]
fn mux_jobs_in_flight(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.tmux_state.mux_jobs_in_flight()
}

/// `WindowManager::about_to_wait` drives the script command loop once per
/// wake, and nothing schedules the wake that *looks* for a new command
/// (commands that change the screen already `request_redraw`). Gate on the
/// process actually running — an exited script must not hold the faster
/// cadence. Every tab counts and focus does not: `sync_script_running_state`
/// services every tab of every window.
fn script_is_live(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    ws.tab_manager.tabs_mut().iter_mut().any(|tab| {
        (0..tab.scripting.script_ids.len()).any(|i| match tab.scripting.script_ids[i] {
            Some(id) => tab.scripting.script_manager.is_running(id),
            None => false,
        })
    })
}

/// Active or recently finished transfers keep the progress overlay
/// drawing at 1 FPS, request `Poll` (see [`PollTask::needs_poll`]), and
/// bypass render batching for responsive feedback.
fn file_transfers_active(ws: &mut WindowState, _: &DrainOutcome) -> bool {
    !ws.file_transfer_state.active_uploads.is_empty()
        || !ws.file_transfer_state.recent_transfers.is_empty()
}
