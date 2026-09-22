//! claude arm: hooks are entries inside the user's jsonc
//! `~/.claude/settings.json`, merged by the engine in [`super`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    home_dir, hook_asset_path as asset_path_for, hook_command_for, merge_session_start,
    remove_marked_file, save_settings, unmerge_session_start, write_hook_asset,
};

const CLAUDE_HOOK_ASSET_POSIX: &str =
    include_str!("../../mux_hooks/par-mux-claude-session-hook.sh");
const CLAUDE_HOOK_ASSET_WINDOWS: &str =
    include_str!("../../mux_hooks/par-mux-claude-session-hook.ps1");

/// The asset file name for the current platform — the `.ps1` port on
/// Windows, the `.sh` reporter elsewhere. Runtime-selected (not cfg-split)
/// so both arms compile on every platform and the selection is assertable
/// in tests instead of invisible to the non-Windows build.
fn claude_hook_install_name() -> &'static str {
    if cfg!(windows) {
        "par-mux-claude-session-hook.ps1"
    } else {
        "par-mux-claude-session-hook.sh"
    }
}

/// The hook asset contents for the current platform.
fn claude_hook_asset() -> &'static str {
    if cfg!(windows) {
        CLAUDE_HOOK_ASSET_WINDOWS
    } else {
        CLAUDE_HOOK_ASSET_POSIX
    }
}

/// Marker identifying the installed script as ours regardless of which build
/// wrote it — the uninstall path refuses to remove a same-named file without
/// one, exactly like the extension installer.
pub const CLAUDE_HOOK_MARKER: &str = "PAR_MUX_INTEGRATION_ID=claude";

/// Claude's documented SessionStart sources. Grok imports claude's hook format
/// but emits `new`/`load`; the matcher keeps this hook from firing there.
const SESSION_START_MATCHER: &str = "^(startup|resume|clear|compact|fork)$";

const CLAUDE_CONFIG_DIR_ENV_VAR: &str = "CLAUDE_CONFIG_DIR";

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
fn hook_asset_path() -> PathBuf {
    asset_path_for(claude_hook_install_name())
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
    let merged = merge_session_start(
        &content,
        settings_path,
        &command,
        Some(SESSION_START_MATCHER),
    )?;

    write_hook_asset(hook_path, claude_hook_asset())?;

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
        if let Some(updated) = unmerge_session_start(&content, settings_path, &command)? {
            save_settings(settings_path, &updated)?;
            settings_changed = true;
        }
    }

    let hook_removed = remove_marked_file(hook_path, CLAUDE_HOOK_MARKER)?;

    Ok(ClaudeHookUninstall {
        settings_path: settings_path.to_path_buf(),
        hook_path: hook_path.to_path_buf(),
        settings_changed,
        hook_removed,
    })
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
        let hook = root.path().join("hooks").join(claude_hook_install_name());

        let result = install_claude_hook_into(&settings, &hook).unwrap();

        assert!(result.settings_changed, "a fresh file is a change");
        let content = fs::read_to_string(&settings).unwrap();
        let commands = session_start_commands(&content);
        assert_eq!(commands, vec![installed_command(&hook)]);

        let asset = fs::read_to_string(&hook).unwrap();
        assert_eq!(asset, claude_hook_asset());
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
        let hook = root
            .path()
            .join("my hooks")
            .join(claude_hook_install_name());

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

    #[test]
    fn windows_command_uses_herdrs_powershell_file_shape() {
        let hook = Path::new(
            "C:\\Users\\me\\AppData\\Roaming\\par-term\\hooks\\par-mux-claude-session-hook.ps1",
        );
        assert_eq!(
            super::super::platform_hook_command(hook, None, true),
            "powershell -NoProfile -ExecutionPolicy Bypass -File \"C:\\Users\\me\\AppData\\\
             Roaming\\par-term\\hooks\\par-mux-claude-session-hook.ps1\"",
            "herdr's exact shape: powershell, no profile, bypassed execution \
             policy, double-quoted -File path"
        );
        // Spaced paths stay inside the one double-quoted argument.
        let spaced = Path::new("C:\\Users\\My Name\\hooks\\par-mux-claude-session-hook.ps1");
        let command = super::super::platform_hook_command(spaced, None, true);
        assert!(
            command.contains("\"C:\\Users\\My Name\\hooks\\"),
            "the quoted argument spans the space: {command}"
        );
    }

    #[test]
    fn windows_action_command_appends_the_action_after_the_quoted_path() {
        let hook = Path::new("C:\\hooks\\par-mux-codex-session-hook.ps1");
        assert_eq!(
            super::super::platform_hook_command(hook, Some("session"), true),
            "powershell -NoProfile -ExecutionPolicy Bypass -File \
             \"C:\\hooks\\par-mux-codex-session-hook.ps1\" session",
            "the action follows the quoted path, unquoted"
        );
        // The POSIX action form is byte-identical to the shipped codex/grok
        // command (`sh '<path>' session`, always quoted).
        assert_eq!(
            super::super::platform_hook_command(hook, Some("session"), false),
            format!("sh '{}' session", hook.display()),
            "the POSIX action form must not drift from the shipped command"
        );
    }

    #[test]
    fn the_windows_ps1_asset_is_a_marked_reporter_port() {
        // The uninstall marker and the reporter contract, checked against the
        // asset contents directly so the .ps1 variant cannot drift from the
        // .sh reporter shape it ports (the file is never installed on this
        // platform, so nothing else would catch it).
        assert!(CLAUDE_HOOK_ASSET_WINDOWS.contains(CLAUDE_HOOK_MARKER));
        assert!(
            CLAUDE_HOOK_ASSET_WINDOWS.contains("pane.report_agent_session"),
            "speaks the same control-socket method"
        );
        assert!(CLAUDE_HOOK_ASSET_WINDOWS.contains("par-mux:claude"));
        assert!(CLAUDE_HOOK_ASSET_WINDOWS.contains("session_resume_argv"));
        assert!(CLAUDE_HOOK_ASSET_WINDOWS.contains("PAR_MUX_PANE_ID"));
        assert!(CLAUDE_HOOK_ASSET_WINDOWS.contains("PAR_MUX_SOCKET"));
    }
}
