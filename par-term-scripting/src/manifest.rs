//! Plugin manifest types, discovery, and validation.
//!
//! A plugin is a directory under the plugins root whose name equals its manifest
//! `id`, containing a `manifest.json` and the executable its entry points name.
//! Discovery is deliberately forgiving: one broken manifest produces a warning
//! and is skipped, never blocking the discovery of its neighbours. A plugin is
//! never spawned as a side effect of discovery — the host spawns only what the
//! enabled set names, which is what makes "lands disabled" structural.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use par_term_config::{RestartPolicy, StatusBarSection};
use serde::{Deserialize, Serialize};

/// The status-bar-widget plugin kind. Unknown kinds in a manifest skip the
/// whole plugin with a visible warning rather than loading partially.
pub const KIND_STATUS_BAR_WIDGET: &str = "status-bar-widget";

/// The action-contributor plugin kind: the plugin contributes entries to
/// the command palette.
pub const KIND_ACTION_CONTRIBUTOR: &str = "action-contributor";

/// The panel plugin kind: the plugin pushes markdown panel content through
/// the `SetPanel`/`ClearPanel` protocol commands (the same surface a tab
/// script's `SetPanel` drives), rendered in the Settings plugins section.
pub const KIND_PANEL: &str = "panel";

/// The overlay plugin kind: the plugin owns a persistent surface drawn over
/// the terminal through the `SetOverlay`/`ClearOverlay` protocol commands
/// (design: docs/plans/2026-09-24-overlay-plugin-design.md). Phase 1 is
/// display-only; `interactive` requires the `overlay.interactive` capability
/// and is forced off without it.
pub const KIND_OVERLAY: &str = "overlay";

/// Entry-point map key for the [`KIND_STATUS_BAR_WIDGET`] kind.
pub const ENTRY_POINT_STATUS_BAR_WIDGET: &str = "statusBarWidget";

/// Entry-point map key for the [`KIND_ACTION_CONTRIBUTOR`] kind.
pub const ENTRY_POINT_ACTION_CONTRIBUTOR: &str = "actionContributor";

/// Entry-point map key for the [`KIND_PANEL`] kind.
pub const ENTRY_POINT_PANEL: &str = "panel";

/// Entry-point map key for the [`KIND_OVERLAY`] kind.
pub const ENTRY_POINT_OVERLAY: &str = "overlay";

/// Activation policy declared by the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginActivation {
    /// Discovered but never auto-started; the user enables it in Settings.
    #[default]
    Manual,
    /// Starts with the host once enabled (still gated by the enabled set).
    OnStartup,
}

/// A setting's value type, as declared in a manifest's settings schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingType {
    /// Free-form text.
    String,
    /// Whole number; JSON floats with a fraction are rejected.
    Integer,
    /// Any JSON number.
    Number,
    /// True/false.
    Boolean,
}

impl SettingType {
    /// Human name used in validation error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            SettingType::String => "string",
            SettingType::Integer => "integer",
            SettingType::Number => "number",
            SettingType::Boolean => "boolean",
        }
    }
}

/// One typed setting declared by a plugin's settings schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingSchemaEntry {
    /// Setting key; must be non-empty and unique within the schema.
    pub key: String,
    /// Value type (serialized as `type`).
    #[serde(rename = "type")]
    pub setting_type: SettingType,
    /// Settings-editor label.
    pub label: String,
    /// Inclusive lower bound for numeric settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Inclusive upper bound for numeric settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Drag step for numeric settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// Value used when the persisted settings omit the key.
    pub default_value: serde_json::Value,
}

/// The `statusBarWidget` manifest block: how a status-bar-widget plugin
/// presents itself and what the user can configure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBarWidgetKind {
    /// Name shown in the status-bar Settings list.
    pub display_name: String,
    /// Section the widget entry lands in when first enabled; the user can
    /// move it afterwards. Defaults to the right section.
    #[serde(default = "default_widget_section")]
    pub section: StatusBarSection,
    /// Settings values used when the user has configured nothing.
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,
    /// Typed settings the host renders an editor for.
    pub schema: Vec<SettingSchemaEntry>,
}

fn default_widget_section() -> StatusBarSection {
    StatusBarSection::Right
}

/// One action a plugin contributes to the command palette when it declares
/// the [`KIND_ACTION_CONTRIBUTOR`] kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionContribution {
    /// Action id; must match `[a-zA-Z0-9_-]+` and be unique within the
    /// manifest.
    pub id: String,
    /// Text shown in the palette entry.
    pub label: String,
    /// Longer explanation shown alongside the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One named executable entry point. `command` is relative to the plugin
/// directory only — never a PATH lookup, never allowed to escape the
/// directory (enforced by [`discover_plugins`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntryPoint {
    /// Executable file name relative to the plugin directory.
    pub command: String,
    /// Extra argv passed before the host's own settings argument.
    #[serde(default)]
    pub args: Vec<String>,
}

