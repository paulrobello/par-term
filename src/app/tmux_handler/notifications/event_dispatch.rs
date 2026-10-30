//! Typed dispatch boundary shared by the tmux gateway and par-mux daemon.
//!
//! Transport/parser details end at `TmuxNotification`. Classification into
//! `EventBatch` is exhaustive and independent of `WindowState`. Consumers
//! apply direct events, then structural events, layouts, and output in that
//! order. par-mux defers focus until layouts have created the new pane map.
//! Daemon-only roster, telemetry, and held-pane events are still extracted
//! upstream of this boundary by `mux_drain`.

use crate::app::window_state::WindowState;
use crate::tmux::TmuxNotification;
use par_term_tmux::TmuxPaneId;

/// Events that cannot be translated through `TmuxSync`.
pub(super) enum DirectEvent {
    ControlModeStarted,
    SessionStarted(String),
    SessionRenamed(String),
    PaneFocusChanged(TmuxPaneId),
    Error(String),
    Notification { title: String, body: String },
}

/// Dependency-ordered groups. Input order is preserved within each group.
#[derive(Default)]
pub(super) struct EventBatch {
    pub(super) direct: Vec<DirectEvent>,
    pub(super) session: Vec<TmuxNotification>,
    pub(super) layout: Vec<TmuxNotification>,
    pub(super) output: Vec<TmuxNotification>,
    pub(super) other: Vec<TmuxNotification>,
}

impl EventBatch {
    pub(super) fn classify(notifications: Vec<TmuxNotification>) -> Self {
        let mut batch = Self::default();
        for notification in notifications {
            match notification {
                TmuxNotification::ControlModeStarted => {
                    batch.direct.push(DirectEvent::ControlModeStarted);
                }
                TmuxNotification::SessionStarted(name) => {
                    batch.direct.push(DirectEvent::SessionStarted(name));
                }
                TmuxNotification::SessionRenamed(name) => {
                    batch.direct.push(DirectEvent::SessionRenamed(name));
                }
                TmuxNotification::PaneFocusChanged { pane_id } => {
                    batch.direct.push(DirectEvent::PaneFocusChanged(pane_id));
                }
                TmuxNotification::Error(message) => {
                    batch.direct.push(DirectEvent::Error(message));
                }
                TmuxNotification::Notification { title, body } => {
                    batch.direct.push(DirectEvent::Notification { title, body });
                }
                event @ (TmuxNotification::WindowAdd(_)
                | TmuxNotification::WindowClose(_)
                | TmuxNotification::WindowRenamed { .. }
                | TmuxNotification::SessionEnded) => batch.session.push(event),
                event @ TmuxNotification::LayoutChange { .. } => batch.layout.push(event),
                event @ TmuxNotification::Output { .. } => batch.output.push(event),
                event @ (TmuxNotification::Pause | TmuxNotification::Continue) => {
                    batch.other.push(event);
                }
            }
        }
        batch
    }
}

/// The two backends differ only at session start and pane focus: mux focus
/// must wait for this batch's layouts, whereas gateway focus applies now.
#[derive(Clone, Copy)]
pub(super) enum EventSource {
    Gateway,
    #[cfg(feature = "mux")]
    Mux,
}

#[derive(Default)]
pub(super) struct DirectEffects {
    pub(super) needs_redraw: bool,
    #[cfg(feature = "mux")]
    pub(super) deferred_focus: Option<TmuxPaneId>,
}

/// Apply typed direct events without adding another dispatcher to the
/// WindowState facade. Notification delivery remains injectable for the
/// daemon's attention tests. Errors intentionally do not request redraw,
/// matching the original handlers' contract.
pub(super) fn dispatch_direct(
    ws: &mut WindowState,
    events: Vec<DirectEvent>,
    source: EventSource,
    mut notify: impl FnMut(&WindowState, &str, &str),
) -> DirectEffects {
    let mut effects = DirectEffects::default();
    for event in events {
        match event {
            DirectEvent::ControlModeStarted => {
                crate::debug_info!("TMUX", "Control mode started");
            }
            DirectEvent::SessionStarted(name) => {
                match source {
                    EventSource::Gateway => ws.handle_tmux_session_started(&name),
                    #[cfg(feature = "mux")]
                    EventSource::Mux => ws.handle_mux_session_started(&name),
                }
                effects.needs_redraw = true;
            }
            DirectEvent::SessionRenamed(name) => {
                ws.handle_tmux_session_renamed(&name);
                effects.needs_redraw = true;
            }
            DirectEvent::PaneFocusChanged(pane_id) => {
                match source {
                    EventSource::Gateway => ws.handle_tmux_pane_focus_changed(pane_id),
                    #[cfg(feature = "mux")]
                    EventSource::Mux => effects.deferred_focus = Some(pane_id),
                }
                effects.needs_redraw = true;
            }
            DirectEvent::Notification { title, body } => {
                notify(ws, &title, &body);
                effects.needs_redraw = true;
            }
            DirectEvent::Error(message) => ws.handle_tmux_error(&message),
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_preserves_structural_order_and_types_direct_events() {
        let batch = EventBatch::classify(vec![
            TmuxNotification::Output {
                pane_id: 4,
                data: b"first".to_vec(),
            },
            TmuxNotification::WindowAdd(2),
            TmuxNotification::PaneFocusChanged { pane_id: 4 },
            TmuxNotification::Pause,
            TmuxNotification::WindowClose(3),
            TmuxNotification::SessionStarted("name".to_string()),
            TmuxNotification::Output {
                pane_id: 4,
                data: b"second".to_vec(),
            },
        ]);
        assert!(
            matches!(batch.direct.as_slice(), [DirectEvent::PaneFocusChanged(4), DirectEvent::SessionStarted(name)] if name == "name")
        );
        assert!(matches!(
            batch.session.as_slice(),
            [
                TmuxNotification::WindowAdd(2),
                TmuxNotification::WindowClose(3)
            ]
        ));
        assert!(
            matches!(batch.output.as_slice(), [TmuxNotification::Output { data: a, .. }, TmuxNotification::Output { data: b, .. }] if a == b"first" && b == b"second")
        );
        assert!(batch.layout.is_empty());
        assert!(matches!(batch.other.as_slice(), [TmuxNotification::Pause]));
    }
}
