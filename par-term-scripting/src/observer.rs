//! Observer bridge that converts core library `TerminalEvent`s into scripting `ScriptEvent`s.
//!
//! [`ScriptEventForwarder`] implements the `TerminalObserver` trait from
//! `par-term-emu-core-rust`.  It captures events into a thread-safe buffer
//! that the main event loop drains and forwards to script sub-processes.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use par_term_emu_core_rust::observer::TerminalObserver;
use par_term_emu_core_rust::terminal::TerminalEvent;

use super::protocol::{ScriptEvent, ScriptEventData};

/// Maximum events retained in a forwarder's buffer.
///
/// The buffer is filled from the PTY reader thread and emptied by the event
/// loop, so anything that stops the loop draining a particular forwarder makes
/// it grow for the life of the process. A script that exits on its own leaves
/// its observer registered — only an explicit stop unregisters it — so that
/// case is reachable no matter how thoroughly the event loop sweeps its tabs,
/// and this cap is the only thing bounding it.
const MAX_BUFFERED_EVENTS: usize = 1024;

/// The buffered events plus the once-only overflow latch, behind one lock.
struct EventBuffer {
    events: VecDeque<ScriptEvent>,
    /// Set the first time the cap is reached, so overflow is reported once per
    /// forwarder instead of once per discarded event. Never cleared: a drain
    /// does not mean the underlying leak was fixed, and re-arming would let a
    /// forwarder that overflows and drains repeatedly flood the log.
    overflow_reported: bool,
}

/// Bridge between core terminal observer events and the scripting JSON protocol.
///
/// Register an `Arc<ScriptEventForwarder>` with `Terminal::add_observer()`.
/// The forwarder buffers converted events; the owner drains them via
/// [`drain_events`] and serialises them to script sub-processes.
pub struct ScriptEventForwarder {
    /// Optional subscription filter expressed as snake_case kind names.
    /// `None` means "forward everything".
    subscription_filter: Option<HashSet<String>>,
    /// Thread-safe event buffer (uses std Mutex since observer callbacks are
    /// invoked from the PTY reader thread), capped at [`MAX_BUFFERED_EVENTS`].
    event_buffer: Mutex<EventBuffer>,
}

impl ScriptEventForwarder {
    /// Create a new forwarder.
    ///
    /// # Arguments
    /// * `subscriptions` - If `Some`, only events whose snake_case kind name
    ///   is in the set will be captured. If `None`, all events are captured.
    pub fn new(subscriptions: Option<HashSet<String>>) -> Self {
        Self {
            subscription_filter: subscriptions,
            event_buffer: Mutex::new(EventBuffer {
                events: VecDeque::new(),
                overflow_reported: false,
            }),
        }
    }

    /// Drain all buffered events, returning them and clearing the buffer.
    pub fn drain_events(&self) -> Vec<ScriptEvent> {
        let mut buf = self.event_buffer.lock().expect("event_buffer poisoned");
        buf.events.drain(..).collect()
    }

