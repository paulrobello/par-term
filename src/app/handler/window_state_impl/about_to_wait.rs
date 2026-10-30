//! Per-frame update logic for WindowState (`about_to_wait`).
//!
//! `about_to_wait` runs once per event-loop iteration as a fixed sequence of
//! phase functions (AUDIT.md ARC-002):
//!
//! 1. [`drain_queues`] — consume everything that arrived since the last
//!    iteration. Its call order is the queue-priority contract; see there.
//! 2. [`frame_budget`] — the render cadence, computed once.
//! 3. Render-delay batching — [`flicker_delay`], [`throughput_delay`].
//! 4. Timers — cursor blink, overlay/toast/leader timers, the animated
//!    background shader, anti-idle.
//! 5. [`poll_tasks::run_poll_tasks`] — the declarative poll-task table
//!    (gate, redraw policy, wake request). A new poller is a new row there.
//! 6. [`self_heal_redraw`] — re-arm a frame stranded by the FPS gate. Runs
//!    after every phase above because it reads their `needs_redraw`.
//! 7. [`trigger_and_report`] — issue the redraw and build the
//!    [`WakeRequest`]. Last, because the idle-sleep cap reads the final
//!    redraw state.
//!
//! Nothing here touches `ControlFlow`: each window reports a
//! [`WakeRequest`] and `WindowManager::about_to_wait` reduces them (see
//! `crate::app::handler::wake`).

mod poll_tasks;
#[cfg(test)]
mod poll_tasks_tests;

use std::time::{Duration, Instant};

use crate::app::handler::wake::WakeRequest;
use crate::app::window_state::WindowState;

use poll_tasks::{DrainOutcome, FrameBudget, TaskEffects, WakeSchedule};

/// Longest explicit sleep taken per idle iteration when the window is focused.
const FOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 50;
/// Longest explicit sleep taken per idle iteration when the window is unfocused.
const UNFOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 100;

impl WindowState {
    /// Process per-window updates in about_to_wait.
    ///
    /// Returns what this window wants from the event loop, or `None` when it is
    /// shutting down and wants nothing. Nothing here touches `ControlFlow`:
    /// there is one event loop and many windows, so the requests are reduced to
    /// a single decision by `WindowManager::about_to_wait`. Setting it here made
    /// the last window iterated decide the cadence for every window.
    #[must_use]
    pub(crate) fn about_to_wait(&mut self) -> Option<WakeRequest> {
        if self.is_shutting_down {
            return None;
        }
        trace_loop_gap(self);

        let drain = drain_queues(self);

        let now = Instant::now();
        let mut schedule = WakeSchedule::new(now);
        let budget = frame_budget(self, now);

        let delay_for_flicker = flicker_delay(self, budget, &mut schedule);
        let delay_for_throughput = throughput_delay(self, &mut schedule);
        let should_delay_render = delay_for_throughput || delay_for_flicker;

        tick_cursor_blink(self, budget, &mut schedule);
        tick_overlay_timers(self, budget, &mut schedule);
        tick_background_shader(self, budget, &mut schedule);
        let effects = poll_tasks::run_poll_tasks(self, &drain, budget, &mut schedule);
        if let Some(next_anti_idle) = self.handle_anti_idle(now) {
            schedule.wake_by(next_anti_idle);
        }

        self_heal_redraw(self, budget, &mut schedule);
        Some(trigger_and_report(
            self,
            should_delay_render,
            effects,
            schedule,
        ))
    }
}

/// Log an inter-frame gap over 100 ms (event-loop stall detection) and the
/// periodic `try_lock` telemetry summary (two atomic loads when quiet).
fn trace_loop_gap(ws: &mut WindowState) {
    let t0 = Instant::now();
    if let Some(prev) = ws.debug.last_about_to_wait {
        let gap_ms = prev.elapsed().as_millis();
        if gap_ms > 100 {
            crate::debug_info!(
                "EVENT_LOOP",
                "about_to_wait gap: {}ms since last call",
                gap_ms
            );
        }
    }
    ws.debug.last_about_to_wait = Some(t0);
    crate::debug::maybe_log_try_lock_telemetry();
}

