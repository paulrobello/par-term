//! Plugins section of the automation settings tab.
//!
//! Trust surface first: every discovered plugin shows its author, version,
//! license, and the exact entry command *above* the enable toggle, so the
//! user reads what runs before allowing it to run. The section never spawns
//! anything itself — it only writes `plugins:` state entries (and, on first
//! enable of a widget-kind plugin, a status-bar widget row) into the edited
//! config; the host's per-frame reconcile picks the change up after save.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use par_term_config::PluginStateConfig;
use par_term_config::status_bar::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use par_term_scripting::manifest::{
    ActionContribution, DiscoveredPlugin, ENTRY_POINT_ACTION_CONTRIBUTOR, ENTRY_POINT_PANEL,
    ENTRY_POINT_STATUS_BAR_WIDGET, KIND_ACTION_CONTRIBUTOR, KIND_PANEL, KIND_STATUS_BAR_WIDGET,
    PluginEntryPoint, PluginManifest, SettingSchemaEntry, SettingType, discover_plugins,
};
use std::collections::HashSet;

/// Show the Plugins section, filtered by the settings search query.
pub(super) fn show_plugins_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Plugins",
        &[
            "plugin",
            "plugins",
            "status bar widget",
            "manifest",
            "widget",
            "extensions",
            "plugin action",
            "palette actions",
        ],
    ) {
        show_plugins_collapsing(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_plugins_collapsing(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Plugins", "automation_plugins", true, collapsed, |ui| {
        ui.label(
            "Local plugins run as subprocesses and publish status-bar widgets, contribute command-palette actions, or push panel content.",
        );
        ui.label("Plugins are disabled until enabled here; install means copying a directory into the plugins folder.");
        ui.add_space(4.0);

        // The scan runs once (and on Rescan): a per-frame directory walk
        // would be wasted work for a list that only changes on disk.
        if ui.small_button("Rescan").clicked() {
            settings.automation_tab.plugin_scan = None;
        }
        if settings.automation_tab.plugin_scan.is_none() {
            let root = par_term_config::Config::config_dir().join("plugins");
            settings.automation_tab.plugin_scan = Some(discover_plugins(&root));
        }
        // Clone out of the tab state so the loop below can freely mutate
        // `settings.config` (discovery order is deterministic by id).
        let (discovered, warnings) = settings
            .automation_tab
            .plugin_scan
            .clone()
            .unwrap_or_default();
        let mut discovered = discovered;
        discovered.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));

        for warning in &warnings {
            ui.label(
                egui::RichText::new(format!("skipped '{}': {}", warning.dir, warning.reason))
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        let mut first = true;
        for plugin in &discovered {
            if !first {
                ui.add_space(6.0);
            }
            first = false;
            show_plugin_row(ui, settings, changes_this_frame, plugin);
        }
        if discovered.is_empty() && warnings.is_empty() {
            ui.label(egui::RichText::new("No plugins found.").color(egui::Color32::GRAY));
        }

        // Config entries whose plugin directory is gone keep their state:
        // the id may come back, and removing it silently would lose the
        // user's settings.
        let missing = missing_state_ids(&settings.config, &discovered);
        if !missing.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Missing plugins").strong());
            for id in missing {
                let enabled = settings
                    .config
                    .automation
                    .plugins
                    .iter()
                    .any(|state| state.id == id && state.enabled);
                let suffix = if enabled { " (enabled)" } else { "" };
                ui.label(
                    egui::RichText::new(format!("{id} — not found — state kept{suffix}"))
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }
        }
    });
}

