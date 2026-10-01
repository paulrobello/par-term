//! Source-scan gates for UX.md SC2 (units) and SC6 (no hard-coded chords).
//!
//! These read the crate's own source rather than rendering it: a label or
//! tooltip string is text whichever path draws it, and a numeric control's
//! builder chain says whether it carries a unit. `#[cfg(test)]` code is
//! skipped.

use std::path::{Path, PathBuf};

fn source_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path.to_string_lossy().ends_with("_tests.rs")
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
    out.sort();
    out
}

fn rel(path: &Path) -> String {
    path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Production source: line comments removed (doc text about chords does not
/// count) and every `#[cfg(test)]` item blanked wherever it sits, the way
/// `scripts/check_line_counts.py` does. Cutting at the first attribute would
/// silently skip production code that follows a mid-file test helper.
/// Line structure is kept, so reported line numbers stay right.
fn production_source(path: &Path) -> String {
    let text = std::fs::read_to_string(path).expect("read source");
    let mut text: String = text
        .lines()
        .map(|line| match line.trim_start().starts_with("//") {
            true => "",
            false => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    while let Some(at) = text.find("#[cfg(test)]") {
        let end = cfg_test_item_end(&text, at + "#[cfg(test)]".len());
        let blank: String = text[at..end]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect();
        text.replace_range(at..end, &blank);
    }
    text
}

/// End of the item a `#[cfg(test)]` attribute applies to: the matching `}`
/// of its first brace block, or the `;` that ends it, whichever comes first.
fn cfg_test_item_end(text: &str, from: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b';' => return i + 1,
            b'{' => {
                let end = matching_close(bytes, i);
                // `thread_local! { .. }` and similar macro items end at `}`;
                // a trailing `;` belongs to the same item.
                return if bytes.get(end) == Some(&b';') {
                    end + 1
                } else {
                    end
                };
            }
            b'(' | b'[' => i = matching_close(bytes, i),
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Index just past the bracket matching `text[open]`, skipping string and
/// char literals.
fn matching_close(text: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < text.len() {
        match text[i] {
            b'"' => {
                i += 1;
                while i < text.len() && text[i] != b'"' {
                    if text[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'\'' if i + 2 < text.len() && text[i + 2] == b'\'' => i += 2,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    text.len()
}

/// Method names chained onto the call that ends at `end`.
fn chained_methods(text: &str, mut end: usize) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut names = Vec::new();
    loop {
        let rest = &text[end..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('.') {
            return names;
        }
        let start = end + (rest.len() - trimmed.len()) + 1;
        let name_len = text[start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(0);
        let name = &text[start..start + name_len];
        let after = start + name_len;
        if name.is_empty() || bytes.get(after) != Some(&b'(') {
            return names;
        }
        names.push(name.to_string());
        end = matching_close(bytes, after);
    }
}

/// Numeric controls with no unit, each with the reason a unit does not apply.
const UNITLESS: &[(&str, &str, &str)] = &[
    (
        "automation_tab/plugins_section.rs",
        "&mut number",
        "plugin-defined setting; the plugin's manifest names what it means",
    ),
    (
        "background_tab/shader_settings/uniform_controls.rs",
        "",
        "shader uniform; the shader defines its meaning",
    ),
    (
        "background_tab/pane_backgrounds.rs",
        "&mut index",
        "pane index, a count with no unit",
    ),
    (
        "window_tab/behavior.rs",
        "monitor_index",
        "monitor index (0 = primary)",
    ),
    (
        "window_tab/behavior.rs",
        "space_number",
        "Mission Control Space number",
    ),
    (
        "notifications_tab/anti_idle.rs",
        "code",
        "ASCII code; its hex form is shown in the picker",
    ),
];

/// UX.md SS4: every numeric control bound straight to a `settings.config`
/// field offers ↺ reset. Checked by field path: the file that draws the
/// control must also pass the same path to `reset_button`. The quick
/// settings strip mirrors controls whose owning page has the reset.
#[test]
fn every_config_numeric_control_has_a_reset() {
    let mut missing = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        if file == "quick_settings.rs" {
            continue;
        }
        let text = production_source(&path);
        let squashed: String = text.split_whitespace().collect();
        for pattern in ["Slider::new(", "DragValue::new("] {
            for (at, _) in text.match_indices(pattern) {
                let open = at + pattern.len() - 1;
                let close = matching_close(text.as_bytes(), open);
                let args: String = text[open + 1..close - 1].split_whitespace().collect();
                let Some(field) = args.strip_prefix("&mutsettings.config.") else {
                    continue;
                };
                let field = field.split([',', ')']).next().unwrap_or(field);
                if !squashed.contains(&format!("&mutc.{field});"))
                    && !squashed.contains(&format!("&mutc.{field}}}"))
                {
                    let line = text[..at].matches('\n').count() + 1;
                    missing.push(format!("{file}:{line} {field}"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "config-bound numeric controls without ↺ reset (SS4):\n{}",
        missing.join("\n")
    );
}

/// UX.md SQ2: every checkbox, slider, and drag value bound straight to a
/// `settings.config` field is searchable by its YAML key, so a unit suffix
/// is never the only way to find a numeric control. The key is the field's
/// last path segment (every sub-config is `#[serde(flatten)]`).
#[test]
fn every_config_bound_control_is_tagged_with_its_yaml_key() {
    let mut missing = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        if file == "quick_settings.rs" {
            continue;
        }
        let text = production_source(&path);
        let squashed: String = text.split_whitespace().collect();
        for pattern in [".checkbox(", "Slider::new(", "DragValue::new("] {
            for (at, _) in text.match_indices(pattern) {
                let open = at + pattern.len() - 1;
                let close = matching_close(text.as_bytes(), open);
                let args: String = text[open + 1..close - 1].split_whitespace().collect();
                let Some(field) = args.strip_prefix("&mutsettings.config.") else {
                    continue;
                };
                let field = field.split([',', ')']).next().unwrap_or(field);
                let key = field.rsplit('.').next().unwrap_or(field);
                // `cursor_shadow_offset[0]` is one element of a YAML list.
                let key = key.split('[').next().unwrap_or(key);
                if !squashed.contains(&format!(".search_tag(&[\"{key}\"])")) {
                    let line = text[..at].matches('\n').count() + 1;
                    missing.push(format!("{file}:{line} {key}"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "config-bound controls without .search_tag(YAML key) (SQ2):\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_numeric_control_shows_a_unit() {
    let mut missing = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        let text = production_source(&path);
        for pattern in ["Slider::new(", "DragValue::new("] {
            for (at, _) in text.match_indices(pattern) {
                let open = at + pattern.len() - 1;
                let close = matching_close(text.as_bytes(), open);
                let args = &text[open..close];
                let mut methods = chained_methods(&text, close);
                // `crate::units::percent(Slider::new(..))` formats as a percent.
                let before = text[..at].trim_end_matches("egui::");
                if before.ends_with("units::percent(") {
                    methods.push("percent".to_string());
                }
                let has_unit = methods
                    .iter()
                    .any(|m| m == "suffix" || m == "custom_formatter" || m == "percent");
                let allowed = UNITLESS
                    .iter()
                    .any(|(f, needle, _)| *f == file && args.contains(needle));
                if !has_unit && !allowed {
                    let line = text[..at].matches('\n').count() + 1;
                    missing.push(format!(
                        "{file}:{line} {}",
                        args.split_whitespace().collect::<Vec<_>>().join(" ")
                    ));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "numeric controls without a unit (SC2):\n{}",
        missing.join("\n")
    );
}

/// Files whose chord text is data, not prose.
const CHORD_DATA_FILES: &[(&str, &str)] = &[(
    "input_tab/actions_table.rs",
    "the default-chord table; the root crate's chord_tests gate every entry against dispatch",
)];

/// Key names that are not par-term keybindings, so naming them cannot go
/// stale after a rebind: keys a widget handles itself, mouse gestures, the
/// characters a modifier produces, and vi keys in copy mode.
const FIXED_KEYS: &[(&str, &str)] = &[
    (
        "Shift+Enter",
        "find field: previous match, handled by the text field",
    ),
    (
        "Option+Click",
        "mouse gesture governed by the checkbox it labels",
    ),
    (
        "Alt+Click",
        "mouse gesture governed by the checkbox it labels",
    ),
    (
        "Option+F",
        "example of the character the Option key produces",
    ),
    (
        "{OPTION_KEY}+F",
        "example of the character the Option/Alt key produces",
    ),
    (
        "Ctrl+V for visual select",
        "vi-style copy-mode key, not a keybinding",
    ),
    (
        "Ctrl+Z work consistently",
        "example of layout-independent matching, no action named",
    ),
];

#[test]
fn no_label_or_tooltip_hard_codes_a_chord() {
    let chord = |s: &str| -> bool {
        let modifiers = [
            "Cmd", "Ctrl", "Alt", "Shift", "Option", "Opt", "Super", "Control", "Command",
        ];
        modifiers.iter().any(|m| {
            s.match_indices(m).any(|(i, _)| {
                let before_ok = i == 0 || !s.as_bytes()[i - 1].is_ascii_alphanumeric();
                let after = &s[i + m.len()..];
                before_ok && (after.starts_with('+') || after.starts_with("/Ctrl+"))
            })
        })
    };
    let mut hits = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        if CHORD_DATA_FILES.iter().any(|(f, _)| *f == file) {
            continue;
        }
        let text = production_source(&path);
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\'') {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b'"' {
                    if bytes[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                let literal = &text[start..j.min(text.len())];
                let mut scrubbed = literal.to_string();
                for (fixed, _) in FIXED_KEYS {
                    scrubbed = scrubbed.replace(fixed, "");
                }
                if chord(&scrubbed) {
                    let line = text[..i].matches('\n').count() + 1;
                    hits.push(format!("{file}:{line} {literal:?}"));
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
    }
    assert!(
        hits.is_empty(),
        "hard-coded chords in UI text (SC6/B53):\n{}",
        hits.join("\n")
    );
}

/// Scroll areas allowed inside the Settings content area (which scrolls
/// itself), each with the reason (UX.md SC9).
const NESTED_SCROLL_ALLOWED: &[(&str, &str)] = &[
    (
        "settings_ui/sections.rs",
        "the sidebar and the content area themselves",
    ),
    (
        "shader_editor.rs",
        "a separate editor window, not inside the content area",
    ),
    (
        "cursor_shader_editor.rs",
        "a separate editor window, not inside the content area",
    ),
    (
        "profile_modal_ui/edit_view.rs",
        "the icon picker popup, drawn on its own layer",
    ),
    (
        "automation_tab/coprocesses_section.rs",
        "live output log, unbounded and pinned to the bottom",
    ),
    (
        "automation_tab/plugins_section.rs",
        "live plugin panel text, unbounded",
    ),
    (
        "scripts_tab/list.rs",
        "live script output and panel text, unbounded",
    ),
];

#[test]
fn no_scroll_area_inside_the_content_scroll_area() {
    let mut hits = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        if NESTED_SCROLL_ALLOWED.iter().any(|(f, _)| *f == file) {
            continue;
        }
        let text = production_source(&path);
        for (at, _) in text.match_indices("ScrollArea::") {
            hits.push(format!("{file}:{}", text[..at].matches('\n').count() + 1));
        }
    }
    assert!(
        hits.is_empty(),
        "nested scroll areas (SC9):\n{}",
        hits.join("\n")
    );
}

/// Files that may draw an Edit, Delete, or Remove button themselves, each
/// with the reason (UX.md SC4: every list's rows use `list_editor`).
const ROW_BUTTONS_ALLOWED: &[(&str, &str)] = &[
    ("list_editor.rs", "the shared row component itself"),
    (
        "delete_confirm.rs",
        "the two-click confirm primitive the row component uses",
    ),
    (
        "advanced_tab/import_export.rs",
        "Import & Replace and Fetch & Replace: whole-config actions, not rows",
    ),
    (
        "integrations_tab.rs",
        "shader bundle Uninstall: a whole-bundle action, not a row",
    ),
    (
        "background_tab/mod.rs",
        "Delete for the selected shader file beside the picker, not a list row",
    ),
    (
        "shader_dialogs.rs",
        "the Delete button of the delete-shader confirmation dialog",
    ),
];

/// UX.md SC4: list rows draw Edit and Delete only through
/// `list_editor::row_actions`, so every list gets the same buttons,
/// confirmation, reorder, and duplicate.
#[test]
fn every_list_row_uses_the_shared_row_component() {
    let patterns = [
        "small_button(\"Edit\")",
        ".button(\"Edit\")",
        "small_button(\"Delete\")",
        ".button(\"Delete\")",
        "small_button(\"Remove\")",
        ".button(\"Remove\")",
        "confirm_delete_button(",
        "confirm_action_button(",
    ];
    let mut hits = Vec::new();
    for path in source_files() {
        let file = rel(&path);
        if ROW_BUTTONS_ALLOWED.iter().any(|(f, _)| *f == file) {
            continue;
        }
        let text = production_source(&path);
        for pattern in patterns {
            for (at, _) in text.match_indices(pattern) {
                hits.push(format!(
                    "{file}:{} {pattern}",
                    text[..at].matches('\n').count() + 1
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "list rows drawing their own Edit/Delete (SC4, use list_editor::row_actions):\n{}",
        hits.join("\n")
    );
}
