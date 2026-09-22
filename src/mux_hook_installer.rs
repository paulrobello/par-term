//! par-mux session-hook installer for config-entry agents (claude arm).
//!
//! pi and omp load extensions from a directory ([`crate::mux_extension_installer`]
//! drops a file in). claude has no extension directory: its hooks are ENTRIES
//! inside the user's `~/.claude/settings.json`, so installing means MERGING
//! into a file the user owns and edits. This module does that without
//! disturbing anything it did not add — the user's entries, comments, and
//! formatting survive, proven by round-trip tests below.
//!
//! Merge discipline (herdr's claude integration is the proven precedent):
//! - parse before touching anything; a file that fails the strict parse
//!   (duplicate keys, structurally invalid) is a clear error and NOTHING is
//!   written — not even the hook asset;
//! - insertion is a pure text splice at AST-computed offsets replicating the
//!   file's own delimiter style, so untouched bytes never move;
//! - the merged result is re-parsed and compared against the desired value
//!   before it is allowed near the disk (fail closed on any disagreement);
//! - the settings file is rewritten through the atomic sibling-temp rename
//!   ([`crate::atomic_save`]), preserving its mode;
//! - reinstall is a no-op while our command is already present (matched by
//!   command string, whatever the user has done to the formatting around it),
//!   and uninstall removes only our command entries — each cut takes one
//!   adjacent separator comma so `[{ours}, {user}]` becomes `[{user}]`, never
//!   `[, {user}]` — pruning a container only when our removals emptied it.
//!
//! The hook asset itself is the kimi-port reporter shape (core
//! `tests/assets/par-mux-agent-state.sh`): on claude SessionStart it sends one
//! `pane.report_agent_session` line over `$PAR_MUX_SOCKET` carrying the session
//! id, transcript path, and a `claude --resume` argv. The script is inert
//! outside a par-mux pane (env guards) and silent on every failure.
//!
//! The module is deliberately core-free (plain fs + jsonc parsing), so it
//! builds and lints without the `mux` feature. grok (claude's hook format,
//! different SessionStart sources) and codex (TOML) land as siblings reusing
//! this merger; the Windows `.ps1` asset variant is follow-up work — the
//! merge machinery here is platform-neutral.

use crate::config::Config;
use jsonc_parser::ast::{
    Array as AstArray, Object as AstObject, ObjectPropName, Value as AstValue,
};
use jsonc_parser::common::{Range, Ranged};
use jsonc_parser::{ParseOptions, parse_to_ast, parse_to_serde_value};
use serde_json::{Value as JsonValue, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CLAUDE_HOOK_INSTALL_NAME: &str = "par-mux-claude-session-hook.sh";
const CLAUDE_HOOK_ASSET: &str = include_str!("../mux_hooks/par-mux-claude-session-hook.sh");

/// Marker identifying the installed script as ours regardless of which build
/// wrote it — the uninstall path refuses to remove a same-named file without
/// one, exactly like the extension installer.
pub const CLAUDE_HOOK_MARKER: &str = "PAR_MUX_INTEGRATION_ID=claude";

/// Claude's documented SessionStart sources. Grok imports claude's hook format
/// but emits `new`/`load`; the matcher keeps this hook from firing there.
const SESSION_START_MATCHER: &str = "^(startup|resume|clear|compact|fork)$";

const CLAUDE_CONFIG_DIR_ENV_VAR: &str = "CLAUDE_CONFIG_DIR";
const HOOK_TIMEOUT_SECS: u64 = 10;

/// Outcome of a claude hook install.
#[derive(Debug)]
pub struct ClaudeHookInstall {
    /// The settings file that was merged into.
    pub settings_path: PathBuf,
    /// Where the hook script asset was written.
    pub hook_path: PathBuf,
    /// Whether the settings file changed (false = our entry was already
    /// present; the asset is still refreshed).
    pub settings_changed: bool,
}

/// Outcome of a claude hook uninstall.
#[derive(Debug)]
pub struct ClaudeHookUninstall {
    pub settings_path: PathBuf,
    pub hook_path: PathBuf,
    pub settings_changed: bool,
    /// Whether the hook script asset was actually ours to delete.
    pub hook_removed: bool,
}

/// Resolve the claude settings path (`$CLAUDE_CONFIG_DIR` honored, else
/// `~/.claude/settings.json`).
pub fn claude_settings_path() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CLAUDE_CONFIG_DIR_ENV_VAR).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir).join("settings.json"));
    }
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

