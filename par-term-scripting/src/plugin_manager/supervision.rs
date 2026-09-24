//! Per-kind process supervision for [`super::PluginHost`]: spawning,
//! restart backoff, per-frame command draining, and teardown.
//!
//! Split out of `plugin_manager.rs` (parsight backlog
//! `01a0c581fb837263a23025558d93cd5f`): these are the process-lifecycle and
//! restart-supervision internals the parent module's own doc comment names
//! as one of its responsibilities. [`spawn_kind_leg`](PluginHost::spawn_kind_leg),
//! [`poll_one`](PluginHost::poll_one), and [`teardown`](PluginHost::teardown)
//! are `pub(super)` because [`super::PluginHost::apply_enabled`] and
//! [`super::PluginHost::poll`] drive them; everything else here is called
//! only from within this module.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use par_term_config::RestartPolicy;

use crate::manager::{ScriptId, ScriptManager};
use crate::observer::ScriptEventForwarder;
use crate::process::ScriptStatus;
use crate::protocol::ScriptCommand;
use crate::restart::{RestartAction, ScriptRestartState};

use super::{
    KindSlot, PLUGIN_RESTART_DELAY_MS, PluginHost, SETTINGS_ARG, action_entry_args,
    overlay_entry_args, panel_entry_args, widget_entry_args,
};

