//! par-mux session-hook installer for config-entry agents.
//!
//! pi and omp load extensions from a directory ([`crate::mux_extension_installer`]
//! drops a file in). claude, codex and grok have no extension directory: their
//! hooks are ENTRIES inside config files the user owns and edits, so
//! installing means MERGING into those files without disturbing anything
//! par-term did not add — the user's entries, comments, and formatting
//! survive, proven by round-trip tests in the arm modules.
//!
//! Merge discipline (herdr's claude integration is the proven precedent):
//! - parse before touching anything; a file that fails the strict parse
//!   (duplicate keys, structurally invalid) is a clear error and NOTHING is
//!   written — not even the hook asset;
//! - insertion is a pure text splice at AST-computed offsets replicating the
//!   file's own delimiter style, so untouched bytes never move;
//! - the merged result is re-parsed and compared against the desired value
//!   before it is allowed near the disk (fail closed on any disagreement);
//! - the config file is rewritten through the atomic sibling-temp rename
//!   ([`crate::atomic_save`]), preserving its mode;
//! - reinstall is a no-op while our command is already present (matched by
//!   command string, whatever the user has done to the formatting around it),
//!   and uninstall removes only our command entries — each cut takes one
//!   adjacent separator comma so `[{ours}, {user}]` becomes `[{user}]`, never
//!   `[, {user}]` — pruning a container only when our removals emptied it.
//!
//! The hook assets are the kimi-port reporter shape (core
//! `tests/assets/par-mux-agent-state.sh`): on the agent's session-start event
//! each sends one `pane.report_agent_session` line over `$PAR_MUX_SOCKET`.
//! The scripts are inert outside a par-mux pane (env guards) and silent on
//! every failure.
//!
//! The engine is deliberately core-free (plain fs + jsonc parsing), so it
//! builds and lints without the `mux` feature. Each arm installs a
//! platform-selected asset — the `.sh` reporter on POSIX, a native
//! PowerShell `.ps1` port on Windows (herdr's `powershell -File` command
//! shape) — through the runtime-selected seam here, so both variants
//! compile and are asserted on every platform.

mod claude;
mod codex;
mod grok;

pub use claude::{
    CLAUDE_HOOK_MARKER, ClaudeHookInstall, ClaudeHookUninstall, claude_settings_path,
    install_claude_hook, install_claude_hook_into, uninstall_claude_hook,
    uninstall_claude_hook_into,
};
pub use codex::{
    CODEX_HOOK_MARKER, CodexHookInstall, CodexHookUninstall, codex_config_dir, install_codex_hook,
    install_codex_hook_into, uninstall_codex_hook, uninstall_codex_hook_into,
};
pub use grok::{
    GROK_HOOK_MARKER, GrokHookInstall, GrokHookUninstall, grok_config_dir, install_grok_hook,
    install_grok_hook_into, uninstall_grok_hook, uninstall_grok_hook_into,
};

use jsonc_parser::ast::{
    Array as AstArray, Object as AstObject, ObjectPropName, Value as AstValue,
};
use jsonc_parser::common::{Range, Ranged};
use jsonc_parser::{ParseOptions, parse_to_ast, parse_to_serde_value};
use serde_json::{Value as JsonValue, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Config;

const HOOK_TIMEOUT_SECS: u64 = 10;

/// Where hook script assets are written: par-term's own config directory, the
/// same tree the shell integration scripts are written to.
pub(crate) fn hook_asset_path(install_name: &str) -> PathBuf {
    Config::config_dir().join("hooks").join(install_name)
}

/// The settings entry's command string for the current platform (no action
/// argument — the claude arm's form).
pub(crate) fn hook_command_for(hook_path: &Path) -> String {
    platform_hook_command(hook_path, None, cfg!(windows))
}

/// The settings entry's command string with a trailing action argument (the
/// codex/grok arms' `… session` form).
pub(crate) fn hook_command_with_action(hook_path: &Path, action: &str) -> String {
    platform_hook_command(hook_path, Some(action), cfg!(windows))
}

/// Platform core (test seam — `windows` is injected so both arms compile and
/// are asserted on every platform, leaving no cfg(windows) blind spot):
///
/// - POSIX runs the `.sh` asset. The claude form leaves the path bare when
///   it needs no quoting so the common case stays readable; the action form
///   (`sh '<path>' session`) always quotes, the grok-arm heritage.
/// - Windows runs the `.ps1` asset through herdr's command shape —
///   `powershell -NoProfile -ExecutionPolicy Bypass -File "<path>"` — with
///   the path always double-quoted (spaces are the common case under
///   `%APPDATA%`) and embedded quotes backslash-escaped.
pub(crate) fn platform_hook_command(
    hook_path: &Path,
    action: Option<&str>,
    windows: bool,
) -> String {
    let text = hook_path.display().to_string();
    let mut command = if windows {
        format!(
            "powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
            text.replace('"', "\\\"")
        )
    } else if let Some(action) = action {
        format!("sh {} {action}", shell_single_quote(&text))
    } else {
        let safe = !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_/.:=@%+".contains(c));
        if safe {
            text
        } else {
            shell_single_quote(&text)
        }
    };
    if let Some(action) = action
        && windows
    {
        command.push(' ');
        command.push_str(action);
    }
    command
}

pub(crate) fn shell_single_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub(crate) fn write_hook_asset(hook_path: &Path, asset: &str) -> io::Result<()> {
    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(hook_path, asset)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hook_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Delete a hook asset only when it carries our marker: a same-named file
/// without one is a user file we must not touch.
pub(crate) fn remove_marked_file(path: &Path, marker: &str) -> io::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    if !fs::read_to_string(path)?.contains(marker) {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
}

pub(crate) fn save_settings(settings_path: &Path, contents: &str) -> io::Result<()> {
    crate::atomic_save::save_string_atomic_preserving_mode(settings_path, contents).map_err(|err| {
        io::Error::other(format!(
            "failed to write {}: {err:#}",
            settings_path.display()
        ))
    })
}

pub(crate) fn home_dir() -> io::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| io::Error::other("Could not determine home directory"))
}