/// Phase 1: drain every inbound queue, in priority order.
///
/// The order is a contract, not a convenience:
///
/// - The ACP agent queue drains **first** and unconditionally (never gated
///   on a frame being drawn): two of its three inbound variants are
///   synchronous RPCs the agent subprocess is blocked on, and every
///   `needs_redraw = true` in its handler sits after the drain, so a
///   render-path drain could never start itself.
/// - A finished integrations install is applied here for the same reason.
/// - Orphaned pending actions are pruned and approved targeted actions run
///   **before** `check_trigger_actions`, which resolves against the active
///   tab and gives up early when that tab's terminal lock is contended —
///   neither condition applies to a targeted action.
/// - tmux/par-mux notifications are applied **before** the window-title and
///   badge sync, so those read this iteration's shell-integration state.
fn drain_queues(ws: &mut WindowState) -> DrainOutcome {
    ws.process_agent_messages_tick();
    let integrations_install_running = ws.poll_integrations_install();

    ws.check_notifications();
    ws.check_notification_clicks();
    ws.check_file_transfers();
    ws.check_bell();
    ws.check_pointer_shape();

    ws.prune_orphaned_pending_actions();
    ws.execute_approved_targeted_actions();
    ws.check_trigger_actions();

    ws.check_clipboard_sync();
    ws.check_activity_idle_notifications();
    ws.check_session_exit_notifications();

    if ws.check_shader_reload() {
        log::debug!("Shader hot reload triggered redraw");
    }

    // Config file changes (e.g. from an ACP agent) and the MCP request files.
    ws.check_config_reload();
    ws.check_config_update_file();
    ws.check_screenshot_request_file();
    ws.check_shader_diagnostics_request_file();

    // Agent-authored commands: reload when MCP command_create/delete wrote
    // the commands dir, then run what the confirmation dialog approved
    // (drained here — the egui closure cannot run &mut self executors).
    if ws.agent_commands.poll() {
        ws.focus_state.needs_redraw = true;
    }
    crate::agent_commands_store::run_approved_agent_commands(ws);

    if ws.check_tmux_notifications() {
        ws.focus_state.needs_redraw = true;
    }
    ws.update_window_title_with_shell_integration();
    ws.sync_badge_shell_integration();

    if ws.check_auto_profile_switch() {
        ws.focus_state.needs_redraw = true;
    }

    DrainOutcome {
        integrations_install_running,
    }
}

/// Phase 2: the frame interval (slower when unfocused with
/// `pause_refresh_on_blur`) and whether the FPS throttle allows a frame now.
fn frame_budget(ws: &WindowState, now: Instant) -> FrameBudget {
    let config = ws.config.load();
    let fps = if config.power.pause_refresh_on_blur && !ws.focus_state.is_focused {
        config.power.unfocused_fps
    } else {
        config.rendering.max_fps
    };
    let frame_interval = Duration::from_millis(u64::from(1000 / fps.max(1)));
    // No render yet: allow one immediately.
    let since_last_render = ws
        .focus_state
        .last_render_time
        .map_or(frame_interval, |t| now.duration_since(t));
    FrameBudget {
        frame_interval,
        can_render: since_last_render >= frame_interval,
    }
}

/// Phase 3a — flicker reduction: while the cursor is hidden (bulk output),
/// delay rendering up to `reduce_flicker_delay_ms` to batch updates, unless
/// a modal or the resize overlay is up. Returns whether to delay.
fn flicker_delay(ws: &mut WindowState, budget: FrameBudget, schedule: &mut WakeSchedule) -> bool {
    let now = schedule.now;
    let config = ws.config.load();
    let delay = Duration::from_millis(u64::from(config.rendering.reduce_flicker_delay_ms));
    let should_delay = config.rendering.reduce_flicker && {
        // try_lock: intentional — about_to_wait runs on the sync event loop.
        // On a miss assume the cursor is visible, so rendering is never
        // delayed on a guess (conservative; never a stale frame).
        let lock_visibility = config.cursor.lock_cursor_visibility;
        let cursor_hidden = ws
            .tab_manager
            .active_tab()
            .and_then(|tab| {
                tab.try_with_terminal_mut(|term| !term.is_cursor_visible() && !lock_visibility)
            })
            .unwrap_or(false);

        if cursor_hidden {
            let hidden_since = *ws.focus_state.cursor_hidden_since.get_or_insert(now);
            let delay_expired = now.duration_since(hidden_since) >= delay;
            let any_ui_visible =
                ws.any_modal_ui_visible() || ws.overlay_state.resize_overlay_visible;
            !delay_expired && !any_ui_visible
        } else {
            // Cursor visible again: render what accumulated.
            if ws.focus_state.cursor_hidden_since.take().is_some() {
                ws.focus_state.flicker_pending_render = false;
                ws.focus_state.needs_redraw = true;
            }
            false
        }
    };

    if should_delay {
        ws.focus_state.flicker_pending_render = true;
        if let Some(hidden_since) = ws.focus_state.cursor_hidden_since {
            schedule.wake_by(hidden_since + delay);
        }
    } else if ws.focus_state.flicker_pending_render {
        ws.focus_state.flicker_pending_render = false;
        if budget.can_render {
            ws.focus_state.needs_redraw = true;
        }
    }
    should_delay
}