/// One discovered plugin: trust surface, enable toggle, schema-driven
/// settings editor, section placement, and — for the panel kind — the live
/// pushed panel viewer.
fn show_plugin_row(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    plugin: &DiscoveredPlugin,
) {
    let manifest = &plugin.manifest;
    let state = settings
        .config
        .automation
        .plugins
        .iter()
        .find(|state| state.id == manifest.id);
    let is_enabled = state.is_some_and(|state| state.enabled);
    let has_widget_row = settings
        .config
        .status_bar
        .status_bar_widgets
        .iter()
        .any(|w| w.id == WidgetId::Plugin(manifest.id.clone()));

    // Trust surface: who wrote it and what will run.
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&manifest.name).strong());
        ui.label(egui::RichText::new(format!("v{}", manifest.version)).small());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut enabled = is_enabled;
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                set_plugin_enabled(&mut settings.config, plugin, enabled);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
    let mut meta = vec![manifest.id.clone()];
    if let Some(author) = &manifest.author {
        meta.push(author.clone());
    }
    if let Some(license) = &manifest.license {
        meta.push(license.clone());
    }
    ui.label(
        egui::RichText::new(meta.join(" · "))
            .small()
            .color(egui::Color32::GRAY),
    );
    if let Some(description) = &manifest.description {
        ui.label(egui::RichText::new(description).small());
    }
    for line in runs_lines(manifest) {
        ui.label(egui::RichText::new(line).small());
    }
    if manifest.kinds.iter().any(|k| k == KIND_ACTION_CONTRIBUTOR) {
        let summary = action_summary(&manifest.actions);
        if !summary.is_empty() {
            ui.label(egui::RichText::new(summary).small());
        }
    }

    // Live panel viewer for the panel kind: the plugin's pushed SetPanel
    // content, mirrored from the focused window's host. A plain
    // CollapsingHeader id-salted per plugin — the section's shared
    // `collapsed` set cannot be borrowed inside this row (it already backs
    // the enclosing section), and per-plugin state belongs in egui memory
    // anyway.
    if manifest.kinds.iter().any(|k| k == KIND_PANEL) {
        if let Some((title, content)) = settings.plugin_panels.get(&manifest.id) {
            let panel_title = format!("Panel: {title}");
            let panel_id = format!("plugin_panel_{}", manifest.id);
            let panel_scroll_id = format!("plugin_panel_scroll_{}", manifest.id);
            egui::CollapsingHeader::new(&panel_title)
                .id_salt(&panel_id)
                .default_open(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(&panel_scroll_id)
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(content)
                                    .monospace()
                                    .small()
                                    .color(egui::Color32::from_rgb(200, 200, 200)),
                            );
                        });
                });
        } else if is_enabled {
            ui.label(
                egui::RichText::new("Panel: no content pushed yet")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }
    }

    // Schema-driven settings editor, bound to the persisted settings map.
    if let Some(widget) = manifest.status_bar_widget.as_ref()
        && !widget.schema.is_empty()
    {
        ui.add_space(2.0);
        for entry in &widget.schema {
            let current = settings
                .config
                .automation
                .plugins
                .iter()
                .find(|state| state.id == manifest.id)
                .and_then(|state| state.settings.get(&entry.key))
                .cloned()
                .unwrap_or_else(|| entry.default_value.clone());
            show_setting_editor(
                ui,
                settings,
                changes_this_frame,
                &manifest.id,
                current,
                entry,
            );
        }
    }

    // Placement: the same edit the status-bar tab makes for built-ins.
    if has_widget_row {
        let current = settings
            .config
            .status_bar
            .status_bar_widgets
            .iter()
            .find(|w| w.id == WidgetId::Plugin(manifest.id.clone()))
            .map(|w| w.section)
            .unwrap_or_default();
        let mut selected = current;
        ui.horizontal(|ui| {
            ui.label("Section:");
            egui::ComboBox::from_id_salt(format!("plugin_section_{}", manifest.id))
                .selected_text(section_label(selected))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, StatusBarSection::Left, "Left");
                    ui.selectable_value(&mut selected, StatusBarSection::Center, "Center");
                    ui.selectable_value(&mut selected, StatusBarSection::Right, "Right");
                });
        });
        if selected != current {
            set_plugin_placement(&mut settings.config, &manifest.id, selected);
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    }
}