/// Settings files are jsonc in practice: comments and trailing commas are
/// part of what the agents accept, everything looser is not.
pub(crate) fn parse_options() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    }
}

pub(crate) fn parse_root_object<'a>(
    content: &'a str,
    settings_path: &Path,
) -> io::Result<AstObject<'a>> {
    let parsed = parse_to_ast(content, &Default::default(), &parse_options()).map_err(|err| {
        io::Error::other(format!(
            "failed to parse {}: {err}",
            settings_path.display()
        ))
    })?;
    match parsed.value {
        Some(AstValue::Object(object)) => Ok(object),
        _ => Err(io::Error::other(format!(
            "config at {} must be a JSON object",
            settings_path.display()
        ))),
    }
}

pub(crate) fn parse_serde(content: &str, settings_path: &Path) -> io::Result<JsonValue> {
    parse_to_serde_value(content, &parse_options()).map_err(|err| {
        io::Error::other(format!(
            "failed to parse {}: {err}",
            settings_path.display()
        ))
    })
}

/// Duplicate keys make the file ambiguous — which value survives a merge is
/// not ours to decide, so refuse.
pub(crate) fn reject_duplicate_keys(object: &AstObject, settings_path: &Path) -> io::Result<()> {
    let mut names = std::collections::HashSet::new();
    for property in &object.properties {
        let Some(name) = prop_name_text(&property.name) else {
            return Err(io::Error::other("JSON object property is missing a name"));
        };
        if !names.insert(name) {
            return Err(io::Error::other(format!(
                "duplicate key \"{name}\" in {} — fix the file before installing",
                settings_path.display()
            )));
        }
    }
    for property in &object.properties {
        match &property.value {
            AstValue::Object(inner) => reject_duplicate_keys(inner, settings_path)?,
            AstValue::Array(array) => {
                for element in &array.elements {
                    if let AstValue::Object(inner) = element {
                        reject_duplicate_keys(inner, settings_path)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn prop_name_text<'a>(name: &'a ObjectPropName<'a>) -> Option<&'a str> {
    match name {
        ObjectPropName::String(lit) => Some(lit.value.as_ref()),
        ObjectPropName::Word(word) => Some(word.value),
    }
}

pub(crate) fn string_value<'a>(value: &'a AstValue<'_>) -> Option<&'a str> {
    match value {
        AstValue::StringLit(lit) => Some(lit.value.as_ref()),
        _ => None,
    }
}

/// The canonical SessionStart entry for `command`, as compact JSON text (used
/// for the splice) and as a value (used for the presence check and verify).
/// `matcher` scopes the entry to claude's SessionStart sources; codex has no
/// matcher space and passes `None`.
pub(crate) fn canonical_entry_json(matcher: Option<&str>, command: &str) -> String {
    let command_json = serde_json::to_string(command).expect("command is serializable");
    let matcher_json = match matcher {
        Some(matcher) => format!(
            "\"matcher\":{},",
            serde_json::to_string(matcher).expect("matcher is serializable",)
        ),
        None => String::new(),
    };
    format!(
        "{{{matcher_json}\"hooks\":[{{\"type\":\"command\",\"command\":{command_json},\"timeout\":{HOOK_TIMEOUT_SECS}}}]}}"
    )
}

pub(crate) fn canonical_entry_value(matcher: Option<&str>, command: &str) -> JsonValue {
    let mut entry = json!({});
    if let Some(matcher) = matcher {
        entry["matcher"] = json!(matcher);
    }
    entry["hooks"] = json!([{
        "type": "command",
        "command": command,
        "timeout": HOOK_TIMEOUT_SECS,
    }]);
    entry
}

/// Whether any SessionStart group already carries our exact command.
pub(crate) fn has_command(
    content: &str,
    settings_path: &Path,
    command: &str,
    event: &str,
) -> io::Result<bool> {
    let value = parse_serde(content, settings_path)?;
    Ok(value
        .get("hooks")
        .and_then(|hooks| hooks.get(event))
        .and_then(JsonValue::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                group
                    .get("hooks")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            entry.get("command").and_then(JsonValue::as_str) == Some(command)
                        })
                    })
            })
        }))
}

