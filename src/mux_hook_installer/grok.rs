//! grok arm: grok merges every `<config>/hooks/*.json` it finds, so par-term
//! owns a dedicated config file beside its script and never touches a user
//! file (herdr's proven model, ported).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    HOOK_TIMEOUT_SECS, home_dir, hook_command_with_action, remove_marked_file, write_hook_asset,
};

const GROK_HOOK_CONFIG_INSTALL_NAME: &str = "par-mux-grok-hooks.json";
const GROK_HOOK_ASSET_POSIX: &str = include_str!("../../mux_hooks/par-mux-grok-session-hook.sh");
const GROK_HOOK_ASSET_WINDOWS: &str = include_str!("../../mux_hooks/par-mux-grok-session-hook.ps1");

/// The asset file name for the current platform — the `.ps1` port on
/// Windows, the `.sh` reporter elsewhere. Runtime-selected (not cfg-split)
/// so both arms compile on every platform and the selection is assertable
/// in tests instead of invisible to the non-Windows build.
fn grok_hook_install_name() -> &'static str {
    if cfg!(windows) {
        "par-mux-grok-session-hook.ps1"
    } else {
        "par-mux-grok-session-hook.sh"
    }
}

/// The hook asset contents for the current platform.
fn grok_hook_asset() -> &'static str {
    if cfg!(windows) {
        GROK_HOOK_ASSET_WINDOWS
    } else {
        GROK_HOOK_ASSET_POSIX
    }
}

/// Marker identifying the installed grok script as ours.
pub const GROK_HOOK_MARKER: &str = "PAR_MUX_INTEGRATION_ID=grok";

const GROK_HOME_ENV_VAR: &str = "GROK_HOME";

/// Outcome of a grok hook install.
#[derive(Debug)]
pub struct GrokHookInstall {
    pub hook_path: PathBuf,
    pub config_path: PathBuf,
}

/// Outcome of a grok hook uninstall.
#[derive(Debug)]
pub struct GrokHookUninstall {
    pub hook_path: PathBuf,
    pub config_path: PathBuf,
    pub hook_removed: bool,
    pub config_removed: bool,
}

/// Resolve the grok config home (`$GROK_HOME`, which the grok CLI honors, else
/// `~/.grok`).
pub fn grok_config_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(GROK_HOME_ENV_VAR).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    Ok(home_dir()?.join(".grok"))
}

/// Install the grok session hook: script asset plus an OWNED config file.
pub fn install_grok_hook() -> io::Result<GrokHookInstall> {
    install_grok_hook_into(&grok_config_dir()?)
}

/// Install with an explicit config directory (test/install seam).
pub fn install_grok_hook_into(dir: &Path) -> io::Result<GrokHookInstall> {
    if !dir.is_dir() {
        return Err(io::Error::other(format!(
            "grok config directory not found at {}. install grok cli first",
            dir.display()
        )));
    }
    // grok merges every <config>/hooks/*.json, so par-term owns a dedicated
    // config file there and never edits the user's other hooks (herdr's
    // proven model). The hook script and its config live side by side.
    let hooks_dir = dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join(grok_hook_install_name());
    write_hook_asset(&hook_path, grok_hook_asset())?;
    let config_path = hooks_dir.join(GROK_HOOK_CONFIG_INSTALL_NAME);
    fs::write(&config_path, grok_hook_config(&hook_path))?;
    Ok(GrokHookInstall {
        hook_path,
        config_path,
    })
}

/// Uninstall the grok session hook: straight delete of the two files we own.
pub fn uninstall_grok_hook() -> io::Result<GrokHookUninstall> {
    uninstall_grok_hook_into(&grok_config_dir()?)
}

/// Uninstall with an explicit config directory (test/install seam).
pub fn uninstall_grok_hook_into(dir: &Path) -> io::Result<GrokHookUninstall> {
    let hooks_dir = dir.join("hooks");
    let hook_path = hooks_dir.join(grok_hook_install_name());
    let config_path = hooks_dir.join(GROK_HOOK_CONFIG_INSTALL_NAME);

    // The config name is par-mux-prefixed, so it is ours by name; the script
    // is marker-checked like every other asset we remove.
    let config_removed = if config_path.is_file() {
        fs::remove_file(&config_path)?;
        true
    } else {
        false
    };
    let hook_removed = remove_marked_file(&hook_path, GROK_HOOK_MARKER)?;

    Ok(GrokHookUninstall {
        hook_path,
        config_path,
        hook_removed,
        config_removed,
    })
}