/// One schema entry's editor. `current` is an owned snapshot of the
/// persisted value (or the schema default); changes write straight back
/// into the settings map.
fn show_setting_editor(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    plugin_id: &str,
    current: serde_json::Value,
    entry: &SettingSchemaEntry,
) {
    let value = current;
    ui.horizontal(|ui| {
        ui.label(&entry.label);
        match entry.setting_type {
            SettingType::Boolean => {
                let mut checked = value.as_bool().unwrap_or(false);
                if ui.checkbox(&mut checked, "").changed() {
                    write_plugin_setting(
                        &mut settings.config,
                        plugin_id,
                        &entry.key,
                        checked.into(),
                    );
                    dirty(settings, changes_this_frame);
                }
            }
            SettingType::Integer => {
                let mut number = value
                    .as_i64()
                    .or_else(|| entry.default_value.as_i64())
                    .unwrap_or(0);
                let response =
                    ui.add(egui::DragValue::new(&mut number).speed(entry.step.unwrap_or(1.0)));
                let clamped = clamp_i64(number, entry.min, entry.max);
                if response.changed() || clamped != number {
                    write_plugin_setting(
                        &mut settings.config,
                        plugin_id,
                        &entry.key,
                        clamped.into(),
                    );
                    dirty(settings, changes_this_frame);
                }
            }
            SettingType::Number => {
                let mut number = value
                    .as_f64()
                    .or_else(|| entry.default_value.as_f64())
                    .unwrap_or(0.0);
                let response = ui.add(
                    egui::DragValue::new(&mut number)
                        .speed(entry.step.unwrap_or(0.1))
                        .fixed_decimals(2),
                );
                let clamped = clamp_f64(number, entry.min, entry.max);
                if response.changed() || clamped != number {
                    write_plugin_setting(
                        &mut settings.config,
                        plugin_id,
                        &entry.key,
                        serde_json::json!(clamped),
                    );
                    dirty(settings, changes_this_frame);
                }
            }
            SettingType::String => {
                let mut text = value
                    .as_str()
                    .or_else(|| entry.default_value.as_str())
                    .unwrap_or("")
                    .to_string();
                if ui.add(egui::TextEdit::singleline(&mut text)).changed() {
                    write_plugin_setting(&mut settings.config, plugin_id, &entry.key, text.into());
                    dirty(settings, changes_this_frame);
                }
            }
        }
    });
}

fn dirty(settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    settings.has_changes = true;
    *changes_this_frame = true;
}

fn section_label(section: StatusBarSection) -> &'static str {
    match section {
        StatusBarSection::Left => "Left",
        StatusBarSection::Center => "Center",
        StatusBarSection::Right => "Right",
    }
}

fn clamp_i64(value: i64, min: Option<f64>, max: Option<f64>) -> i64 {
    let mut value = value;
    if let Some(min) = min {
        value = value.max(min as i64);
    }
    if let Some(max) = max {
        value = value.min(max as i64);
    }
    value
}

fn clamp_f64(value: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let mut value = value;
    if let Some(min) = min {
        value = value.max(min);
    }
    if let Some(max) = max {
        value = value.min(max);
    }
    value
}

/// One-line trust-surface summary of a plugin's contributed palette
/// actions, e.g. `contributes 2 actions: Greet, Stamp`. Empty input
/// renders nothing (widget-kind plugins' rows stay unchanged).
fn action_summary(actions: &[ActionContribution]) -> String {
    match actions.len() {
        0 => String::new(),
        1 => format!("contributes 1 action: {}", actions[0].label),
        n => {
            let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
            format!("contributes {n} actions: {}", labels.join(", "))
        }
    }
}

