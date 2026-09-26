//! Per-frame plugin upkeep for the status bar.
//!
//! [`StatusBarUI::update_plugins`] lives here rather than in `mod.rs` to keep
//! that file near the 500-line warn line: discovery refresh, the enabled-set
//! reconcile, and process polling form one self-contained unit.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use par_term_scripting::manifest::{SettingSchemaEntry, validate_settings};
use par_term_scripting::observer::ScriptEventForwarder;
use par_term_scripting::plugin_manager::{EnabledPlugin, PluginHost};

use crate::config::Config;
use crate::pane::PaneId;
use crate::tab::{TabId, TabManager};

use super::StatusBarUI;

/// How often the plugins root is re-scanned for new or changed manifests.
const PLUGIN_DISCOVERY_INTERVAL: Duration = Duration::from_secs(300);

impl StatusBarUI {
    /// Per-frame plugin upkeep: periodic discovery refresh, enabled-set
    /// reconcile, and process polling.
    ///
    /// Called unconditionally from the render pipeline every frame — NOT from
    /// [`StatusBarUI::render`], which only runs while the bar is visible. A
    /// plugin's `SetWidget` output must keep flowing with the bar hidden.
    pub(crate) fn update_plugins(&mut self, config: &Config) {
        // Scan on the first call and every interval thereafter; a re-scan
        // never stops or spawns anything by itself.
        if self
            .plugins_last_discovery
            .is_none_or(|at| at.elapsed() >= PLUGIN_DISCOVERY_INTERVAL)
        {
            self.plugins
                .refresh_discovery(&Config::config_dir().join("plugins"));
            self.plugins_last_discovery = Some(Instant::now());
        }

        let enabled: Vec<EnabledPlugin> = config
            .automation
            .plugins
            .iter()
            .filter(|state| state.enabled)
            .map(|state| EnabledPlugin {
                id: state.id.clone(),
                settings_json: self.validated_settings_json(state),
            })
            .collect();
        // A plugin leaving the enabled set ends its invalid-settings episode,
        // so re-enabling it warns again instead of staying silent forever.
        let enabled_ids: HashSet<String> = enabled.iter().map(|e| e.id.clone()).collect();
        self.plugin_settings_warned.clear_except(&enabled_ids);
        self.plugins.apply_enabled(&enabled);
        self.plugins.poll();
    }

    /// Reconcile plugin-event observer registrations against the window's
    /// tabs and pump drained events into the running plugins.
    ///
    /// Called once per event-loop wake beside the tab-script sweep — NOT
    /// from [`StatusBarUI::update_plugins`], which runs on the render path
    /// and never sees the tab list. Each subscribed plugin's forwarder is
    /// registered as an observer on every tab terminal of this window, and
    /// on every pane terminal of a mux tab (whose panes, not `terminal`,
    /// carry the daemon-fed output the user sees): delivery is
    /// window-wide (each event originates at exactly one terminal, so
    /// nothing is duplicated; events carry no tab attribution in v1). A
    /// registration that misses on a contended terminal lock retries on
    /// the next wake.
    pub(crate) fn pump_plugin_events(&mut self, tabs: &TabManager) {
        // The common case — no subscribed plugins, nothing registered —
        // exits before touching the tab list.
        if self.plugins.subscription_forwarders().is_empty() && self.plugin_observer_ids.is_empty()
        {
            return;
        }

        // Pass A — the desired registration set: every live forwarder on
        // every live tab's terminal and, for a mux tab, on every mirror
        // pane terminal. `None` names the tab terminal; `Some(id)` a pane.
        let mut desired: HashSet<(String, TabId, Option<PaneId>)> = HashSet::new();
        for plugin_id in self.plugins.subscription_forwarders().keys() {
            for tab in tabs.tabs() {
                desired.insert((plugin_id.clone(), tab.id, None));
                if tab.is_mux_tab()
                    && let Some(pm) = tab.pane_manager()
                {
                    for pane in pm.all_panes() {
                        desired.insert((plugin_id.clone(), tab.id, Some(pane.id)));
                    }
                }
            }
        }

        // Register anything desired but missing.
        for key in &desired {
            if self.plugin_observer_ids.contains_key(key) {
                continue;
            }
            let Some(tab) = tabs.tabs().iter().find(|tab| tab.id == key.1) else {
                continue;
            };
            let Some(forwarder) = self.plugins.subscription_forwarders().get(&key.0) else {
                continue;
            };
            let terminal = match key.2 {
                // A pane registration targets that pane's terminal — the
                // pane may be gone already (the reconcile is per-frame);
                // a missing pane simply retries or is dropped as stale.
                None => &tab.terminal,
                Some(pane_id) => match tab.pane_manager().and_then(|pm| pm.get_pane(pane_id)) {
                    Some(pane) => &pane.terminal,
                    None => continue,
                },
            };
            let Ok(terminal) = terminal.try_read() else {
                continue;
            };
            let observer_id = terminal.add_observer(forwarder.clone());
            self.plugin_observer_ids.insert(key.clone(), observer_id);
        }

        // Unregister anything registered but no longer desired.
        let stale: Vec<(String, TabId, Option<PaneId>)> = self
            .plugin_observer_ids
            .keys()
            .filter(|key| !desired.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(observer_id) = self.plugin_observer_ids.remove(&key) {
                // A closed tab's terminal (and the observer registry inside
                // it) is gone with the tab — dropping the map entry is the
                // whole cleanup there, and a closed pane is gone with its
                // terminal the same way. Only a live terminal needs an
                // explicit unregister.
                if let Some(tab) = tabs.tabs().iter().find(|tab| tab.id == key.1) {
                    let terminal = match key.2 {
                        None => Some(tab.terminal.clone()),
                        Some(pane_id) => tab
                            .pane_manager()
                            .and_then(|pm| pm.get_pane(pane_id))
                            .map(|pane| pane.terminal.clone()),
                    };
                    if let Some(terminal) = terminal
                        && let Ok(term) = terminal.try_read()
                    {
                        term.remove_observer(observer_id);
                    }
                }
            }
        }

        // Pass B — drain each forwarder and deliver to its plugin. The Arc
        // clones release the host borrow before delivery mutates it.
        let to_drain: Vec<(String, std::sync::Arc<ScriptEventForwarder>)> = self
            .plugins
            .subscription_forwarders()
            .iter()
            .map(|(id, forwarder)| (id.clone(), std::sync::Arc::clone(forwarder)))
            .collect();
        for (plugin_id, forwarder) in &to_drain {
            let events = forwarder.drain_events();
            if !events.is_empty() {
                self.plugins.deliver_events(plugin_id, &events);
            }
        }
    }

