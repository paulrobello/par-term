//! SP4: the Settings docs are generated from the layout and the SP1 search
//! registry, and these tests fail when they drift.
//!
//! - `docs/features/SETTINGS.md` is the tab > page > section > control >
//!   YAML key listing, harvested from the rendered tabs.
//! - The `Settings` column of `docs/CONFIG_REFERENCE.md` names the one
//!   section that draws each key, or marks it YAML-only or internal, using
//!   the same lists `coverage_tests` enforces.
//!
//! Regenerate both with `make docs-settings` (runs these tests with
//! `UPDATE_SETTINGS_DOCS=1`). The registry depends on the platform (some
//! controls draw on macOS only), so the files are generated on macOS and
//! other platforms check that nothing they draw is missing from them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::coverage_tests::{INTERNAL, SUMMARY_SECTIONS, YAML_ONLY, config_keys, variants};
use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

/// Fields of nested tables in CONFIG_REFERENCE.md (rows of a list, not
/// top-level keys), and where their editor lives.
const NESTED: &[(&str, &str)] = &[
    ("id", "Assistant & Agents \u{203a} Agents \u{203a} Agents"),
    ("name", "Assistant & Agents \u{203a} Agents \u{203a} Agents"),
    (
        "command",
        "Assistant & Agents \u{203a} Agents \u{203a} Agents",
    ),
    (
        "autonomy_args",
        "Assistant & Agents \u{203a} Agents \u{203a} Agents",
    ),
    (
        "default",
        "Assistant & Agents \u{203a} Agents \u{203a} Agents",
    ),
    (
        "tmux_session_name",
        "Profiles \u{203a} Profiles (profile editor, Session sub-tab)",
    ),
    (
        "tmux_connection_mode",
        "Profiles \u{203a} Profiles (profile editor, Session sub-tab)",
    ),
];

const SEP: &str = " \u{203a} ";

fn docs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_SETTINGS_DOCS").is_some()
}

#[derive(Default, Clone)]
struct Control {
    label: String,
    keys: Vec<String>,
}

#[derive(Default, Clone)]
struct Section {
    id: String,
    title: String,
    /// Keys the section owns through its keywords (list fields edited row by
    /// row), not through one control.
    keys: Vec<String>,
    controls: Vec<Control>,
    /// Index into the page's section list of the enclosing section.
    parent: Option<usize>,
}

struct Page {
    title: &'static str,
    sections: Vec<Section>,
}

struct Model {
    tabs: Vec<(SettingsTab, Vec<Page>)>,
}

fn push_unique(list: &mut Vec<String>, item: &str) {
    if !list.iter().any(|k| k == item) {
        list.push(item.to_string());
    }
}

