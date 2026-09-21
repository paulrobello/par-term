//! Per-frame plugin upkeep for the status bar.
//!
//! [`StatusBarUI::update_plugins`] lives here rather than in `mod.rs` to keep
//! that file near the 500-line warn line: discovery refresh, the enabled-set
//! reconcile, and process polling form one self-contained unit.

use std::time::{Duration, Instant};

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
    ///
    /// Takes the config now because Task 4 reads the enabled set from it
    /// (`plugins:` state); until that type lands the empty set keeps every
    /// plugin stopped — land-disabled is structural, `apply_enabled` is the
    /// only spawn path.
    pub(crate) fn update_plugins(&mut self, _config: &Config) {
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
        self.plugins.apply_enabled(&[]);
        self.plugins.poll();
    }
}