/// Where the hook script asset lives: par-term's own config directory, the
/// same tree the shell integration scripts are written to.
pub fn hook_asset_path() -> PathBuf {
    Config::config_dir()
        .join("hooks")
        .join(CLAUDE_HOOK_INSTALL_NAME)
}

/// Install the claude session hook: write the script asset and merge the
/// SessionStart entry into the user's settings.
pub fn install_claude_hook() -> io::Result<ClaudeHookInstall> {
    install_claude_hook_into(&claude_settings_path()?, &hook_asset_path())
}

/// Install with explicit paths (test/install seam).
pub fn install_claude_hook_into(
    settings_path: &Path,
    hook_path: &Path,
) -> io::Result<ClaudeHookInstall> {
    // The agent must be installed: its config directory has to exist. A
    // missing settings FILE is fine (claude creates it on demand), a missing
    // DIRECTORY is an install-claude-first condition, not a silent skip.
    let agent_dir = settings_path.parent().ok_or_else(|| {
        io::Error::other(format!(
            "claude settings path has no parent: {settings_path:?}"
        ))
    })?;
    if !agent_dir.is_dir() {
        return Err(io::Error::other(format!(
            "claude settings directory not found at {}. install claude first",
            agent_dir.display()
        )));
    }

    let content = match fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => "{}\n".to_string(),
        Err(err) => return Err(err),
    };

    // Merge and verify BEFORE writing anything: a malformed user config must
    // fail with the file — and the asset — untouched.
    let command = hook_command_for(hook_path);
    let merged = merge_claude_settings(&content, settings_path, &command)?;

    write_hook_asset(hook_path)?;

    if let Some(updated) = &merged {
        save_settings(settings_path, updated)?;
    }

    Ok(ClaudeHookInstall {
        settings_path: settings_path.to_path_buf(),
        hook_path: hook_path.to_path_buf(),
        settings_changed: merged.is_some(),
    })
}

/// Uninstall the claude session hook: remove our entries from settings and
/// delete the script asset when it is ours.
pub fn uninstall_claude_hook() -> io::Result<ClaudeHookUninstall> {
    uninstall_claude_hook_into(&claude_settings_path()?, &hook_asset_path())
}

/// Uninstall with explicit paths (test/install seam).
pub fn uninstall_claude_hook_into(
    settings_path: &Path,
    hook_path: &Path,
) -> io::Result<ClaudeHookUninstall> {
    let mut settings_changed = false;
    if settings_path.is_file() {
        let content = fs::read_to_string(settings_path)?;
        let command = hook_command_for(hook_path);
        if let Some(updated) = unmerge_claude_settings(&content, settings_path, &command)? {
            save_settings(settings_path, &updated)?;
            settings_changed = true;
        }
    }

    let hook_removed = remove_hook_asset(hook_path)?;

    Ok(ClaudeHookUninstall {
        settings_path: settings_path.to_path_buf(),
        hook_path: hook_path.to_path_buf(),
        settings_changed,
        hook_removed,
    })
}