/// Phase 3b — throughput mode: with `maximize_throughput`, batch renders on
/// a fixed `throughput_render_interval_ms` regardless of cursor state.
/// Returns whether to delay.
fn throughput_delay(ws: &mut WindowState, schedule: &mut WakeSchedule) -> bool {
    let now = schedule.now;
    let config = ws.config.load();
    if !config.rendering.maximize_throughput {
        ws.focus_state.throughput_batch_start = None;
        return false;
    }
    let interval = Duration::from_millis(u64::from(config.rendering.throughput_render_interval_ms));
    let batch_start = *ws.focus_state.throughput_batch_start.get_or_insert(now);
    if now.duration_since(batch_start) >= interval {
        // Interval elapsed: render, and start the next batch.
        ws.focus_state.throughput_batch_start = Some(now);
        return false;
    }
    schedule.wake_by(batch_start + interval);
    true
}

/// Phase 4a — cursor blink: wake exactly at the next toggle. Paused when
/// unfocused with `pause_refresh_on_blur`.
fn tick_cursor_blink(ws: &mut WindowState, budget: FrameBudget, schedule: &mut WakeSchedule) {
    let config = ws.config.load();
    if !config.cursor.cursor_blink
        || !(ws.focus_state.is_focused || !config.power.pause_refresh_on_blur)
    {
        return;
    }
    let now = schedule.now;
    let blink_interval = Duration::from_millis(config.cursor.cursor_blink_interval);
    let next_blink = *ws
        .cursor_anim
        .cursor_blink_timer
        .get_or_insert(now + blink_interval);
    if now >= next_blink {
        if budget.can_render {
            ws.focus_state.needs_redraw = true;
        }
        ws.cursor_anim.cursor_blink_timer = Some(now + blink_interval);
    } else {
        schedule.wake_by(next_blink);
    }
}

/// A hide-at-deadline overlay: past the deadline it reports `true` (the
/// caller hides it and redraws); before it, it draws if throttling allows
/// and wakes at the deadline.
fn tick_hide_deadline(
    ws: &mut WindowState,
    hide_time: Instant,
    budget: FrameBudget,
    schedule: &mut WakeSchedule,
) -> bool {
    if schedule.now >= hide_time {
        ws.focus_state.needs_redraw = true;
        return true;
    }
    if budget.can_render {
        ws.focus_state.needs_redraw = true;
    }
    schedule.wake_by(hide_time);
    false
}

/// Phase 4b — overlay timers: the resize overlay, the toast stack (UX.md
/// OV7; errors never expire), the leader which-key (UX.md K4), the pane
/// index overlay, stale pane-hint/resize modes, and session-undo expiry.
fn tick_overlay_timers(ws: &mut WindowState, budget: FrameBudget, schedule: &mut WakeSchedule) {
    let now = schedule.now;

    if ws.overlay_state.resize_overlay_visible
        && let Some(hide_time) = ws.overlay_state.resize_overlay_hide_time
        && tick_hide_deadline(ws, hide_time, budget, schedule)
    {
        ws.overlay_state.resize_overlay_visible = false;
        ws.overlay_state.resize_overlay_hide_time = None;
    }

    let (toasts_changed, next_toast_expiry) = ws.overlay_state.toasts.expire(now);
    if toasts_changed {
        ws.focus_state.needs_redraw = true;
    }
    if let Some(expiry) = next_toast_expiry {
        if budget.can_render {
            ws.focus_state.needs_redraw = true;
        }
        schedule.wake_by(expiry);
    }

    if let Some(leader_wake) = ws.tick_leader(now) {
        schedule.wake_by(leader_wake);
    }

    if let Some(hide_time) = ws.overlay_state.pane_identify_hide_time
        && tick_hide_deadline(ws, hide_time, budget, schedule)
    {
        ws.overlay_state.pane_identify_hide_time = None;
    }

    // Pane-hint badges describe one tab only: cancel when it is no longer
    // active (switched or closed).
    ws.cancel_pane_hint_select_if_stale();
    ws.cancel_pane_resize_mode_if_stale();

    let undo_timeout_secs = ws.config.load().session_restore.session_undo_timeout_secs;
    if !ws.overlay_state.closed_tabs.is_empty() && undo_timeout_secs > 0 {
        let timeout = Duration::from_secs(u64::from(undo_timeout_secs));
        ws.overlay_state
            .closed_tabs
            .retain(|info| now.duration_since(info.closed_at) < timeout);
    }
}