/// Merge our entry for `event` into `content`. `Ok(None)` = already present,
/// byte-exact no-op.
pub(crate) fn merge_hook_event(
    content: &str,
    settings_path: &Path,
    command: &str,
    matcher: Option<&str>,
    event: &str,
) -> io::Result<Option<String>> {
    let root = parse_root_object(content, settings_path)?;
    reject_duplicate_keys(&root, settings_path)?;
    if has_command(content, settings_path, command, event)? {
        return Ok(None);
    }

    let canonical = canonical_entry_json(matcher, command);
    let updated = match root.get_object("hooks") {
        Some(hooks) => match hooks.get_array(event) {
            Some(event_array) => append_array_element(content, event_array, &canonical),
            None => append_object_property(content, hooks, event, &format!("[{canonical}]")),
        },
        None => append_object_property(
            content,
            &root,
            "hooks",
            &format!("{{\"{event}\":[{canonical}]}}"),
        ),
    };

    // Fail closed: the spliced text must parse back to exactly the desired
    // value before the caller is allowed to write it. The desired value
    // mirrors the splice branch above.
    let mut desired = parse_serde(content, settings_path)?;
    match desired
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut(event))
        .and_then(JsonValue::as_array_mut)
    {
        Some(entries) => entries.push(canonical_entry_value(matcher, command)),
        None => {
            let Some(root_object) = desired.as_object_mut() else {
                return Err(io::Error::other(format!(
                    "config at {} must be a JSON object",
                    settings_path.display()
                )));
            };
            match root_object.get_mut("hooks") {
                Some(hooks_value) => {
                    let Some(hooks_object) = hooks_value.as_object_mut() else {
                        return Err(io::Error::other(format!(
                            "hooks at {} must be a JSON object",
                            settings_path.display()
                        )));
                    };
                    hooks_object.insert(
                        event.to_string(),
                        json!([canonical_entry_value(matcher, command)]),
                    );
                }
                None => {
                    let mut fresh_hooks = serde_json::Map::new();
                    fresh_hooks.insert(
                        event.to_string(),
                        json!([canonical_entry_value(matcher, command)]),
                    );
                    root_object.insert("hooks".to_string(), JsonValue::Object(fresh_hooks));
                }
            }
        }
    }
    verify_update(updated, settings_path, &desired).map(Some)
}

/// Remove our command entries from `content`. Each entry pairs the hook
/// event its command lives under with the command string; only those
/// events' arrays are touched, so a user's own hooks — including empty
/// arrays under events we never install — survive byte-exact.
/// `Ok(None)` = nothing to remove.
pub(crate) fn unmerge_hook_entries(
    content: &str,
    settings_path: &Path,
    entries: &[(&str, &str)],
) -> io::Result<Option<String>> {
    let root = parse_root_object(content, settings_path)?;
    reject_duplicate_keys(&root, settings_path)?;
    let original = parse_serde(content, settings_path)?;
    let events: Vec<&str> = entries.iter().map(|(event, _)| *event).collect();

    let mut updated = content.to_string();
    let mut changed = false;

    // One cut per pass, re-parsing between cuts so every span is computed
    // against the text it is applied to. Settings files are small.
    while let Some(cut) = find_next_our_cut(&updated, settings_path, entries)? {
        let mut next = String::with_capacity(updated.len());
        next.push_str(&updated[..cut.start]);
        next.push_str(&updated[cut.end..]);
        updated = next;
        changed = true;
    }

    if !changed {
        return Ok(None);
    }

    // Prune containers our removals emptied: the events we cut from, then
    // hooks. Each prune is its own re-parsed pass for the same span-safety
    // reason.
    while let Some(cut) = find_next_empty_container_cut(&updated, settings_path, &events)? {
        let mut next = String::with_capacity(updated.len());
        next.push_str(&updated[..cut.start]);
        next.push_str(&updated[cut.end..]);
        updated = next;
    }

    let desired = desired_after_removal(original, entries);
    verify_update(updated, settings_path, &desired).map(Some)
}