/// Merge the harvest of every config variant into one tab > page > section
/// model, in layout order.
fn model() -> Model {
    let keys = config_keys();
    // (tab, section id) -> merged section, in first-seen order.
    let mut merged: BTreeMap<(usize, String), Section> = BTreeMap::new();
    let mut parent_id: BTreeMap<(usize, String), String> = BTreeMap::new();
    let mut order: BTreeMap<(usize, String), usize> = BTreeMap::new();
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
            let entries = crate::search::harvest_tab(&mut settings, *tab);
            for entry in &entries {
                let k = (tab.index(), entry.id.clone());
                let n = order.len();
                order.entry(k.clone()).or_insert(n);
                if let Some(p) = entry.parent {
                    parent_id.insert(k.clone(), entries[p].id.clone());
                }
                let section = merged.entry(k).or_insert_with(|| Section {
                    id: entry.id.clone(),
                    title: entry.title.clone(),
                    ..Default::default()
                });
                for kw in &entry.keywords {
                    if keys.contains(kw) {
                        push_unique(&mut section.keys, kw);
                    }
                }
                for control in &entry.controls {
                    let at = section
                        .controls
                        .iter()
                        .position(|c| c.label == control.label)
                        .unwrap_or_else(|| {
                            section.controls.push(Control {
                                label: control.label.clone(),
                                keys: Vec::new(),
                            });
                            section.controls.len() - 1
                        });
                    for term in &control.extra {
                        if keys.contains(term) {
                            push_unique(&mut section.controls[at].keys, term);
                        }
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    for tab in SettingsTab::all() {
        let mut pages = Vec::new();
        for def in crate::layout::pages(*tab) {
            let mut sections: Vec<Section> = Vec::new();
            for sdef in def.sections {
                let k = (tab.index(), sdef.id.to_string());
                let Some(top) = merged.get(&k) else {
                    continue;
                };
                let top_at = sections.len();
                sections.push(top.clone());
                // Nested sections drawn inside this one, in render order.
                let mut children: Vec<(usize, &Section)> = merged
                    .iter()
                    .filter(|(ck, _)| {
                        ck.0 == tab.index() && parent_id.get(*ck) == Some(&sdef.id.to_string())
                    })
                    .map(|(ck, s)| (order[ck], s))
                    .collect();
                children.sort_by_key(|(o, _)| *o);
                for (_, child) in children {
                    let mut child = child.clone();
                    child.parent = Some(top_at);
                    sections.push(child);
                }
            }
            pages.push(Page {
                title: def.title,
                sections,
            });
        }
        out.push((*tab, pages));
    }
    Model { tabs: out }
}

/// YAML key -> "Tab > Page > Section" of the one section that draws it.
fn locations(model: &Model) -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for (tab, pages) in &model.tabs {
        for page in pages {
            for section in &page.sections {
                if SUMMARY_SECTIONS.contains(&section.id.as_str()) {
                    continue;
                }
                let place = format!(
                    "{}{SEP}{}{SEP}{}",
                    tab.display_name(),
                    page.title,
                    section.title
                );
                let all = section
                    .keys
                    .iter()
                    .chain(section.controls.iter().flat_map(|c| c.keys.iter()));
                for key in all {
                    map.entry(key.clone()).or_insert_with(|| place.clone());
                }
            }
        }
    }
    map
}

fn render_settings_md(model: &Model) -> String {
    let mut out = String::new();
    out.push_str(
        "# Settings Reference\n\n\
         <!-- Generated by `make docs-settings` from the Settings layout and search registry\n\
         (par-term-settings-ui/src/docs_tests.rs). Do not edit by hand: the test\n\
         `settings_md_is_current` fails when this file drifts. -->\n\n\
         Every control in the Settings window, by tab, page, and section, with the\n\
         `config.yaml` key it edits. Open Settings with `F12` (`Cmd + ,` on macOS).\n\
         For how edits are previewed, saved, and reverted, see\n\
         [Saving and reverting](#saving-and-reverting). For the full key list with\n\
         types and defaults, see the [Configuration Reference](../CONFIG_REFERENCE.md).\n\n\
         ## Saving and reverting\n\n\
         - Edits preview live in every window. Nothing is written to `config.yaml` until you\n\
         press **Save**.\n\
         - **Save** writes the config and any profile edits together. **Revert** restores every\n\
         window to how it was when Settings opened or last saved.\n\
         - An `* Unsaved changes` marker in the footer tracks pending edits. Closing with\n\
         unsaved edits asks whether to save, revert, or cancel.\n\
         - Font fields wait for **Apply font changes**. Save applies them too.\n\
         - Prompts, agent commands, plugins, shader installs, and shell integration write to\n\
         their own files at once. Save and Revert do not apply to them.\n\
         - A small reset arrow next to a control restores its default as an ordinary edit.\n\
         - A failed save, or a `config.yaml` that changed on disk, shows a banner at the top.\n\n\
         Some settings take effect only after a restart or in new windows. The control is\n\
         badged, and the banner after Save lists them.\n\n",
    );

    out.push_str("## Contents\n\n");
    for (tab, pages) in &model.tabs {
        let titles: Vec<&str> = pages.iter().map(|p| p.title).collect();
        out.push_str(&format!(
            "- [{}](#{}): {}\n",
            tab.display_name(),
            anchor(tab.display_name()),
            titles.join(", ")
        ));
    }
    out.push('\n');

    for (tab, pages) in &model.tabs {
        out.push_str(&format!("## {}\n\n", tab.display_name()));
        for page in pages {
            out.push_str(&format!("### {}\n\n", page.title));
            for section in &page.sections {
                let level = if section.parent.is_some() {
                    "#####"
                } else {
                    "####"
                };
                out.push_str(&format!("{level} {}\n\n", section.title));
                if SUMMARY_SECTIONS.contains(&section.id.as_str()) {
                    out.push_str(
                        "A summary of the most-used settings. Each control also lives in the\n\
                         section that owns it.\n\n",
                    );
                    continue;
                }
                let mut lines = Vec::new();
                for control in section.controls.iter().filter(|c| !c.keys.is_empty()) {
                    let keys: Vec<String> = control.keys.iter().map(|k| format!("`{k}`")).collect();
                    lines.push(format!(
                        "- {} \u{2014} {}\n",
                        control.label,
                        keys.join(", ")
                    ));
                }
                if !section.keys.is_empty() {
                    let keys: Vec<String> = section.keys.iter().map(|k| format!("`{k}`")).collect();
                    lines.push(format!(
                        "- Edited row by row \u{2014} {}\n",
                        keys.join(", ")
                    ));
                }
                if lines.is_empty() {
                    out.push_str("No `config.yaml` keys. Actions and display only.\n\n");
                } else {
                    out.extend(lines);
                    out.push('\n');
                }
            }
        }
    }

    out.push_str("## YAML-only keys\n\nThese keys have no Settings control on purpose.\n\n");
    for (key, reason) in YAML_ONLY {
        out.push_str(&format!("- `{key}`: {reason}\n"));
    }
    out.push_str(
        "\n## Internal keys\n\npar-term writes these itself. Settings does not show them, \
         and edits would be overwritten.\n\n",
    );
    for (key, reason) in INTERNAL {
        out.push_str(&format!("- `{key}`: {reason}\n"));
    }
    out
}

fn anchor(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect::<String>()
        .replace(' ', "-")
}

/// Rewrite a `| Field | Type | Default | Description |` table so it ends with
/// a `Settings` column. The column goes last because
/// `par-term-config/tests/doc_config_values.rs` reads the first four columns
/// by position. Idempotent.
fn annotate_config_reference(text: &str, locs: &BTreeMap<String, String>) -> String {
    let yaml_only: BTreeSet<&str> = YAML_ONLY.iter().map(|(k, _)| *k).collect();
    let internal: BTreeSet<&str> = INTERNAL.iter().map(|(k, _)| *k).collect();
    let mut out = Vec::new();
    for line in text.lines() {
        out.push(annotate_line(line, locs, &yaml_only, &internal));
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn split_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cur.push_str("\\|");
            chars.next();
        } else if c == '|' {
            cells.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    cells.push(cur);
    // Leading and trailing empty cells come from the outer pipes.
    cells[1..cells.len() - 1]
        .iter()
        .map(|c| c.trim().to_string())
        .collect()
}

/// Set the last cell to `value`, adding it when the row has four cells.
fn set_settings_cell(cells: &mut Vec<String>, value: String) {
    if cells.len() == 5 {
        cells[4] = value;
    } else {
        cells.push(value);
    }
}

fn annotate_line(
    line: &str,
    locs: &BTreeMap<String, String>,
    yaml_only: &BTreeSet<&str>,
    internal: &BTreeSet<&str>,
) -> String {
    if !line.starts_with('|') {
        return line.to_string();
    }
    let mut cells = split_cells(line);
    if cells.first().is_some_and(|c| c == "Field") && cells.get(1).is_some_and(|c| c == "Type") {
        set_settings_cell(&mut cells, "Settings".to_string());
        return format!("| {} |", cells.join(" | "));
    }
    if cells.first().is_some_and(|c| c.starts_with("-------")) {
        set_settings_cell(&mut cells, "--------".to_string());
        return format!("|{}|", cells.join("|"));
    }
    let Some(key) = cells
        .first()
        .and_then(|c| c.strip_prefix('`'))
        .and_then(|c| c.strip_suffix('`'))
        .map(str::to_string)
    else {
        return line.to_string();
    };
    let cell = if yaml_only.contains(key.as_str()) {
        "YAML only".to_string()
    } else if internal.contains(key.as_str()) {
        "Internal".to_string()
    } else {
        locs.get(&key)
            .cloned()
            .or_else(|| {
                NESTED
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.to_string())
            })
            .unwrap_or_default()
    };
    set_settings_cell(&mut cells, cell);
    format!("| {} |", cells.join(" | "))
}

fn check_or_write(path: &PathBuf, expected: &str, what: &str) {
    if updating() {
        std::fs::write(path, expected).expect("write generated doc");
        return;
    }
    let current = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        current == expected,
        "{what} is out of date. Run `make docs-settings` and commit the result."
    );
}

#[test]
fn settings_md_is_current() {
    let model = model();
    let expected = render_settings_md(&model);
    let path = docs_dir().join("features/SETTINGS.md");
    if cfg!(target_os = "macos") {
        check_or_write(&path, &expected, "docs/features/SETTINGS.md");
        return;
    }
    // Controls and labels differ by platform, so elsewhere check that every
    // section heading and key line this platform draws is in the file.
    let current = std::fs::read_to_string(&path).expect("SETTINGS.md exists");
    for line in expected.lines().filter(|l| l.starts_with('#')) {
        assert!(current.contains(line), "SETTINGS.md lacks heading: {line}");
    }
    for (key, place) in locations(&model) {
        assert!(
            current.contains(&format!("`{key}`")),
            "SETTINGS.md lacks `{key}` ({place})"
        );
    }
}

#[test]
fn config_reference_names_a_settings_location_for_every_key() {
    let model = model();
    let locs = locations(&model);
    let path = docs_dir().join("CONFIG_REFERENCE.md");
    let current = std::fs::read_to_string(&path).expect("CONFIG_REFERENCE.md exists");
    let expected = annotate_config_reference(&current, &locs);
    if cfg!(target_os = "macos") {
        check_or_write(&path, &expected, "docs/CONFIG_REFERENCE.md");
    } else {
        // Platform-only controls are absent from this harvest, so elsewhere
        // check only that every table carries the Settings column.
        let rows = current.lines().filter(|l| l.starts_with("| `"));
        for row in rows {
            assert_eq!(split_cells(row).len(), 5, "no Settings column: {row}");
        }
    }
}

/// Every top-level config key documented in CONFIG_REFERENCE.md has a
/// non-empty Settings cell: a section, `YAML only`, or `Internal`.
#[test]
fn no_documented_key_lacks_a_settings_cell() {
    if updating() {
        return;
    }
    let keys = config_keys();
    let text = std::fs::read_to_string(docs_dir().join("CONFIG_REFERENCE.md")).expect("read");
    let mut empty = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("| `")) {
        let cells = split_cells(line);
        let key = cells[0].trim_matches('`');
        let nested = NESTED.iter().any(|(k, _)| *k == key);
        if (keys.contains(key) || nested) && cells.get(4).is_none_or(String::is_empty) {
            empty.push(key.to_string());
        }
    }
    if cfg!(target_os = "macos") {
        assert!(empty.is_empty(), "no Settings cell: {empty:?}");
    }
}