/// Phase 4c — animated custom background shader: draw at the frame
/// interval (the unfocused rate under `pause_refresh_on_blur`), scheduled
/// from the last render and never in the past.
fn tick_background_shader(ws: &mut WindowState, budget: FrameBudget, schedule: &mut WakeSchedule) {
    if !ws
        .renderer
        .as_ref()
        .is_some_and(|r| r.needs_continuous_render())
    {
        return;
    }
    if budget.can_render {
        ws.focus_state.needs_redraw = true;
    }
    let now = schedule.now;
    let next_frame = ws
        .focus_state
        .last_render_time
        .map_or(now, |t| t + budget.frame_interval);
    let next_frame = if next_frame <= now {
        now + budget.frame_interval
    } else {
        next_frame
    };
    schedule.wake_by(next_frame);
}

/// Phase 6: re-arm a frame the FPS gate stranded, or wake at the earliest
/// renderable time so the gap self-heals. Three cases:
///
/// 1. Renderer dirty state set by a window event (e.g. `Focused(false)`)
///    whose render the FPS gate skipped, with nothing else to re-arm it.
/// 2. Pending egui input: a `RedrawRequested` rejected by
///    `should_render_frame()` strands events already fed to `egui_winit`
///    (a tab click's press+release) — the "click tab twice" bug.
/// 3. Stale-cell retry: the last render served cells older than the live
///    terminal (core lock contended); the edge-triggered output heartbeat
///    has moved on, so re-arm until a fresh gather catches up.
fn self_heal_redraw(ws: &mut WindowState, budget: FrameBudget, schedule: &mut WakeSchedule) {
    let renderer_dirty = ws.renderer.as_ref().is_some_and(|r| r.is_dirty());
    let stranded = renderer_dirty
        || ws.focus_state.pending_egui_repaint
        || ws.focus_state.stale_cells_pending_retry;
    if ws.focus_state.needs_redraw || !stranded {
        return;
    }
    match ws.focus_state.last_render_time {
        Some(last_render) if !budget.can_render => {
            schedule.wake_by(last_render + budget.frame_interval);
        }
        _ => ws.focus_state.needs_redraw = true,
    }
}

/// Phase 7: request the redraw unless batching delays it (file transfers
/// bypass the delay), then report the cadence.
///
/// `needs_poll`: `WaitUntil` prevents `RedrawRequested` delivery on macOS
/// while PTY events keep the loop busy. `idle_sleep_cap`: on macOS
/// `WaitUntil` does not always stop the loop spinning (CVDisplayLink /
/// NSRunLoop), so an idle window offers an explicit sleep — only a cap, the
/// reducer sleeps once for the shortest span any window tolerates. The cap
/// is independent of `max_fps`: tying it to the frame interval wakes idle
/// focused windows at render cadence.
fn trigger_and_report(
    ws: &mut WindowState,
    should_delay_render: bool,
    effects: TaskEffects,
    schedule: WakeSchedule,
) -> WakeRequest {
    let mut redraw_requested = false;
    if ws.focus_state.needs_redraw && (!should_delay_render || effects.bypass_render_delay) {
        ws.request_redraw();
        ws.focus_state.needs_redraw = false;
        redraw_requested = true;
    }

    let idle_sleep_cap = (!ws.focus_state.needs_redraw && !redraw_requested).then(|| {
        let max_idle_spin_sleep = if ws.focus_state.is_focused {
            FOCUSED_IDLE_SPIN_SLEEP_MS
        } else {
            UNFOCUSED_IDLE_SPIN_SLEEP_MS
        };
        schedule.now + Duration::from_millis(max_idle_spin_sleep)
    });

    WakeRequest {
        needs_poll: effects.needs_poll,
        wake_at: schedule.next_wake,
        idle_sleep_cap,
    }
}