    /// Map a `TerminalEvent` to its snake_case kind name used by the protocol.
    fn event_kind_name(event: &TerminalEvent) -> String {
        match event {
            TerminalEvent::BellRang(_) => "bell_rang".to_string(),
            TerminalEvent::TitleChanged(_) => "title_changed".to_string(),
            TerminalEvent::SizeChanged(_, _) => "size_changed".to_string(),
            TerminalEvent::ModeChanged(_, _) => "mode_changed".to_string(),
            TerminalEvent::GraphicsAdded(_) => "graphics_added".to_string(),
            TerminalEvent::HyperlinkAdded { .. } => "hyperlink_added".to_string(),
            TerminalEvent::DirtyRegion(_, _) => "dirty_region".to_string(),
            TerminalEvent::CwdChanged(_) => "cwd_changed".to_string(),
            TerminalEvent::TriggerMatched(_) => "trigger_matched".to_string(),
            TerminalEvent::UserVarChanged { .. } => "user_var_changed".to_string(),
            TerminalEvent::ProgressBarChanged { .. } => "progress_bar_changed".to_string(),
            TerminalEvent::BadgeChanged(_) => "badge_changed".to_string(),
            TerminalEvent::ShellIntegrationEvent { .. } => "command_complete".to_string(),
            TerminalEvent::ZoneOpened { .. } => "zone_opened".to_string(),
            TerminalEvent::ZoneClosed { .. } => "zone_closed".to_string(),
            TerminalEvent::ZoneScrolledOut { .. } => "zone_scrolled_out".to_string(),
            TerminalEvent::EnvironmentChanged { .. } => "environment_changed".to_string(),
            TerminalEvent::RemoteHostTransition { .. } => "remote_host_transition".to_string(),
            TerminalEvent::SubShellDetected { .. } => "sub_shell_detected".to_string(),
            TerminalEvent::FileTransferStarted { .. } => "file_transfer_started".to_string(),
            TerminalEvent::FileTransferProgress { .. } => "file_transfer_progress".to_string(),
            TerminalEvent::FileTransferCompleted { .. } => "file_transfer_completed".to_string(),
            TerminalEvent::FileTransferFailed { .. } => "file_transfer_failed".to_string(),
            TerminalEvent::UploadRequested { .. } => "upload_requested".to_string(),
            TerminalEvent::ScreenCleared { .. } => "screen_cleared".to_string(),
        }
    }

    /// Convert a core `TerminalEvent` into the scripting protocol `ScriptEvent`.
    fn convert_event(event: &TerminalEvent) -> ScriptEvent {
        let kind = Self::event_kind_name(event);

        let data = match event {
            TerminalEvent::BellRang(_) => ScriptEventData::Empty {},

            TerminalEvent::TitleChanged(title) => ScriptEventData::TitleChanged {
                title: title.clone(),
            },

            TerminalEvent::SizeChanged(cols, rows) => ScriptEventData::SizeChanged {
                cols: *cols,
                rows: *rows,
            },

            TerminalEvent::CwdChanged(cwd_change) => ScriptEventData::CwdChanged {
                cwd: cwd_change.new_cwd.clone(),
            },

            TerminalEvent::UserVarChanged {
                name,
                value,
                old_value,
            } => ScriptEventData::VariableChanged {
                name: name.clone(),
                value: value.clone(),
                old_value: old_value.clone(),
            },

            TerminalEvent::EnvironmentChanged {
                key,
                value,
                old_value,
            } => ScriptEventData::EnvironmentChanged {
                key: key.clone(),
                value: value.clone(),
                old_value: old_value.clone(),
            },

            TerminalEvent::BadgeChanged(text) => {
                ScriptEventData::BadgeChanged { text: text.clone() }
            }

            TerminalEvent::ShellIntegrationEvent {
                command, exit_code, ..
            } => ScriptEventData::CommandComplete {
                command: command.clone().unwrap_or_default(),
                exit_code: *exit_code,
            },

            TerminalEvent::TriggerMatched(trigger_match) => ScriptEventData::TriggerMatched {
                pattern: format!("trigger:{}", trigger_match.trigger_id),
                matched_text: trigger_match.text.clone(),
                line: trigger_match.row,
            },

            TerminalEvent::ZoneOpened {
                zone_id, zone_type, ..
            } => ScriptEventData::ZoneEvent {
                zone_id: *zone_id as u64,
                zone_type: zone_type.to_string(),
                event: "opened".to_string(),
            },

            TerminalEvent::ZoneClosed {
                zone_id, zone_type, ..
            } => ScriptEventData::ZoneEvent {
                zone_id: *zone_id as u64,
                zone_type: zone_type.to_string(),
                event: "closed".to_string(),
            },

            TerminalEvent::ZoneScrolledOut {
                zone_id, zone_type, ..
            } => ScriptEventData::ZoneEvent {
                zone_id: *zone_id as u64,
                zone_type: zone_type.to_string(),
                event: "scrolled_out".to_string(),
            },

            // Fallback: capture arbitrary fields via Debug representation.
            other => {
                let mut fields = HashMap::new();
                fields.insert(
                    "debug".to_string(),
                    serde_json::Value::String(format!("{:?}", other)),
                );
                ScriptEventData::Generic { fields }
            }
        };

        ScriptEvent { kind, data }
    }
}

