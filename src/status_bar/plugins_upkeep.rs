//! Per-frame plugin upkeep for the status bar.
//!
//! [`StatusBarUI::update_plugins`] lives here rather than in `mod.rs` to keep
//! that file near the 500-line warn line: discovery refresh, the enabled-set
//! reconcile, and process polling form one self-contained unit.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use par_term_scripting::manifest::{SettingSchemaEntry, validate_settings};
use par_term_scripting::plugin_manager::EnabledPlugin;

use crate::config::Config;

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
        self.plugins.apply_enabled(&enabled);
        self.plugins.poll();
    }

    /// Persisted settings serialized as the one argv JSON string, validated
    /// against the discovered manifest's schema. Invalid persisted values
    /// warn and fall back to the schema defaults rather than blocking the
    /// plugin; an undiscovered plugin's settings pass through untouched (the
    /// host warns on apply).
    fn validated_settings_json(&self, state: &crate::config::PluginStateConfig) -> String {
        let schema: &[SettingSchemaEntry] = self
            .plugins
            .discovered(&state.id)
            .and_then(|found| found.manifest.status_bar_widget.as_ref())
            .map(|widget| widget.schema.as_slice())
            .unwrap_or(&[]);
        let settings = match validate_settings(schema, &state.settings) {
            Ok(valid) => valid,
            Err(reason) => {
                log::warn!(
                    "plugin '{}' has invalid persisted settings ({}); using schema defaults",
                    state.id,
                    reason
                );
                validate_settings(schema, &HashMap::new()).unwrap_or_default()
            }
        };
        serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string())
    }
}
