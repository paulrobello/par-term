//! SX3 coverage rule (UX.md 15.4, SP3 criterion 2): every config field is
//! rendered in exactly one Settings section, or is listed here as
//! YAML-only or internal, with the reason.
//!
//! "Rendered" is read from the search harvest, not from source: a field
//! counts when a section draws a control tagged with its YAML key
//! (`.search_tag(&["key"])`), or, for a list edited row by row (triggers,
//! snippets, ...), when the owning section names the key in its keywords.
//! Several controls only draw in one mode (a background image's controls
//! need image mode, shader controls need a shader), so the harvest runs on
//! a few config variants and the homes are merged.

use std::collections::{BTreeMap, BTreeSet};

use par_term_config::Config;

use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

/// Fields with no Settings control on purpose.
const YAML_ONLY: &[(&str, &str)] = &[
    (
        "pane_title_font",
        "no runtime reader (UX.md B7); a control would be a dead control",
    ),
    (
        "status_bar_font",
        "no runtime reader; a control would be a dead control",
    ),
    (
        "font_ranges",
        "per-codepoint-range font list; edited in YAML until a range editor exists",
    ),
    (
        "shell_env",
        "free-form environment map; edited in YAML until a key/value editor exists",
    ),
    (
        "tmux_status_bar_use_native_format",
        "no runtime reader (the native-format query is never issued)",
    ),
    (
        "working_directory",
        "legacy key still read (it overrides startup_directory_mode when set); \
         startup_directory_mode and startup_directory are its Settings controls",
    ),
    (
        "enable_text_shaping",
        "renderer ignores it (UX.md B39); the control was removed until it is wired",
    ),
    (
        "enable_ligatures",
        "renderer ignores it (UX.md B39); the control was removed until it is wired",
    ),
    (
        "enable_kerning",
        "renderer ignores it (UX.md B39); the control was removed until it is wired",
    ),
];

/// Fields par-term writes itself; showing them would invite edits that get
/// overwritten.
const INTERNAL: &[(&str, &str)] = &[
    (
        "collapsed_settings_sections",
        "Settings' own collapse state",
    ),
    (
        "last_working_directory",
        "recorded on exit for startup_directory_mode: previous",
    ),
    ("last_download_directory", "recorded after a download"),
    ("last_update_check", "timestamp of the last update check"),
    (
        "last_notified_version",
        "suppresses a repeat update notification",
    ),
    (
        "skipped_version",
        "set by Skip This Version; shown and cleared on General › Updates",
    ),
    (
        "shader_install_prompt",
        "state of the first-run shader install prompt",
    ),
    (
        "shell_integration_state",
        "state of the shell integration install prompt",
    ),
    (
        "agent_skill_state",
        "state of the agent skill install prompt",
    ),
    ("integration_versions", "installed integration versions"),
    (
        "insecure_trigger_names",
        "computed at load, never serialized",
    ),
    (
        "unaccepted_risk_trigger_names",
        "computed at load, never serialized",
    ),
    (
        "shader_configs",
        "per-shader overrides, edited through each shader's controls",
    ),
    (
        "cursor_shader_configs",
        "per-cursor-shader overrides, edited through each shader's controls",
    ),
    (
        "pane_backgrounds",
        "per-pane images, edited row by row in Panes › Pane Backgrounds",
    ),
];

/// Summary pages may repeat an owned control (UX.md SC7); they are not
/// homes.
const SUMMARY_SECTIONS: &[&str] = &["general_common"];

/// Optional fields omitted from a default config's YAML (skipped while
/// empty), so the top-level key list below does not include them.
const OMITTED_AT_DEFAULT: &[&str] = &[
    "last_download_directory",
    "collapsed_settings_sections",
    "dynamic_profile_sources",
    "auto_restore_arrangement",
    "ai_inspector_extra_agent_roots",
    "ai_inspector_custom_agents",
    "insecure_trigger_names",
    "unaccepted_risk_trigger_names",
];

/// Every top-level YAML key a config can carry.
fn config_keys() -> BTreeSet<String> {
    let value = serde_yaml_ng::to_value(Config::default()).expect("Config serialises");
    let serde_yaml_ng::Value::Mapping(map) = value else {
        panic!("Config must serialise as a mapping");
    };
    map.keys()
        .filter_map(|k| k.as_str().map(str::to_string))
        .chain(OMITTED_AT_DEFAULT.iter().map(|k| k.to_string()))
        .collect()
}

