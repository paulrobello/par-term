//! Plugin manifest types, discovery, and validation.
//!
//! A plugin is a directory under the plugins root whose name equals its manifest
//! `id`, containing a `manifest.json` and the executable its entry points name.
//! Discovery is deliberately forgiving: one broken manifest produces a warning
//! and is skipped, never blocking the discovery of its neighbours. A plugin is
//! never spawned as a side effect of discovery — the host spawns only what the
//! enabled set names, which is what makes "lands disabled" structural.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use par_term_config::StatusBarSection;
use serde::{Deserialize, Serialize};

/// The only plugin kind v1 understands. Unknown kinds in a manifest skip the
/// whole plugin with a visible warning rather than loading partially.
pub const KIND_STATUS_BAR_WIDGET: &str = "status-bar-widget";

/// Entry-point map key for the [`KIND_STATUS_BAR_WIDGET`] kind.
pub const ENTRY_POINT_STATUS_BAR_WIDGET: &str = "statusBarWidget";

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
}

/// A plugin that passed validation, with confinement-checked paths.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredPlugin {
    /// The parsed manifest.
    pub manifest: PluginManifest,
    /// Canonicalized plugin directory.
    pub dir: PathBuf,
    /// Canonicalized, confinement-checked entry executable for the widget kind.
    pub entry_path: PathBuf,
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
fn validate_plugin_dir(dir: &Path) -> Result<DiscoveredPlugin, String> {
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
    let unknown: Vec<&str> = manifest
        .kinds
        .iter()
        .map(String::as_str)
        .filter(|k| *k != KIND_STATUS_BAR_WIDGET)
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown kinds: {} (known: {KIND_STATUS_BAR_WIDGET})",
            unknown.join(", ")
        ));
    }

    let widget = manifest
        .status_bar_widget
        .as_ref()
        .ok_or_else(|| format!("{KIND_STATUS_BAR_WIDGET} kind requires a statusBarWidget block"))?;
    if widget.schema.is_empty() {
        return Err("statusBarWidget.schema must declare at least one setting".to_string());
    }
    if widget.schema.iter().any(|e| e.key.is_empty()) {
        return Err("statusBarWidget.schema entries must have non-empty keys".to_string());
    }

    let entry = manifest
        .entry_points
        .get(ENTRY_POINT_STATUS_BAR_WIDGET)
        .ok_or_else(|| {
            format!(
                "{KIND_STATUS_BAR_WIDGET} kind requires entryPoints.{ENTRY_POINT_STATUS_BAR_WIDGET}"
            )
        })?;

    // The security line: the entry command resolves inside the plugin
    // directory only — canonicalize both sides and require containment, so a
    // `..` or symlink escape cannot point execution outside the plugin.
    let dir_canon =
        std::fs::canonicalize(dir).map_err(|e| format!("cannot read plugin dir: {e}"))?;
    let entry_path = dir_canon.join(&entry.command);
    let entry_canon = std::fs::canonicalize(&entry_path)
        .map_err(|_| format!("entry point `{}` not found", entry.command))?;
    if !entry_canon.starts_with(&dir_canon) {
        return Err(format!(
            "entry point `{}` escapes the plugin directory",
            entry.command
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&entry_canon)
            .map_err(|e| format!("entry point unreadable: {e}"))?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err(format!("entry point `{}` is not executable", entry.command));
        }
    }

    Ok(DiscoveredPlugin {
        manifest,
        dir: dir_canon,
        entry_path: entry_canon,
    })
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

    /// Write a plugin directory: manifest plus an executable entry file.
    /// `manifest` may use `{id}` placeholders for the directory name so tests
    /// can keep manifest id and dir name in sync.
    fn write_plugin(root: &Path, dir_name: &str, manifest: &str, entry: Option<&str>) {
        let dir = root.join(dir_name);
        fs::create_dir_all(&dir).expect("create plugin dir");
        let manifest = manifest.replace("{id}", dir_name);
        fs::write(dir.join("manifest.json"), manifest).expect("write manifest");
        if let Some(entry_name) = entry {
            let path = dir.join(entry_name);
            fs::write(&path, "#!/bin/sh\nsleep 30\n").expect("write entry");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).expect("entry metadata").permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).expect("chmod entry");
            }
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
        let manifest = MINIMAL_MANIFEST.replace("status-bar-widget", "action-contributor");
        write_plugin(tmp.path(), "com.example.test", &manifest, Some("widget.py"));
        let (plugins, warnings) = discover_plugins(tmp.path());
        assert!(plugins.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .reason
                .contains("unknown kinds: action-contributor")
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
        write_plugin(
            tmp.path(),
            "com.example.test",
            MINIMAL_MANIFEST,
            Some("widget.py"),
        );
        let entry = tmp.path().join("com.example.test/widget.py");
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