/// The settings entry's command string. The path is shell-quoted only when it
/// needs it, so the common case stays the readable bare path.
fn hook_command_for(hook_path: &Path) -> String {
    let text = hook_path.display().to_string();
    let safe = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_/.:=@%+".contains(c));
    if safe {
        text
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

fn write_hook_asset(hook_path: &Path) -> io::Result<()> {
    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(hook_path, CLAUDE_HOOK_ASSET)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hook_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Delete the hook asset only when it carries our marker: a same-named file
/// without one is a user file we must not touch.
fn remove_hook_asset(hook_path: &Path) -> io::Result<bool> {
    if !hook_path.is_file() {
        return Ok(false);
    }
    if !fs::read_to_string(hook_path)?.contains(CLAUDE_HOOK_MARKER) {
        return Ok(false);
    }
    fs::remove_file(hook_path)?;
    Ok(true)
}

fn save_settings(settings_path: &Path, contents: &str) -> io::Result<()> {
    crate::atomic_save::save_string_atomic_preserving_mode(settings_path, contents).map_err(|err| {
        io::Error::other(format!(
            "failed to write {}: {err:#}",
            settings_path.display()
        ))
    })
}

/// Settings files are jsonc in practice: comments and trailing commas are
/// part of what claude accepts, everything looser is not.
fn parse_options() -> ParseOptions {
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

fn parse_root_object<'a>(content: &'a str, settings_path: &Path) -> io::Result<AstObject<'a>> {
    let parsed = parse_to_ast(content, &Default::default(), &parse_options()).map_err(|err| {
        io::Error::other(format!(
            "failed to parse {}: {err}",
            settings_path.display()
        ))
    })?;
    match parsed.value {
        Some(AstValue::Object(object)) => Ok(object),
        _ => Err(io::Error::other(format!(
            "claude settings at {} must be a JSON object",
            settings_path.display()
        ))),
    }
}

fn parse_serde(content: &str, settings_path: &Path) -> io::Result<JsonValue> {
    parse_to_serde_value(content, &parse_options()).map_err(|err| {
        io::Error::other(format!(
            "failed to parse {}: {err}",
            settings_path.display()
        ))
    })
}

/// Duplicate keys make the file ambiguous — which value survives a merge is
/// not ours to decide, so refuse.
fn reject_duplicate_keys(object: &AstObject, settings_path: &Path) -> io::Result<()> {
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

fn prop_name_text<'a>(name: &'a ObjectPropName<'a>) -> Option<&'a str> {
    match name {
        ObjectPropName::String(lit) => Some(lit.value.as_ref()),
        ObjectPropName::Word(word) => Some(word.value),
    }
}

fn string_value<'a>(value: &'a AstValue<'_>) -> Option<&'a str> {
    match value {
        AstValue::StringLit(lit) => Some(lit.value.as_ref()),
        _ => None,
    }
}

/// The canonical SessionStart entry for `command`, as compact JSON text (used
/// for the splice) and as a value (used for the presence check and verify).
fn canonical_entry_json(command: &str) -> String {
    let command_json = serde_json::to_string(command).expect("command is serializable");
    format!(
        "{{\"matcher\":\"{SESSION_START_MATCHER}\",\"hooks\":[{{\"type\":\"command\",\"command\":{command_json},\"timeout\":{HOOK_TIMEOUT_SECS}}}]}}"
    )
}

fn canonical_entry_value(command: &str) -> JsonValue {
    json!({
        "matcher": SESSION_START_MATCHER,
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": HOOK_TIMEOUT_SECS,
        }],
    })
}

/// Whether any SessionStart group already carries our exact command.
fn has_command(content: &str, settings_path: &Path, command: &str) -> io::Result<bool> {
    let value = parse_serde(content, settings_path)?;
    Ok(value
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
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

/// Merge our SessionStart entry into `content`. `Ok(None)` = already present,
/// byte-exact no-op.
fn merge_claude_settings(
    content: &str,
    settings_path: &Path,
    command: &str,
) -> io::Result<Option<String>> {
    let root = parse_root_object(content, settings_path)?;
    reject_duplicate_keys(&root, settings_path)?;
    if has_command(content, settings_path, command)? {
        return Ok(None);
    }

    let canonical = canonical_entry_json(command);
    let updated = match root.get_object("hooks") {
        Some(hooks) => match hooks.get_array("SessionStart") {
            Some(session_start) => append_array_element(content, session_start, &canonical),
            None => {
                append_object_property(content, hooks, "SessionStart", &format!("[{canonical}]"))
            }
        },
        None => append_object_property(
            content,
            &root,
            "hooks",
            &format!("{{\"SessionStart\":[{canonical}]}}"),
        ),
    };

    // Fail closed: the spliced text must parse back to exactly the desired
    // value before the caller is allowed to write it. The desired value
    // mirrors the splice branch above.
    let mut desired = parse_serde(content, settings_path)?;
    match desired
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("SessionStart"))
        .and_then(JsonValue::as_array_mut)
    {
        Some(entries) => entries.push(canonical_entry_value(command)),
        None => {
            let Some(root_object) = desired.as_object_mut() else {
                return Err(io::Error::other(format!(
                    "claude settings at {} must be a JSON object",
                    settings_path.display()
                )));
            };
            match root_object.get_mut("hooks") {
                Some(hooks_value) => {
                    let Some(hooks_object) = hooks_value.as_object_mut() else {
                        return Err(io::Error::other(format!(
                            "claude settings hooks at {} must be a JSON object",
                            settings_path.display()
                        )));
                    };
                    hooks_object.insert(
                        "SessionStart".to_string(),
                        json!([canonical_entry_value(command)]),
                    );
                }
                None => {
                    root_object.insert(
                        "hooks".to_string(),
                        json!({"SessionStart": [canonical_entry_value(command)]}),
                    );
                }
            }
        }
    }
    verify_update(updated, settings_path, &desired).map(Some)
}

/// Remove our command entries from `content`. `Ok(None)` = nothing to remove.
fn unmerge_claude_settings(
    content: &str,
    settings_path: &Path,
    command: &str,
) -> io::Result<Option<String>> {
    let root = parse_root_object(content, settings_path)?;
    reject_duplicate_keys(&root, settings_path)?;
    let original = parse_serde(content, settings_path)?;

    let mut updated = content.to_string();
    let mut changed = false;

    // One cut per pass, re-parsing between cuts so every span is computed
    // against the text it is applied to. Settings files are small.
    while let Some(cut) = find_next_our_cut(&updated, settings_path, command)? {
        let mut next = String::with_capacity(updated.len());
        next.push_str(&updated[..cut.start]);
        next.push_str(&updated[cut.end..]);
        updated = next;
        changed = true;
    }

    if !changed {
        return Ok(None);
    }

    // Prune containers our removals emptied: SessionStart, then hooks. Each
    // prune is its own re-parsed pass for the same span-safety reason.
    while let Some(cut) = find_next_empty_container_cut(&updated, settings_path)? {
        let mut next = String::with_capacity(updated.len());
        next.push_str(&updated[..cut.start]);
        next.push_str(&updated[cut.end..]);
        updated = next;
    }

    let desired = desired_after_removal(original, command);
    verify_update(updated, settings_path, &desired).map(Some)
}

/// The next single removal to make: a whole group when the group holds only
/// our commands, else one of our command entries inside a mixed group.
fn find_next_our_cut(
    content: &str,
    settings_path: &Path,
    command: &str,
) -> io::Result<Option<Range>> {
    let root = parse_root_object(content, settings_path)?;
    let Some(hooks) = root.get_object("hooks") else {
        return Ok(None);
    };
    let Some(session_start) = hooks.get_array("SessionStart") else {
        return Ok(None);
    };

    for (group_index, group) in session_start.elements.iter().enumerate() {
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
                    .and_then(|prev| session_start.elements.get(prev))
                    .map(Ranged::range)
                    .map(|range| range.end),
                group_range.start,
                group_range.end,
                session_start
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
    Ok(None)
}

/// The next empty-container prune: a SessionStart array with no elements
/// (drop the property from hooks), or a hooks object with no properties
/// (drop the property from root).
fn find_next_empty_container_cut(content: &str, settings_path: &Path) -> io::Result<Option<Range>> {
    let root = parse_root_object(content, settings_path)?;
    let Some(hooks) = root.get_object("hooks") else {
        return Ok(None);
    };
    if hooks.properties.is_empty() {
        return Ok(Some(property_cut(&root.properties, "hooks")));
    }
    let Some(session_start) = hooks.get_array("SessionStart") else {
        return Ok(None);
    };
    if !session_start.elements.is_empty() {
        return Ok(None);
    }
    Ok(Some(property_cut(&hooks.properties, "SessionStart")))
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
/// emptied containers pruned.
fn desired_after_removal(mut value: JsonValue, command: &str) -> JsonValue {
    let Some(root) = value.as_object_mut() else {
        return value;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(JsonValue::as_object_mut) else {
        return value;
    };
    if let Some(session_start) = hooks
        .get_mut("SessionStart")
        .and_then(JsonValue::as_array_mut)
    {
        let mut kept_groups = Vec::with_capacity(session_start.len());
        for group in session_start.drain(..) {
            let mut group = group;
            let pruned_empty = match group.get_mut("hooks").and_then(JsonValue::as_array_mut) {
                Some(entries) => {
                    entries.retain(|entry| {
                        entry.get("command").and_then(JsonValue::as_str) != Some(command)
                    });
                    entries.is_empty()
                }
                None => false,
            };
            if !pruned_empty {
                kept_groups.push(group);
            }
        }
        *session_start = kept_groups;
        if session_start.is_empty() {
            hooks.remove("SessionStart");
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
            "failed to safely update claude settings at {}",
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

// --- path helpers (same resolution rules as the extension installer) ---

fn home_dir() -> io::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| io::Error::other("Could not determine home directory"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    /// A realistic claude settings.json: comments, nested config, and a user
    /// who already has hooks of their own.
    const REAL_WORLD_SETTINGS: &str = r#"{
  // personal settings
  "model": "opus",
  "permissions": {
    "allow": ["Bash(ls:*)"],
    "deny": []
  },
  "statusLine": {
    "type": "command",
    "command": "~/.claude/statusline.sh"
  },
  "hooks": {
    // existing user hooks
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/usr/local/bin/lint-hook",
            "timeout": 30
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": "startup",
        "hooks": [
          {
            "type": "command",
            "command": "/usr/local/bin/motd"
          }
        ]
      }
    ]
  }
}
"#;

    fn installed_command(hook_path: &Path) -> String {
        hook_command_for(hook_path)
    }

    fn session_start_commands(content: &str) -> Vec<String> {
        let value: JsonValue = parse_serde(content, Path::new("<test>")).expect("parses");
        value
            .get("hooks")
            .and_then(|hooks| hooks.get("SessionStart"))
            .and_then(JsonValue::as_array)
            .expect("SessionStart array")
            .iter()
            .flat_map(|group| {
                group
                    .get("hooks")
                    .and_then(JsonValue::as_array)
                    .map(|entries| {
                        entries
                            .iter()
                            .filter_map(|entry| entry.get("command").and_then(JsonValue::as_str))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .map(String::from)
            .collect()
    }

    #[test]
    fn install_into_a_fresh_directory_creates_settings_and_asset() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("hooks").join(CLAUDE_HOOK_INSTALL_NAME);

        let result = install_claude_hook_into(&settings, &hook).unwrap();

        assert!(result.settings_changed, "a fresh file is a change");
        let content = fs::read_to_string(&settings).unwrap();
        let commands = session_start_commands(&content);
        assert_eq!(commands, vec![installed_command(&hook)]);

        let asset = fs::read_to_string(&hook).unwrap();
        assert_eq!(asset, CLAUDE_HOOK_ASSET);
        assert!(asset.contains(CLAUDE_HOOK_MARKER));
        assert!(asset.contains("pane.report_agent_session"));
        assert!(!asset.contains("HERDR_"), "fully env-renamed port");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&hook).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "the hook must be executable");
        }
    }

    #[test]
    fn install_preserves_user_entries_comments_and_formatting() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();

        install_claude_hook_into(&settings, &hook).unwrap();

        let updated = fs::read_to_string(&settings).unwrap();
        // Every line of the user's file survives verbatim.
        for line in REAL_WORLD_SETTINGS.lines() {
            assert!(
                updated.contains(line),
                "user line must survive untouched: {line}"
            );
        }
        // Our entry was added alongside the user's, not instead of it.
        let mut commands = session_start_commands(&updated);
        commands.sort();
        let mut expected = vec!["/usr/local/bin/motd".to_string(), installed_command(&hook)];
        expected.sort();
        assert_eq!(commands, expected);
        // The matcher scopes us to claude's sources, not grok's.
        assert!(updated.contains(SESSION_START_MATCHER));
    }

    #[test]
    fn install_is_a_byte_exact_noop_when_already_installed() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();

        install_claude_hook_into(&settings, &hook).unwrap();
        let once = fs::read_to_string(&settings).unwrap();

        let second = install_claude_hook_into(&settings, &hook).unwrap();
        assert!(!second.settings_changed, "reinstall is a no-op");
        assert_eq!(fs::read_to_string(&settings).unwrap(), once);
    }

    #[test]
    fn install_is_still_a_noop_after_the_user_reformats() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();

        install_claude_hook_into(&settings, &hook).unwrap();

        // The user pretty-prints the whole file (comments are lost to their
        // tool, our command survives): reinstall must not duplicate it.
        let value: JsonValue =
            parse_serde(&fs::read_to_string(&settings).unwrap(), Path::new("<test>")).unwrap();
        fs::write(&settings, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        let reformatted = fs::read_to_string(&settings).unwrap();

        let second = install_claude_hook_into(&settings, &hook).unwrap();
        assert!(!second.settings_changed, "presence is matched by command");
        assert_eq!(fs::read_to_string(&settings).unwrap(), reformatted);
        assert_eq!(
            session_start_commands(&reformatted)
                .iter()
                .filter(|c| **c == installed_command(&hook))
                .count(),
            1,
            "exactly one of our entries"
        );
    }

    #[test]
    fn install_rejects_duplicate_keys_and_writes_nothing() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        let original = "{\n  \"model\": \"a\",\n  \"model\": \"b\"\n}\n";
        fs::write(&settings, original).unwrap();

        let err = install_claude_hook_into(&settings, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("duplicate key"),
            "error names the problem: {err}"
        );
        assert_eq!(fs::read_to_string(&settings).unwrap(), original);
        assert!(
            !hook.exists(),
            "not even the asset is written on a malformed config"
        );
    }

    #[test]
    fn install_rejects_structurally_invalid_content() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        let original = "{\"hooks\":";
        fs::write(&settings, original).unwrap();

        let err = install_claude_hook_into(&settings, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("failed to parse"),
            "error names the file: {err}"
        );
        assert_eq!(fs::read_to_string(&settings).unwrap(), original);
        assert!(!hook.exists());
    }

    #[test]
    fn missing_claude_directory_is_an_actionable_error() {
        let root = temp_root();
        let settings = root.path().join("never").join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");

        let err = install_claude_hook_into(&settings, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("claude settings directory not found at"),
            "error must name the path: {err}"
        );
        assert!(
            err.contains("install claude first"),
            "error must say what to do: {err}"
        );
    }

    #[test]
    fn uninstall_removes_only_our_entries() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();
        install_claude_hook_into(&settings, &hook).unwrap();

        let result = uninstall_claude_hook_into(&settings, &hook).unwrap();

        assert!(result.settings_changed);
        assert!(result.hook_removed);
        let after = fs::read_to_string(&settings).unwrap();
        // The user's file is byte-identical to before we ever touched it.
        assert_eq!(after, REAL_WORLD_SETTINGS);
        assert!(!hook.exists());
    }

    #[test]
    fn uninstall_removes_our_entry_from_a_mixed_group_and_keeps_the_users() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        let command = installed_command(&hook);
        // A group the user built that carries BOTH their command and ours.
        let mixed = format!(
            r#"{{"hooks":{{"SessionStart":[{{"matcher":"*", "hooks":[{{"type":"command","command":"/usr/local/bin/motd"}},{{"type":"command","command":"{command}"}}]}}]}}}}"#
        );
        fs::write(&settings, mixed).unwrap();

        uninstall_claude_hook_into(&settings, &hook).unwrap();

        let after = fs::read_to_string(&settings).unwrap();
        assert_eq!(
            session_start_commands(&after),
            vec!["/usr/local/bin/motd".to_string()],
            "the user's entry survives, ours is gone"
        );
    }

    #[test]
    fn uninstall_noops_when_absent_and_leaves_foreign_assets_alone() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();
        // A same-named asset that is NOT ours.
        fs::write(&hook, "// user's own script, no marker\n").unwrap();

        let result = uninstall_claude_hook_into(&settings, &hook).unwrap();

        assert!(!result.settings_changed);
        assert!(
            !result.hook_removed,
            "a file without our marker is not ours"
        );
        assert_eq!(fs::read_to_string(&settings).unwrap(), REAL_WORLD_SETTINGS);
        assert_eq!(
            fs::read_to_string(&hook).unwrap(),
            "// user's own script, no marker\n"
        );
    }

    #[test]
    fn install_preserves_the_settings_file_mode() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("par-mux-claude-session-hook.sh");
        fs::write(&settings, REAL_WORLD_SETTINGS).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&settings, fs::Permissions::from_mode(0o600)).unwrap();

            install_claude_hook_into(&settings, &hook).unwrap();

            let mode = fs::metadata(&settings).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "the atomic rewrite preserves the mode");
        }
    }

    #[test]
    fn a_hook_path_with_spaces_is_shell_quoted() {
        let root = temp_root();
        let settings = root.path().join("settings.json");
        let hook = root.path().join("my hooks").join(CLAUDE_HOOK_INSTALL_NAME);

        install_claude_hook_into(&settings, &hook).unwrap();

        let content = fs::read_to_string(&settings).unwrap();
        let commands = session_start_commands(&content);
        assert_eq!(commands.len(), 1);
        assert!(
            commands[0].starts_with('\'') && commands[0].ends_with('\''),
            "spaced path is quoted: {}",
            commands[0]
        );
        // The quoted command still parses back out of the JSON intact.
        assert!(commands[0].contains("my hooks"));
    }
}
