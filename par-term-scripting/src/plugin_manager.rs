//! Window-scoped plugin host over the script runtime.
//!
//! [`PluginHost`] owns its own [`ScriptManager`] instance — the same registry
//! type the per-tab script system uses, kept entirely separate so a plugin
//! can never be confused with a tab script. Discovery, per-kind process
//! lifecycle (one supervised process per declared kind, design D5), restart
//! supervision (pin P3: hardcoded on-failure with the existing crash-loop
//! cap), action invocation, and the `SetWidget` text map all live here.
//!
//! Land-disabled is structural (design D3): [`PluginHost::apply_enabled`]
//! receives only the enabled set, and discovery never spawns anything by
//! itself — a newly dropped-in plugin cannot run until the Settings layer
//! says so.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use par_term_config::RestartPolicy;

use super::manager::{ScriptId, ScriptManager};
use super::manifest::{
    DiscoveredPlugin, ENTRY_POINT_ACTION_CONTRIBUTOR, ENTRY_POINT_STATUS_BAR_WIDGET,
    KIND_ACTION_CONTRIBUTOR, KIND_STATUS_BAR_WIDGET, discover_plugins,
};
use super::process::ScriptStatus;
use super::protocol::{PLUGIN_ACTION_INVOKED_KIND, ScriptCommand, ScriptEvent, ScriptEventData};
use super::restart::{RestartAction, ScriptRestartState};

/// Restart delay for a failed plugin (pin P3: the policy itself is hardcoded
/// on-failure; manifests carry no restart field in v1).
const PLUGIN_RESTART_DELAY_MS: u64 = 250;

/// Settings argv marker passed to every plugin entry point (design D2).
const SETTINGS_ARG: &str = "--par-term-settings";

/// One plugin the host should be running (Task 4 maps config state to this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnabledPlugin {
    /// Manifest id; must match a discovered plugin.
    pub id: String,
    /// Settings values serialized as one JSON object string, validated
    /// against the manifest schema by the caller before landing here.
    pub settings_json: String,
}

/// One command-palette entry contributed by an enabled action plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginActionRow {
    /// Palette action id, `plugin-action:<plugin-id>:<action-id>` (design D1).
    pub wire_id: String,
    /// Palette label: the manifest action label, then `" · "`, then the
    /// manifest plugin name (design D2).
    pub label: String,
}

/// Extra argv the widget kind's entry point declares before the settings.
fn widget_entry_args(plugin: &DiscoveredPlugin) -> &[String] {
    plugin
        .manifest
        .entry_points
        .get(ENTRY_POINT_STATUS_BAR_WIDGET)
        .map(|entry| entry.args.as_slice())
        .unwrap_or_default()
}

/// Extra argv the action kind's entry point declares before the settings.
fn action_entry_args(plugin: &DiscoveredPlugin) -> &[String] {
    plugin
        .manifest
        .entry_points
        .get(ENTRY_POINT_ACTION_CONTRIBUTOR)
        .map(|entry| entry.args.as_slice())
        .unwrap_or_default()
}

/// Deduplicates a repeating fault warning to once per condition episode.
///
/// [`PluginHost::apply_enabled`] runs every render frame, so an ungated
/// steady-state fault (an enabled plugin that is not discovered) emits the
/// same warn ~60×/s — measured at ~480 identical warns in one 8 s run.
/// `should_warn` returns `true` only on the first call per key; `clear`
/// re-arms the key when the condition resolves (successful spawn, teardown)
/// so a recurrence warns again.
#[derive(Debug, Default)]
pub struct WarnOnce {
    warned: HashSet<String>,
}

impl WarnOnce {
    /// Whether the caller should emit the warn: `true` the first time `key`
    /// is seen, `false` while the same key persists.
    pub fn should_warn(&mut self, key: &str) -> bool {
        self.warned.insert(key.to_string())
    }

    /// Re-arm `key` so a later recurrence warns again.
    pub fn clear(&mut self, key: &str) {
        self.warned.remove(key);
    }

    /// Re-arm every key except those in `keep` — call when the monitored
    /// set shrinks, so an entry that left and later returns warns again.
    pub fn clear_except(&mut self, keep: &HashSet<String>) {
        self.warned.retain(|key| keep.contains(key));
    }
}

/// Window-scoped plugin host: discovery cache, running processes, restart
/// supervision, and the last `SetWidget` text per plugin.
#[derive(Default)]
pub struct PluginHost {
    /// Last discovery scan's valid plugins.
    discovered: Vec<DiscoveredPlugin>,
    /// Running (or supervised) widget-kind plugin id → process id. An exited
    /// process stays mapped while its restart is pending so the supervisor
    /// keeps being polled; teardown is the only removal path.
    running: HashMap<String, ScriptId>,
    /// Running (or supervised) action-kind plugin id → process id — one
    /// supervised process per (plugin, kind), so a both-kinds manifest runs
    /// both entry points independently (design D5).
    action_running: HashMap<String, ScriptId>,
    /// The host's own script registry; never shared with tab scripts.
    manager: ScriptManager,
    /// Last `SetWidget` text per plugin id (last write wins).
    widget_texts: HashMap<String, String>,
    /// Successful [`Self::invoke_action`] stdin writes this session — the
    /// `plugin_action_dispatched` ui-test operand's source. Monotonic for
    /// the session; never reset.
    actions_dispatched: u64,
    /// Per-plugin widget-kind restart supervisor (pin P3).
    restart: HashMap<String, ScriptRestartState>,
    /// Per-plugin action-kind restart supervisor, driven identically to
    /// [`Self::restart`] but independently per kind.
    action_restart: HashMap<String, ScriptRestartState>,
    /// Settings argv each running plugin was spawned with, kept so a
    /// supervisor restart re-spawns with the same settings. Shared by both
    /// kinds of one plugin (same settings argv, design D2).
    settings_json: HashMap<String, String>,
    /// Enabled plugin ids retained from the last [`Self::apply_enabled`]
    /// call, so the palette can enumerate enabled plugins' actions without
    /// a config read.
    enabled_ids: HashSet<String>,
    /// Commands refused because v1 plugins may only send `SetWidget`.
    ignored_lines: Vec<String>,
    /// Per-fault warn gates: steady faults warn once per episode, not per
    /// frame (see [`WarnOnce`]).
    warned_not_discovered: WarnOnce,
    warned_spawn_failed: WarnOnce,
    /// Gate for invoke misses on the action kind (not running, kind absent,
    /// unknown action, failed write) — a stuck keybinding can retry a
    /// dispatch at frame rate.
    warned_action_not_running: WarnOnce,
}