// The core library's `TerminalEventKind` subscription filter is separate from
// our string-based filter. We implement *both*:
//  1. `subscriptions()` returns `None` so the core dispatches every event to us.
//  2. `on_event()` applies our string-based filter before buffering.
//
// This keeps the filtering logic in one place (the string names match the
// scripting protocol) while still being efficient — the core won't call us
// for events we've filtered via `TerminalEventKind` if we chose to use that,
// but since our filter is string-based we handle it ourselves.

impl TerminalObserver for ScriptEventForwarder {
    fn on_event(&self, event: &TerminalEvent) {
        // Apply string-based subscription filter.
        if let Some(ref filter) = self.subscription_filter {
            let kind = Self::event_kind_name(event);
            if !filter.contains(&kind) {
                return;
            }
        }

        let script_event = Self::convert_event(event);
        let mut buf = self.event_buffer.lock().expect("event_buffer poisoned");

        if buf.events.len() >= MAX_BUFFERED_EVENTS {
            // Drop the oldest rather than the newest. A script acts on what just
            // happened, so discarding the incoming event would hide the very
            // thing that is still worth reacting to and leave a stale prefix.
            buf.events.pop_front();
            if !buf.overflow_reported {
                buf.overflow_reported = true;
                log::warn!(
                    "Script event buffer reached its {} event cap; discarding oldest \
                     events from here on. Nothing is draining this forwarder — the \
                     script most likely exited while its observer stayed registered.",
                    MAX_BUFFERED_EVENTS
                );
            }
        }

        buf.events.push_back(script_event);
    }