    /// Discovery-cache size for the `plugins_loaded` ui-test operand.
    pub(crate) fn plugins_discovered_count(&self) -> usize {
        self.plugins.discovered_count()
    }

    /// Whether any plugin has published a non-empty widget text, for the
    /// `plugin_widget_set` ui-test operand.
    pub(crate) fn any_plugin_widget_text(&self) -> bool {
        self.plugins.widget_texts().values().any(|t| !t.is_empty())
    }

    /// Whether any panel plugin has pushed a panel, for the
    /// `plugin_panel_set` ui-test operand.
    pub(crate) fn any_plugin_panel_pushed(&self) -> bool {
        !self.plugins.panel_contents().is_empty()
    }

    /// Whether any plugin action invocation was successfully delivered
    /// this session, for the `plugin_action_dispatched` ui-test operand.
    pub(crate) fn any_plugin_action_dispatched(&self) -> bool {
        self.plugins.actions_dispatched_count() >= 1
    }

    /// Mutable access to the plugin host, for keybinding dispatch of
    /// plugin-contributed palette actions
    /// (`plugin-action:<plugin_id>:<action_id>`).
    pub(crate) fn plugin_host_mut(&mut self) -> &mut PluginHost {
        &mut self.plugins
    }

    /// Read-only access to the plugin host, for enumerating plugin-contributed
    /// palette actions when the command palette opens.
    pub(crate) fn plugin_host(&self) -> &PluginHost {
        &self.plugins
    }

    /// Persisted settings serialized as the one argv JSON string, validated
    /// against the discovered manifest's schema. Invalid persisted values
    /// warn (once per episode — this runs every frame) and fall back to the
    /// schema defaults rather than blocking the plugin; an undiscovered
    /// plugin's settings pass through untouched (the host warns on apply).
    fn validated_settings_json(&mut self, state: &crate::config::PluginStateConfig) -> String {
        let schema: &[SettingSchemaEntry] = self
            .plugins
            .discovered(&state.id)
            .and_then(|found| found.manifest.status_bar_widget.as_ref())
            .map(|widget| widget.schema.as_slice())
            .unwrap_or(&[]);
        let settings = match validate_settings(schema, &state.settings) {
            Ok(valid) => {
                self.plugin_settings_warned.clear(&state.id);
                valid
            }
            Err(reason) => {
                if self.plugin_settings_warned.should_warn(&state.id) {
                    log::warn!(
                        "plugin '{}' has invalid persisted settings ({}); using schema defaults",
                        state.id,
                        reason
                    );
                }
                validate_settings(schema, &HashMap::new()).unwrap_or_default()
            }
        };
        serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string())
    }
}