impl PluginHost {
    /// Create an empty host. Nothing runs until
    /// [`refresh_discovery`](Self::refresh_discovery) +
    /// [`apply_enabled`](Self::apply_enabled) say otherwise.
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan a plugins root, replacing the discovery cache. Running
    /// plugins are preserved — a re-scan never stops or spawns anything.
    pub fn refresh_discovery(&mut self, root: &Path) -> &[DiscoveredPlugin] {
        let (plugins, warnings) = discover_plugins(root);
        for warning in &warnings {
            log::warn!(
                "plugin discovery skipped '{}': {}",
                warning.dir,
                warning.reason
            );
        }
        self.discovered = plugins;
        &self.discovered
    }

    /// The discovery-cache entry for a plugin id, if the last scan found it.
    pub fn discovered(&self, id: &str) -> Option<&DiscoveredPlugin> {
        self.discovered.iter().find(|d| d.manifest.id == id)
    }

    /// Number of plugins the last discovery scan accepted.
    pub fn discovered_count(&self) -> usize {
        self.discovered.len()
    }

    /// Reconcile running plugins with the enabled set (diff → spawn/stop).
    ///
    /// Land-disabled is structural: only ids present in `enabled` AND in the
    /// discovery cache are spawned, so dropping a plugin directory into the
    /// root does nothing until the Settings layer enables it. A both-kinds
    /// manifest spawns both entry points in this one pass, with the same
    /// settings argv and one supervision slot per kind (design D5).
    pub fn apply_enabled(&mut self, enabled: &[EnabledPlugin]) {
        let now = Instant::now();

        let stale: Vec<String> = self
            .running
            .keys()
            .chain(self.action_running.keys())
            .filter(|id| !enabled.iter().any(|e| e.id == **id))
            .cloned()
            .collect();
        for id in &stale {
            self.teardown(id);
        }

        // A plugin leaving the enabled set ends its fault episodes, so a
        // disable/enable cycle of a still-missing plugin warns again.
        let enabled_ids: HashSet<String> = enabled.iter().map(|e| e.id.clone()).collect();
        self.enabled_ids = enabled_ids.clone();
        self.warned_not_discovered.clear_except(&enabled_ids);
        self.warned_spawn_failed.clear_except(&enabled_ids);

        for plugin in enabled {
            let Some(found) = self.discovered.iter().find(|d| d.manifest.id == plugin.id) else {
                if self.warned_not_discovered.should_warn(&plugin.id) {
                    log::warn!(
                        "plugin '{}' is enabled but not discovered; not spawned",
                        plugin.id
                    );
                }
                continue;
            };
            let has_widget = found
                .manifest
                .kinds
                .iter()
                .any(|k| k == KIND_STATUS_BAR_WIDGET);
            let has_action = found
                .manifest
                .kinds
                .iter()
                .any(|k| k == KIND_ACTION_CONTRIBUTOR);

            // Extract both kinds' spawn descriptors up front so the
            // discovery borrow ends before the supervisor/spawn calls below.
            let widget_entry =
                has_widget.then(|| (found.entry_path.clone(), widget_entry_args(found).to_vec()));
            // The action kind routes through its own confinement-checked
            // entry — never `entry_path`, which belongs to the widget kind.
            let action_entry = if has_action {
                found
                    .action_entry_path
                    .clone()
                    .map(|path| (path, action_entry_args(found).to_vec()))
            } else {
                None
            };
            // Unreachable for a validated manifest (the action kind requires
            // an action entry point); guarded so a host bug neither spawns
            // the widget entry in its place nor silently skips.
            let missing_action_entry = has_action && action_entry.is_none();

            // Widget leg (the kind gate keeps an action-only manifest out of
            // the widget slot).
            if let Some((entry_path, entry_args)) = widget_entry
                && !self.running.contains_key(&plugin.id)
                && !self.delayed_by_backoff(&plugin.id, false, now)
            {
                match Self::spawn_entry(
                    &mut self.manager,
                    &entry_path,
                    &entry_args,
                    &plugin.settings_json,
                ) {
                    Ok(sid) => {
                        self.running.insert(plugin.id.clone(), sid);
                        self.settings_json
                            .insert(plugin.id.clone(), plugin.settings_json.clone());
                        self.restart
                            .entry(plugin.id.clone())
                            .or_insert_with(|| {
                                ScriptRestartState::new(
                                    RestartPolicy::OnFailure,
                                    PLUGIN_RESTART_DELAY_MS,
                                )
                            })
                            .on_started(now);
                        // A successful spawn resolves both fault episodes.
                        self.warned_not_discovered.clear(&plugin.id);
                        self.warned_spawn_failed.clear(&plugin.id);
                    }
                    Err(error) => {
                        if self.warned_spawn_failed.should_warn(&plugin.id) {
                            log::warn!("failed to spawn plugin '{}': {}", plugin.id, error);
                        }
                        self.arm_backoff(&plugin.id, false, now);
                    }
                }
            }

            // Action leg: same pass, same settings argv, its own supervision
            // slot.
            if missing_action_entry {
                if self.warned_spawn_failed.should_warn(&plugin.id) {
                    log::warn!(
                        "plugin '{}' has no action entry point; not spawned",
                        plugin.id
                    );
                }
            } else if let Some((entry_path, entry_args)) = action_entry
                && !self.action_running.contains_key(&plugin.id)
                && !self.delayed_by_backoff(&plugin.id, true, now)
            {
                match Self::spawn_entry(
                    &mut self.manager,
                    &entry_path,
                    &entry_args,
                    &plugin.settings_json,
                ) {
                    Ok(sid) => {
                        self.action_running.insert(plugin.id.clone(), sid);
                        self.settings_json
                            .insert(plugin.id.clone(), plugin.settings_json.clone());
                        self.action_restart
                            .entry(plugin.id.clone())
                            .or_insert_with(|| {
                                ScriptRestartState::new(
                                    RestartPolicy::OnFailure,
                                    PLUGIN_RESTART_DELAY_MS,
                                )
                            })
                            .on_started(now);
                        self.warned_not_discovered.clear(&plugin.id);
                        self.warned_spawn_failed.clear(&plugin.id);
                        // A running action process starts a fresh invoke
                        // episode.
                        self.warned_action_not_running.clear(&plugin.id);
                    }
                    Err(error) => {
                        if self.warned_spawn_failed.should_warn(&plugin.id) {
                            log::warn!(
                                "failed to spawn action entry for plugin '{}': {}",
                                plugin.id,
                                error
                            );
                        }
                        self.arm_backoff(&plugin.id, true, now);
                    }
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
    fn delayed_by_backoff(&mut self, id: &str, action: bool, now: Instant) -> bool {
        let state = if action {
            self.action_restart.get_mut(id)
        } else {
            self.restart.get_mut(id)
        };
        match state {
            Some(state) if state.pending() => state.poll(now) != RestartAction::Restart,
            _ => false,
        }
    }

    /// Arm the supervisor's capped backoff after a failed spawn (same policy
    /// as a failed restart). When the attempt cap is reached the state is
    /// dropped so the next attempt starts a fresh capped round — the reset
    /// the crash-loop path gets via [`Self::stop_kind`].
    fn arm_backoff(&mut self, id: &str, action: bool, now: Instant) {
        let gave_up = {
            let state = if action {
                self.action_restart.entry(id.to_string())
            } else {
                self.restart.entry(id.to_string())
            }
            .or_insert_with(|| {
                ScriptRestartState::new(RestartPolicy::OnFailure, PLUGIN_RESTART_DELAY_MS)
            });
            state.reschedule(now) == RestartAction::Stop
        };
        if gave_up {
            if action {
                self.action_restart.remove(id);
            } else {
                self.restart.remove(id);
            }
        }
    }

    /// Advance supervision one frame: drain each running plugin process's
    /// commands (both kinds), observe exits, and drive each kind's restart
    /// supervisor (on-failure, capped) — one frame advance covers the widget
    /// and action maps identically.
    ///
    /// Commands are drained before exit handling so a plugin's final
    /// `SetWidget` before a crash still lands.
    pub fn poll(&mut self) {
        let now = Instant::now();
        let widget_ids: Vec<String> = self.running.keys().cloned().collect();
        for id in widget_ids {
            self.poll_one(&id, false, now);
        }
        let action_ids: Vec<String> = self.action_running.keys().cloned().collect();
        for id in action_ids {
            self.poll_one(&id, true, now);
        }
    }

    /// Advance one plugin's supervision for one kind (widget or action
    /// entry): drain commands, observe an exit, and act on the supervisor's
    /// decision. Shared by both kinds so their supervision is identical.
    fn poll_one(&mut self, id: &str, action: bool, now: Instant) {
        let Some(&sid) = (if action {
            self.action_running.get(id)
        } else {
            self.running.get(id)
        }) else {
            return;
        };

        for cmd in self.manager.read_commands(sid) {
            match cmd {
                ScriptCommand::SetWidget { text } => {
                    self.widget_texts.insert(id.to_string(), text);
                }
                other => {
                    // v1 plugins are display-only (design D2); anything
                    // else is refused with an error-style line.
                    let line = format!(
                        "[error] plugin '{}' sent {}; only SetWidget is accepted from plugins in v1 — ignored",
                        id,
                        other.command_name()
                    );
                    log::warn!("{}", line);
                    self.ignored_lines.push(line);
                }
            }
        }

        if let ScriptStatus::Exited { success } = self.manager.poll_status(sid) {
            let decision = match if action {
                self.action_restart.get_mut(id)
            } else {
                self.restart.get_mut(id)
            } {
                Some(state) if !state.pending() => state.on_exit(now, success),
                Some(state) => state.poll(now),
                None => RestartAction::Stop,
            };
            match decision {
                RestartAction::Stop => self.stop_kind(id, action),
                RestartAction::Restart => {
                    if let Err(error) = self.respawn(id, action, now) {
                        log::warn!(
                            "plugin '{}' failed to restart its {} entry: {}",
                            id,
                            if action { "action" } else { "widget" },
                            error
                        );
                        if let Some(state) = if action {
                            self.action_restart.get_mut(id)
                        } else {
                            self.restart.get_mut(id)
                        } {
                            state.reschedule(now);
                        }
                    }
                }
                RestartAction::Wait | RestartAction::Idle => {}
            }
        }
    }

    /// Last `SetWidget` text recorded for a plugin, if any.
    pub fn widget_text(&self, plugin_id: &str) -> Option<&str> {
        self.widget_texts.get(plugin_id).map(String::as_str)
    }

    /// All recorded widget texts (plugin id → text), for the status bar.
    pub fn widget_texts(&self) -> &HashMap<String, String> {
        &self.widget_texts
    }

    /// Successful [`Self::invoke_action`] stdin writes this session.
    /// Monotonic for the session and never reset — delivered-means-true
    /// dispatch has no undo, so the count only grows.
    pub fn actions_dispatched_count(&self) -> u64 {
        self.actions_dispatched
    }

    /// Ids of plugins with any running or supervised process, of either
    /// kind. A both-kinds plugin appears once.
    pub fn running_plugin_ids(&self) -> Vec<String> {
        self.running
            .keys()
            .chain(self.action_running.keys())
            .cloned()
            .collect()
    }

    /// Invoke one of a plugin's contributed palette actions: write a single
    /// [`ScriptEvent`] line carrying [`ScriptEventData::PluginActionInvoked`]
    /// to the plugin's running action process.
    ///
    /// Returns `true` only when the plugin is discovered, declares the
    /// action-contributor kind, the action id is in its manifest, a process
    /// is running, and the stdin write succeeded. Every miss returns `false`
    /// behind [`WarnOnce`] gates (`warned_not_discovered`, and
    /// `warned_action_not_running` for every post-discovery miss) — a stuck
    /// keybinding can retry this at frame rate.
    pub fn invoke_action(&mut self, plugin_id: &str, action_id: &str) -> bool {
        let Some(found) = self.discovered.iter().find(|d| d.manifest.id == plugin_id) else {
            if self.warned_not_discovered.should_warn(plugin_id) {
                log::warn!(
                    "plugin '{}' is not discovered; cannot invoke action '{}'",
                    plugin_id,
                    action_id
                );
            }
            return false;
        };
        if !found
            .manifest
            .kinds
            .iter()
            .any(|k| k == KIND_ACTION_CONTRIBUTOR)
        {
            if self.warned_action_not_running.should_warn(plugin_id) {
                log::warn!(
                    "plugin '{}' does not declare kind '{}'; cannot invoke action '{}'",
                    plugin_id,
                    KIND_ACTION_CONTRIBUTOR,
                    action_id
                );
            }
            return false;
        }
        if !found.manifest.actions.iter().any(|a| a.id == action_id) {
            if self.warned_action_not_running.should_warn(plugin_id) {
                log::warn!(
                    "plugin '{}' has no action '{}'; cannot invoke it",
                    plugin_id,
                    action_id
                );
            }
            return false;
        }
        let Some(&sid) = self.action_running.get(plugin_id) else {
            if self.warned_action_not_running.should_warn(plugin_id) {
                log::warn!(
                    "plugin '{}' has no running action process; action '{}' not invoked",
                    plugin_id,
                    action_id
                );
            }
            return false;
        };
        let event = ScriptEvent {
            kind: PLUGIN_ACTION_INVOKED_KIND.to_string(),
            data: ScriptEventData::PluginActionInvoked {
                action: action_id.to_string(),
            },
        };
        match self.manager.send_event(sid, &event) {
            Ok(()) => {
                // A landed invoke starts a fresh miss episode.
                self.warned_action_not_running.clear(plugin_id);
                self.actions_dispatched += 1;
                true
            }
            Err(error) => {
                if self.warned_action_not_running.should_warn(plugin_id) {
                    log::warn!(
                        "failed to invoke action '{}' on plugin '{}': {}",
                        action_id,
                        plugin_id,
                        error
                    );
                }
                false
            }
        }
    }

    /// Palette rows for every action of every discovered, enabled,
    /// action-contributor plugin (design D2), in discovery order.
    pub fn palette_actions(&self) -> Vec<PluginActionRow> {
        let mut rows = Vec::new();
        for found in &self.discovered {
            if !self.enabled_ids.contains(&found.manifest.id) {
                continue;
            }
            if !found
                .manifest
                .kinds
                .iter()
                .any(|k| k == KIND_ACTION_CONTRIBUTOR)
            {
                continue;
            }
            for action in &found.manifest.actions {
                rows.push(PluginActionRow {
                    wire_id: format!("plugin-action:{}:{}", found.manifest.id, action.id),
                    label: format!("{} · {}", action.label, found.manifest.name),
                });
            }
        }
        rows
    }

    /// Take the lines accumulated from refused plugin commands (the
    /// read-errors-style surface for commands v1 ignores).
    pub fn drain_ignored(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ignored_lines)
    }

    /// Stop every plugin and drop all supervision state.
    pub fn stop_all(&mut self) {
        self.manager.stop_all();
        self.running.clear();
        self.action_running.clear();
        self.restart.clear();
        self.action_restart.clear();
        self.widget_texts.clear();
        self.settings_json.clear();
        self.enabled_ids.clear();
        self.warned_not_discovered = WarnOnce::default();
        self.warned_spawn_failed = WarnOnce::default();
        self.warned_action_not_running = WarnOnce::default();
    }

    /// Stop one plugin entirely: both kind slots plus all shared state.
    fn teardown(&mut self, id: &str) {
        self.stop_kind(id, false);
        self.stop_kind(id, true);
    }

    /// Stop one kind's process and drop its supervision. Shared per-plugin
    /// state (settings argv, warn gates) survives while the other kind still
    /// runs, so a crash-looping action entry never tears down a healthy
    /// widget entry of the same plugin (design D5).
    fn stop_kind(&mut self, id: &str, action: bool) {
        let slot = if action {
            self.action_running.remove(id)
        } else {
            self.running.remove(id)
        };
        if let Some(sid) = slot {
            self.manager.stop_script(sid);
        }
        if action {
            self.action_restart.remove(id);
        } else {
            self.restart.remove(id);
            self.widget_texts.remove(id);
        }
        let other_running = if action {
            self.running.contains_key(id)
        } else {
            self.action_running.contains_key(id)
        };
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
    fn respawn(&mut self, id: &str, action: bool, now: Instant) -> Result<ScriptId, String> {
        let settings = self
            .settings_json
            .get(id)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        let Some(found) = self.discovered.iter().find(|d| d.manifest.id == id) else {
            return Err(format!("plugin '{}' is no longer discovered", id));
        };
        let (entry_path, entry_args) = if action {
            let Some(path) = found.action_entry_path.clone() else {
                return Err(format!(
                    "plugin '{}' no longer resolves an action entry point",
                    id
                ));
            };
            (path, action_entry_args(found).to_vec())
        } else {
            (found.entry_path.clone(), widget_entry_args(found).to_vec())
        };
        let new_sid = Self::spawn_entry(&mut self.manager, &entry_path, &entry_args, &settings)?;
        let old = if action {
            self.action_running.insert(id.to_string(), new_sid)
        } else {
            self.running.insert(id.to_string(), new_sid)
        };
        if let Some(old_sid) = old {
            self.manager.stop_script(old_sid);
        }
        if let Some(state) = if action {
            self.action_restart.get_mut(id)
        } else {
            self.restart.get_mut(id)
        } {
            state.on_started(now);
        }
        Ok(new_sid)
    }

    /// Spawn one plugin entry point: the manifest's entry args, then the
    /// settings marker and the settings JSON as a single argv string (design
    /// D2 — stdin stays pure NDJSON). Empty env: v1 plugins receive no
    /// events (pin P1), so there is nothing tab-bound to inject.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    /// A plugin that emits two `SetWidget` lines and then stays alive.
    const WIDGET_SCRIPT: &str = r#"
import json, time
def emit(text):
    print(json.dumps({"type": "SetWidget", "text": text}), flush=True)
emit("first")
emit("second")
while True:
    time.sleep(0.2)
"#;

    /// A plugin that emits a `Log` command (refused in v1) and stays alive.
    const LOG_SCRIPT: &str = r#"
import json, time
print(json.dumps({"type": "Log", "level": "info", "message": "hello"}), flush=True)
while True:
    time.sleep(0.2)
"#;

    /// A plugin that exits immediately with a failure code.
    const CRASH_SCRIPT: &str = "import sys\nsys.exit(3)\n";

    fn manifest_json(id: &str) -> String {
        format!(
            "{{\"schemaVersion\":1,\"id\":\"{id}\",\"name\":\"T\",\"version\":\"0.1.0\",\
\"kinds\":[\"status-bar-widget\"],\"activation\":\"manual\",\
\"entryPoints\":{{\"statusBarWidget\":{{\"command\":\"widget.py\",\"args\":[]}}}},\
\"statusBarWidget\":{{\"displayName\":\"T\",\"section\":\"right\",\"defaults\":{{}},\
\"schema\":[{{\"key\":\"on\",\"type\":\"boolean\",\"label\":\"On\",\"defaultValue\":true}}]}}}}"
        )
    }

    /// Write one valid plugin directory under `root`; executable on unix so
    /// discovery's permissions check accepts the entry.
    fn write_plugin(root: &Path, id: &str, script: &str) {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest_json(id)).unwrap();
        let entry = dir.join("widget.py");
        fs::write(&entry, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&entry, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn enabled(id: &str) -> EnabledPlugin {
        EnabledPlugin {
            id: id.to_string(),
            settings_json: "{}".to_string(),
        }
    }

    fn plugin_running(host: &PluginHost, id: &str) -> bool {
        host.running_plugin_ids().iter().any(|i| i == id)
    }

    /// Drive `poll` until `check` holds or ~10 s elapse (the fixture is a
    /// real subprocess; its first output line can take a moment to arrive).
    fn wait_until(host: &mut PluginHost, mut check: impl FnMut(&mut PluginHost) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            host.poll();
            if check(host) {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn skip_without_interpreter() -> bool {
        if crate::manager::python_interpreter().is_some() {
            return false;
        }
        eprintln!("skipping: no Python interpreter on PATH");
        true
    }

    #[test]
    fn discovery_never_spawns_even_for_an_absent_root() {
        let mut host = PluginHost::new();
        let root = std::env::temp_dir().join("par-term-plugin-test-absent-root");
        assert!(host.refresh_discovery(&root).is_empty());
        host.apply_enabled(&[enabled("com.test.widget")]);
        assert!(host.running_plugin_ids().is_empty());
        host.poll(); // must not panic on an empty host
    }

    #[test]
    fn apply_enabled_spawns_and_last_set_widget_wins() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.widget", WIDGET_SCRIPT);

        let mut host = PluginHost::new();
        assert_eq!(host.refresh_discovery(root.path()).len(), 1);
        host.apply_enabled(&[enabled("com.test.widget")]);
        // Nothing is drained until poll runs, so no widget text yet.
        assert_eq!(host.widget_text("com.test.widget"), None);

        let settled = wait_until(&mut host, |h| h.widget_text("com.test.widget").is_some());
        assert!(settled, "fixture's SetWidget never arrived");
        // The fixture emits "first" then "second"; both lines are typically
        // drained by one poll, so the surviving text proves last-write-wins.
        assert_eq!(host.widget_text("com.test.widget"), Some("second"));
        host.stop_all();
    }

    #[test]
    fn disabling_stops_the_plugin_and_its_widget_text() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.widget", WIDGET_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.widget")]);
        assert!(wait_until(&mut host, |h| h
            .widget_text("com.test.widget")
            .is_some()));

        host.apply_enabled(&[]);
        assert!(!plugin_running(&host, "com.test.widget"));
        assert_eq!(host.widget_text("com.test.widget"), None);
        // A disabled plugin must not be resurrected by the supervisor.
        for _ in 0..10 {
            host.poll();
            thread::sleep(Duration::from_millis(50));
        }
        assert!(host.running_plugin_ids().is_empty());
    }

    #[test]
    fn crash_loop_is_capped_without_panicking() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.crash", CRASH_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.crash")]);

        // 5 supervised restarts at a 250 ms delay, plus spawn overhead —
        // generous wall-clock budget, hard deadline so a bug fails fast.
        let gave_up = wait_until(&mut host, |h| !plugin_running(h, "com.test.crash"));
        assert!(
            gave_up,
            "supervisor never gave up; still running: {:?}",
            host.running_plugin_ids()
        );
        // Polling a given-up slot stays a no-op.
        host.poll();
        assert!(host.running_plugin_ids().is_empty());
    }

    #[test]
    fn non_widget_commands_are_refused_with_an_error_line() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.log", LOG_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.log")]);

        let mut refused: Vec<String> = Vec::new();
        let seen = wait_until(&mut host, |h| {
            refused.extend(h.drain_ignored());
            refused.iter().any(|line| line.contains("Log"))
        });
        assert!(
            seen,
            "Log command was never refused; lines seen: {:?}, widget text: {:?}",
            refused,
            host.widget_text("com.test.log")
        );
        assert!(refused.iter().all(|line| line.starts_with("[error]")));
        host.stop_all();
    }

    #[test]
    fn failing_spawn_retries_at_the_restart_delay_not_per_frame() {
        let root = tempfile::tempdir().unwrap();
        // An executable non-.py entry with a broken interpreter line:
        // discovery accepts it, direct exec fails — a persistently
        // failing spawn. (.py entries bypass the shebang via the
        // interpreter route in `spawn_command`, so the entry must not be
        // python.)
        let dir = root.path().join("com.test.broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("manifest.json"),
            manifest_json("com.test.broken").replace("widget.py", "widget.sh"),
        )
        .unwrap();
        fs::write(
            dir.join("widget.sh"),
            "#!/nonexistent/par-term-test-interpreter\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir.join("widget.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        assert_eq!(host.discovered_count(), 1, "fixture must be discovered");

        // First reconcile: one spawn attempt, it fails, the supervisor's
        // backoff arms.
        host.apply_enabled(&[enabled("com.test.broken")]);
        assert!(!plugin_running(&host, "com.test.broken"));
        let after_first = host
            .restart
            .get("com.test.broken")
            .expect("supervisor state armed by a failed spawn")
            .consecutive_failures();
        assert_eq!(after_first, 1);

        // Reconcile again immediately (well inside the 250 ms delay): no
        // re-attempt — a second exec would have pushed the failure count.
        host.apply_enabled(&[enabled("com.test.broken")]);
        assert_eq!(
            host.restart
                .get("com.test.broken")
                .unwrap()
                .consecutive_failures(),
            after_first,
            "spawn must not be retried inside the restart delay"
        );

        // Once the delay has elapsed: exactly one retry.
        thread::sleep(Duration::from_millis(300));
        host.apply_enabled(&[enabled("com.test.broken")]);
        assert_eq!(
            host.restart
                .get("com.test.broken")
                .unwrap()
                .consecutive_failures(),
            after_first + 1,
            "exactly one retry after the delay elapses"
        );
    }

    #[test]
    fn stop_all_drains_every_plugin() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.a", WIDGET_SCRIPT);
        write_plugin(root.path(), "com.test.b", WIDGET_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.a"), enabled("com.test.b")]);
        assert!(wait_until(&mut host, |h| h.running_plugin_ids().len() == 2));

        host.stop_all();
        assert!(host.running_plugin_ids().is_empty());
        assert!(host.widget_texts().is_empty());
        host.poll(); // no panic after a full drain
    }

    /// Records every `log::warn!` this test binary emits so tests can assert
    /// on warn *frequency*, not just occurrence. Installed once per process;
    /// this crate's tests never install a file logger, so capturing here
    /// changes nothing for the other tests.
    static WARNED: std::sync::LazyLock<std::sync::Mutex<Vec<String>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

    struct CountingLogger;

    impl log::Log for CountingLogger {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            if record.level() == log::Level::Warn {
                WARNED.lock().unwrap().push(record.args().to_string());
            }
        }

        fn flush(&self) {}
    }

    fn install_counting_logger() {
        static INSTALL: std::sync::Once = std::sync::Once::new();
        static COUNTING_LOGGER: CountingLogger = CountingLogger;
        INSTALL.call_once(|| {
            // An error here would mean another logger is already installed;
            // capture works regardless.
            let _ = log::set_logger(&COUNTING_LOGGER);
            log::set_max_level(log::LevelFilter::Warn);
        });
    }

    fn warns_containing(fragment: &str) -> usize {
        WARNED
            .lock()
            .unwrap()
            .iter()
            .filter(|msg| msg.contains(fragment))
            .count()
    }

    // Both dedup tests run in parallel against the shared warn capture, so
    // each filters on its own plugin id, not the shared phrase.
    #[test]
    fn fault_warns_fire_once_per_transition_not_per_frame() {
        install_counting_logger();
        let fragment = "'com.test.missing' is enabled but not discovered";
        let mut host = PluginHost::new();
        // An empty root: the enabled plugin is never discovered.
        let root = tempfile::tempdir().unwrap();
        host.refresh_discovery(root.path());

        for _ in 0..5 {
            host.apply_enabled(&[enabled("com.test.missing")]);
        }
        let warns = warns_containing(fragment);
        assert_eq!(
            warns, 1,
            "5 frames of one steady fault must produce exactly 1 warn, got {warns}"
        );

        // Disabling re-arms the warn: re-enabling the same fault warns again
        // rather than staying silent forever.
        host.apply_enabled(&[]);
        for _ in 0..2 {
            host.apply_enabled(&[enabled("com.test.missing")]);
        }
        let warns = warns_containing(fragment);
        assert_eq!(
            warns, 2,
            "warn must re-arm after a disable/enable, got {warns}"
        );
    }

    #[test]
    fn fault_warn_rearms_after_recovery() {
        if skip_without_interpreter() {
            return;
        }
        install_counting_logger();
        let empty = tempfile::tempdir().unwrap();
        let populated = tempfile::tempdir().unwrap();
        write_plugin(populated.path(), "com.test.flapping", WIDGET_SCRIPT);

        let mut host = PluginHost::new();
        // Fault: enabled but not discovered.
        let fragment = "'com.test.flapping' is enabled but not discovered";
        host.refresh_discovery(empty.path());
        host.apply_enabled(&[enabled("com.test.flapping")]);
        assert_eq!(warns_containing(fragment), 1);

        // Recovery: the plugin appears and spawns, which re-arms the warn.
        host.refresh_discovery(populated.path());
        host.apply_enabled(&[enabled("com.test.flapping")]);
        assert!(wait_until(&mut host, |h| h
            .widget_text("com.test.flapping")
            .is_some()));

        // The plugin is disabled, disappears, and is enabled again: the same
        // fault must warn a second time.
        host.apply_enabled(&[]);
        host.refresh_discovery(empty.path());
        host.apply_enabled(&[enabled("com.test.flapping")]);
        assert_eq!(
            warns_containing(fragment),
            2,
            "a recovered-then-failed-again plugin must warn again"
        );
    }

    /// An action entry that echoes every stdin event back as a `SetWidget`
    /// naming the invoked action — the observable seam for invoke writes
    /// (same pattern as `WIDGET_SCRIPT` for spawned plugin stdout).
    const ACTION_SCRIPT: &str = r#"