/// The next single removal to make: a whole group when the group holds only
/// our commands, else one of our command entries inside a mixed group.
fn find_next_our_cut(
    content: &str,
    settings_path: &Path,
    entries: &[(&str, &str)],
) -> io::Result<Option<Range>> {
    let root = parse_root_object(content, settings_path)?;
    let Some(hooks) = root.get_object("hooks") else {
        return Ok(None);
    };

    for (event, command) in entries {
        let Some(event_array) = hooks.get_array(event) else {
            continue;
        };
        for (group_index, group) in event_array.elements.iter().enumerate() {
            let AstValue::Object(group_object) = group else {
                continue;
            };
            let Some(group_hooks) = group_object.get_array("hooks") else {
                continue;
            };
            let Some(first_our_entry) = group_hooks
                .elements
                .iter()
                .position(|entry| entry_is_our_command(entry, command))
            else {
                continue;
            };
            if group_hooks
                .elements
                .iter()
                .all(|entry| entry_is_our_command(entry, command))
            {
                let group_range = group.range();
                let cut = member_cut(
                    group_index
                        .checked_sub(1)
                        .and_then(|prev| event_array.elements.get(prev))
                        .map(Ranged::range)
                        .map(|range| range.end),
                    group_range.start,
                    group_range.end,
                    event_array
                        .elements
                        .get(group_index + 1)
                        .map(Ranged::range)
                        .map(|range| range.start),
                );
                return Ok(Some(cut));
            }
            let entry_range = group_hooks.elements[first_our_entry].range();
            let cut = member_cut(
                first_our_entry
                    .checked_sub(1)
                    .and_then(|prev| group_hooks.elements.get(prev))
                    .map(Ranged::range)
                    .map(|range| range.end),
                entry_range.start,
                entry_range.end,
                group_hooks
                    .elements
                    .get(first_our_entry + 1)
                    .map(Ranged::range)
                    .map(|range| range.start),
            );
            return Ok(Some(cut));
        }
    }
    Ok(None)
}

/// The next empty-container prune: an event array we cut from with no
/// elements left (drop the property from hooks), or a hooks object with no
/// properties (drop the property from root). Events we never touched are
/// not candidates — a user's pre-existing empty array survives uninstall.
fn find_next_empty_container_cut(
    content: &str,
    settings_path: &Path,
    events: &[&str],
) -> io::Result<Option<Range>> {
    let root = parse_root_object(content, settings_path)?;
    let Some(hooks) = root.get_object("hooks") else {
        return Ok(None);
    };
    if hooks.properties.is_empty() {
        return Ok(Some(property_cut(&root.properties, "hooks")));
    }
    for event in events {
        if let Some(event_array) = hooks.get_array(event)
            && event_array.elements.is_empty()
        {
            return Ok(Some(property_cut(&hooks.properties, event)));
        }
    }
    Ok(None)
}

