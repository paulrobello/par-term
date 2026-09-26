//! codex arm: codex reads claude-format hooks from `~/.codex/hooks.json`,
//! gated by a `[features] hooks = true` flag in `~/.codex/config.toml`
//! (herdr's proven model). Installing merges a matcher-less SessionStart
//! entry into hooks.json with the engine in [`super`] and enables the flag
//! with a comment-preserving line edit; uninstall removes the entries and
//! the script but leaves the flag — without hook entries it enables nothing.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    has_command, home_dir, hook_asset_path as asset_path_for, hook_command_with_action,
    merge_hook_event, remove_marked_file, save_settings, unmerge_hook_entries, write_hook_asset,
};

const CODEX_HOOK_ASSET_POSIX: &str = include_str!("../../mux_hooks/par-mux-codex-session-hook.sh");
const CODEX_HOOK_ASSET_WINDOWS: &str =
    include_str!("../../mux_hooks/par-mux-codex-session-hook.ps1");

/// The asset file name for the current platform — the `.ps1` port on
/// Windows, the `.sh` reporter elsewhere. Runtime-selected (not cfg-split)
/// so both arms compile on every platform and the selection is assertable
/// in tests instead of invisible to the non-Windows build.
fn codex_hook_install_name() -> &'static str {
    if cfg!(windows) {
        "par-mux-codex-session-hook.ps1"
    } else {
        "par-mux-codex-session-hook.sh"
    }
}

/// The hook asset contents for the current platform.
fn codex_hook_asset() -> &'static str {
    if cfg!(windows) {
        CODEX_HOOK_ASSET_WINDOWS
    } else {
        CODEX_HOOK_ASSET_POSIX
    }
}

/// Marker identifying the installed codex script as ours.
pub const CODEX_HOOK_MARKER: &str = "PAR_MUX_INTEGRATION_ID=codex";

const CODEX_HOME_ENV_VAR: &str = "CODEX_HOME";
const CODEX_HOOKS_FILE_NAME: &str = "hooks.json";
const CODEX_CONFIG_FILE_NAME: &str = "config.toml";

/// Outcome of a codex hook install.
#[derive(Debug)]
pub struct CodexHookInstall {
    /// Where the hook script asset was written.
    pub hook_path: PathBuf,
    /// The claude-format hooks file that was merged into.
    pub hooks_path: PathBuf,
    /// The TOML config whose `[features] hooks` flag was enabled.
    pub config_path: PathBuf,
    /// Whether hooks.json changed (false = our entry was already present).
    pub hooks_changed: bool,
    /// Whether config.toml changed (false = the flag was already enabled).
    pub config_changed: bool,
}

/// Outcome of a codex hook uninstall.
#[derive(Debug)]
pub struct CodexHookUninstall {
    pub hook_path: PathBuf,
    pub hooks_path: PathBuf,
    /// The flag in this file is deliberately left behind — see the module doc.
    pub config_path: PathBuf,
    pub hooks_changed: bool,
    pub hook_removed: bool,
}

/// Resolve the codex config home (`$CODEX_HOME`, which codex honors, else
/// `~/.codex`).
pub fn codex_config_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CODEX_HOME_ENV_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    Ok(home_dir()?.join(".codex"))
}

/// Where the hook script asset lives: par-term's own config directory, the
/// same tree the other hook scripts are written to.
fn hook_asset_path() -> PathBuf {
    asset_path_for(codex_hook_install_name())
}

/// Install the codex session hook: write the script asset, merge the
/// SessionStart entry into hooks.json, and enable `[features] hooks`.
pub fn install_codex_hook() -> io::Result<CodexHookInstall> {
    install_codex_hook_into(&codex_config_dir()?, &hook_asset_path())
}

