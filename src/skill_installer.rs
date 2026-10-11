//! par-mux agent skill installer, delegated to the par-mux binary.
//!
//! par-mux owns the skill now: the binary embeds it and installs it via
//! `--install-skill`, so this module resolves the par-mux binary the same
//! way the mux library spawn path does — next to our own executable first,
//! then on `PATH` — runs the flag, and returns the installed skill path
//! the binary printed. The skill an agent reads therefore always matches
//! the shipped binary, never a copy embedded here that could drift.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The `par-mux: installed agent skill to <path>` stdout line prefix the
/// binary prints on success (owned by the par-mux CLI).
const INSTALLED_TO_PREFIX: &str = "par-mux: installed agent skill to ";

/// Install (or refresh) the par-mux agent skill via the shipped binary.
///
/// Tries each candidate (exe sibling first, then `PATH`); the combined
/// error names every candidate tried, so the resolution order stays
/// visible at the failure where it mattered.
pub fn install_agent_skill() -> io::Result<PathBuf> {
    let mut failures = Vec::new();
    for bin in par_mux_binary_candidates()? {
        match run_install(&bin) {
            Ok(path) => return Ok(path),
            Err(err) => failures.push(format!("{}: {err}", bin.display())),
        }
    }
    Err(io::Error::other(format!(
        "could not install the par-mux agent skill — {}",
        failures.join("; ")
    )))
}

/// Run `bin --install-skill` and return the skill path it printed.
fn run_install(bin: &Path) -> io::Result<PathBuf> {
    let output = Command::new(bin)
        .arg("--install-skill")
        .output()
        .map_err(|err| io::Error::other(format!("running it failed: {err}")))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "`--install-skill` exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_installed_path(&String::from_utf8_lossy(&output.stdout))
}

/// Extract the installed path from the binary stdout.
fn parse_installed_path(stdout: &str) -> io::Result<PathBuf> {
    let line = stdout.lines().last().unwrap_or_default();
    let path = line.strip_prefix(INSTALLED_TO_PREFIX).unwrap_or_default();
    if path.is_empty() {
        return Err(io::Error::other(format!(
            "unrecognized --install-skill output: {line:?}"
        )));
    }
    Ok(PathBuf::from(path))
}

/// Where the par-mux binary is looked up, in order: next to our own
/// executable first, then on `PATH` — the bundled binary (shipped beside
/// the par-term binary in releases) wins over whatever an older install
/// left on `PATH`.
pub(crate) fn par_mux_binary_candidates() -> io::Result<Vec<PathBuf>> {
    #[cfg(unix)]
    let file_name = "par-mux";
    #[cfg(windows)]
    let file_name = "par-mux.exe";

    let mut candidates = vec![exe_sibling(file_name)?];
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(file_name);
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    Ok(candidates)
}

/// The exe-relative candidate: the par-mux binary beside our own.
fn exe_sibling(file_name: &str) -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let Some(mut dir) = exe.parent().map(Path::to_path_buf) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "cannot locate the par-mux binary: {} has no parent directory",
                exe.display()
            ),
        ));
    };
    // Integration-test binaries live in <target>/<profile>/deps while
    // bins sit in <target>/<profile> — walk out of deps to the sibling.
    if dir.file_name() == Some(std::ffi::OsStr::new("deps"))
        && let Some(parent) = dir.parent()
    {
        dir = parent.to_path_buf();
    }
    Ok(dir.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_installed_path_from_binary_stdout() {
        let path = parse_installed_path(
            "par-mux: installed agent skill to /home/u/.claude/skills/par-mux/SKILL.md\n",
        )
        .expect("parses");
        assert_eq!(
            path,
            PathBuf::from("/home/u/.claude/skills/par-mux/SKILL.md")
        );
    }

    #[test]
    fn unrecognized_output_is_an_error() {
        let err = parse_installed_path("something else\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("unrecognized --install-skill output"), "{err}");
    }

    #[test]
    fn empty_output_is_an_error() {
        assert!(parse_installed_path("").is_err());
    }
}
