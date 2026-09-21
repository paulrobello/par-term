//! Window-scoped plugin host over the script runtime.
//!
//! [`PluginHost`] owns its own [`ScriptManager`] instance — the same registry
//! type the per-tab script system uses, kept entirely separate so a plugin
//! can never be confused with a tab script. Discovery, process lifecycle,
//! restart supervision (pin P3: hardcoded on-failure with the existing
//! crash-loop cap), and the `SetWidget` text map all live here.
//!
//! Land-disabled is structural (design D3): [`PluginHost::apply_enabled`]
//! receives only the enabled set, and discovery never spawns anything by
//! itself — a newly dropped-in plugin cannot run until the Settings layer
//! says so.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use par_term_config::RestartPolicy;

use super::manager::{ScriptId, ScriptManager};
use super::manifest::{DiscoveredPlugin, ENTRY_POINT_STATUS_BAR_WIDGET, discover_plugins};
use super::process::ScriptStatus;
use super::protocol::ScriptCommand;
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

/// Extra argv the widget kind's entry point declares before the settings.
fn widget_entry_args(plugin: &DiscoveredPlugin) -> &[String] {
    plugin
        .manifest
        .entry_points
        .get(ENTRY_POINT_STATUS_BAR_WIDGET)
        .map(|entry| entry.args.as_slice())
        .unwrap_or_default()
}

/// Window-scoped plugin host: discovery cache, running processes, restart
/// supervision, and the last `SetWidget` text per plugin.
#[derive(Default)]
pub struct PluginHost {
    /// Last discovery scan's valid plugins.
    discovered: Vec<DiscoveredPlugin>,
    /// Running (or supervised) plugin id → process id. An exited process
    /// stays mapped while its restart is pending so the supervisor keeps
    /// being polled; teardown is the only removal path.
    running: HashMap<String, ScriptId>,
    /// The host's own script registry; never shared with tab scripts.
    manager: ScriptManager,
    /// Last `SetWidget` text per plugin id (last write wins).
    widget_texts: HashMap<String, String>,
    /// Per-plugin restart supervisor (pin P3).
    restart: HashMap<String, ScriptRestartState>,
    /// Settings argv each running plugin was spawned with, kept so a
    /// supervisor restart re-spawns with the same settings.
    settings_json: HashMap<String, String>,
    /// Commands refused because v1 plugins may only send `SetWidget`.
    ignored_lines: Vec<String>,
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

    /// Reconcile running plugins with the enabled set (diff → spawn/stop).
    ///
    /// Land-disabled is structural: only ids present in `enabled` AND in the
    /// discovery cache are spawned, so dropping a plugin directory into the
    /// root does nothing until the Settings layer enables it.
    pub fn apply_enabled(&mut self, enabled: &[EnabledPlugin]) {
        let now = Instant::now();

        let stale: Vec<String> = self
            .running
            .keys()
            .filter(|id| !enabled.iter().any(|e| e.id == **id))
            .cloned()
            .collect();
        for id in &stale {
            self.teardown(id);
        }

        for plugin in enabled {
            if self.running.contains_key(&plugin.id) {
                continue; // already up
            }
            if self
                .restart
                .get(&plugin.id)
                .is_some_and(|state| state.pending())
            {
                continue; // the supervisor owns this slot right now
            }
            let Some(found) = self.discovered.iter().find(|d| d.manifest.id == plugin.id) else {
                log::warn!(
                    "plugin '{}' is enabled but not discovered; not spawned",
                    plugin.id
                );
                continue;
            };
            let entry_path = found.entry_path.clone();
            let entry_args = widget_entry_args(found).to_vec();
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
                }
                Err(error) => {
                    log::warn!("failed to spawn plugin '{}': {}", plugin.id, error);
                }
            }
        }
    }

    /// Advance supervision one frame: drain each plugin's commands, observe
    /// exits, and drive the restart supervisor (on-failure, capped).
    ///
    /// Commands are drained before exit handling so a plugin's final
    /// `SetWidget` before a crash still lands.
    pub fn poll(&mut self) {
        let now = Instant::now();
        let ids: Vec<String> = self.running.keys().cloned().collect();
        for id in ids {
            let Some(&sid) = self.running.get(&id) else {
                continue;
            };

            for cmd in self.manager.read_commands(sid) {
                match cmd {
                    ScriptCommand::SetWidget { text } => {
                        self.widget_texts.insert(id.clone(), text);
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
                let action = match self.restart.get_mut(&id) {
                    Some(state) if !state.pending() => state.on_exit(now, success),
                    Some(state) => state.poll(now),
                    None => RestartAction::Stop,
                };
                match action {
                    RestartAction::Stop => self.teardown(&id),
                    RestartAction::Restart => match self.respawn(&id, now) {
                        Ok(new_sid) => {
                            self.running.insert(id, new_sid);
                        }
                        Err(error) => {
                            log::warn!("plugin '{}' failed to restart: {}", id, error);
                            if let Some(state) = self.restart.get_mut(&id) {
                                state.reschedule(now);
                            }
                        }
                    },
                    RestartAction::Wait | RestartAction::Idle => {}
                }
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

    /// Ids of plugins currently running or under restart supervision.
    pub fn running_plugin_ids(&self) -> Vec<String> {
        self.running.keys().cloned().collect()
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
        self.restart.clear();
        self.widget_texts.clear();
        self.settings_json.clear();
    }

    /// Stop one plugin and forget its supervision and widget state.
    fn teardown(&mut self, id: &str) {
        if let Some(sid) = self.running.remove(id) {
            self.manager.stop_script(sid);
        }
        self.restart.remove(id);
        self.widget_texts.remove(id);
        self.settings_json.remove(id);
    }

    /// Re-spawn a plugin after a supervised restart, reusing its settings.
    ///
    /// The exited process slot stays mapped until the new spawn succeeds —
    /// removing it first would orphan the supervisor (a pending restart
    /// nothing polls).
    fn respawn(&mut self, id: &str, now: Instant) -> Result<ScriptId, String> {
        let settings = self
            .settings_json
            .get(id)
            .cloned()
            .unwrap_or_else(|| "{}".to_string());
        let Some(found) = self.discovered.iter().find(|d| d.manifest.id == id) else {
            return Err(format!("plugin '{}' is no longer discovered", id));
        };
        let entry_path = found.entry_path.clone();
        let entry_args = widget_entry_args(found).to_vec();
        let new_sid = Self::spawn_entry(&mut self.manager, &entry_path, &entry_args, &settings)?;
        if let Some(old_sid) = self.running.insert(id.to_string(), new_sid) {
            self.manager.stop_script(old_sid);
        }
        if let Some(state) = self.restart.get_mut(id) {
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
}