/// Install with explicit paths (test/install seam).
pub fn install_codex_hook_into(codex_dir: &Path, hook_path: &Path) -> io::Result<CodexHookInstall> {
    if !codex_dir.is_dir() {
        return Err(io::Error::other(format!(
            "codex config directory not found at {}. install codex first",
            codex_dir.display()
        )));
    }
    let hooks_path = codex_dir.join(CODEX_HOOKS_FILE_NAME);
    let config_path = codex_dir.join(CODEX_CONFIG_FILE_NAME);

    let hooks_content = read_or_default(&hooks_path, "{}\n")?;
    let config_content = read_or_default(&config_path, "")?;

    // Merge and verify BOTH files before writing anything: a malformed user
    // config must fail with every file — and the asset — untouched.
    let command = codex_hook_command(hook_path);
    let merged = merge_hook_event(&hooks_content, &hooks_path, &command, None, "SessionStart")?;
    let config_updated = ensure_features_hooks_true(&config_content, &config_path)?;

    write_hook_asset(hook_path, codex_hook_asset())?;

    if let Some(updated) = &merged {
        save_settings(&hooks_path, updated)?;
    }
    if let Some(updated) = &config_updated {
        save_settings(&config_path, updated)?;
    }

    Ok(CodexHookInstall {
        hook_path: hook_path.to_path_buf(),
        hooks_path,
        config_path,
        hooks_changed: merged.is_some(),
        config_changed: config_updated.is_some(),
    })
}

/// Uninstall the codex session hook: remove our entries from hooks.json and
/// delete the script asset when it is ours. The `[features] hooks` flag in
/// config.toml is left behind: without hook entries it enables nothing, and
/// removing a flag the user may have set themselves is not ours to decide.
pub fn uninstall_codex_hook() -> io::Result<CodexHookUninstall> {
    uninstall_codex_hook_into(&codex_config_dir()?, &hook_asset_path())
}

/// Uninstall with explicit paths (test/install seam).
pub fn uninstall_codex_hook_into(
    codex_dir: &Path,
    hook_path: &Path,
) -> io::Result<CodexHookUninstall> {
    let hooks_path = codex_dir.join(CODEX_HOOKS_FILE_NAME);
    let config_path = codex_dir.join(CODEX_CONFIG_FILE_NAME);

    let mut hooks_changed = false;
    if hooks_path.is_file() {
        let content = fs::read_to_string(&hooks_path)?;
        let command = codex_hook_command(hook_path);
        if let Some(updated) =
            unmerge_hook_entries(&content, &hooks_path, &[("SessionStart", command.as_str())])?
        {
            save_settings(&hooks_path, &updated)?;
            hooks_changed = true;
        }
    }

    let hook_removed = remove_marked_file(hook_path, CODEX_HOOK_MARKER)?;

    Ok(CodexHookUninstall {
        hook_path: hook_path.to_path_buf(),
        hooks_path,
        config_path,
        hooks_changed,
        hook_removed,
    })
}

/// The hooks.json command: `sh <script> session` on POSIX, herdr's
/// powershell -File form on Windows — both always quote the path (the
/// grok-arm heritage).
fn codex_hook_command(hook_path: &Path) -> String {
    hook_command_with_action(hook_path, "session")
}

fn read_or_default(path: &Path, default: &str) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(default.to_string()),
        Err(err) => Err(err),
    }
}

/// Enable `[features] hooks = true` in codex config.toml text. `Ok(None)` =
/// already enabled, byte-exact no-op. The edit is a line splice that never
/// moves untouched bytes, and the result is verified by a real TOML parse
/// before the caller is allowed to write it.
fn ensure_features_hooks_true(content: &str, config_path: &Path) -> io::Result<Option<String>> {
    // Parse before touching: a config that is not valid TOML is a clear
    // error with nothing written, mirroring the jsonc arms.
    let parsed = toml::from_str::<toml::Table>(content).map_err(|err| {
        io::Error::other(format!("failed to parse {}: {err}", config_path.display()))
    })?;
    if features_hooks_enabled(&parsed) {
        return Ok(None);
    }

    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let trailing_newline = content.ends_with('\n');
    let mut features_header_index = None;
    let mut hooks_line_index = None;
    let mut in_features = false;

    for (index, line) in lines.iter().enumerate() {
        if let Some(header) = toml_table_header(line) {
            in_features = header == "[features]";
            if in_features && features_header_index.is_none() {
                features_header_index = Some(index);
            }
            continue;
        }
        if in_features && hooks_line_index.is_none() && is_toml_key(line, "hooks") {
            hooks_line_index = Some(index);
        }
    }

    let updated = if let Some(hooks_index) = hooks_line_index {
        let line = &lines[hooks_index];
        let comment = toml_comment_start(line).map(str::to_string);
        lines[hooks_index] = match comment {
            Some(comment) => format!("hooks = true {comment}"),
            None => "hooks = true".to_string(),
        };
        join_toml_lines(lines, trailing_newline)
    } else if let Some(header_index) = features_header_index {
        lines.insert(header_index + 1, "hooks = true".to_string());
        join_toml_lines(lines, trailing_newline)
    } else {
        let mut result = content.trim_end_matches('\n').to_string();
        if !result.is_empty() {
            result.push('\n');
            result.push('\n');
        }
        result.push_str("[features]\nhooks = true\n");
        result
    };

    // Fail closed: the edited text must parse back with the flag enabled
    // before the caller is allowed to write it. A dotted
    // `features = { hooks = false }` conflicts with the appended table and
    // fails here — correctly, since a line edit cannot flip that form.
    let reparsed = toml::from_str::<toml::Table>(&updated).map_err(|err| {
        io::Error::other(format!(
            "failed to safely update codex config at {}: {err}",
            config_path.display()
        ))
    })?;
    if !features_hooks_enabled(&reparsed) {
        return Err(io::Error::other(format!(
            "failed to safely update codex config at {}",
            config_path.display()
        )));
    }
    Ok(Some(updated))
}