impl PluginHost {
    /// Spawn one declared kind's entry unless it is already running or
    /// backed off. Shared by all three kind legs in
    /// [`Self::apply_enabled`] so their supervision cannot drift apart —
    /// the per-kind differences (which map, which warn gates) hang off
    /// `slot` alone.
    ///
    /// `entry` is `None` in exactly two cases: the kind is not declared
    /// (nothing owed — return), or it is declared but its entry point did
    /// not resolve, which is unreachable for a validated manifest and is
    /// warned, never spawned in its place.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_kind_leg(
        &mut self,
        id: &str,
        slot: KindSlot,
        entry: Option<(std::path::PathBuf, Vec<String>)>,
        settings_json: &str,
        subscriptions: &[String],
        restart_policy: RestartPolicy,
        now: Instant,
    ) {
        let Some((entry_path, entry_args)) = entry else {
            if self.warned_spawn_failed.should_warn(id) {
                log::warn!(
                    "plugin '{id}' has no {} entry point; not spawned",
                    slot.as_str()
                );
            }
            return;
        };
        if !self.running_map(slot).contains_key(id) && !self.delayed_by_backoff(id, slot, now) {
            match Self::spawn_entry(&mut self.manager, &entry_path, &entry_args, settings_json) {
                Ok(sid) => {
                    self.running_map(slot).insert(id.to_string(), sid);
                    self.settings_json
                        .insert(id.to_string(), settings_json.to_string());
                    self.restart_map(slot)
                        .entry(id.to_string())
                        .or_insert_with(|| {
                            ScriptRestartState::new(restart_policy, PLUGIN_RESTART_DELAY_MS)
                        })
                        .on_started(now);
                    self.ensure_subscription_forwarder(id, subscriptions);
                    // A successful spawn resolves the fault episodes.
                    self.warned_not_discovered.clear(id);
                    self.warned_spawn_failed.clear(id);
                    if slot == KindSlot::Action {
                        // A running action process starts a fresh invoke
                        // episode.
                        self.warned_action_not_running.clear(id);
                    }
                    self.warned_event_delivery.clear(id);
                }
                Err(error) => {
                    if self.warned_spawn_failed.should_warn(id) {
                        log::warn!(
                            "failed to spawn {} entry for plugin '{}': {}",
                            slot.as_str(),
                            id,
                            error
                        );
                    }
                    self.arm_backoff(id, slot, restart_policy, now);
                }
            }
        }
    }

    /// Whether a kind's supervisor currently owns the spawn slot with a
    /// pending backoff: `true` means do not attempt a spawn this frame (the
    /// restart delay has not elapsed). A never-started plugin has no running
    /// process for [`Self::poll`] to drive, so its backoff deadline is
    /// polled here — without this gate, a persistently-failing entry
    /// re-executes once per render frame.
    fn delayed_by_backoff(&mut self, id: &str, slot: KindSlot, now: Instant) -> bool {
        let state = self.restart_map(slot).get_mut(id);
        match state {
            Some(state) if state.pending() => state.poll(now) != RestartAction::Restart,
            _ => false,
        }
    }

    /// Arm the supervisor's capped backoff after a failed spawn (same policy
    /// as a failed restart). When the attempt cap is reached the state is
    /// dropped so the next attempt starts a fresh capped round — the reset
    /// the crash-loop path gets via [`Self::stop_kind`].
    fn arm_backoff(&mut self, id: &str, slot: KindSlot, policy: RestartPolicy, now: Instant) {
        let gave_up = {
            let state = self
                .restart_map(slot)
                .entry(id.to_string())
                .or_insert_with(|| ScriptRestartState::new(policy, PLUGIN_RESTART_DELAY_MS));
            state.reschedule(now) == RestartAction::Stop
        };
        if gave_up {
            self.restart_map(slot).remove(id);
        }
    }

    /// Advance one plugin's supervision for one kind: drain commands,
    /// observe an exit, and act on the supervisor's decision. Shared by all
    /// kinds so their supervision is identical; only the accepted command
    /// set differs per kind (v1 plugins are display-only, design D2).
    pub(super) fn poll_one(&mut self, id: &str, slot: KindSlot, now: Instant) {
        let Some(&sid) = self.running_map(slot).get(id) else {
            return;
        };

        for cmd in self.manager.read_commands(sid) {
            match cmd {
                ScriptCommand::SetWidget { text } if slot != KindSlot::Panel => {
                    self.widget_texts.insert(id.to_string(), text);
                }
                ScriptCommand::SetPanel { title, content } if slot == KindSlot::Panel => {
                    self.panel_contents.insert(id.to_string(), (title, content));
                }
                ScriptCommand::ClearPanel {} if slot == KindSlot::Panel => {
                    self.panel_contents.remove(id);
                }
                ScriptCommand::SetOverlay {
                    id: overlay_id,
                    position,
                    size,
                    opacity,
                    content,
                    ..
                } if slot == KindSlot::Overlay => {
                    // Phase 1 is display-only: the interactive flag is
                    // dropped here, pending the manifest capability + focus
                    // work (design phasing).
                    self.overlays.insert(
                        id.to_string(),
                        crate::protocol::PluginOverlay {
                            id: overlay_id,
                            position,
                            size,
                            opacity: opacity.clamp(0.0, 1.0),
                            content,
                        },
                    );
                }
                ScriptCommand::ClearOverlay { id: overlay_id } if slot == KindSlot::Overlay => {
                    let plugin_id = id;
                    if self
                        .overlays
                        .get(plugin_id)
                        .is_some_and(|live| live.id == overlay_id)
                    {
                        self.overlays.remove(plugin_id);
                    }
                }
                other => {
                    // v1 plugins are display-only (design D2); anything
                    // outside the kind's accepted set is refused with an
                    // error-style line.
                    let accepted = match slot {
                        KindSlot::Panel => {
                            "only SetPanel/ClearPanel are accepted from a panel plugin in v1"
                        }
                        KindSlot::Overlay => {
                            "only SetOverlay/ClearOverlay are accepted from an overlay plugin in v1"
                        }
                        _ => "only SetWidget is accepted from plugins in v1",
                    };
                    let line = format!(
                        "[error] plugin '{}' sent {}; {accepted} — ignored",
                        id,
                        other.command_name()
                    );
                    log::warn!("{}", line);
                    self.ignored_lines.push(line);
                }
            }
        }

        if let ScriptStatus::Exited { success } = self.manager.poll_status(sid) {
            let decision = match self.restart_map(slot).get_mut(id) {
                Some(state) if !state.pending() => state.on_exit(now, success),
                Some(state) => state.poll(now),
                None => RestartAction::Stop,
            };
            match decision {
                RestartAction::Stop => self.stop_kind(id, slot),
                RestartAction::Restart => {
                    if let Err(error) = self.respawn(id, slot, now) {
                        log::warn!(
                            "plugin '{}' failed to restart its {} entry: {}",
                            id,
                            slot.as_str(),
                            error
                        );
                        if let Some(state) = self.restart_map(slot).get_mut(id) {
                            state.reschedule(now);
                        }
                    }
                }
                RestartAction::Wait | RestartAction::Idle => {}
            }
        }
    }

    /// Stop one plugin entirely: every kind slot plus all shared state.
    pub(super) fn teardown(&mut self, id: &str) {
        self.stop_kind(id, KindSlot::Widget);
        self.stop_kind(id, KindSlot::Action);
        self.stop_kind(id, KindSlot::Panel);
        self.subscription_forwarders.remove(id);
        self.warned_event_delivery.clear(id);
    }

    /// The running-process map for one kind. Every per-kind code path
    /// selects its pair through this so the three maps cannot drift apart.
    fn running_map(&mut self, slot: KindSlot) -> &mut HashMap<String, ScriptId> {
        match slot {
            KindSlot::Widget => &mut self.running,
            KindSlot::Action => &mut self.action_running,
            KindSlot::Panel => &mut self.panel_running,
            KindSlot::Overlay => &mut self.overlay_running,
        }
    }

    /// The restart-supervisor map for one kind — see [`Self::running_map`].
    fn restart_map(&mut self, slot: KindSlot) -> &mut HashMap<String, ScriptRestartState> {
        match slot {
            KindSlot::Widget => &mut self.restart,
            KindSlot::Action => &mut self.action_restart,
            KindSlot::Panel => &mut self.panel_restart,
            KindSlot::Overlay => &mut self.overlay_restart,
        }
    }

    /// Stop one kind's process and drop its supervision. Shared per-plugin
    /// state (settings argv, warn gates) survives while another kind still
    /// runs, so a crash-looping action entry never tears down a healthy
    /// widget entry of the same plugin (design D5). Stopping the widget or
    /// panel kind also drops its display state (widget text / panel
    /// content) so a disabled plugin leaves no orphaned surface.
    fn stop_kind(&mut self, id: &str, slot: KindSlot) {
        let stopped = self.running_map(slot).remove(id);
        if let Some(sid) = stopped {
            self.manager.stop_script(sid);
        }
        self.restart_map(slot).remove(id);
        match slot {
            KindSlot::Widget => {
                self.widget_texts.remove(id);
            }
            KindSlot::Panel => {
                self.panel_contents.remove(id);
            }
            KindSlot::Overlay => {
                self.overlays.remove(id);
            }
            KindSlot::Action => {}
        }
        let other_running = [
            KindSlot::Widget,
            KindSlot::Action,
            KindSlot::Panel,
            KindSlot::Overlay,
        ]
        .iter()
        .any(|s| self.running_map(*s).contains_key(id));
        if !other_running {
            self.settings_json.remove(id);
            // Disarming the fault gates here makes a disable/enable cycle
            // warn again — a fresh user action deserves a fresh diagnostic.
            self.warned_not_discovered.clear(id);
            self.warned_spawn_failed.clear(id);
            self.warned_action_not_running.clear(id);
        }
    }

    /// Re-spawn a plugin's entry of one kind after a supervised restart,
    /// reusing its settings.
    ///
    /// The exited process slot stays mapped until the new spawn succeeds —
    /// removing it first would orphan the supervisor (a pending restart
    /// nothing polls).
    fn respawn(&mut self, id: &str, slot: KindSlot, now: Instant) -> Result<ScriptId, String> {
        let settings = self
            .settings_json
            .get(id)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        let Some(found) = self.discovered.iter().find(|d| d.manifest.id == id) else {
            return Err(format!("plugin '{}' is no longer discovered", id));
        };
        let (entry_path, entry_args) = match slot {
            KindSlot::Action => {
                let Some(path) = found.action_entry_path.clone() else {
                    return Err(format!(
                        "plugin '{}' no longer resolves an action entry point",
                        id
                    ));
                };
                (path, action_entry_args(found).to_vec())
            }
            KindSlot::Panel => {
                let Some(path) = found.panel_entry_path.clone() else {
                    return Err(format!(
                        "plugin '{}' no longer resolves a panel entry point",
                        id
                    ));
                };
                (path, panel_entry_args(found).to_vec())
            }
            KindSlot::Overlay => {
                let Some(path) = found.overlay_entry_path.clone() else {
                    return Err(format!(
                        "plugin '{}' no longer resolves an overlay entry point",
                        id
                    ));
                };
                (path, overlay_entry_args(found).to_vec())
            }
            KindSlot::Widget => (found.entry_path.clone(), widget_entry_args(found).to_vec()),
        };
        let new_sid = Self::spawn_entry(&mut self.manager, &entry_path, &entry_args, &settings)?;
        let old = self.running_map(slot).insert(id.to_string(), new_sid);
        if let Some(old_sid) = old {
            self.manager.stop_script(old_sid);
        }
        if let Some(state) = self.restart_map(slot).get_mut(id) {
            state.on_started(now);
        }
        Ok(new_sid)
    }

    /// Spawn one plugin entry point: the manifest's entry args, then the
    /// settings marker and the settings JSON as a single argv string (design
    /// D2 — stdin stays pure NDJSON). Empty env: the event stream reaches
    /// plugin stdin through the per-plugin forwarder
    /// ([`Self::subscription_forwarders`]), not the environment.
    fn spawn_entry(
        manager: &mut ScriptManager,
        entry_path: &Path,
        entry_args: &[String],
        settings_json: &str,
    ) -> Result<ScriptId, String> {
        let mut argv = entry_args.to_vec();
        argv.push(SETTINGS_ARG.to_string());
        argv.push(settings_json.to_string());
        manager.spawn_command(&entry_path.to_string_lossy(), &argv, &HashMap::new())
    }

    /// Create the plugin's event forwarder on its first successful spawn
    /// when its manifest declares subscriptions. Idempotent per plugin id:
    /// the second kind's spawn finds the first kind's forwarder already in
    /// place, and a supervised respawn reuses it (registration on terminals
    /// is by `Arc`, so the process id changing underneath changes nothing).
    fn ensure_subscription_forwarder(&mut self, id: &str, subscriptions: &[String]) {
        if subscriptions.is_empty() {
            return;
        }
        self.subscription_forwarders
            .entry(id.to_string())
            .or_insert_with(|| {
                let filter = subscriptions.iter().cloned().collect();
                Arc::new(ScriptEventForwarder::new(Some(filter)))
            });
    }
}
