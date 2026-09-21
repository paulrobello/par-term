//! Window-scoped plugin host over the script runtime.
//!
//! [`PluginHost`] owns its own [`ScriptManager`] instance — the same registry
//! type the per-tab script system uses, kept entirely separate so a plugin
//! can never be confused with a tab script. Discovery, per-kind process
//! lifecycle (one supervised process per declared kind, design D5), restart
//! supervision (manifest-declared mode — `on_failure` default, `never`,
//! `always` — under the host-owned delay and crash-loop cap), action
//! invocation, and the `SetWidget` text map all live here.
//!
//! Land-disabled is structural (design D3): [`PluginHost::apply_enabled`]
//! receives only the enabled set, and discovery never spawns anything by
//! itself — a newly dropped-in plugin cannot run until the Settings layer
//! says so.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use par_term_config::RestartPolicy;

use super::manager::{ScriptId, ScriptManager};
use super::manifest::{
    DiscoveredPlugin, ENTRY_POINT_ACTION_CONTRIBUTOR, ENTRY_POINT_PANEL,
    ENTRY_POINT_STATUS_BAR_WIDGET, KIND_ACTION_CONTRIBUTOR, KIND_PANEL, KIND_STATUS_BAR_WIDGET,
    discover_plugins,
};
use super::observer::ScriptEventForwarder;
use super::process::ScriptStatus;
use super::protocol::{PLUGIN_ACTION_INVOKED_KIND, ScriptCommand, ScriptEvent, ScriptEventData};
use super::restart::{RestartAction, ScriptRestartState};

/// Restart delay for a plugin process, host-owned alongside the crash-loop
/// cap (the manifest declares the restart *mode* only — parsight decision
/// 95: a manifest must not be able to tune its way out of the cap).
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

/// Extra argv the panel kind's entry point declares before the settings.
fn panel_entry_args(plugin: &DiscoveredPlugin) -> &[String] {
    plugin
        .manifest
        .entry_points
        .get(ENTRY_POINT_PANEL)
        .map(|entry| entry.args.as_slice())
        .unwrap_or_default()
}

/// One declared plugin kind's supervision slot. Every per-kind map pair is
/// selected through this so the three kinds cannot drift apart — the same
/// role the `action: bool` toggle played for two kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindSlot {
    /// `status-bar-widget` — owns the shared `SetWidget` text map.
    Widget,
    /// `action-contributor` — palette actions; also writes `SetWidget`.
    Action,
    /// `panel` — owns the `SetPanel`/`ClearPanel` content map.
    Panel,
}

