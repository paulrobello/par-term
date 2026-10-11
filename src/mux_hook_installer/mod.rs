//! par-mux agent-hook integrations, delegated to the par-mux binary.
//!
//! par-mux owns the hook assets and the config-entry merges now (its
//! `--install-agent-hooks` / `--uninstall-agent-hooks` /
//! `--agent-hooks-status` / `--heal-agent-hooks` one-shot flags). This
//! module keeps par-term call surface — the Settings status rows, the
//! startup self-heal, the CLI prompts — and executes by invoking the
//! shipped par-mux binary, resolved the way [`crate::skill_installer`]
//! resolves it: beside our own executable first, then on `PATH`.

use std::io;
use std::path::PathBuf;
use std::process::Command;

/// Startup self-heal for the agent-hook script assets: ask par-mux to
/// rewrite a registered script that went missing. Best-effort — a missing
/// binary or a heal error is logged and never blocks startup.
pub fn heal_hook_assets() {
    if let Err(err) = run_capture("--heal-agent-hooks") {
        log::info!("par-mux hook heal skipped: {err}");
    }
}

/// Read-only install state of each agent session hook, for Settings >
/// Sessions > par-mux (UX.md SX1). Never writes anything. An unresolvable
/// par-mux binary yields an empty list (the rows simply do not show).
pub fn hook_status() -> Vec<par_term_settings_ui::sessions_tab::MuxHookStatus> {
    let Ok(stdout) = run_capture("--agent-hooks-status") else {
        return Vec::new();
    };
    parse_status(&stdout)
}

/// Parse the `<agent> installed|absent` lines par-mux prints.
fn parse_status(stdout: &str) -> Vec<par_term_settings_ui::sessions_tab::MuxHookStatus> {
    stdout
        .lines()
        .filter_map(|line| {
            let (agent, state) = line.split_once(' ')?;
            if !matches!(state, "installed" | "absent") {
                return None;
            }
            Some(par_term_settings_ui::sessions_tab::MuxHookStatus {
                agent: agent.to_string(),
                installed: state == "installed",
            })
        })
        .collect()
}

/// Run one par-mux integration flag and capture its stdout.
fn run_capture(flag: &str) -> io::Result<String> {
    let mut failures = Vec::new();
    for bin in crate::skill_installer::par_mux_binary_candidates()? {
        let output = match Command::new(&bin).arg(flag).output() {
            Ok(output) => output,
            Err(err) => {
                failures.push(format!("{}: {err}", bin.display()));
                continue;
            }
        };
        if !output.status.success() {
            failures.push(format!(
                "{}: {}",
                bin.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            continue;
        }
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(io::Error::other(failures.join("; ")))
}

/// Run one par-mux integration flag with inherited stdio, so its per-arm
/// report lines stream straight to the user (the CLI verbs).
pub fn run_flag_inheriting(flag: &str) -> io::Result<()> {
    let mut failures = Vec::new();
    for bin in crate::skill_installer::par_mux_binary_candidates()? {
        match Command::new(&bin).arg(flag).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => failures.push(format!("{} exited {status}", bin.display())),
            Err(err) => failures.push(format!("{}: {err}", bin.display())),
        }
    }
    Err(io::Error::other(failures.join("; ")))
}

/// Where the claude settings live (prompt display only; the merge runs in
/// par-mux). `$CLAUDE_CONFIG_DIR` honored, else `~/.claude/settings.json`.
pub fn claude_settings_path() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir).join("settings.json"));
    }
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

/// The codex config home (`$CODEX_HOME`, else `~/.codex`) — display only.
pub fn codex_config_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CODEX_HOME")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir));
    }
    Ok(home_dir()?.join(".codex"))
}

/// The grok config home (`$GROK_HOME`, else `~/.grok`) — display only.
pub fn grok_config_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("GROK_HOME")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir));
    }
    Ok(home_dir()?.join(".grok"))
}

fn home_dir() -> io::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| io::Error::other("home directory not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines_parse_into_rows() {
        let rows = parse_status("claude installed\ncodex absent\ngrok installed\n");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].agent, "claude");
        assert!(rows[0].installed);
        assert_eq!(rows[1].agent, "codex");
        assert!(!rows[1].installed);
    }

    #[test]
    fn foreign_lines_are_dropped() {
        assert!(parse_status("par-mux: something else\n").is_empty());
        assert!(parse_status("claude maybe\n").is_empty());
    }
}