/// The owned grok hook config: a matcher-less SessionStart entry (grok's
/// new/load sources sit outside claude's matcher space) whose script
/// self-filters on hook_event_name and GROK_SESSION_ID.
fn grok_hook_config(hook_path: &Path) -> String {
    let command = hook_command_with_action(hook_path, "session");
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "SessionStart": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": command,
                            "timeout": HOOK_TIMEOUT_SECS,
                        }
                    ]
                }
            ]
        }
    }))
    .expect("hook config is serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;
    use std::fs;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn grok_install_writes_script_and_owned_config() {
        let root = temp_root();
        let grok_home = root.path().join("grok-home");
        fs::create_dir_all(&grok_home).unwrap();

        let result = install_grok_hook_into(&grok_home).unwrap();

        let script = fs::read_to_string(&result.hook_path).unwrap();
        assert_eq!(script, grok_hook_asset());
        assert!(script.contains(GROK_HOOK_MARKER));
        assert!(script.contains("pane.report_agent_session"));
        assert!(script.contains("GROK_SESSION_ID"), "grok's own env is kept");
        assert!(!script.contains("HERDR_"), "fully env-renamed port");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&result.hook_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o755, "the hook must be executable");
        }

        // The owned config parses and points at the script.
        let config: JsonValue =
            serde_json::from_str(&fs::read_to_string(&result.config_path).unwrap()).unwrap();
        let entry = &config["hooks"]["SessionStart"][0]["hooks"][0];
        assert_eq!(entry["type"], "command");
        assert_eq!(entry["timeout"], HOOK_TIMEOUT_SECS);
        let command = entry["command"].as_str().unwrap();
        assert!(command.starts_with("sh ") && command.ends_with(" session"));
        assert!(command.contains(grok_hook_install_name()));
        assert!(
            config["hooks"]["SessionStart"][0].get("matcher").is_none(),
            "grok's entry is matcher-less: new/load sit outside claude's space"
        );
    }

    #[test]
    fn grok_install_leaves_sibling_hook_configs_alone() {
        let root = temp_root();
        let grok_home = root.path().join("grok-home");
        fs::create_dir_all(grok_home.join("hooks")).unwrap();
        let sibling = grok_home.join("hooks").join("user-hooks.json");
        fs::write(&sibling, "{\"hooks\":{}}").unwrap();

        install_grok_hook_into(&grok_home).unwrap();

        assert_eq!(
            fs::read_to_string(&sibling).unwrap(),
            "{\"hooks\":{}}",
            "grok merges every hooks/*.json; we add a file, we never edit theirs"
        );
    }

    #[test]
    fn grok_reinstall_overwrites_cleanly() {
        let root = temp_root();
        let grok_home = root.path().join("grok-home");
        fs::create_dir_all(&grok_home).unwrap();

        let first = install_grok_hook_into(&grok_home).unwrap();
        fs::write(&first.hook_path, "# user edit").unwrap();
        let second = install_grok_hook_into(&grok_home).unwrap();

        assert_eq!(
            fs::read_to_string(&second.hook_path).unwrap(),
            grok_hook_asset()
        );
        assert_eq!(
            fs::read_to_string(&second.config_path).unwrap(),
            grok_hook_config(&second.hook_path),
            "both owned files are rewritten deterministically"
        );
    }

    #[test]
    fn grok_uninstall_removes_both_owned_files_and_noops() {
        let root = temp_root();
        let grok_home = root.path().join("grok-home");
        fs::create_dir_all(&grok_home).unwrap();
        install_grok_hook_into(&grok_home).unwrap();

        let result = uninstall_grok_hook_into(&grok_home).unwrap();

        assert!(result.hook_removed && result.config_removed);
        assert!(!result.hook_path.exists());
        assert!(!result.config_path.exists());

        // Idempotent.
        let again = uninstall_grok_hook_into(&grok_home).unwrap();
        assert!(!again.hook_removed && !again.config_removed);
    }

    #[test]
    fn grok_uninstall_leaves_a_foreign_script_with_our_name_alone() {
        let root = temp_root();
        let grok_home = root.path().join("grok-home");
        let hooks_dir = grok_home.join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        // Our config name (removed by name) but a foreign script (no marker).
        fs::write(hooks_dir.join(GROK_HOOK_CONFIG_INSTALL_NAME), "{}").unwrap();
        fs::write(
            hooks_dir.join(grok_hook_install_name()),
            "# user's own script, no marker\n",
        )
        .unwrap();

        let result = uninstall_grok_hook_into(&grok_home).unwrap();

        assert!(result.config_removed, "the par-mux-named config is ours");
        assert!(
            !result.hook_removed,
            "a script without our marker is not ours"
        );
        assert!(result.hook_path.exists());
    }

    #[test]
    fn grok_missing_config_directory_is_an_actionable_error() {
        let root = temp_root();

        let err = install_grok_hook_into(&root.path().join("never"))
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("grok config directory not found at"),
            "error must name the path: {err}"
        );
        assert!(
            err.contains("install grok cli first"),
            "error must say what to do: {err}"
        );
    }

    #[test]
    fn the_windows_ps1_asset_is_a_marked_reporter_port() {
        // The uninstall marker and the reporter contract, checked against the
        // asset contents directly so the .ps1 variant cannot drift from the
        // .sh reporter shape it ports (the file is never installed on this
        // platform, so nothing else would catch it).
        assert!(GROK_HOOK_ASSET_WINDOWS.contains(GROK_HOOK_MARKER));
        assert!(
            GROK_HOOK_ASSET_WINDOWS.contains("pane.report_agent_session"),
            "speaks the same control-socket method"
        );
        assert!(GROK_HOOK_ASSET_WINDOWS.contains("par-mux:grok"));
        // The grok-specific guards survive the port: the event-name
        // tolerance and the injected session env.
        assert!(GROK_HOOK_ASSET_WINDOWS.contains("session_start"));
        assert!(GROK_HOOK_ASSET_WINDOWS.contains("GROK_SESSION_ID"));
    }
}