import json, sys
def emit(text):
    print(json.dumps({"type": "SetWidget", "text": text}), flush=True)
for line in iter(sys.stdin.readline, ""):
    line = line.strip()
    if not line:
        continue
    try:
        event = json.loads(line)
    except ValueError:
        continue
    emit("invoked:" + str(event.get("data", {}).get("action", "?")))
"#;

    fn action_manifest_json(id: &str) -> String {
        format!(
            "{{\"schemaVersion\":1,\"id\":\"{id}\",\"name\":\"Test Actions\",\"version\":\"0.1.0\",\
\"kinds\":[\"action-contributor\"],\"activation\":\"manual\",\
\"entryPoints\":{{\"actionContributor\":{{\"command\":\"actions.py\",\"args\":[]}}}},\
\"actions\":[{{\"id\":\"say-hello\",\"label\":\"Say hello\"}}]}}"
        )
    }

    fn both_kinds_manifest_json(id: &str) -> String {
        format!(
            "{{\"schemaVersion\":1,\"id\":\"{id}\",\"name\":\"Test Both\",\"version\":\"0.1.0\",\
\"kinds\":[\"status-bar-widget\",\"action-contributor\"],\"activation\":\"manual\",\
\"entryPoints\":{{\"statusBarWidget\":{{\"command\":\"widget.py\",\"args\":[]}},\
\"actionContributor\":{{\"command\":\"actions.py\",\"args\":[]}}}},\
\"actions\":[{{\"id\":\"ping\",\"label\":\"Ping\"}}],\
\"statusBarWidget\":{{\"displayName\":\"T\",\"section\":\"right\",\"defaults\":{{}},\
\"schema\":[{{\"key\":\"on\",\"type\":\"boolean\",\"label\":\"On\",\"defaultValue\":true}}]}}}}"
        )
    }

    /// Write one plugin directory with a manifest plus named entry files,
    /// executable on unix so discovery's permissions check accepts them.
    fn write_plugin_files(root: &Path, id: &str, manifest: &str, files: &[(&str, &str)]) {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest).unwrap();
        for (name, script) in files {
            let entry = dir.join(name);
            fs::write(&entry, script).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&entry, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }

    fn write_action_plugin(root: &Path, id: &str, script: &str) {
        write_plugin_files(
            root,
            id,
            &action_manifest_json(id),
            &[("actions.py", script)],
        );
    }

    #[test]
    fn invoke_action_miss_paths_return_false_without_a_process() {
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.only-widget", WIDGET_SCRIPT);
        write_action_plugin(root.path(), "com.test.only-action", ACTION_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[]); // nothing enabled, so nothing spawns

        // Unknown plugin: not discovered.
        assert!(!host.invoke_action("com.test.nope", "greet"));
        // Discovered but disabled: the action process is not running.
        assert!(!host.invoke_action("com.test.only-action", "say-hello"));
        // Discovered, but the action id is not in the manifest.
        assert!(!host.invoke_action("com.test.only-action", "bogus"));
        // Discovered widget-only plugin: the action kind is absent.
        assert!(!host.invoke_action("com.test.only-widget", "say-hello"));

        assert!(host.running_plugin_ids().is_empty());
        assert!(host.action_running.is_empty());
    }

    #[test]
    fn invoke_action_not_running_warns_once_per_episode() {
        install_counting_logger();
        let root = tempfile::tempdir().unwrap();
        write_action_plugin(root.path(), "com.test.invoke-miss", ACTION_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[]);

        // A stuck keybinding can retry dispatch at frame rate; the warn must
        // fire once per episode, not per call.
        let fragment = "'com.test.invoke-miss' has no running action process";
        for _ in 0..5 {
            assert!(!host.invoke_action("com.test.invoke-miss", "say-hello"));
        }
        assert_eq!(
            warns_containing(fragment),
            1,
            "5 invokes of one steady miss must produce exactly 1 warn"
        );
    }

    #[test]
    fn palette_actions_list_enabled_action_plugins_only() {
        let root = tempfile::tempdir().unwrap();
        write_action_plugin(root.path(), "com.test.pal-on", ACTION_SCRIPT);
        write_action_plugin(root.path(), "com.test.pal-off", ACTION_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());

        host.apply_enabled(&[]);
        assert!(
            host.palette_actions().is_empty(),
            "nothing enabled -> no rows"
        );

        host.apply_enabled(&[enabled("com.test.pal-on")]);
        let rows = host.palette_actions();
        assert_eq!(rows.len(), 1, "one action on the enabled plugin: {rows:?}");
        assert_eq!(rows[0].wire_id, "plugin-action:com.test.pal-on:say-hello");
        assert_eq!(rows[0].label, "Say hello · Test Actions");
        host.stop_all();
    }

    #[test]
    fn both_kinds_manifest_runs_one_process_per_kind() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.both",
            &both_kinds_manifest_json("com.test.both"),
            &[("widget.py", WIDGET_SCRIPT), ("actions.py", ACTION_SCRIPT)],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.both")]);

        assert!(
            wait_until(&mut host, |h| h.running.len() == 1
                && h.action_running.len() == 1),
            "a both-kinds manifest must run both entry points after one pass"
        );
        // Widget behavior is untouched: the widget entry's output still lands.
        assert!(wait_until(&mut host, |h| h
            .widget_text("com.test.both")
            .is_some()));
        // The action entry answers invokes.
        assert!(host.invoke_action("com.test.both", "ping"));
        host.stop_all();
    }

    #[test]
    fn invoke_action_writes_the_event_to_the_running_action_process() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_action_plugin(root.path(), "com.test.invoke", ACTION_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.invoke")]);

        assert!(
            host.invoke_action("com.test.invoke", "say-hello"),
            "writing to a running action process must succeed"
        );
        // The fixture echoes the invoked action id back as its widget text,
        // so the round-trip through stdin is observable at the spawned-
        // process seam the crate's SetWidget tests already use.
        let landed = wait_until(&mut host, |h| {
            h.widget_text("com.test.invoke") == Some("invoked:say-hello")
        });
        assert!(
            landed,
            "PluginActionInvoked never reached the plugin's stdin; widget text: {:?}",
            host.widget_text("com.test.invoke")
        );
        host.stop_all();
    }

    #[test]
    fn actions_dispatched_count_counts_only_successful_writes() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_action_plugin(root.path(), "com.test.count", ACTION_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());

        // A miss (not running) must not bump the counter.
        host.apply_enabled(&[]);
        assert!(!host.invoke_action("com.test.count", "say-hello"));
        assert_eq!(
            host.actions_dispatched_count(),
            0,
            "a missed invoke is not a successful write"
        );

        // Each delivered invoke bumps it exactly once.
        host.apply_enabled(&[enabled("com.test.count")]);
        assert!(wait_until(&mut host, |h| !h.action_running.is_empty()));
        for expected in 1..=2u64 {
            assert!(host.invoke_action("com.test.count", "say-hello"));
            assert_eq!(host.actions_dispatched_count(), expected);
        }
        host.stop_all();
    }
}