fn features_hooks_enabled(config: &toml::Table) -> bool {
    config
        .get("features")
        .and_then(|features| features.get("hooks"))
        .and_then(|hooks| hooks.as_bool())
        == Some(true)
}

fn join_toml_lines(lines: Vec<String>, trailing_newline: bool) -> String {
    let mut result = lines.join("\n");
    if trailing_newline || result.is_empty() {
        result.push('\n');
    }
    result
}

/// The table header of a TOML line (`[features]`, `[[hooks]]`), or None for
/// anything else — including a `[features]` inside brackets-quoted keys, which
/// codex config does not use. Ported from herdr's codex integration.
fn toml_table_header(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || !trimmed.starts_with('[') {
        return None;
    }

    let header_end = if trimmed.starts_with("[[") {
        trimmed.find("]]").map(|index| index + 2)?
    } else {
        trimmed.find(']').map(|index| index + 1)?
    };
    let header = &trimmed[..header_end];
    let rest = trimmed[header_end..].trim_start();
    if !rest.is_empty() && !rest.starts_with('#') {
        return None;
    }

    Some(header)
}

/// Whether a line assigns to `key` at table-body level (`key = ...`).
/// `codex_hooks` and `hooks_extra` do not match `hooks`. Ported from herdr.
fn is_toml_key(line: &str, key: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || !trimmed.starts_with(key) {
        return false;
    }

    trimmed[key.len()..].trim_start().starts_with('=')
}

/// The ` # comment` suffix of a TOML value, honoring quotes so a `#` inside a
/// string is not mistaken for one. Ported from herdr.
fn toml_comment_start(value: &str) -> Option<&str> {
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '#' if index == 0 || value[..index].ends_with(char::is_whitespace) => {
                return Some(&value[index..]);
            }
            _ => {}
        }
    }

    None
}

/// Startup self-heal: rewrite the script asset when hooks.json still carries
/// our entry but the script is gone — the hooks directory has repeatedly
/// been deleted out from under live registrations while the entries in the
/// agent's own config survived.
pub fn heal_codex_hook_asset() {
    let Ok(codex_dir) = codex_config_dir() else {
        return; // no home to resolve against — nothing to heal
    };
    if let Err(err) = heal_codex_hook_asset_into(&codex_dir, &hook_asset_path()) {
        log::warn!("codex mux-hook heal skipped: {err}");
    }
}