/// The cut span for the named property, including one adjacent separator
/// comma — same rule as element cuts.
fn property_cut(properties: &[jsonc_parser::ast::ObjectProp<'_>], name: &str) -> Range {
    let Some(index) = properties
        .iter()
        .position(|property| prop_name_text(&property.name) == Some(name))
    else {
        return Range { start: 0, end: 0 };
    };
    let property = &properties[index];
    member_cut(
        index
            .checked_sub(1)
            .map(|prev| &properties[prev])
            .map(|prev| prev.range.end),
        property.range.start,
        property.range.end,
        properties.get(index + 1).map(|next| next.range.start),
    )
}

/// The cut span for removing container member `[start, end)`: the separator
/// in front when there is a predecessor, else the one behind, so exactly one
/// comma goes with the member and the surviving text stays well-formed.
fn member_cut(
    prev_end: Option<usize>,
    start: usize,
    end: usize,
    next_start: Option<usize>,
) -> Range {
    match (prev_end, next_start) {
        (Some(prev), _) => Range { start: prev, end },
        (None, Some(next)) => Range { start, end: next },
        (None, None) => Range { start, end },
    }
}

fn entry_is_our_command(entry: &AstValue, command: &str) -> bool {
    let AstValue::Object(entry_object) = entry else {
        return false;
    };
    entry_object.properties.iter().any(|property| {
        prop_name_text(&property.name) == Some("command")
            && string_value(&property.value) == Some(command)
    })
}

/// The value the file must parse back to after uninstall: our commands gone,
/// emptied containers pruned. Only the events our entries were installed
/// under are pruned, and only when removing ours emptied them — a user's
/// pre-existing empty array under any event survives.
fn desired_after_removal(mut value: JsonValue, entries: &[(&str, &str)]) -> JsonValue {
    let Some(root) = value.as_object_mut() else {
        return value;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(JsonValue::as_object_mut) else {
        return value;
    };
    for (event, command) in entries {
        let Some(event_array) = hooks.get_mut(*event).and_then(JsonValue::as_array_mut) else {
            continue;
        };
        let had_groups = !event_array.is_empty();
        let mut kept_groups = Vec::with_capacity(event_array.len());
        for group in event_array.drain(..) {
            let mut group = group;
            let pruned_empty = match group.get_mut("hooks").and_then(JsonValue::as_array_mut) {
                Some(entries) => {
                    entries.retain(|entry| {
                        entry.get("command").and_then(JsonValue::as_str) != Some(*command)
                    });
                    entries.is_empty()
                }
                None => false,
            };
            if !pruned_empty {
                kept_groups.push(group);
            }
        }
        *event_array = kept_groups;
        if had_groups && event_array.is_empty() {
            hooks.remove(*event);
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
    value
}

fn verify_update(updated: String, settings_path: &Path, desired: &JsonValue) -> io::Result<String> {
    let actual = parse_serde(&updated, settings_path)?;
    if &actual != desired {
        return Err(io::Error::other(format!(
            "failed to safely update config at {}",
            settings_path.display()
        )));
    }
    Ok(updated)
}

// --- text-splice machinery (ported from herdr's claude integration) ---

fn append_object_property(
    content: &str,
    object: &AstObject<'_>,
    name: &str,
    value: &str,
) -> String {
    let key = serde_json::to_string(name).expect("JSON object keys are serializable");
    let key_value_separator = object
        .properties
        .first()
        .map(|property| &content[property.name.range().end..property.value.range().start])
        .unwrap_or(":");
    let insertion = format!("{key}{key_value_separator}{value}");
    let delimiter = object_delimiter(content, object);
    append_to_container(
        content,
        object.range,
        !object.properties.is_empty(),
        delimiter,
        &insertion,
    )
}

fn append_array_element(content: &str, array: &AstArray<'_>, value: &str) -> String {
    let delimiter = array_delimiter(content, array);
    append_to_container(
        content,
        array.range,
        !array.elements.is_empty(),
        delimiter,
        value,
    )
}

fn object_delimiter<'a>(content: &'a str, object: &AstObject<'_>) -> &'a str {
    match object.properties.as_slice() {
        [first, second, ..] => delimiter_suffix(&content[first.range.end..second.range.start]),
        [first] => &content[object.range.start + 1..first.range.start],
        [] => "",
    }
}

fn array_delimiter<'a>(content: &'a str, array: &AstArray<'_>) -> &'a str {
    match array.elements.as_slice() {
        [first, second, ..] => delimiter_suffix(&content[first.range().end..second.range().start]),
        [first] => &content[array.range.start + 1..first.range().start],
        [] => "",
    }
}

fn delimiter_suffix(delimiter: &str) -> &str {
    delimiter
        .split_once(',')
        .map(|(_, suffix)| suffix)
        .unwrap_or(delimiter)
}

fn append_to_container(
    content: &str,
    range: Range,
    has_elements: bool,
    delimiter: &str,
    value: &str,
) -> String {
    let closing = range.end - 1;
    // Insert right after the last non-whitespace member and leave the file's
    // own trailing gap (newline + indent before the closer) AFTER the new
    // member — that is what makes the uninstall cut (back to the previous
    // member's end) restore the original bytes exactly.
    let insertion_index = if has_elements {
        content[..closing]
            .trim_end_matches([' ', '\t', '\n', '\r'])
            .len()
    } else {
        closing
    };
    let mut updated = String::with_capacity(content.len() + delimiter.len() + value.len() + 1);
    updated.push_str(&content[..insertion_index]);
    if has_elements {
        updated.push(',');
        updated.push_str(delimiter);
    }
    updated.push_str(value);
    updated.push_str(&content[insertion_index..]);
    updated
}