/// Configs that between them draw every mode-dependent control, each with
/// the tabs its mode changes (the default config draws every tab).
fn variants() -> Vec<(Config, &'static [SettingsTab])> {
    use SettingsTab::*;
    let mut image = Config::default();
    image.image.background_mode = par_term_config::BackgroundMode::Image;
    let mut color = Config::default();
    color.image.background_mode = par_term_config::BackgroundMode::Color;
    let mut modes = Config::default();
    modes.shader.custom_shader = Some("variant.glsl".to_string());
    modes.shader.cursor_shader = Some("variant_cursor.glsl".to_string());
    modes.semantic_history.semantic_history_editor_mode =
        par_term_config::SemanticHistoryEditorMode::Custom;
    vec![
        (Config::default(), SettingsTab::all()),
        (image, &[Effects]),
        (color, &[Effects]),
        (modes, &[Effects, Appearance, General]),
    ]
}

/// YAML key → the section ids that render it, across every variant.
fn homes() -> BTreeMap<String, BTreeSet<String>> {
    let keys = config_keys();
    let mut homes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (config, tabs) in variants() {
        let mut settings = SettingsUI::new_for_tests(config);
        settings.temp_custom_shader = settings
            .config
            .shader
            .custom_shader
            .clone()
            .unwrap_or_default();
        settings.temp_cursor_shader = settings
            .config
            .shader
            .cursor_shader
            .clone()
            .unwrap_or_default();
        settings.collapsed_sections.clear();
        for tab in tabs {
            for section in crate::search::harvest_tab(&mut settings, *tab) {
                if SUMMARY_SECTIONS.contains(&section.id.as_str()) {
                    continue;
                }
                let terms = section
                    .controls
                    .iter()
                    .flat_map(|c| c.extra.iter())
                    .chain(section.keywords.iter());
                for term in terms {
                    if keys.contains(term) {
                        homes
                            .entry(term.clone())
                            .or_default()
                            .insert(section.id.clone());
                    }
                }
            }
        }
    }
    homes
}

#[test]
fn every_config_field_is_rendered_once_or_listed() {
    let keys = config_keys();
    let homes = homes();
    let listed = |key: &str| {
        YAML_ONLY.iter().any(|(k, _)| *k == key) || INTERNAL.iter().any(|(k, _)| *k == key)
    };

    let mut unrendered = Vec::new();
    let mut doubled = Vec::new();
    let mut listed_but_rendered = Vec::new();
    for key in &keys {
        let found = homes.get(key).map(BTreeSet::len).unwrap_or(0);
        match (listed(key), found) {
            (true, 0) => {}
            (true, _) => listed_but_rendered.push(format!("{key} {:?}", homes[key])),
            (false, 0) => unrendered.push(key.clone()),
            (false, 1) => {}
            (false, _) => doubled.push(format!("{key} {:?}", homes[key])),
        }
    }
    for (key, _) in YAML_ONLY.iter().chain(INTERNAL) {
        assert!(
            keys.contains(*key),
            "{key} is listed but is not a config key"
        );
    }
    assert!(
        unrendered.is_empty() && doubled.is_empty() && listed_but_rendered.is_empty(),
        "{} fields have no Settings home and no yaml-only/internal entry:\n  {}\n\
         {} fields render in more than one section:\n  {}\n\
         {} listed fields also render:\n  {}",
        unrendered.len(),
        unrendered.join("\n  "),
        doubled.len(),
        doubled.join("\n  "),
        listed_but_rendered.len(),
        listed_but_rendered.join("\n  "),
    );
}

/// The coverage test reads keys from a default config's YAML, which drops
/// optional fields skipped while empty. Each such field must be listed in
/// `OMITTED_AT_DEFAULT`, or it would escape the rule.
#[test]
fn every_skipped_field_is_listed_as_omitted() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../par-term-config/src/config/config_struct");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("config_struct dir") {
        let path = entry.expect("entry").path();
        let text = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let skips = line.contains("skip_serializing_if") || line.contains("serde(skip");
            if !skips || line.trim_start().starts_with("//") {
                continue;
            }
            let field = lines[i + 1..]
                .iter()
                .find_map(|l| {
                    l.trim()
                        .strip_prefix("pub ")
                        .and_then(|rest| rest.split(':').next())
                })
                .unwrap_or_default();
            // Fields of nested row types are covered by their list's key.
            let nested = ["section", "id", "enabled", "settings"];
            if !field.is_empty() && !nested.contains(&field) && !OMITTED_AT_DEFAULT.contains(&field)
            {
                missing.push(format!("{}: {field}", path.display()));
            }
        }
    }
    assert!(missing.is_empty(), "add to OMITTED_AT_DEFAULT: {missing:?}");
}