/// Heal with explicit paths (test seam). `Ok(())` in every skip case:
/// script present, agent not installed, or our entry not registered. A
/// malformed hooks.json surfaces as an error with nothing written. Neither
/// hooks.json nor config.toml is ever touched.
pub(crate) fn heal_codex_hook_asset_into(codex_dir: &Path, hook_path: &Path) -> io::Result<()> {
    if hook_path.exists() {
        return Ok(());
    }
    let hooks_path = codex_dir.join(CODEX_HOOKS_FILE_NAME);
    let Ok(content) = fs::read_to_string(&hooks_path) else {
        return Ok(()); // agent not installed — nothing registers the script
    };
    let command = codex_hook_command(hook_path);
    if has_command(&content, &hooks_path, &command, "SessionStart")? {
        write_hook_asset(hook_path, codex_hook_asset())?;
        // The deleter is unidentified; this line dates any recurrence in the debug log.
        log::info!("mux hook heal: restored {}", hook_path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;
    use std::fs;
    use std::path::Path;

    use super::super::parse_serde;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn heal_rewrites_a_deleted_script_and_leaves_agent_files_untouched() {
        let root = temp_root();
        let codex_dir = root.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), REAL_WORLD_CONFIG).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        let hook = root.path().join("par-mux-codex-session-hook.sh");
        install_codex_hook_into(&codex_dir, &hook).unwrap();
        let hooks_after_install = fs::read_to_string(codex_dir.join("hooks.json")).unwrap();
        let config_after_install = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        fs::remove_file(&hook).unwrap();

        heal_codex_hook_asset_into(&codex_dir, &hook).unwrap();

        assert_eq!(fs::read_to_string(&hook).unwrap(), codex_hook_asset());
        assert_eq!(
            fs::read_to_string(codex_dir.join("hooks.json")).unwrap(),
            hooks_after_install
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            config_after_install
        );
    }

    #[test]
    fn heal_does_not_create_a_script_without_registration() {
        let root = temp_root();
        let codex_dir = root.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        let hook = root.path().join("par-mux-codex-session-hook.sh");

        heal_codex_hook_asset_into(&codex_dir, &hook).unwrap();

        assert!(!hook.exists());
    }

    /// A realistic codex config.toml: comments, nested tables — including a
    /// profile-scoped features table that must not be mistaken for the
    /// top-level one — and no hooks flag yet.
    const REAL_WORLD_CONFIG: &str = r#"# codex configuration
model = "gpt-5-codex"
notify = ["notify-send", "--app-name", "codex"]

[profiles.work.features]
hooks = false

[features]
# experimental toggles
web_search = true

[mcp_servers.github]
command = "gh"
args = ["mcp", "serve"]
"#;

    /// A hooks.json a user might already have: their own SessionStart hook
    /// and an unrelated event.
    const REAL_WORLD_HOOKS: &str = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/local/bin/codex-motd"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/local/bin/codex-done"
          }
        ]
      }
    ]
  }
}
"#;

    fn installed_command(hook_path: &Path) -> String {
        codex_hook_command(hook_path)
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

    fn features_hooks(config: &str) -> Option<bool> {
        toml::from_str::<toml::Table>(config)
            .expect("parses")
            .get("features")
            .and_then(|f| f.get("hooks"))
            .and_then(|v| v.as_bool())
    }

    #[test]
    fn install_preserves_user_toml_entries_comments_and_formatting() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), REAL_WORLD_CONFIG).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        let result = install_codex_hook_into(&codex_dir, &hook).unwrap();

        assert!(result.hooks_changed && result.config_changed);

        // Every line of the user's TOML survives verbatim.
        let config = fs::read_to_string(&result.config_path).unwrap();
        for line in REAL_WORLD_CONFIG.lines() {
            assert!(
                config.contains(line),
                "user toml line must survive untouched: {line}"
            );
        }
        assert_eq!(features_hooks(&config), Some(true));
        assert_eq!(
            toml::from_str::<toml::Table>(&config)
                .unwrap()
                .get("profiles")
                .and_then(|p| p.get("work"))
                .and_then(|w| w.get("features"))
                .and_then(|f| f.get("hooks"))
                .and_then(|v| v.as_bool()),
            Some(false),
            "the profile-scoped features table is not ours to touch"
        );

        // Every line of the user's hooks.json survives, ours joins theirs.
        let hooks = fs::read_to_string(&result.hooks_path).unwrap();
        for line in REAL_WORLD_HOOKS.lines() {
            assert!(
                hooks.contains(line),
                "user hooks line must survive untouched: {line}"
            );
        }
        let mut commands = session_start_commands(&hooks);
        commands.sort();
        let mut expected = vec![
            "/usr/local/bin/codex-motd".to_string(),
            installed_command(&hook),
        ];
        expected.sort();
        assert_eq!(commands, expected);

        // The entry is matcher-less (codex has no claude matcher space).
        let value: JsonValue = parse_serde(&hooks, Path::new("<test>")).unwrap();
        let ours = &value["hooks"]["SessionStart"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| {
                group
                    .get("hooks")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            entry.get("command").and_then(JsonValue::as_str)
                                == Some(installed_command(&hook).as_str())
                        })
                    })
            })
            .expect("our group");
        assert!(ours.get("matcher").is_none());

        let asset = fs::read_to_string(&hook).unwrap();
        assert_eq!(asset, codex_hook_asset());
        assert!(asset.contains(CODEX_HOOK_MARKER));
        assert!(asset.contains("CODEX_THREAD_ID"));
        let resume_argv = if cfg!(windows) {
            r#"@("codex", "resume", "$sessionId")"#
        } else {
            r#""codex", "resume", session_id"#
        };
        assert!(asset.contains(resume_argv));
        assert!(!asset.contains("HERDR_"), "fully env-renamed port");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&hook).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "the hook must be executable");
        }
    }

    #[test]
    fn a_disabled_flag_is_flipped_and_its_comment_preserved() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(
            codex_dir.join("config.toml"),
            "[features]\nhooks = false # keep me\n",
        )
        .unwrap();
        let hook = root.path().join(codex_hook_install_name());

        install_codex_hook_into(&codex_dir, &hook).unwrap();

        let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        assert_eq!(config, "[features]\nhooks = true # keep me\n");
    }

    #[test]
    fn a_config_without_features_gets_the_table_appended() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
        let hook = root.path().join(codex_hook_install_name());

        install_codex_hook_into(&codex_dir, &hook).unwrap();

        let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        assert!(config.starts_with("model = \"gpt-5\"\n\n[features]\nhooks = true\n"));
        assert_eq!(features_hooks(&config), Some(true));
    }

    #[test]
    fn a_missing_config_and_hooks_file_are_created() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        let result = install_codex_hook_into(&codex_dir, &hook).unwrap();

        assert!(result.hooks_changed && result.config_changed);
        let config = fs::read_to_string(&result.config_path).unwrap();
        assert_eq!(config, "[features]\nhooks = true\n");
        let commands = session_start_commands(&fs::read_to_string(&result.hooks_path).unwrap());
        assert_eq!(commands, vec![installed_command(&hook)]);
    }

    #[test]
    fn reinstall_is_a_byte_exact_noop() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), REAL_WORLD_CONFIG).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        install_codex_hook_into(&codex_dir, &hook).unwrap();
        let config_once = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        let hooks_once = fs::read_to_string(codex_dir.join("hooks.json")).unwrap();

        let second = install_codex_hook_into(&codex_dir, &hook).unwrap();
        assert!(
            !second.hooks_changed && !second.config_changed,
            "reinstall is a no-op on both files"
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            config_once
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("hooks.json")).unwrap(),
            hooks_once
        );
    }

    #[test]
    fn a_dotted_features_table_fails_closed_and_writes_nothing() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        // A line edit cannot flip a dotted/inline features table without
        // creating a duplicate definition — refuse instead.
        fs::write(
            codex_dir.join("config.toml"),
            "model = \"gpt-5\"\nfeatures = { hooks = false }\n",
        )
        .unwrap();
        let original_hooks = "{}\n";
        fs::write(codex_dir.join("hooks.json"), original_hooks).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        let err = install_codex_hook_into(&codex_dir, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("failed to safely update codex config"),
            "error names the file and the refusal: {err}"
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            "model = \"gpt-5\"\nfeatures = { hooks = false }\n"
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("hooks.json")).unwrap(),
            original_hooks
        );
        assert!(!hook.exists(), "not even the asset is written");
    }

    #[test]
    fn malformed_toml_fails_and_writes_nothing() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let original_config = "model = \n";
        fs::write(codex_dir.join("config.toml"), original_config).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        let err = install_codex_hook_into(&codex_dir, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("failed to parse"),
            "error names the file: {err}"
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            original_config
        );
        assert!(!hook.exists());
    }

    #[test]
    fn malformed_hooks_json_fails_and_writes_nothing() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
        let original_hooks = "{oops";
        fs::write(codex_dir.join("hooks.json"), original_hooks).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        let err = install_codex_hook_into(&codex_dir, &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("failed to parse"),
            "error names the file: {err}"
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("hooks.json")).unwrap(),
            original_hooks
        );
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            "model = \"gpt-5\"\n",
            "the verified-but-unwritten config edit never reaches disk"
        );
        assert!(!hook.exists());
    }

    #[test]
    fn missing_codex_directory_is_an_actionable_error() {
        let root = temp_root();
        let hook = root.path().join(codex_hook_install_name());

        let err = install_codex_hook_into(&root.path().join("never"), &hook)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("codex config directory not found at"),
            "error must name the path: {err}"
        );
        assert!(
            err.contains("install codex first"),
            "error must say what to do: {err}"
        );
    }

    #[test]
    fn uninstall_restores_hooks_json_and_leaves_the_flag() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), REAL_WORLD_CONFIG).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        let hook = root.path().join(codex_hook_install_name());
        install_codex_hook_into(&codex_dir, &hook).unwrap();

        let result = uninstall_codex_hook_into(&codex_dir, &hook).unwrap();

        assert!(result.hooks_changed && result.hook_removed);
        // hooks.json is byte-identical to before we ever touched it.
        assert_eq!(
            fs::read_to_string(&result.hooks_path).unwrap(),
            REAL_WORLD_HOOKS
        );
        // The flag stays: without hook entries it enables nothing, and it may
        // predate our install.
        assert_eq!(
            features_hooks(&fs::read_to_string(&result.config_path).unwrap()),
            Some(true)
        );
        assert!(!hook.exists());
    }

    #[test]
    fn uninstall_removes_our_entry_from_a_mixed_group_and_keeps_the_users() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let command = installed_command(&root.path().join(codex_hook_install_name()));
        // A group the user built that carries BOTH their command and ours.
        let mixed = format!(
            r#"{{"hooks":{{"SessionStart":[{{"hooks":[{{"type":"command","command":"/usr/local/bin/codex-motd"}},{{"type":"command","command":{command}}}]}}]}}}}"#,
            command = serde_json::to_string(&command).unwrap()
        );
        fs::write(codex_dir.join("hooks.json"), mixed).unwrap();
        let hook = root.path().join(codex_hook_install_name());

        uninstall_codex_hook_into(&codex_dir, &hook).unwrap();

        let after = fs::read_to_string(codex_dir.join("hooks.json")).unwrap();
        assert_eq!(
            session_start_commands(&after),
            vec!["/usr/local/bin/codex-motd".to_string()],
            "the user's entry survives, ours is gone"
        );
    }

    #[test]
    fn uninstall_noops_when_absent_and_leaves_foreign_assets_alone() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        // A same-named asset that is NOT ours.
        let hook = root.path().join(codex_hook_install_name());
        fs::write(&hook, "// user's own script, no marker\n").unwrap();

        let result = uninstall_codex_hook_into(&codex_dir, &hook).unwrap();

        assert!(!result.hooks_changed);
        assert!(
            !result.hook_removed,
            "a file without our marker is not ours"
        );
        assert_eq!(
            fs::read_to_string(&result.hooks_path).unwrap(),
            REAL_WORLD_HOOKS
        );
        assert_eq!(
            fs::read_to_string(&hook).unwrap(),
            "// user's own script, no marker\n"
        );
    }

    #[test]
    fn install_preserves_both_files_modes() {
        let root = temp_root();
        let codex_dir = root.path().join("codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), REAL_WORLD_CONFIG).unwrap();
        fs::write(codex_dir.join("hooks.json"), REAL_WORLD_HOOKS).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                codex_dir.join("config.toml"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            fs::set_permissions(
                codex_dir.join("hooks.json"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            let hook = root.path().join(codex_hook_install_name());

            install_codex_hook_into(&codex_dir, &hook).unwrap();

            let config_mode = fs::metadata(codex_dir.join("config.toml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let hooks_mode = fs::metadata(codex_dir.join("hooks.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(config_mode, 0o600, "the atomic rewrite preserves the mode");
            assert_eq!(hooks_mode, 0o600, "the atomic rewrite preserves the mode");
        }
    }

    #[test]
    fn the_windows_ps1_asset_is_a_marked_reporter_port() {
        // The uninstall marker and the reporter contract, checked against the
        // asset contents directly so the .ps1 variant cannot drift from the
        // .sh reporter shape it ports (the file is never installed on this
        // platform, so nothing else would catch it).
        assert!(CODEX_HOOK_ASSET_WINDOWS.contains(CODEX_HOOK_MARKER));
        assert!(
            CODEX_HOOK_ASSET_WINDOWS.contains("pane.report_agent_session"),
            "speaks the same control-socket method"
        );
        assert!(CODEX_HOOK_ASSET_WINDOWS.contains("par-mux:codex"));
        assert!(CODEX_HOOK_ASSET_WINDOWS.contains("session_resume_argv"));
        // The codex-specific guards survive the port.
        assert!(CODEX_HOOK_ASSET_WINDOWS.contains("CODEX_THREAD_ID"));
        assert!(CODEX_HOOK_ASSET_WINDOWS.contains("SessionStart"));
    }
}
