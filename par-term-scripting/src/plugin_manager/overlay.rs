//! Overlay focus, event delivery, and the `SetOverlay` update-rate clamp for
//! [`super::PluginHost`].
//!
//! Split out of `mod.rs` to keep it under the 800-line production limit.
//! [`clear_focus_if`](PluginHost::clear_focus_if) and
//! [`overlay_update_permitted`](PluginHost::overlay_update_permitted) are
//! `pub(super)` because the supervisor's command drain and teardown call them.

use std::collections::HashMap;
use std::time::Instant;

use crate::protocol::{ScriptEvent, ScriptEventData};

use super::PluginHost;

impl PluginHost {
    /// Live plugin overlays (plugin id → overlay), for the render layer.
    pub fn overlays(&self) -> &HashMap<String, crate::protocol::PluginOverlay> {
        &self.overlays
    }

    /// The focused overlay's plugin id, if any — the render layer's
    /// click-to-focus target check and the key router's focus check.
    pub fn focused_overlay(&self) -> Option<&str> {
        self.focused_overlay.as_deref()
    }

    /// Focus one plugin's overlay (the render layer's click path). Only an
    /// overlay that is live AND interactive can take focus — everything
    /// else clears focus, so the call doubles as the unfocus path.
    pub fn focus_overlay(&mut self, plugin_id: &str) {
        self.focused_overlay = self
            .overlays
            .get(plugin_id)
            .filter(|o| o.interactive)
            .map(|_| plugin_id.to_string());
    }

    /// Drop overlay focus entirely (Escape path; also called when the
    /// focused overlay stops being focusable).
    pub fn unfocus_overlay(&mut self) {
        self.focused_overlay = None;
    }

    /// Clear focus when it names this plugin's overlay (the upsert and
    /// teardown paths) — other plugins' focus is untouched.
    pub(super) fn clear_focus_if(&mut self, plugin_id: &str) {
        if self.focused_overlay.as_deref() == Some(plugin_id) {
            self.focused_overlay = None;
        }
    }

    /// Deliver one semantic overlay-widget event to a plugin's running
    /// overlay process (the renderer's interaction sink). Returns false
    /// without warning when there is nothing to deliver to — the renderer
    /// only calls this for a focused overlay it just drew, so a miss means
    /// the process raced an exit, which the supervisor reports.
    pub fn send_overlay_event(
        &mut self,
        plugin_id: &str,
        widget_id: &str,
        event: crate::protocol::OverlayWidgetEvent,
    ) -> bool {
        let Some(&sid) = self.overlay_running.get(plugin_id) else {
            return false;
        };
        let Some(overlay) = self.overlays.get(plugin_id) else {
            return false;
        };
        let script_event = ScriptEvent {
            kind: crate::protocol::OVERLAY_EVENT_KIND.to_string(),
            data: ScriptEventData::OverlayEvent {
                overlay: overlay.id.clone(),
                widget: widget_id.to_string(),
                event,
            },
        };
        match self.manager.send_event(sid, &script_event) {
            Ok(()) => {
                self.overlay_events_dispatched += 1;
                true
            }
            Err(error) => {
                if self.warned_event_delivery.should_warn(plugin_id) {
                    log::warn!(
                        "failed to deliver overlay event to plugin '{}': {}",
                        plugin_id,
                        error
                    );
                }
                false
            }
        }
    }

    /// Successful overlay-event stdin writes this session — the
    /// `plugin_overlay_event` ui-test operand's source. Monotonic.
    pub fn overlay_events_dispatched_count(&self) -> u64 {
        self.overlay_events_dispatched
    }

    /// The SetOverlay rate clamp's budget: accepted upserts per sliding
    /// second (design names ~30/sec with a warning).
    const OVERLAY_UPDATE_WINDOW_SECS: u64 = 1;
    const OVERLAY_UPDATE_MAX_PER_WINDOW: usize = 30;

    /// Whether one more `SetOverlay` from this plugin lands inside the
    /// update-rate budget; records the attempt either way. Over-budget
    /// upserts are dropped after the first, which warns once per episode
    /// (the episode resets after a full quiet second, so a plugin that
    /// bursts, stops, and bursts again warns once per burst).
    pub(super) fn overlay_update_permitted(&mut self, id: &str, now: Instant) -> bool {
        let times = self.overlay_update_times.entry(id.to_string()).or_default();
        times.retain(|t| now.duration_since(*t).as_secs() < Self::OVERLAY_UPDATE_WINDOW_SECS);
        if times.len() >= Self::OVERLAY_UPDATE_MAX_PER_WINDOW {
            if self
                .warned_event_delivery
                .should_warn(&format!("{id}/overlay-rate"))
            {
                log::warn!(
                    "plugin '{id}' is pushing SetOverlay faster than {} per second; \
                     dropping the excess (update-rate clamp)",
                    Self::OVERLAY_UPDATE_MAX_PER_WINDOW
                );
            }
            return false;
        }
        times.push(now);
        true
    }
}