/// A parsed `manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Manifest format version; only 1 is understood.
    pub schema_version: u32,
    /// Reverse-DNS-ish plugin id; must equal the directory name.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Plugin version, displayed at the enable toggle.
    pub version: String,
    /// Author, displayed at the enable toggle (trust surface).
    #[serde(default)]
    pub author: Option<String>,
    /// License, displayed at the enable toggle (trust surface).
    #[serde(default)]
    pub license: Option<String>,
    /// What the plugin does, shown in Settings.
    #[serde(default)]
    pub description: Option<String>,
    /// Kinds this plugin provides; each must be known to this host.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// When the plugin starts once enabled.
    #[serde(default)]
    pub activation: PluginActivation,
    /// Named entry points; the key selects the kind's executable.
    #[serde(default)]
    pub entry_points: HashMap<String, PluginEntryPoint>,
    /// Required iff `kinds` contains [`KIND_STATUS_BAR_WIDGET`].
    #[serde(default)]
    pub status_bar_widget: Option<StatusBarWidgetKind>,
    /// Actions contributed to the command palette; required non-empty iff
    /// `kinds` contains [`KIND_ACTION_CONTRIBUTOR`], ignored otherwise.
    #[serde(default)]
    pub actions: Vec<ActionContribution>,
    /// Terminal event kinds the plugin wants delivered to its stdin. Empty or
    /// absent means self-scheduled: the plugin receives no terminal events
    /// (unlike tab scripts, where an empty subscription list means all
    /// events). Each name must be one of
    /// [`crate::observer::EVENT_KINDS`]; an unknown or duplicate name skips
    /// the plugin at discovery so a typo'd subscription can never silently
    /// never-fire.
    #[serde(default)]
    pub subscriptions: Vec<String>,
    /// When the host restarts the plugin's exited process: `on_failure`
    /// (the default and the only behaviour before the field existed),
    /// `never`, or `always` — the same policy enum tab scripts configure.
    /// Mode-only by design: backoff parameters (restart delay, crash-loop
    /// cap) are host-owned safety policy a manifest cannot tune its way
    /// out of (parsight decision 95).
    #[serde(default = "default_plugin_restart")]
    pub restart: RestartPolicy,
}

/// The manifest default for [`PluginManifest::restart`] — today's
/// hardcoded behaviour, NOT the config enum's derived default (`Never` is
/// the scripts-config default; a plugin without the field must keep
/// restarting on failure exactly as it always has).
fn default_plugin_restart() -> RestartPolicy {
    RestartPolicy::OnFailure
}

/// A plugin that passed validation, with confinement-checked paths.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredPlugin {
    /// The parsed manifest.
    pub manifest: PluginManifest,
    /// Canonicalized plugin directory.
    pub dir: PathBuf,
    /// Canonicalized, confinement-checked entry executable for the widget
    /// kind. A plugin declaring only the action-contributor kind has no
    /// widget entry and carries its action entry here.
    pub entry_path: PathBuf,
    /// Canonicalized, confinement-checked entry executable for the
    /// action-contributor kind; `None` when the plugin does not declare
    /// that kind.
    pub action_entry_path: Option<PathBuf>,
    /// Canonicalized, confinement-checked entry executable for the panel
    /// kind; `None` when the plugin does not declare that kind.
    pub panel_entry_path: Option<PathBuf>,
    /// Canonicalized, confinement-checked entry executable for the overlay
    /// kind; `None` when the plugin does not declare that kind.
    pub overlay_entry_path: Option<PathBuf>,
}

/// Why a candidate plugin directory was skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryWarning {
    /// Directory name under the plugins root.
    pub dir: String,
    /// Human-readable skip reason.
    pub reason: String,
}

/// Scan a plugins root for valid plugin directories.
///
/// An absent root is an empty result, not a warning — a machine with no
/// plugins installed yet has not done anything wrong. Directories are visited
/// in name order so discovery output is deterministic.
pub fn discover_plugins(root: &Path) -> (Vec<DiscoveredPlugin>, Vec<DiscoveryWarning>) {
    let mut plugins = Vec::new();
    let mut warnings = Vec::new();

    let Ok(entries) = std::fs::read_dir(root) else {
        return (plugins, warnings);
    };

    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();

    for dir in dirs {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match validate_plugin_dir(&dir) {
            Ok(plugin) => plugins.push(plugin),
            Err(reason) => warnings.push(DiscoveryWarning { dir: name, reason }),
        }
    }

    (plugins, warnings)
}