/// `./command arg arg` form of one entry point, as the trust surface
/// shows it.
fn entry_command(entry: &PluginEntryPoint) -> String {
    let mut command = format!("./{}", entry.command);
    for arg in &entry.args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

/// The trust surface's entry-command lines: one line per declared kind's
/// entry point — a bare `Runs: ./x` while only one kind is declared, one
/// kind-labeled line per executable when both are, so the user can tell
/// which command serves which kind. An undeclared kind's stray entry point
/// earns no line: the host never runs it.
fn runs_lines(manifest: &PluginManifest) -> Vec<String> {
    let has_widget = manifest.kinds.iter().any(|k| k == KIND_STATUS_BAR_WIDGET);
    let has_action = manifest.kinds.iter().any(|k| k == KIND_ACTION_CONTRIBUTOR);
    let has_panel = manifest.kinds.iter().any(|k| k == KIND_PANEL);
    let mut lines = Vec::new();
    if has_widget && let Some(entry) = manifest.entry_points.get(ENTRY_POINT_STATUS_BAR_WIDGET) {
        let label = if has_action || has_panel {
            "Runs widget:"
        } else {
            "Runs:"
        };
        lines.push(format!("{label} {}", entry_command(entry)));
    }
    if has_action && let Some(entry) = manifest.entry_points.get(ENTRY_POINT_ACTION_CONTRIBUTOR) {
        let label = if has_widget || has_panel {
            "Runs actions:"
        } else {
            "Runs:"
        };
        lines.push(format!("{label} {}", entry_command(entry)));
    }
    if has_panel && let Some(entry) = manifest.entry_points.get(ENTRY_POINT_PANEL) {
        let label = if has_widget || has_action {
            "Runs panel:"
        } else {
            "Runs:"
        };
        lines.push(format!("{label} {}", entry_command(entry)));
    }
    lines
}

/// Ids with a persisted `plugins:` entry but no discovered manifest.
fn missing_state_ids(
    config: &par_term_config::Config,
    discovered: &[DiscoveredPlugin],
) -> Vec<String> {
    config
        .automation
        .plugins
        .iter()
        .filter(|state| !discovered.iter().any(|p| p.manifest.id == state.id))
        .map(|state| state.id.clone())
        .collect()
}

/// Flip a plugin's enabled bit, creating the state entry on first enable
/// (settings seeded from the manifest defaults). Enabling a widget-kind
/// plugin also ensures it has a status-bar widget row; an action-only
/// plugin gets no row. Disabling only flips the bit — the row self-hides
/// while the plugin publishes no text, and comes back with its placement
/// intact on re-enable.
fn set_plugin_enabled(
    config: &mut par_term_config::Config,
    plugin: &DiscoveredPlugin,
    enabled: bool,
) {
    let id = &plugin.manifest.id;
    match config
        .automation
        .plugins
        .iter_mut()
        .find(|state| &state.id == id)
    {
        Some(state) => state.enabled = enabled,
        None => {
            if enabled {
                let defaults = plugin
                    .manifest
                    .status_bar_widget
                    .as_ref()
                    .map(|widget| widget.defaults.clone())
                    .unwrap_or_default();
                config.automation.plugins.push(PluginStateConfig {
                    id: id.clone(),
                    enabled: true,
                    settings: defaults,
                    section: None,
                });
            }
            // Disabling a plugin with no state entry is a no-op.
        }
    }
    if enabled {
        ensure_widget_row(config, plugin);
    }
}

/// Add the plugin's widget row if absent and the plugin declares the
/// status-bar-widget kind — an action-only plugin has nothing to place in
/// the bar and must not get a phantom row. Placement prefers a persisted
/// section override, then the manifest default, then the right section.
fn ensure_widget_row(config: &mut par_term_config::Config, plugin: &DiscoveredPlugin) {
    if !plugin
        .manifest
        .kinds
        .iter()
        .any(|k| k == KIND_STATUS_BAR_WIDGET)
    {
        return;
    }
    let id = WidgetId::Plugin(plugin.manifest.id.clone());
    if config
        .status_bar
        .status_bar_widgets
        .iter()
        .any(|w| w.id == id)
    {
        return;
    }
    let section = config
        .automation
        .plugins
        .iter()
        .find(|state| state.id == plugin.manifest.id)
        .and_then(|state| state.section)
        .or_else(|| {
            plugin
                .manifest
                .status_bar_widget
                .as_ref()
                .map(|widget| widget.section)
        })
        .unwrap_or(StatusBarSection::Right);
    let order = config
        .status_bar
        .status_bar_widgets
        .iter()
        .map(|w| w.order)
        .max()
        .unwrap_or(0)
        + 1;
    config
        .status_bar
        .status_bar_widgets
        .push(StatusBarWidgetConfig {
            id,
            enabled: true,
            section,
            order,
            format: None,
        });
}

/// Write one settings value, upserting the state entry. A plugin configured
/// before its first enable keeps `enabled: false` — configuring is not
/// consent to run.
fn write_plugin_setting(
    config: &mut par_term_config::Config,
    plugin_id: &str,
    key: &str,
    value: serde_json::Value,
) {
    match config
        .automation
        .plugins
        .iter_mut()
        .find(|state| state.id == plugin_id)
    {
        Some(state) => {
            state.settings.insert(key.to_string(), value);
        }
        None => {
            config.automation.plugins.push(PluginStateConfig {
                id: plugin_id.to_string(),
                enabled: false,
                settings: [(key.to_string(), value)].into_iter().collect(),
                section: None,
            });
        }
    }
}

/// Move the plugin's widget row to a section and record the placement in
/// the state entry, so a disable/re-enable cycle restores it.
fn set_plugin_placement(
    config: &mut par_term_config::Config,
    plugin_id: &str,
    section: StatusBarSection,
) {
    if let Some(widget) = config
        .status_bar
        .status_bar_widgets
        .iter_mut()
        .find(|w| w.id == WidgetId::Plugin(plugin_id.to_string()))
    {
        widget.section = section;
    }
    if let Some(state) = config
        .automation
        .plugins
        .iter_mut()
        .find(|state| state.id == plugin_id)
    {
        state.section = Some(section);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.fixture",
        "name": "Fixture",
        "version": "0.1.0",
        "author": "test",
        "license": "MIT",
        "kinds": ["status-bar-widget"],
        "activation": "manual",
        "entryPoints": { "statusBarWidget": { "command": "fixture.sh", "args": [] } },
        "statusBarWidget": {
            "displayName": "Fixture",
            "section": "right",
            "defaults": { "format24h": true },
            "schema": [
                { "key": "format24h", "type": "boolean", "label": "24-hour",
                  "defaultValue": true }
            ]
        }
    }"#;

    fn fixture_plugin() -> DiscoveredPlugin {
        let manifest: par_term_scripting::manifest::PluginManifest =
            serde_json::from_str(FIXTURE_MANIFEST).expect("fixture manifest parses");
        DiscoveredPlugin {
            manifest,
            dir: "/plugins/com.example.fixture".into(),
            entry_path: "/plugins/com.example.fixture/fixture.sh".into(),
            action_entry_path: None,
            panel_entry_path: None,
        }
    }

    const ACTION_ONLY_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.actions",
        "name": "Actions Fixture",
        "version": "0.1.0",
        "author": "test",
        "license": "MIT",
        "kinds": ["action-contributor"],
        "activation": "manual",
        "entryPoints": { "actionContributor": { "command": "actions.sh", "args": [] } },
        "actions": [
            { "id": "greet", "label": "Greet" },
            { "id": "stamp", "label": "Stamp" }
        ]
    }"#;

    fn action_only_plugin() -> DiscoveredPlugin {
        let manifest: par_term_scripting::manifest::PluginManifest =
            serde_json::from_str(ACTION_ONLY_MANIFEST).expect("fixture manifest parses");
        DiscoveredPlugin {
            manifest,
            dir: "/plugins/com.example.actions".into(),
            entry_path: "/plugins/com.example.actions/actions.sh".into(),
            action_entry_path: Some("/plugins/com.example.actions/actions.sh".into()),
            panel_entry_path: None,
        }
    }

    const BOTH_KINDS_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.both",
        "name": "Both Fixture",
        "version": "0.1.0",
        "author": "test",
        "license": "MIT",
        "kinds": ["status-bar-widget", "action-contributor"],
        "activation": "manual",
        "entryPoints": {
            "statusBarWidget": { "command": "widget.sh", "args": [] },
            "actionContributor": { "command": "actions.sh", "args": ["--demo"] }
        },
        "statusBarWidget": {
            "displayName": "Both",
            "section": "right",
            "defaults": { "format24h": true },
            "schema": [
                { "key": "format24h", "type": "boolean", "label": "24-hour",
                  "defaultValue": true }
            ]
        },
        "actions": [ { "id": "greet", "label": "Greet" } ]
    }"#;

    fn both_kinds_plugin() -> DiscoveredPlugin {
        let manifest: par_term_scripting::manifest::PluginManifest =
            serde_json::from_str(BOTH_KINDS_MANIFEST).expect("fixture manifest parses");
        DiscoveredPlugin {
            manifest,
            dir: "/plugins/com.example.both".into(),
            entry_path: "/plugins/com.example.both/widget.sh".into(),
            action_entry_path: Some("/plugins/com.example.both/actions.sh".into()),
            panel_entry_path: None,
        }
    }

    const STRAY_WIDGET_ENTRY_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.stray",
        "name": "Stray Widget Entry Fixture",
        "version": "0.1.0",
        "author": "test",
        "license": "MIT",
        "kinds": ["action-contributor"],
        "activation": "manual",
        "entryPoints": {
            "statusBarWidget": { "command": "stray.sh", "args": [] },
            "actionContributor": { "command": "actions.sh", "args": [] }
        },
        "actions": [ { "id": "greet", "label": "Greet" } ]
    }"#;

    fn stray_widget_entry_plugin() -> DiscoveredPlugin {
        let manifest: par_term_scripting::manifest::PluginManifest =
            serde_json::from_str(STRAY_WIDGET_ENTRY_MANIFEST).expect("fixture manifest parses");
        DiscoveredPlugin {
            manifest,
            dir: "/plugins/com.example.stray".into(),
            entry_path: "/plugins/com.example.stray/actions.sh".into(),
            action_entry_path: Some("/plugins/com.example.stray/actions.sh".into()),
            panel_entry_path: None,
        }
    }

    const PANEL_ONLY_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.panel",
        "name": "Panel Fixture",
        "version": "0.1.0",
        "kinds": ["panel"],
        "entryPoints": { "panel": { "command": "notes.py", "args": [] } }
    }"#;

    fn panel_only_plugin() -> DiscoveredPlugin {
        let manifest: par_term_scripting::manifest::PluginManifest =
            serde_json::from_str(PANEL_ONLY_MANIFEST).expect("fixture manifest parses");
        DiscoveredPlugin {
            manifest,
            dir: "/plugins/com.example.panel".into(),
            entry_path: "/plugins/com.example.panel/notes.py".into(),
            action_entry_path: None,
            panel_entry_path: Some("/plugins/com.example.panel/notes.py".into()),
        }
    }

    #[test]
    fn runs_lines_panel_only_keeps_bare_runs_line() {
        assert_eq!(
            runs_lines(&panel_only_plugin().manifest),
            ["Runs: ./notes.py".to_string()]
        );
    }

    #[test]
    fn enable_seeds_settings_and_adds_widget_row() {
        let mut config = par_term_config::Config::default();
        set_plugin_enabled(&mut config, &fixture_plugin(), true);
        let state = &config.automation.plugins[0];
        assert_eq!(state.id, "com.example.fixture");
        assert!(state.enabled);
        assert_eq!(
            state.settings.get("format24h"),
            Some(&serde_json::json!(true)),
            "first enable seeds settings from manifest defaults"
        );
        let row = config
            .status_bar
            .status_bar_widgets
            .iter()
            .find(|w| w.id == WidgetId::Plugin("com.example.fixture".into()))
            .expect("enable adds a widget row");
        assert_eq!(row.section, StatusBarSection::Right);
        assert!(row.enabled);
    }

    #[test]
    fn disable_flips_state_and_keeps_widget_row() {
        let mut config = par_term_config::Config::default();
        let plugin = fixture_plugin();
        set_plugin_enabled(&mut config, &plugin, true);
        set_plugin_enabled(&mut config, &plugin, false);
        assert_eq!(config.automation.plugins.len(), 1, "state entry kept");
        assert!(!config.automation.plugins[0].enabled);
        assert!(
            config
                .status_bar
                .status_bar_widgets
                .iter()
                .any(|w| w.id == WidgetId::Plugin("com.example.fixture".into())),
            "widget row kept on disable (it self-hides)"
        );
    }

    #[test]
    fn enable_honors_state_section_over_manifest_default() {
        let mut config = par_term_config::Config::default();
        config.automation.plugins.push(PluginStateConfig {
            id: "com.example.fixture".into(),
            enabled: false,
            settings: Default::default(),
            section: Some(StatusBarSection::Left),
        });
        set_plugin_enabled(&mut config, &fixture_plugin(), true);
        let row = config
            .status_bar
            .status_bar_widgets
            .iter()
            .find(|w| w.id == WidgetId::Plugin("com.example.fixture".into()))
            .unwrap();
        assert_eq!(row.section, StatusBarSection::Left);
    }

    #[test]
    fn write_setting_upserts_entry_preserving_enabled() {
        let mut config = par_term_config::Config::default();
        // No entry yet: configuring creates a disabled one.
        write_plugin_setting(
            &mut config,
            "com.example.fixture",
            "format24h",
            false.into(),
        );
        let state = &config.automation.plugins[0];
        assert!(!state.enabled, "configuring is not consent to run");
        assert_eq!(
            state.settings.get("format24h"),
            Some(&serde_json::json!(false))
        );
        // Existing entry: value replaced, enabled preserved.
        config.automation.plugins[0].enabled = true;
        write_plugin_setting(&mut config, "com.example.fixture", "format24h", true.into());
        let state = &config.automation.plugins[0];
        assert!(state.enabled);
        assert_eq!(
            state.settings.get("format24h"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn placement_updates_row_and_state() {
        let mut config = par_term_config::Config::default();
        let plugin = fixture_plugin();
        set_plugin_enabled(&mut config, &plugin, true);
        set_plugin_placement(&mut config, "com.example.fixture", StatusBarSection::Center);
        let row = config
            .status_bar
            .status_bar_widgets
            .iter()
            .find(|w| w.id == WidgetId::Plugin("com.example.fixture".into()))
            .unwrap();
        assert_eq!(row.section, StatusBarSection::Center);
        assert_eq!(
            config.automation.plugins[0].section,
            Some(StatusBarSection::Center)
        );
    }

    #[test]
    fn missing_state_ids_lists_only_undiscovered() {
        let mut config = par_term_config::Config::default();
        config.automation.plugins.push(PluginStateConfig {
            id: "com.example.fixture".into(),
            enabled: false,
            settings: Default::default(),
            section: None,
        });
        config.automation.plugins.push(PluginStateConfig {
            id: "com.example.gone".into(),
            enabled: false,
            settings: Default::default(),
            section: None,
        });
        let discovered = vec![fixture_plugin()];
        assert_eq!(
            missing_state_ids(&config, &discovered),
            vec!["com.example.gone".to_string()]
        );
    }

    fn action(id: &str, label: &str) -> par_term_scripting::manifest::ActionContribution {
        par_term_scripting::manifest::ActionContribution {
            id: id.into(),
            label: label.into(),
            description: None,
        }
    }

    #[test]
    fn action_summary_plural_lists_labels() {
        let actions = [action("greet", "Greet"), action("stamp", "Stamp")];
        assert_eq!(
            action_summary(&actions),
            "contributes 2 actions: Greet, Stamp"
        );
    }

    #[test]
    fn action_summary_singular_uses_singular_noun() {
        let actions = [action("greet", "Greet")];
        assert_eq!(action_summary(&actions), "contributes 1 action: Greet");
    }

    #[test]
    fn action_summary_empty_renders_nothing() {
        assert_eq!(action_summary(&[]), "");
    }

    #[test]
    fn runs_lines_widget_only_keeps_bare_runs_line() {
        assert_eq!(
            runs_lines(&fixture_plugin().manifest),
            ["Runs: ./fixture.sh".to_string()]
        );
    }

    #[test]
    fn runs_lines_action_only_shows_action_entry_command() {
        assert_eq!(
            runs_lines(&action_only_plugin().manifest),
            ["Runs: ./actions.sh".to_string()]
        );
    }

    #[test]
    fn runs_lines_both_kinds_labels_each_entry() {
        assert_eq!(
            runs_lines(&both_kinds_plugin().manifest),
            [
                "Runs widget: ./widget.sh".to_string(),
                "Runs actions: ./actions.sh --demo".to_string(),
            ]
        );
    }

    #[test]
    fn runs_lines_ignores_stray_widget_entry_of_action_plugin() {
        // Forward-compat posture: an undeclared kind's entry point may sit in
        // the map; only declared kinds earn a Runs line at the consent toggle.
        assert_eq!(
            runs_lines(&stray_widget_entry_plugin().manifest),
            ["Runs: ./actions.sh".to_string()]
        );
    }

    #[test]
    fn enable_action_only_plugin_adds_no_widget_row() {
        let mut config = par_term_config::Config::default();
        set_plugin_enabled(&mut config, &action_only_plugin(), true);
        let state = &config.automation.plugins[0];
        assert_eq!(state.id, "com.example.actions");
        assert!(state.enabled, "state entry still created — actions run");
        assert!(
            !config
                .status_bar
                .status_bar_widgets
                .iter()
                .any(|w| w.id == WidgetId::Plugin("com.example.actions".into())),
            "an action-only plugin must not gain a phantom widget row"
        );
    }

    #[test]
    fn enable_both_kinds_plugin_keeps_widget_row() {
        let mut config = par_term_config::Config::default();
        set_plugin_enabled(&mut config, &both_kinds_plugin(), true);
        assert!(
            config
                .status_bar
                .status_bar_widgets
                .iter()
                .any(|w| w.id == WidgetId::Plugin("com.example.both".into())),
            "a both-kinds plugin keeps its widget row"
        );
    }

    #[test]
    fn automation_tab_matches_plugin_search() {
        // The criterion-3 path: settings search must find the section via
        // the tab keywords, through the sidebar's real matching function.
        for query in [
            "plugin",
            "plugins",
            "manifest",
            "widget",
            "extensions",
            "plugin action",
            "palette actions",
        ] {
            assert!(
                crate::sidebar::tab_matches_search(crate::sidebar::SettingsTab::Automation, query),
                "search '{query}' should match the Automation tab"
            );
        }
    }
}
