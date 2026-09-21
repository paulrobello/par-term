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

use std::collections::{HashMap, HashSet};
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
    /// Per-fault warn gates: steady faults warn once per episode, not per
    /// frame (see [`WarnOnce`]).
    warned_not_discovered: WarnOnce,
    warned_spawn_failed: WarnOnce,
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

        // A plugin leaving the enabled set ends its fault episodes, so a
        // disable/enable cycle of a still-missing plugin warns again.
        let enabled_ids: HashSet<String> = enabled.iter().map(|e| e.id.clone()).collect();
        self.warned_not_discovered.clear_except(&enabled_ids);
        self.warned_spawn_failed.clear_except(&enabled_ids);

        for plugin in enabled {
            if self.running.contains_key(&plugin.id) {
                continue; // already up
            }
            if self
                .restart
                .get(&plugin.id)
                .is_some_and(|state| state.pending())
            {
                // The supervisor owns this slot right now. A never-started
                // plugin has no running process for `poll` to drive, so its
                // backoff deadline is polled here: attempt a re-spawn only
                // once the restart delay has elapsed, not every frame.
                if self
                    .restart
                    .get_mut(&plugin.id)
                    .is_none_or(|state| state.poll(now) != RestartAction::Restart)
                {
                    continue;
                }
            }
            let Some(found) = self.discovered.iter().find(|d| d.manifest.id == plugin.id) else {
                if self.warned_not_discovered.should_warn(&plugin.id) {
                    log::warn!(
                        "plugin '{}' is enabled but not discovered; not spawned",
                        plugin.id
                    );
                }
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
                    // A successful spawn resolves both fault episodes.
                    self.warned_not_discovered.clear(&plugin.id);
                    self.warned_spawn_failed.clear(&plugin.id);
                }
                Err(error) => {
                    if self.warned_spawn_failed.should_warn(&plugin.id) {
                        log::warn!("failed to spawn plugin '{}': {}", plugin.id, error);
                    }
                    // A failed spawn arms the supervisor's capped backoff
                    // (same policy as a failed restart): apply_enabled runs
                    // every render frame, and without a pending deadline a
                    // persistently-failing entry re-execs once per frame.
                    let gave_up = {
                        let state = self.restart.entry(plugin.id.clone()).or_insert_with(|| {
                            ScriptRestartState::new(
                                RestartPolicy::OnFailure,
                                PLUGIN_RESTART_DELAY_MS,
                            )
                        });
                        state.reschedule(now) == RestartAction::Stop
                    };
                    if gave_up {
                        // Attempt cap reached. Drop the state so the next
                        // attempt starts a fresh capped round — the reset
                        // the crash-loop path gets via teardown.
                        self.restart.remove(&plugin.id);
                    }
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
        self.warned_not_discovered = WarnOnce::default();
        self.warned_spawn_failed = WarnOnce::default();
    }

    /// Stop one plugin and forget its supervision and widget state.
    fn teardown(&mut self, id: &str) {
        if let Some(sid) = self.running.remove(id) {
            self.manager.stop_script(sid);
        }
        self.restart.remove(id);
        self.widget_texts.remove(id);
        self.settings_json.remove(id);
        // Disarming the fault gates here makes a disable/enable cycle warn
        // again — a fresh user action deserves a fresh diagnostic.
        self.warned_not_discovered.clear(id);
        self.warned_spawn_failed.clear(id);
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
}