/// Parse and fully validate one candidate plugin directory.
pub(crate) fn validate_plugin_dir(dir: &Path) -> Result<DiscoveredPlugin, String> {
    let manifest_path = dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("missing or unreadable manifest.json: {e}"))?;
    let manifest: PluginManifest =
        serde_json::from_str(&raw).map_err(|e| format!("invalid manifest JSON: {e}"))?;

    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported schemaVersion {} (expected 1)",
            manifest.schema_version
        ));
    }

    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if manifest.id.is_empty() || manifest.id != dir_name {
        return Err(format!(
            "manifest id `{}` does not match directory name `{dir_name}`",
            manifest.id
        ));
    }

    if manifest.kinds.is_empty() {
        return Err("no kinds declared".to_string());
    }
    let known_kinds = [
        KIND_STATUS_BAR_WIDGET,
        KIND_ACTION_CONTRIBUTOR,
        KIND_PANEL,
        KIND_OVERLAY,
    ];
    let unknown: Vec<&str> = manifest
        .kinds
        .iter()
        .map(String::as_str)
        .filter(|k| !known_kinds.contains(k))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown kinds: {} (known: {})",
            unknown.join(", "),
            known_kinds.join(", ")
        ));
    }
    let has_widget = manifest.kinds.iter().any(|k| k == KIND_STATUS_BAR_WIDGET);
    let has_action = manifest.kinds.iter().any(|k| k == KIND_ACTION_CONTRIBUTOR);
    let has_panel = manifest.kinds.iter().any(|k| k == KIND_PANEL);
    let has_overlay = manifest.kinds.iter().any(|k| k == KIND_OVERLAY);

    // A subscription naming a kind the forwarder can never produce would sit
    // inert for the plugin's whole life, so it is rejected at discovery like
    // every other manifest fault. Duplicates are rejected for the same
    // reason as duplicate action ids: an authoring error, not a filter.
    let mut seen_subscriptions: HashSet<&str> = HashSet::new();
    for kind in &manifest.subscriptions {
        if !super::observer::EVENT_KINDS.contains(&kind.as_str()) {
            return Err(format!(
                "unknown subscription kind `{kind}` (valid kinds are the script event \
                 vocabulary, e.g. bell_rang, cwd_changed, command_complete)"
            ));
        }
        if !seen_subscriptions.insert(kind.as_str()) {
            return Err(format!("duplicate subscription kind `{kind}`"));
        }
    }

    // Each declared kind's requirements run only when that kind is present:
    // a both-kinds manifest must satisfy both pairs, an action-only manifest
    // owes no widget block, and vice versa.
    if has_widget {
        let widget = manifest.status_bar_widget.as_ref().ok_or_else(|| {
            format!("{KIND_STATUS_BAR_WIDGET} kind requires a statusBarWidget block")
        })?;
        if widget.schema.is_empty() {
            return Err("statusBarWidget.schema must declare at least one setting".to_string());
        }
        if widget.schema.iter().any(|e| e.key.is_empty()) {
            return Err("statusBarWidget.schema entries must have non-empty keys".to_string());
        }
    }

    if has_action {
        if manifest.actions.is_empty() {
            return Err(format!(
                "{KIND_ACTION_CONTRIBUTOR} kind requires at least one action in `actions`"
            ));
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for action in &manifest.actions {
            // A colon would collide with the wire format's id separator
            // (design D1); ids stay `[a-zA-Z0-9_-]+`.
            if action.id.is_empty()
                || !action
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(format!(
                    "action id `{}` must match [a-zA-Z0-9_-]+",
                    action.id
                ));
            }
            if !seen.insert(action.id.as_str()) {
                return Err(format!("duplicate action id `{}`", action.id));
            }
            if action.label.is_empty() {
                return Err(format!(
                    "action `{}` must have a non-empty label",
                    action.id
                ));
            }
        }
    }

    // The security line: each declared kind's entry command resolves inside
    // the plugin directory only — canonicalize both sides and require
    // containment, so a `..` or symlink escape cannot point execution
    // outside the plugin.
    let dir_canon =
        std::fs::canonicalize(dir).map_err(|e| format!("cannot read plugin dir: {e}"))?;
    let widget_entry = if has_widget {
        let entry = manifest.entry_points.get(ENTRY_POINT_STATUS_BAR_WIDGET).ok_or_else(|| {
            format!(
                "{KIND_STATUS_BAR_WIDGET} kind requires entryPoints.{ENTRY_POINT_STATUS_BAR_WIDGET}"
            )
        })?;
        Some(confinement_check(&dir_canon, &entry.command)?)
    } else {
        None
    };
    let action_entry = if has_action {
        let entry = manifest.entry_points.get(ENTRY_POINT_ACTION_CONTRIBUTOR).ok_or_else(|| {
            format!(
                "{KIND_ACTION_CONTRIBUTOR} kind requires entryPoints.{ENTRY_POINT_ACTION_CONTRIBUTOR}"
            )
        })?;
        Some(confinement_check(&dir_canon, &entry.command)?)
    } else {
        None
    };
    // The panel kind owes no manifest block beyond its entry point — the
    // pushed content is entirely the process's decision at runtime.
    let panel_entry = if has_panel {
        let entry = manifest
            .entry_points
            .get(ENTRY_POINT_PANEL)
            .ok_or_else(|| format!("{KIND_PANEL} kind requires entryPoints.{ENTRY_POINT_PANEL}"))?;
        Some(confinement_check(&dir_canon, &entry.command)?)
    } else {
        None
    };
    // The overlay kind owes no manifest block beyond its entry point — the
    // pushed scene is entirely the process's decision at runtime (same
    // shape as the panel kind).
    let overlay_entry = if has_overlay {
        let entry = manifest
            .entry_points
            .get(ENTRY_POINT_OVERLAY)
            .ok_or_else(|| {
                format!("{KIND_OVERLAY} kind requires entryPoints.{ENTRY_POINT_OVERLAY}")
            })?;
        Some(confinement_check(&dir_canon, &entry.command)?)
    } else {
        None
    };

    // `kinds` is non-empty and every kind is known, so at least one entry
    // resolved; the widget entry is the primary entry when several kinds
    // are declared.
    let entry_path = widget_entry
        .or(action_entry.clone())
        .or(panel_entry.clone())
        .or(overlay_entry.clone())
        .expect("a validated manifest resolves at least one entry point");

    Ok(DiscoveredPlugin {
        manifest,
        dir: dir_canon,
        entry_path,
        action_entry_path: action_entry,
        panel_entry_path: panel_entry,
        overlay_entry_path: overlay_entry,
    })
}