    // We do NOT override `subscriptions()` — returning `None` means
    // "interested in all events" at the core level. Our own string filter
    // is applied in `on_event` above.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_kind_name_bell() {
        let event =
            TerminalEvent::BellRang(par_term_emu_core_rust::terminal::BellEvent::VisualBell);
        assert_eq!(ScriptEventForwarder::event_kind_name(&event), "bell_rang");
    }

    #[test]
    fn test_event_kind_name_title() {
        let event = TerminalEvent::TitleChanged("hello".to_string());
        assert_eq!(
            ScriptEventForwarder::event_kind_name(&event),
            "title_changed"
        );
    }

    #[test]
    fn test_convert_bell_event() {
        let event =
            TerminalEvent::BellRang(par_term_emu_core_rust::terminal::BellEvent::VisualBell);
        let script_event = ScriptEventForwarder::convert_event(&event);
        assert_eq!(script_event.kind, "bell_rang");
        assert_eq!(script_event.data, ScriptEventData::Empty {});
    }

    #[test]
    fn test_convert_title_event() {
        let event = TerminalEvent::TitleChanged("My Title".to_string());
        let script_event = ScriptEventForwarder::convert_event(&event);
        assert_eq!(script_event.kind, "title_changed");
        assert_eq!(
            script_event.data,
            ScriptEventData::TitleChanged {
                title: "My Title".to_string(),
            }
        );
    }

    #[test]
    fn test_convert_size_event() {
        let event = TerminalEvent::SizeChanged(120, 40);
        let script_event = ScriptEventForwarder::convert_event(&event);
        assert_eq!(script_event.kind, "size_changed");
        assert_eq!(
            script_event.data,
            ScriptEventData::SizeChanged {
                cols: 120,
                rows: 40,
            }
        );
    }

    #[test]
    fn test_forwarder_no_filter_captures_all() {
        let fwd = ScriptEventForwarder::new(None);
        let bell = TerminalEvent::BellRang(par_term_emu_core_rust::terminal::BellEvent::VisualBell);
        let title = TerminalEvent::TitleChanged("t".to_string());

        fwd.on_event(&bell);
        fwd.on_event(&title);

        let events = fwd.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "bell_rang");
        assert_eq!(events[1].kind, "title_changed");
    }

    #[test]
    fn test_forwarder_filters_by_subscription() {
        let filter = HashSet::from(["bell_rang".to_string()]);
        let fwd = ScriptEventForwarder::new(Some(filter));

        let bell = TerminalEvent::BellRang(par_term_emu_core_rust::terminal::BellEvent::VisualBell);
        let title = TerminalEvent::TitleChanged("t".to_string());

        fwd.on_event(&bell);
        fwd.on_event(&title);

        let events = fwd.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "bell_rang");
    }

    /// Extract the title from a `title_changed` event, so the drop-oldest tests
    /// can identify which events survived.
    fn title_of(event: &ScriptEvent) -> &str {
        match &event.data {
            ScriptEventData::TitleChanged { title } => title,
            other => panic!("expected a title_changed event, got {other:?}"),
        }
    }

    #[test]
    fn buffer_stops_growing_at_the_cap() {
        let fwd = ScriptEventForwarder::new(None);
        for i in 0..(MAX_BUFFERED_EVENTS * 2) {
            fwd.on_event(&TerminalEvent::TitleChanged(i.to_string()));
        }
        assert_eq!(fwd.drain_events().len(), MAX_BUFFERED_EVENTS);
    }

    #[test]
    fn overflow_drops_the_oldest_events_not_the_newest() {
        let fwd = ScriptEventForwarder::new(None);
        let overflow = 10;
        for i in 0..(MAX_BUFFERED_EVENTS + overflow) {
            fwd.on_event(&TerminalEvent::TitleChanged(i.to_string()));
        }

        let events = fwd.drain_events();
        assert_eq!(events.len(), MAX_BUFFERED_EVENTS);
        // The first `overflow` events were discarded; the most recent survived.
        assert_eq!(title_of(&events[0]), overflow.to_string());
        assert_eq!(
            title_of(&events[MAX_BUFFERED_EVENTS - 1]),
            (MAX_BUFFERED_EVENTS + overflow - 1).to_string()
        );
    }

    #[test]
    fn draining_frees_the_capacity_again() {
        let fwd = ScriptEventForwarder::new(None);
        for i in 0..MAX_BUFFERED_EVENTS {
            fwd.on_event(&TerminalEvent::TitleChanged(i.to_string()));
        }
        assert_eq!(fwd.drain_events().len(), MAX_BUFFERED_EVENTS);

        // A drained forwarder accepts a full buffer again without dropping.
        fwd.on_event(&TerminalEvent::TitleChanged("after-drain".to_string()));
        let events = fwd.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(title_of(&events[0]), "after-drain");
    }

    #[test]
    fn filtered_out_events_do_not_consume_buffer_capacity() {
        let filter = HashSet::from(["bell_rang".to_string()]);
        let fwd = ScriptEventForwarder::new(Some(filter));
        for i in 0..(MAX_BUFFERED_EVENTS * 2) {
            fwd.on_event(&TerminalEvent::TitleChanged(i.to_string()));
        }
        assert!(fwd.drain_events().is_empty());
    }

    #[test]
    fn test_drain_clears_buffer() {
        let fwd = ScriptEventForwarder::new(None);
        let bell = TerminalEvent::BellRang(par_term_emu_core_rust::terminal::BellEvent::VisualBell);

        fwd.on_event(&bell);
        let events = fwd.drain_events();
        assert_eq!(events.len(), 1);

        // Second drain should be empty.
        let events2 = fwd.drain_events();
        assert!(events2.is_empty());
    }
}