impl KindSlot {
    /// Kind name used in log lines.
    fn as_str(self) -> &'static str {
        match self {
            KindSlot::Widget => "widget",
            KindSlot::Action => "action",
            KindSlot::Panel => "panel",
        }
    }
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
    /// Running (or supervised) panel-kind plugin id → process id.
    panel_running: HashMap<String, ScriptId>,
    /// The host's own script registry; never shared with tab scripts.
    manager: ScriptManager,
    /// Last `SetWidget` text per plugin id (last write wins). Either kind's
    /// process writes through the same shared key; stopping the action kind
    /// leaves its last text latched until the widget kind stops or teardown
    /// clears it (documented asymmetry, consistent with the shared-key
    /// contract).
    widget_texts: HashMap<String, String>,
    /// Last `SetPanel` (title, content) per plugin id (last write wins).
    /// Panel-kind processes only; a `ClearPanel` (or teardown — no orphaned
    /// surface) removes the key.
    panel_contents: HashMap<String, (String, String)>,
    /// Successful [`Self::invoke_action`] stdin writes this session — the
    /// `plugin_action_dispatched` ui-test operand's source. Monotonic for
    /// the session; never reset.
    actions_dispatched: u64,
    /// Per-plugin widget-kind restart supervisor (pin P3).
    restart: HashMap<String, ScriptRestartState>,
    /// Per-plugin action-kind restart supervisor, driven identically to
    /// [`Self::restart`] but independently per kind.
    action_restart: HashMap<String, ScriptRestartState>,
    /// Per-plugin panel-kind restart supervisor, driven identically to the
    /// other kinds.
    panel_restart: HashMap<String, ScriptRestartState>,
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
    /// Per-plugin event forwarders for plugins whose manifest declares
    /// subscriptions. Created on the plugin's first successful spawn, dropped
    /// on teardown — the window layer registers each forwarder as an observer
    /// on the window's tab terminals, drains it per sweep, and delivers via
    /// [`Self::deliver_events`] (the same forwarder machinery per-tab scripts
    /// use; per-plugin filtering at the forwarder level is what keeps a
    /// subscription reliable under event floods).
    subscription_forwarders: HashMap<String, Arc<ScriptEventForwarder>>,
    /// Gate for event-delivery write failures (a process exiting
    /// mid-delivery) — the sweep retries delivery every wake, so a dead
    /// process must not warn per frame.
    warned_event_delivery: WarnOnce,
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
    /// root does nothing until the Settings layer enables it. A multi-kind
    /// manifest spawns every declared kind's entry in this one pass, with
    /// the same settings argv and one supervision slot per kind (design D5).
    pub fn apply_enabled(&mut self, enabled: &[EnabledPlugin]) {
        let now = Instant::now();

        let stale: Vec<String> = self
            .running
            .keys()
            .chain(self.action_running.keys())
            .chain(self.panel_running.keys())
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
        self.warned_action_not_running.clear_except(&enabled_ids);
        self.warned_event_delivery.clear_except(&enabled_ids);

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
            let has_panel = found.manifest.kinds.iter().any(|k| k == KIND_PANEL);

            // Extract every declared kind's spawn descriptor up front so the
            // discovery borrow ends before the supervisor/spawn calls below.
            // The action and panel kinds route through their own
            // confinement-checked entries — never `entry_path`, which belongs
            // to the widget kind.
            let widget_entry =
                has_widget.then(|| (found.entry_path.clone(), widget_entry_args(found).to_vec()));
            let action_entry = found
                .action_entry_path
                .clone()
                .filter(|_| has_action)
                .map(|path| (path, action_entry_args(found).to_vec()));
            let panel_entry = found
                .panel_entry_path
                .clone()
                .filter(|_| has_panel)
                .map(|path| (path, panel_entry_args(found).to_vec()));
            // Plugin-level, kind-independent: whichever kind spawns first
            // creates the shared event forwarder.
            let subscriptions = found.manifest.subscriptions.clone();
            // Restart mode is plugin-level too (every kind slot supervises
            // under the same manifest policy; the delay and cap stay
            // host-owned).
            let restart_policy = found.manifest.restart;

            self.spawn_kind_leg(
                &plugin.id,
                KindSlot::Widget,
                widget_entry,
                &plugin.settings_json,
                &subscriptions,
                restart_policy,
                now,
            );
            self.spawn_kind_leg(
                &plugin.id,
                KindSlot::Action,
                action_entry,
                &plugin.settings_json,
                &subscriptions,
                restart_policy,
                now,
            );
            self.spawn_kind_leg(
                &plugin.id,
                KindSlot::Panel,
                panel_entry,
                &plugin.settings_json,
                &subscriptions,
                restart_policy,
                now,
            );
        }
    }

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
    fn spawn_kind_leg(
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

    /// Advance supervision one frame: drain each running plugin process's
    /// commands (every kind), observe exits, and drive each kind's restart
    /// supervisor (manifest-declared mode, capped) — one frame advance
    /// covers the widget, action, and panel maps identically.
    ///
    /// Commands are drained before exit handling so a plugin's final
    /// output before a crash still lands.
    pub fn poll(&mut self) {
        let now = Instant::now();
        let widget_ids: Vec<String> = self.running.keys().cloned().collect();
        for id in widget_ids {
            self.poll_one(&id, KindSlot::Widget, now);
        }
        let action_ids: Vec<String> = self.action_running.keys().cloned().collect();
        for id in action_ids {
            self.poll_one(&id, KindSlot::Action, now);
        }
        let panel_ids: Vec<String> = self.panel_running.keys().cloned().collect();
        for id in panel_ids {
            self.poll_one(&id, KindSlot::Panel, now);
        }
    }

    /// Advance one plugin's supervision for one kind: drain commands,
    /// observe an exit, and act on the supervisor's decision. Shared by all
    /// kinds so their supervision is identical; only the accepted command
    /// set differs per kind (v1 plugins are display-only, design D2).
    fn poll_one(&mut self, id: &str, slot: KindSlot, now: Instant) {
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
                other => {
                    // v1 plugins are display-only (design D2); anything
                    // outside the kind's accepted set is refused with an
                    // error-style line.
                    let accepted = if slot == KindSlot::Panel {
                        "only SetPanel/ClearPanel are accepted from a panel plugin in v1"
                    } else {
                        "only SetWidget is accepted from plugins in v1"
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

    /// Last `SetWidget` text recorded for a plugin, if any.
    pub fn widget_text(&self, plugin_id: &str) -> Option<&str> {
        self.widget_texts.get(plugin_id).map(String::as_str)
    }

    /// Last `SetPanel` (title, content) recorded for a panel plugin, if any.
    pub fn panel_content(&self, plugin_id: &str) -> Option<&(String, String)> {
        self.panel_contents.get(plugin_id)
    }

    /// Live per-plugin event forwarders (plugin id → forwarder), for the
    /// window layer's observer registration and per-sweep drain — the pump
    /// in `StatusBarUI::pump_plugin_events`.
    pub fn subscription_forwarders(&self) -> &HashMap<String, Arc<ScriptEventForwarder>> {
        &self.subscription_forwarders
    }

    /// Deliver drained events to one plugin's running processes (every kind
    /// alike — the manifest's subscriptions are plugin-level, so every
    /// running process of the plugin hears them).
    ///
    /// A plugin with no running process this sweep silently drops the events:
    /// they are moments in time, not state, and its supervisor restarts it
    /// for the next ones. Write failures (a process exiting mid-delivery)
    /// warn once per episode.
    pub fn deliver_events(&mut self, plugin_id: &str, events: &[ScriptEvent]) {
        let targets: Vec<ScriptId> = self
            .running
            .get(plugin_id)
            .into_iter()
            .chain(self.action_running.get(plugin_id))
            .chain(self.panel_running.get(plugin_id))
            .copied()
            .collect();
        if targets.is_empty() {
            return;
        }
        let mut write_failed = false;
        for event in events {
            for &sid in &targets {
                if let Err(error) = self.manager.send_event(sid, event) {
                    write_failed = true;
                    if self.warned_event_delivery.should_warn(plugin_id) {
                        log::warn!(
                            "failed to deliver a {} event to plugin '{}': {}",
                            event.kind,
                            plugin_id,
                            error
                        );
                    }
                }
            }
        }
        if !write_failed {
            self.warned_event_delivery.clear(plugin_id);
        }
    }

    /// All recorded widget texts (plugin id → text), for the status bar.
    pub fn widget_texts(&self) -> &HashMap<String, String> {
        &self.widget_texts
    }

    /// All recorded panel contents (plugin id → (title, content)), for the
    /// settings plugins section.
    pub fn panel_contents(&self) -> &HashMap<String, (String, String)> {
        &self.panel_contents
    }

    /// Successful [`Self::invoke_action`] stdin writes this session.
    /// Monotonic for the session and never reset — delivered-means-true
    /// dispatch has no undo, so the count only grows.
    pub fn actions_dispatched_count(&self) -> u64 {
        self.actions_dispatched
    }

    /// Ids of plugins with any running or supervised process, of any
    /// kind. A multi-kind plugin appears once.
    pub fn running_plugin_ids(&self) -> Vec<String> {
        self.running
            .keys()
            .chain(self.action_running.keys())
            .chain(self.panel_running.keys())
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
        self.panel_running.clear();
        self.restart.clear();
        self.action_restart.clear();
        self.panel_restart.clear();
        self.widget_texts.clear();
        self.panel_contents.clear();
        self.settings_json.clear();
        self.enabled_ids.clear();
        self.subscription_forwarders.clear();
        self.warned_not_discovered = WarnOnce::default();
        self.warned_spawn_failed = WarnOnce::default();
        self.warned_action_not_running = WarnOnce::default();
        self.warned_event_delivery = WarnOnce::default();
    }

    /// Stop one plugin entirely: every kind slot plus all shared state.
    fn teardown(&mut self, id: &str) {
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
        }
    }

    /// The restart-supervisor map for one kind — see [`Self::running_map`].
    fn restart_map(&mut self, slot: KindSlot) -> &mut HashMap<String, ScriptRestartState> {
        match slot {
            KindSlot::Widget => &mut self.restart,
            KindSlot::Action => &mut self.action_restart,
            KindSlot::Panel => &mut self.panel_restart,
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
            KindSlot::Action => {}
        }
        let other_running = [KindSlot::Widget, KindSlot::Action, KindSlot::Panel]
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

    /// A one-shot plugin: emits one widget line and exits 0.
    const ONE_SHOT_SCRIPT: &str = r#"
import json
print(json.dumps({"type": "SetWidget", "text": "once"}), flush=True)
"#;

    /// A widget manifest declaring a restart `mode` (never / always).
    fn restart_manifest_json(id: &str, mode: &str) -> String {
        manifest_json(id).replacen(
            "\"activation\":\"manual\",",
            &format!("\"activation\":\"manual\",\"restart\":\"{mode}\","),
            1,
        )
    }

    #[test]
    fn never_policy_plugin_is_not_restarted_after_a_crash() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.never",
            &restart_manifest_json("com.test.never", "never"),
            &[("widget.py", CRASH_SCRIPT)],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.never")]);

        // The crash exits once and `never` stops it there — no capped round
        // of retries, no supervision state left behind.
        let stopped = wait_until(&mut host, |h| !plugin_running(h, "com.test.never"));
        assert!(stopped, "plugin never stopped");
        assert!(
            !host.restart.contains_key("com.test.never"),
            "stop_kind must drop the supervision state"
        );

        // Well past the 250 ms restart delay: no respawn may appear.
        thread::sleep(Duration::from_millis(700));
        host.poll();
        assert!(
            !plugin_running(&host, "com.test.never"),
            "`never` must not respawn a crashed plugin"
        );
    }

    #[test]
    fn always_policy_plugin_restarts_after_a_clean_exit() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.always",
            &restart_manifest_json("com.test.always", "always"),
            &[("widget.py", ONE_SHOT_SCRIPT)],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.always")]);

        // Under `always` even a clean (exit 0) one-shot is restarted: the
        // first exit schedules attempt 1, the respawn's exit attempt 2, so
        // two consecutive failures prove at least one clean-exit restart
        // fired (on_failure, today's behaviour, would have stopped at the
        // first exit).
        let restarted = wait_until(&mut host, |h| {
            h.restart
                .get("com.test.always")
                .is_some_and(|s| s.consecutive_failures() >= 2)
        });
        assert!(
            restarted,
            "clean exit was never restarted; failures: {:?}",
            host.restart
                .get("com.test.always")
                .map(|s| s.consecutive_failures())
        );
        host.stop_all();
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

    fn panel_manifest_json(id: &str) -> String {
        format!(
            "{{\"schemaVersion\":1,\"id\":\"{id}\",\"name\":\"Test Panel\",\"version\":\"0.1.0\",\
\"kinds\":[\"panel\"],\"activation\":\"manual\",\
\"entryPoints\":{{\"panel\":{{\"command\":\"panel.py\",\"args\":[]}}}}}}"
        )
    }

    /// A panel plugin that emits a `SetWidget` line (refused for the panel
    /// kind) and then a `SetPanel`, then stays alive.
    const PANEL_SCRIPT: &str = r##"
import json, time
def emit(obj):
    print(json.dumps(obj), flush=True)
emit({"type": "SetWidget", "text": "nope"})
emit({"type": "SetPanel", "title": "Notes", "content": "# hello\nworld"})
while True:
    time.sleep(0.2)
"##;

    /// A panel plugin that pushes a panel and then clears it.
    const PANEL_CLEAR_SCRIPT: &str = r#"
import json, time
def emit(obj):
    print(json.dumps(obj), flush=True)
emit({"type": "SetPanel", "title": "T", "content": "c"})
time.sleep(0.3)
emit({"type": "ClearPanel"})
while True:
    time.sleep(0.2)
"#;

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

    fn write_panel_plugin(root: &Path, id: &str, script: &str) {
        write_plugin_files(root, id, &panel_manifest_json(id), &[("panel.py", script)]);
    }

    #[test]
    fn panel_plugin_pushes_content_and_refuses_set_widget() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_panel_plugin(root.path(), "com.test.panel", PANEL_SCRIPT);

        let mut host = PluginHost::new();
        assert_eq!(host.refresh_discovery(root.path()).len(), 1);
        host.apply_enabled(&[enabled("com.test.panel")]);
        let settled = wait_until(&mut host, |h| h.panel_content("com.test.panel").is_some());
        assert!(settled, "fixture's SetPanel never arrived");
        assert_eq!(
            host.panel_content("com.test.panel"),
            Some(&("Notes".to_string(), "# hello\nworld".to_string()))
        );
        // The panel kind's allowlist refused the fixture's SetWidget line
        // with the panel-specific message.
        let ignored = host.drain_ignored().join("\n");
        assert!(ignored.contains("SetWidget"), "got: {ignored}");
        assert!(ignored.contains("SetPanel/ClearPanel"), "got: {ignored}");
        host.stop_all();
    }

    #[test]
    fn clear_panel_from_a_panel_process_removes_the_panel() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_panel_plugin(root.path(), "com.test.panel", PANEL_CLEAR_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.panel")]);
        let pushed = wait_until(&mut host, |h| h.panel_content("com.test.panel").is_some());
        assert!(pushed, "fixture's SetPanel never arrived");
        let cleared = wait_until(&mut host, |h| h.panel_content("com.test.panel").is_none());
        assert!(cleared, "fixture's ClearPanel never landed");
        host.stop_all();
    }

    #[test]
    fn disabling_a_panel_plugin_stops_the_process_and_clears_the_panel() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_panel_plugin(root.path(), "com.test.panel", PANEL_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.panel")]);
        let settled = wait_until(&mut host, |h| h.panel_content("com.test.panel").is_some());
        assert!(settled, "fixture's SetPanel never arrived");
        assert!(plugin_running(&host, "com.test.panel"));

        // Leaving the enabled set tears the plugin down: process stopped AND
        // the pushed panel dropped — no orphaned surface.
        host.apply_enabled(&[]);
        assert!(!plugin_running(&host, "com.test.panel"));
        assert_eq!(host.panel_content("com.test.panel"), None);
        host.stop_all();
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
    fn disable_enable_cycle_rearms_the_action_warn_gate() {
        // A discovered+enabled plugin whose action entry never spawned holds a
        // set action warn gate; disabling must prune it (apply_enabled's
        // clear_except) so a re-enable warns again instead of staying
        // stale-silent across the cycle. Arms the gate directly — the same
        // seam every invoke miss-path uses.
        let mut host = PluginHost::new();
        assert!(host.warned_action_not_running.should_warn("com.test.gate"));
        assert!(
            !host.warned_action_not_running.should_warn("com.test.gate"),
            "second call in the same episode must be silent"
        );

        host.apply_enabled(&[]);
        assert!(
            host.warned_action_not_running.should_warn("com.test.gate"),
            "a disable/enable cycle must re-arm the action warn gate"
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

    /// A plugin that echoes every stdin event's kind back as a `SetWidget`
    /// line prefixed with `tag` — the observable seam for event delivery
    /// (same pattern as `ACTION_SCRIPT` for invokes). `tag` distinguishes
    /// the two kind processes of a both-kinds plugin, which share the
    /// widget-text key.
    fn echo_script(tag: &str) -> String {
        format!(
            r#"
import json, sys
def emit(text):
    print(json.dumps({{"type": "SetWidget", "text": text}}), flush=True)
for line in iter(sys.stdin.readline, ""):
    line = line.strip()
    if not line:
        continue
    try:
        event = json.loads(line)
    except ValueError:
        continue
    emit("{tag}:" + str(event.get("kind", "?")))
"#
        )
    }

    /// A widget-kind manifest declaring `subscriptions_json` (raw JSON array
    /// body, e.g. `"bell_rang"`).
    fn subscribed_manifest_json(id: &str, subscriptions_json: &str) -> String {
        manifest_json(id).replacen(
            r#""kinds":["status-bar-widget"],"#,
            &format!(r#""kinds":["status-bar-widget"],"subscriptions":[{subscriptions_json}],"#),
            1,
        )
    }

    /// A both-kinds manifest declaring `subscriptions_json`.
    fn subscribed_both_kinds_manifest_json(id: &str, subscriptions_json: &str) -> String {
        both_kinds_manifest_json(id).replacen(
            r#""kinds":["status-bar-widget","action-contributor"],"#,
            &format!(
                r#""kinds":["status-bar-widget","action-contributor"],"subscriptions":[{subscriptions_json}],"#
            ),
            1,
        )
    }

    #[test]
    fn subscribed_plugin_gets_a_kind_filtered_forwarder_on_spawn() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.sub",
            &subscribed_manifest_json("com.test.sub", r#""bell_rang""#),
            &[("widget.py", WIDGET_SCRIPT)],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.sub")]);

        let forwarder = host
            .subscription_forwarders()
            .get("com.test.sub")
            .cloned()
            .expect("a subscribed plugin gets a forwarder on spawn");

        // The forwarder carries the plugin's kind filter: a bell buffers, a
        // title change (not subscribed) never enters the buffer.
        use par_term_emu_core_rust::observer::TerminalObserver;
        use par_term_emu_core_rust::terminal::{BellEvent, TerminalEvent};
        forwarder.on_event(&TerminalEvent::BellRang(BellEvent::VisualBell));
        forwarder.on_event(&TerminalEvent::TitleChanged("nope".to_string()));
        let drained = forwarder.drain_events();
        assert_eq!(drained.len(), 1, "unsubscribed kinds must not buffer");
        assert_eq!(drained[0].kind, "bell_rang");
        host.stop_all();
    }

    #[test]
    fn unsubscribed_plugin_gets_no_forwarder() {
        if skip_without_interpreter() {
            return;
        }
        // The self-scheduled contract: a plugin with no subscriptions block
        // must not gain event delivery (or its buffering machinery).
        let root = tempfile::tempdir().unwrap();
        write_plugin(root.path(), "com.test.plain", WIDGET_SCRIPT);

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.plain")]);
        assert!(wait_until(&mut host, |h| !h.running.is_empty()));
        assert!(
            host.subscription_forwarders().is_empty(),
            "no subscriptions declared — no forwarder may exist"
        );
        host.stop_all();
    }

    #[test]
    fn teardown_drops_the_forwarder_when_the_plugin_leaves_the_enabled_set() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.leave",
            &subscribed_manifest_json("com.test.leave", r#""bell_rang""#),
            &[("widget.py", WIDGET_SCRIPT)],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.leave")]);
        assert!(
            host.subscription_forwarders()
                .contains_key("com.test.leave")
        );

        host.apply_enabled(&[]);
        assert!(
            host.subscription_forwarders().is_empty(),
            "teardown must drop the forwarder so its observer is unregistered"
        );
    }

    #[test]
    fn delivered_events_reach_the_running_process() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.echo",
            &subscribed_manifest_json("com.test.echo", r#""bell_rang""#),
            &[("widget.py", &echo_script("event"))],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.echo")]);

        // Simulate the window pump: the forwarder observes a terminal event,
        // is drained, and the drained events are delivered to the plugin.
        let forwarder = host
            .subscription_forwarders()
            .get("com.test.echo")
            .cloned()
            .expect("forwarder exists for a subscribed plugin");
        use par_term_emu_core_rust::observer::TerminalObserver;
        use par_term_emu_core_rust::terminal::{BellEvent, TerminalEvent};
        forwarder.on_event(&TerminalEvent::BellRang(BellEvent::VisualBell));
        let events = forwarder.drain_events();
        assert_eq!(events.len(), 1);
        host.deliver_events("com.test.echo", &events);

        // The fixture echoes the event kind back as its widget text, so the
        // round-trip through plugin stdin is observable at the same seam the
        // crate's SetWidget tests use.
        let landed = wait_until(&mut host, |h| {
            h.widget_text("com.test.echo") == Some("event:bell_rang")
        });
        assert!(
            landed,
            "the bell event never reached the plugin's stdin; widget text: {:?}",
            host.widget_text("com.test.echo")
        );
        host.stop_all();
    }

    /// Widget-kind echo that delays its reply. Both kind processes of a
    /// plugin share one last-write-wins widget-text key, so when one poll
    /// drains both echoes the second masks the first before any observer can
    /// sample it — the delay spaces the two writes so each is seen.
    const WIDGET_ECHO_DELAYED_SCRIPT: &str = r#"
import json, sys, time
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
    time.sleep(0.3)
    emit("w:" + str(event.get("kind", "?")))
"#;

    #[test]
    fn both_kinds_processes_receive_subscribed_events() {
        if skip_without_interpreter() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        write_plugin_files(
            root.path(),
            "com.test.both-echo",
            &subscribed_both_kinds_manifest_json("com.test.both-echo", r#""bell_rang""#),
            &[
                ("widget.py", WIDGET_ECHO_DELAYED_SCRIPT),
                ("actions.py", &echo_script("a")),
            ],
        );

        let mut host = PluginHost::new();
        host.refresh_discovery(root.path());
        host.apply_enabled(&[enabled("com.test.both-echo")]);
        assert!(wait_until(&mut host, |h| h.running.len() == 1
            && h.action_running.len() == 1));

        let forwarder = host
            .subscription_forwarders()
            .get("com.test.both-echo")
            .cloned()
            .expect("forwarder exists for a subscribed plugin");
        use par_term_emu_core_rust::observer::TerminalObserver;
        use par_term_emu_core_rust::terminal::{BellEvent, TerminalEvent};
        forwarder.on_event(&TerminalEvent::BellRang(BellEvent::VisualBell));
        let events = forwarder.drain_events();
        host.deliver_events("com.test.both-echo", &events);

        // Both kind processes echo with their own tag into the shared
        // widget-text key (last write wins), so the proof is having SEEN
        // each tag at some point during polling.
        let mut seen_widget = false;
        let mut seen_action = false;
        let both_seen = wait_until(&mut host, |h| {
            if h.widget_text("com.test.both-echo") == Some("w:bell_rang") {
                seen_widget = true;
            }
            if h.widget_text("com.test.both-echo") == Some("a:bell_rang") {
                seen_action = true;
            }
            seen_widget && seen_action
        });
        assert!(
            both_seen,
            "both kind processes must receive the event (w seen: {seen_widget}, a seen: {seen_action})"
        );
        host.stop_all();
    }

    #[test]
    fn deliver_events_is_silent_with_no_running_process() {
        // A plugin between restarts must not warn or panic when the sweep
        // delivers events nobody is running to receive.
        install_counting_logger();
        let mut host = PluginHost::new();
        let event = ScriptEvent {
            kind: "bell_rang".to_string(),
            data: ScriptEventData::Empty {},
        };
        host.deliver_events("com.test.nobody", &[event.clone()]);
        assert_eq!(warns_containing("com.test.nobody"), 0);
    }
}