/// Canonicalize one entry command and enforce the confinement rules every
/// kind's entry shares: the resolved path must stay inside the plugin
/// directory (no `..` or symlink escape) and, on Unix, carry the exec bit
/// unless it is a `.py` entry.
///
/// `.py` entries run through the resolved Python interpreter (the same
/// `spawn_command` routing), so the file itself never needs the exec bit —
/// requiring it would silently reject every non-chmodded script plugin on
/// Unix while Windows (no exec bit) accepts it.
fn confinement_check(dir_canon: &Path, command: &str) -> Result<PathBuf, String> {
    let entry_path = dir_canon.join(command);
    let entry_canon = std::fs::canonicalize(&entry_path)
        .map_err(|_| format!("entry point `{command}` not found"))?;
    if !entry_canon.starts_with(dir_canon) {
        return Err(format!(
            "entry point `{command}` escapes the plugin directory"
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !command.ends_with(".py") {
            let mode = std::fs::metadata(&entry_canon)
                .map_err(|e| format!("entry point unreadable: {e}"))?
                .permissions()
                .mode();
            if mode & 0o111 == 0 {
                return Err(format!("entry point `{command}` is not executable"));
            }
        }
    }
    Ok(entry_canon)
}

/// Validate persisted settings against a manifest schema.
///
/// Absent keys are filled from the schema defaults; keys the schema no longer
/// declares are dropped (a plugin update must not strand a user's config);
/// values of the wrong type or outside `min`/`max` are errors the caller
/// surfaces instead of spawning with garbage.
pub fn validate_settings(
    schema: &[SettingSchemaEntry],
    values: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, String> {
    let mut out: HashMap<String, serde_json::Value> = schema
        .iter()
        .map(|e| (e.key.clone(), e.default_value.clone()))
        .collect();

    for (key, value) in values {
        let Some(entry) = schema.iter().find(|e| &e.key == key) else {
            continue;
        };
        if !value_matches_type(value, entry.setting_type) {
            return Err(format!(
                "setting `{key}` expects {}, got {value}",
                entry.setting_type.as_str()
            ));
        }
        if matches!(
            entry.setting_type,
            SettingType::Integer | SettingType::Number
        ) {
            if let Some(n) = value.as_f64()
                && let Some(min) = entry.min
                && n < min
            {
                return Err(format!(
                    "setting `{key}` value {n} is below the minimum {min}"
                ));
            }
            if let Some(n) = value.as_f64()
                && let Some(max) = entry.max
                && n > max
            {
                return Err(format!(
                    "setting `{key}` value {n} is above the maximum {max}"
                ));
            }
        }
        out.insert(key.clone(), value.clone());
    }

    Ok(out)
}

/// Whether a JSON value fits a declared setting type.
fn value_matches_type(value: &serde_json::Value, setting_type: SettingType) -> bool {
    match setting_type {
        SettingType::String => value.is_string(),
        SettingType::Integer => value.is_u64() || value.is_i64(),
        SettingType::Number => value.is_number(),
        SettingType::Boolean => value.is_boolean(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// The design doc's D1 example manifest (§3), with real strings for the
    /// ellipsis fields, as written to a fixture plugin directory.
    const DESIGN_D1_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "com.example.agents-usage",
        "name": "Agent Usage",
        "version": "0.1.0",
        "author": "A Plugin Author",
        "license": "MIT",
        "description": "Shows agent usage in the status bar",
        "kinds": ["status-bar-widget"],
        "activation": "manual",
        "entryPoints": { "statusBarWidget": { "command": "agent-widget", "args": [] } },
        "statusBarWidget": {
            "displayName": "Agents",
            "section": "right",
            "defaults": { "refreshIntervalSec": 900 },
            "schema": [
                { "key": "refreshIntervalSec", "type": "integer", "label": "Refresh interval (seconds)",
                  "min": 30, "max": 3600, "step": 30, "defaultValue": 900 }
            ]
        }
    }"#;

    /// Minimal valid manifest, used as the base most mutations start from.
    /// `{id}` is replaced with the fixture directory name by [`write_plugin`].
    const MINIMAL_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Test",
        "version": "0.1.0",
        "kinds": ["status-bar-widget"],
        "entryPoints": { "statusBarWidget": { "command": "widget.py", "args": [] } },
        "statusBarWidget": {
            "displayName": "Test",
            "section": "right",
            "defaults": {},
            "schema": [
                { "key": "format24h", "type": "boolean", "label": "24-hour clock",
                  "defaultValue": true }
            ]
        }
    }"#;

    /// Minimal valid action-contributor manifest, the action-kind counterpart
    /// of [`MINIMAL_MANIFEST`]. `{id}` is replaced by [`write_plugin`].
    const MINIMAL_ACTION_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Test Actions",
        "version": "0.1.0",
        "kinds": ["action-contributor"],
        "entryPoints": { "actionContributor": { "command": "actions.py", "args": [] } },
        "actions": [ { "id": "say-hello", "label": "Say hello" } ]
    }"#;

    /// Minimal valid panel manifest: the kind owes no block beyond its entry
    /// point. `{id}` is replaced by [`write_plugin`].
    const MINIMAL_PANEL_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Test Panel",
        "version": "0.1.0",
        "kinds": ["panel"],
        "entryPoints": { "panel": { "command": "panel.py", "args": [] } }
    }"#;

    /// Both-kinds manifest: one plugin contributing a widget and an action.
    /// `{id}` is replaced by [`write_plugin`].
    const BOTH_KINDS_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Test Both",
        "version": "0.1.0",
        "kinds": ["status-bar-widget", "action-contributor"],
        "entryPoints": {
            "statusBarWidget": { "command": "widget.py", "args": [] },
            "actionContributor": { "command": "actions.py", "args": [] }
        },
        "actions": [ { "id": "ping", "label": "Ping" } ],
        "statusBarWidget": {
            "displayName": "Test",
            "section": "right",
            "defaults": {},
            "schema": [
                { "key": "format24h", "type": "boolean", "label": "24-hour clock",
                  "defaultValue": true }
            ]
        }
    }"#;

    /// Write a plugin directory: manifest plus an executable entry file.
    /// `manifest` may use `{id}` placeholders for the directory name so tests
    /// can keep manifest id and dir name in sync.
    fn write_plugin(root: &Path, dir_name: &str, manifest: &str, entry: Option<&str>) {
        let dir = root.join(dir_name);
        fs::create_dir_all(&dir).expect("create plugin dir");
        let manifest = manifest.replace("{id}", dir_name);
        fs::write(dir.join("manifest.json"), manifest).expect("write manifest");
        if let Some(entry_name) = entry {
            write_exec_file(&dir.join(entry_name));
        }
    }

    /// Write one executable entry file, with the exec bit on Unix.
    fn write_exec_file(path: &Path) {
        fs::write(path, "#!/bin/sh\nsleep 30\n").expect("write entry");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).expect("entry metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod entry");
        }
    }

    #[test]
    fn design_d1_example_parses_with_every_field_populated() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.agents-usage",
            DESIGN_D1_MANIFEST,
            Some("agent-widget"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let p = &plugins[0];
        assert_eq!(p.manifest.id, "com.example.agents-usage");
        assert_eq!(p.manifest.author.as_deref(), Some("A Plugin Author"));
        assert_eq!(p.manifest.license.as_deref(), Some("MIT"));
        assert_eq!(p.manifest.activation, PluginActivation::Manual);
        assert_eq!(p.manifest.kinds, vec![KIND_STATUS_BAR_WIDGET.to_string()]);
        let widget = p.manifest.status_bar_widget.as_ref().unwrap();
        assert_eq!(widget.display_name, "Agents");
        assert_eq!(widget.section, StatusBarSection::Right);
        assert_eq!(
            widget
                .defaults
                .get("refreshIntervalSec")
                .and_then(serde_json::Value::as_u64),
            Some(900)
        );
        assert_eq!(widget.schema.len(), 1);
        assert_eq!(widget.schema[0].setting_type, SettingType::Integer);
        assert_eq!(widget.schema[0].min, Some(30.0));
        assert_eq!(widget.schema[0].max, Some(3600.0));
        assert_eq!(widget.schema[0].step, Some(30.0));
        assert!(p.entry_path.ends_with("agent-widget"));
    }

    #[cfg(unix)]
    #[test]
    fn py_entry_without_exec_bit_is_discovered() {
        // `.py` entries run via the resolved Python interpreter, so
        // validation must not require the exec bit. Regression: a plugin
        // copied in without the bit (fresh `cp -r` of a script plugin) was
        // silently undiscoverable on Unix while Windows accepted it.
        let tmp = TempDir::new().unwrap();
        write_plugin(tmp.path(), "com.example.py-noexec", MINIMAL_MANIFEST, None);
        let entry = tmp.path().join("com.example.py-noexec/widget.py");
        fs::write(&entry, "#!/usr/bin/env python3\n").expect("write entry");
        // Deliberately no chmod: the interpreter routing makes it runnable.
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].entry_path.ends_with("widget.py"));
    }

    #[test]
    fn unknown_extra_keys_parse_for_forward_compatibility() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replacen(
            "\"version\": \"0.1.0\",",
            "\"version\": \"0.1.0\", \"futureField\": {\"nested\": true},",
            1,
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn id_mismatching_directory_name_is_skipped_with_a_warning() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.test",
            DESIGN_D1_MANIFEST,
            Some("agent-widget"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].reason.contains("does not match directory name"));
    }

    #[test]
    fn unknown_kind_skips_the_plugin_with_a_warning() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replace("status-bar-widget", "some-future-kind");
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .reason
                .contains("unknown kinds: some-future-kind")
        );
    }

    #[test]
    fn escaping_entry_command_is_rejected() {
        let tmp = TempDir::new().unwrap();
        // A real executable OUTSIDE the plugin dir the escape targets.
        let outside = tmp.path().join("outside.sh");
        fs::write(&outside, "#!/bin/sh\nsleep 30\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&outside).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&outside, perms).unwrap();
        }
        let manifest = MINIMAL_MANIFEST.replace("\"widget.py\"", "\"../outside.sh\"");
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty(), "`..` escape must not be discovered");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].reason.contains("escapes the plugin directory"));
    }

    #[test]
    fn missing_entry_file_is_skipped_with_a_warning() {
        let tmp = TempDir::new().unwrap();
        write_plugin(tmp.path(), "com.example.test", MINIMAL_MANIFEST, None);
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].reason.contains("not found"));
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_entry_is_skipped_with_a_warning() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        // Non-`.py` entries exec directly, so they must carry the bit;
        // `.py` entries are interpreter-routed (see
        // `py_entry_without_exec_bit_is_discovered`).
        let manifest = MINIMAL_MANIFEST.replace("widget.py", "widget.sh");
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.sh"));
        let entry = tmp.path().join("com.example.test/widget.sh");
        let mut perms = fs::metadata(&entry).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&entry, perms).unwrap();
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].reason.contains("not executable"));
    }

    #[test]
    fn one_broken_plugin_never_blocks_its_neighbours() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.a",
            MINIMAL_MANIFEST,
            Some("widget.py"),
        );
        let dir = tmp.path().join("com.example.broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), "{ not json").unwrap();
        write_plugin(
            tmp.path(),
            "com.example.c",
            MINIMAL_MANIFEST,
            Some("widget.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert_eq!(plugins.len(), 2, "both valid neighbours must be discovered");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].dir, "com.example.broken");
        assert!(warnings[0].reason.contains("invalid manifest JSON"));
    }

    #[test]
    fn widget_kind_without_status_bar_widget_block_is_skipped() {
        let tmp = TempDir::new().unwrap();
        // Rename only the widget BLOCK key (the entryPoints key of the same
        // name sits inline on a different line); the block's contents become
        // an unknown field, so the manifest parses without a widget block.
        let manifest = MINIMAL_MANIFEST.replace(
            "\"statusBarWidget\": {\n            \"displayName\"",
            "\"notAWidgetBlock\": {\n            \"displayName\"",
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0]
                .reason
                .contains("requires a statusBarWidget block")
        );
    }

    #[test]
    fn widget_kind_without_entry_point_is_skipped() {
        let tmp = TempDir::new().unwrap();
        // Remove the whole entryPoints line; the widget block survives, so
        // the failure is specifically the missing entry point.
        let manifest = MINIMAL_MANIFEST.replace(
            "\"entryPoints\": { \"statusBarWidget\": { \"command\": \"widget.py\", \"args\": [] } },\n",
            "",
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0]
                .reason
                .contains("requires entryPoints.statusBarWidget")
        );
    }

    #[test]
    fn action_kind_manifest_discovers_with_action_entry_path() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.actions",
            MINIMAL_ACTION_MANIFEST,
            Some("actions.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        let p = &plugins[0];
        let action_entry = p.action_entry_path.as_ref().expect("action entry set");
        assert!(
            action_entry.starts_with(&p.dir),
            "must stay inside the plugin dir"
        );
        assert!(action_entry.ends_with("actions.py"));
        assert_eq!(p.manifest.actions.len(), 1);
        assert_eq!(p.manifest.actions[0].id, "say-hello");
        assert_eq!(p.manifest.actions[0].label, "Say hello");
        assert_eq!(p.manifest.actions[0].description, None);
        assert!(p.panel_entry_path.is_none());
    }

    #[test]
    fn panel_kind_manifest_discovers_with_panel_entry_path() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.panel",
            MINIMAL_PANEL_MANIFEST,
            Some("panel.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        let p = &plugins[0];
        let panel_entry = p.panel_entry_path.as_ref().expect("panel entry set");
        assert!(
            panel_entry.starts_with(&p.dir),
            "must stay inside the plugin dir"
        );
        assert!(panel_entry.ends_with("panel.py"));
        // A panel-only manifest has no widget block and no actions, and the
        // panel entry is the plugin's primary entry.
        assert!(p.manifest.status_bar_widget.is_none());
        assert!(p.manifest.actions.is_empty());
        assert!(p.entry_path.ends_with("panel.py"));
        assert!(p.action_entry_path.is_none());
    }

    #[test]
    fn panel_kind_without_entry_point_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_PANEL_MANIFEST.replace(
            "\"entryPoints\": { \"panel\": { \"command\": \"panel.py\", \"args\": [] } }",
            "\"entryPoints\": {}",
        );
        write_plugin(tmp.path(), "com.example.panel", &manifest, Some("panel.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0].reason.contains("requires entryPoints.panel"),
            "got: {}",
            warnings[0].reason
        );
    }

    #[test]
    fn both_kinds_manifest_discovers_with_both_entry_paths() {
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.both",
            BOTH_KINDS_MANIFEST,
            Some("widget.py"),
        );
        write_exec_file(&tmp.path().join("com.example.both/actions.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        let p = &plugins[0];
        assert!(
            p.entry_path.ends_with("widget.py"),
            "widget entry stays the primary entry"
        );
        let action_entry = p.action_entry_path.as_ref().expect("action entry set");
        assert!(
            action_entry.starts_with(&p.dir),
            "must stay inside the plugin dir"
        );
        assert!(action_entry.ends_with("actions.py"));
        assert_eq!(
            p.manifest.kinds,
            vec![
                KIND_STATUS_BAR_WIDGET.to_string(),
                KIND_ACTION_CONTRIBUTOR.to_string()
            ]
        );
    }

    #[test]
    fn action_kind_with_empty_actions_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_ACTION_MANIFEST
            .replace(r#"[ { "id": "say-hello", "label": "Say hello" } ]"#, "[]");
        write_plugin(
            tmp.path(),
            "com.example.actions",
            &manifest,
            Some("actions.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0]
                .reason
                .contains("action-contributor kind requires")
        );
    }

    #[test]
    fn action_id_with_a_colon_is_skipped() {
        let tmp = TempDir::new().unwrap();
        // A colon in an action id would collide with the wire format's id
        // separator (design D1), so the manifest is skipped outright.
        let manifest = MINIMAL_ACTION_MANIFEST.replace("\"say-hello\"", "\"say:hello\"");
        write_plugin(
            tmp.path(),
            "com.example.actions",
            &manifest,
            Some("actions.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(warnings[0].reason.contains("action id `say:hello`"));
    }

    #[test]
    fn duplicate_action_ids_are_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_ACTION_MANIFEST.replace(
            r#"{ "id": "say-hello", "label": "Say hello" }"#,
            r#"{ "id": "say-hello", "label": "First" }, { "id": "say-hello", "label": "Second" }"#,
        );
        write_plugin(
            tmp.path(),
            "com.example.actions",
            &manifest,
            Some("actions.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0]
                .reason
                .contains("duplicate action id `say-hello`")
        );
    }

    #[test]
    fn action_with_empty_label_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest =
            MINIMAL_ACTION_MANIFEST.replace("\"label\": \"Say hello\"", "\"label\": \"\"");
        write_plugin(
            tmp.path(),
            "com.example.actions",
            &manifest,
            Some("actions.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(warnings[0].reason.contains("non-empty label"));
    }

    #[test]
    fn actions_without_the_action_kind_are_valid_and_ignored() {
        let tmp = TempDir::new().unwrap();
        // Forward compat: a newer host's action plugin must still discover
        // here as a plain widget plugin; the inert `actions` block is not a
        // reason to skip.
        let manifest = MINIMAL_MANIFEST.replacen(
            "\"kinds\": [\"status-bar-widget\"],",
            concat!(
                "\"kinds\": [\"status-bar-widget\"], ",
                "\"actions\": [ { \"id\": \"orphan\", \"label\": \"Orphan\" } ],"
            ),
            1,
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert!(
            plugins[0].action_entry_path.is_none(),
            "no action kind declared, so no action entry is required or resolved"
        );
    }

    #[test]
    fn subscriptions_parse_and_survive_discovery() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replacen(
            "\"kinds\": [\"status-bar-widget\"],",
            concat!(
                "\"kinds\": [\"status-bar-widget\"], ",
                "\"subscriptions\": [\"bell_rang\", \"command_complete\"],"
            ),
            1,
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].manifest.subscriptions,
            vec!["bell_rang".to_string(), "command_complete".to_string()]
        );
    }

    #[test]
    fn manifest_without_subscriptions_defaults_to_empty() {
        // The self-scheduled contract: an absent subscriptions block must
        // parse to an empty list, never a delivery of every event.
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.test",
            MINIMAL_MANIFEST,
            Some("widget.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].manifest.subscriptions.is_empty());
    }

    #[test]
    fn unknown_subscription_kind_skips_the_plugin() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replacen(
            "\"kinds\": [\"status-bar-widget\"],",
            concat!(
                "\"kinds\": [\"status-bar-widget\"], ",
                "\"subscriptions\": [\"bell_rang\", \"not_an_event\"],"
            ),
            1,
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .reason
                .contains("unknown subscription kind `not_an_event`"),
            "got: {}",
            warnings[0].reason
        );
    }

    #[test]
    fn duplicate_subscription_kind_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replacen(
            "\"kinds\": [\"status-bar-widget\"],",
            concat!(
                "\"kinds\": [\"status-bar-widget\"], ",
                "\"subscriptions\": [\"bell_rang\", \"bell_rang\"],"
            ),
            1,
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0]
                .reason
                .contains("duplicate subscription kind `bell_rang`"),
            "got: {}",
            warnings[0].reason
        );
    }

    #[test]
    fn manifest_without_restart_defaults_to_on_failure() {
        // Today's behaviour is the default: a manifest without the field
        // must keep restarting on failure (the config enum's own default,
        // Never, must NOT leak into plugins).
        let tmp = TempDir::new().unwrap();
        write_plugin(
            tmp.path(),
            "com.example.test",
            MINIMAL_MANIFEST,
            Some("widget.py"),
        );
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(
            plugins[0].manifest.restart,
            par_term_config::RestartPolicy::OnFailure
        );
    }

    #[test]
    fn explicit_restart_never_parses() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replace(
            "\"kinds\": [\"status-bar-widget\"],",
            "\"kinds\": [\"status-bar-widget\"], \"restart\": \"never\",",
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].manifest.restart,
            par_term_config::RestartPolicy::Never
        );
    }

    #[test]
    fn unknown_restart_mode_is_skipped() {
        // The enum parse rejects unknown strings at manifest parse time;
        // the skip reason must name the restart value, not just "invalid".
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replace(
            "\"kinds\": [\"status-bar-widget\"],",
            "\"kinds\": [\"status-bar-widget\"], \"restart\": \"sometimes\",",
        );
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(
            warnings[0].reason.contains("sometimes"),
            "got: {}",
            warnings[0].reason
        );
    }

    #[test]
    fn absent_root_is_empty_with_no_warnings() {
        let (plugins, warnings) = discover_plugins(Path::new("/nonexistent/plugins-root-xyz"));
        assert!(plugins.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn empty_kinds_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest =
            MINIMAL_MANIFEST.replace(r#""kinds": ["status-bar-widget"]"#, r#""kinds": []"#);
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(warnings[0].reason.contains("no kinds declared"));
    }

    #[test]
    fn unsupported_schema_version_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let manifest = MINIMAL_MANIFEST.replace("\"schemaVersion\": 1", "\"schemaVersion\": 2");
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert!(warnings[0].reason.contains("unsupported schemaVersion 2"));
    }

    /// Schema used by the validate_settings tests: one entry per type.
    fn test_schema() -> Vec<SettingSchemaEntry> {
        serde_json::from_str(
            r#"[
                { "key": "format24h", "type": "boolean", "label": "24h", "defaultValue": true },
                { "key": "interval", "type": "integer", "label": "Interval",
                  "min": 30, "max": 3600, "defaultValue": 900 },
                { "key": "label", "type": "string", "label": "Label", "defaultValue": "hi" }
            ]"#,
        )
        .unwrap()
    }

    fn settings(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn validate_settings_fills_defaults_for_absent_keys() {
        let schema = test_schema();
        let values = settings(&[("interval", serde_json::json!(120))]);
        let out = validate_settings(&schema, &values).unwrap();
        assert_eq!(out.get("format24h"), Some(&serde_json::json!(true)));
        assert_eq!(out.get("interval"), Some(&serde_json::json!(120)));
        assert_eq!(out.get("label"), Some(&serde_json::json!("hi")));
    }

    #[test]
    fn validate_settings_drops_keys_the_schema_no_longer_declares() {
        let schema = test_schema();
        let values = settings(&[("goneKey", serde_json::json!(1))]);
        let out = validate_settings(&schema, &values).unwrap();
        assert!(!out.contains_key("goneKey"));
    }

    #[test]
    fn validate_settings_rejects_wrong_type() {
        let schema = test_schema();
        let values = settings(&[("interval", serde_json::json!("fast"))]);
        let err = validate_settings(&schema, &values).unwrap_err();
        assert!(err.contains("`interval` expects integer"), "got: {err}");
    }

    #[test]
    fn validate_settings_rejects_fractional_integers() {
        let schema = test_schema();
        let values = settings(&[("interval", serde_json::json!(30.5))]);
        assert!(validate_settings(&schema, &values).is_err());
    }

    #[test]
    fn validate_settings_enforces_min_and_max() {
        let schema = test_schema();
        let too_low = settings(&[("interval", serde_json::json!(10))]);
        assert!(
            validate_settings(&schema, &too_low)
                .unwrap_err()
                .contains("below the minimum 30")
        );
        let too_high = settings(&[("interval", serde_json::json!(9999))]);
        assert!(
            validate_settings(&schema, &too_high)
                .unwrap_err()
                .contains("above the maximum 3600")
        );
    }
}
