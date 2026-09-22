//! par-mux agent-state extension installer (pi and omp).
//!
//! Bundles the par-mux agent-state extensions — env-renamed ports of herdr's
//! pi/omp integrations, whose reports par-mux's hook endpoint accepts verbatim
//! (`pane.report_agent` / `pane.report_agent_session` over `PAR_MUX_SOCKET`)
//! — and writes them into each agent's extension directory. This is the same
//! drop-a-bundled-file model herdr uses, following the
//! [`crate::shell_integration_installer`] embedding pattern pointed at a
//! different directory. The module is deliberately core-free (plain `fs`), so
//! it builds and lints without the `mux` feature; only the end-to-end test
//! needs a live daemon.
//!
//! Extension directories (mirroring herdr's resolution):
//! - pi:  `$PI_CODING_AGENT_DIR` or `~/.pi/agent`, plus `extensions`
//! - omp: `$PI_CODING_AGENT_DIR` (omp is a pi fork and honors it) or
//!   `$PI_CONFIG_DIR`/`~/.omp` plus `agent/extensions`
//!
//! Because both agents honor `PI_CODING_AGENT_DIR`, setting it makes both
//! resolve to the same directory — [`install_omp_extension`] refuses rather
//! than letting one agent's file shadow the other's.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PI_EXTENSION_INSTALL_NAME: &str = "par-mux-agent-state.ts";
const OMP_EXTENSION_INSTALL_NAME: &str = "par-mux-omp-agent-state.ts";
const PI_EXTENSION_ASSET: &str = include_str!("../mux_extensions/pi/par-mux-agent-state.ts");
const OMP_EXTENSION_ASSET: &str = include_str!("../mux_extensions/omp/par-mux-omp-agent-state.ts");

/// Header markers identifying a file as ours, regardless of which build wrote
/// it — the uninstall path refuses to remove a same-named file without one.
const PI_MARKER: &str = "PAR_MUX_INTEGRATION_ID=pi";
const OMP_MARKER: &str = "PAR_MUX_INTEGRATION_ID=omp";

const PI_CODING_AGENT_DIR_ENV_VAR: &str = "PI_CODING_AGENT_DIR";
const OMP_CONFIG_DIR_ENV_VAR: &str = "PI_CONFIG_DIR";

/// An extension removal outcome: where the file would live, and whether it was
/// actually ours to delete.
pub struct ExtensionRemoval {
    pub path: PathBuf,
    pub removed: bool,
}

/// Resolve the pi extension directory.
pub fn pi_extension_dir() -> io::Result<PathBuf> {
    Ok(
        agent_dir_from_env_or_home(PI_CODING_AGENT_DIR_ENV_VAR, &[".pi", "agent"])?
            .join("extensions"),
    )
}

/// Resolve the omp extension directory.
pub fn omp_extension_dir() -> io::Result<PathBuf> {
    if let Some(value) = env_var_nonempty(PI_CODING_AGENT_DIR_ENV_VAR) {
        return expand_tilde(PathBuf::from(value)).map(|path| path.join("extensions"));
    }

    let config_dir = env_var_nonempty(OMP_CONFIG_DIR_ENV_VAR).unwrap_or_else(|| ".omp".into());
    Ok(home_dir()?
        .join(config_dir)
        .join("agent")
        .join("extensions"))
}

/// Install the pi extension into its resolved directory.
pub fn install_pi_extension() -> io::Result<PathBuf> {
    install_pi_extension_into(&pi_extension_dir()?)
}

/// Install the pi extension into an explicit directory (test/install seam).
pub fn install_pi_extension_into(dir: &Path) -> io::Result<PathBuf> {
    ensure_extension_dir(dir, "pi")?;
    let path = dir.join(PI_EXTENSION_INSTALL_NAME);
    fs::write(&path, PI_EXTENSION_ASSET)?;
    Ok(path)
}

/// Install the omp extension into its resolved directory, refusing when pi and
/// omp resolve to the same place.
pub fn install_omp_extension() -> io::Result<PathBuf> {
    let pi_dir = pi_extension_dir()?;
    let omp_dir = omp_extension_dir()?;
    install_omp_extension_into(&pi_dir, &omp_dir)
}

/// Install the omp extension into an explicit directory (test/install seam).
pub fn install_omp_extension_into(pi_dir: &Path, omp_dir: &Path) -> io::Result<PathBuf> {
    if omp_dir == pi_dir {
        return Err(io::Error::other(format!(
            "Pi and OMP resolve to the same extension directory at {}; configure separate agent directories before installing the OMP extension",
            omp_dir.display()
        )));
    }
    ensure_extension_dir(omp_dir, "omp")?;
    let path = omp_dir.join(OMP_EXTENSION_INSTALL_NAME);
    fs::write(&path, OMP_EXTENSION_ASSET)?;
    Ok(path)
}

/// Remove the pi extension if it is present and ours.
pub fn uninstall_pi_extension() -> io::Result<ExtensionRemoval> {
    uninstall_from(&pi_extension_dir()?, PI_EXTENSION_INSTALL_NAME, PI_MARKER)
}

/// Remove the omp extension if it is present and ours.
pub fn uninstall_omp_extension() -> io::Result<ExtensionRemoval> {
    uninstall_from(
        &omp_extension_dir()?,
        OMP_EXTENSION_INSTALL_NAME,
        OMP_MARKER,
    )
}

/// The directory must already belong to an installed agent: create the
/// extensions leaf when its parent exists, error naming the path otherwise —
/// a missing agent is an install-it-first condition, not a silent skip.
fn ensure_extension_dir(dir: &Path, agent: &str) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    if dir.parent().is_some_and(|parent| parent.is_dir()) {
        return fs::create_dir_all(dir);
    }
    Err(io::Error::other(format!(
        "{agent} extension directory not found at {}. install {agent} first",
        dir.display()
    )))
}

/// Delete `dir/install_name` only when it carries our marker: a same-named
/// file without one is a user file we must not touch.
fn uninstall_from(dir: &Path, install_name: &str, marker: &str) -> io::Result<ExtensionRemoval> {
    let path = dir.join(install_name);
    if !path.is_file() {
        return Ok(ExtensionRemoval {
            path,
            removed: false,
        });
    }
    let content = fs::read_to_string(&path)?;
    if !content.contains(marker) {
        return Ok(ExtensionRemoval {
            path,
            removed: false,
        });
    }
    fs::remove_file(&path)?;
    Ok(ExtensionRemoval {
        path,
        removed: true,
    })
}

fn env_var_nonempty(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

fn agent_dir_from_env_or_home(env_var: &str, home_segments: &[&str]) -> io::Result<PathBuf> {
    if let Some(value) = env_var_nonempty(env_var) {
        return expand_tilde(PathBuf::from(value));
    }
    let mut dir = home_dir()?;
    for segment in home_segments {
        dir = dir.join(segment);
    }
    Ok(dir)
}

fn home_dir() -> io::Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| io::Error::other("Could not determine home directory"))
}

fn expand_tilde(path: PathBuf) -> io::Result<PathBuf> {
    let Some(text) = path.to_str() else {
        return Ok(path);
    };
    if text == "~" {
        return home_dir();
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn install_writes_the_bundled_asset_with_markers() {
        let root = temp_root();
        let pi_home = root.path().join("pi-home");
        fs::create_dir_all(&pi_home).unwrap();

        let path = install_pi_extension_into(&pi_home.join("extensions")).unwrap();

        assert_eq!(
            path,
            pi_home.join("extensions").join(PI_EXTENSION_INSTALL_NAME)
        );
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, PI_EXTENSION_ASSET);
        assert!(content.contains(PI_MARKER));
        assert!(
            !content.contains("HERDR_"),
            "the port must be fully env-renamed: {content}"
        );
        // The kept `pi.events.on("herdr:blocked", …)` subscription is the one
        // sanctioned herdr reference — the emitter's own event name.
        assert!(
            !content.contains("source = \"herdr:"),
            "source ids must be par-mux namespaced: {content}"
        );
    }

    #[test]
    fn omp_asset_carries_the_omp_marker() {
        let root = temp_root();
        let omp_agent_root = root.path().join("omp-home").join("agent");
        fs::create_dir_all(&omp_agent_root).unwrap();

        let path = install_omp_extension_into(
            &root.path().join("pi-home"),
            &omp_agent_root.join("extensions"),
        )
        .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, OMP_EXTENSION_ASSET);
        assert!(content.contains(OMP_MARKER));
        assert!(content.contains("PAR_MUX_OMP_IDLE_DEBOUNCE_MS"));
    }

    #[test]
    fn reinstall_overwrites_cleanly() {
        let root = temp_root();
        let pi_home = root.path().join("pi-home");
        fs::create_dir_all(&pi_home).unwrap();

        let path = install_pi_extension_into(&pi_home.join("extensions")).unwrap();
        fs::write(&path, "// user edit").unwrap();
        install_pi_extension_into(&pi_home.join("extensions")).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), PI_EXTENSION_ASSET);
    }

    #[test]
    fn missing_agent_directory_is_an_actionable_error() {
        let root = temp_root();

        let err = install_pi_extension_into(&root.path().join("never").join("extensions"))
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("pi extension directory not found at"),
            "error must name the agent and the path: {err}"
        );
        assert!(
            err.contains("install pi first"),
            "error must say what to do: {err}"
        );
        assert!(
            err.contains(
                root.path()
                    .join("never")
                    .join("extensions")
                    .display()
                    .to_string()
                    .as_str()
            ),
            "error must contain the resolved path: {err}"
        );
    }

    #[test]
    fn omp_install_refuses_when_dirs_collide() {
        let root = temp_root();
        let same = root.path().join("same").join("extensions");
        fs::create_dir_all(root.path().join("same")).unwrap();

        let err = install_omp_extension_into(&same, &same)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("same extension directory"),
            "collision must be named: {err}"
        );
        assert!(err.contains(same.display().to_string().as_str()));
        assert!(
            !same.exists(),
            "the collision must refuse before writing anything"
        );
    }

    #[test]
    fn uninstall_removes_only_par_terms_own_file() {
        let root = temp_root();
        let pi_home = root.path().join("pi-home");
        let ext_dir = pi_home.join("extensions");
        fs::create_dir_all(&ext_dir).unwrap();

        let path = install_pi_extension_into(&ext_dir).unwrap();
        let sibling = ext_dir.join("user-extension.ts");
        fs::write(&sibling, "// user's own").unwrap();

        let removal = uninstall_from(&ext_dir, PI_EXTENSION_INSTALL_NAME, PI_MARKER).unwrap();

        assert!(removal.removed);
        assert_eq!(removal.path, path);
        assert!(!path.exists(), "our file is gone");
        assert!(sibling.exists(), "other extensions are untouched");

        let again = uninstall_from(&ext_dir, PI_EXTENSION_INSTALL_NAME, PI_MARKER).unwrap();
        assert!(!again.removed, "uninstall is idempotent");
    }

    #[test]
    fn uninstall_leaves_a_foreign_file_with_our_name_alone() {
        let root = temp_root();
        let ext_dir = root.path().join("ext");
        fs::create_dir_all(&ext_dir).unwrap();

        let path = ext_dir.join(PI_EXTENSION_INSTALL_NAME);
        fs::write(&path, "// a user file that happens to share our name").unwrap();

        let removal = uninstall_from(&ext_dir, PI_EXTENSION_INSTALL_NAME, PI_MARKER).unwrap();

        assert!(!removal.removed, "a file without our marker is not ours");
        assert!(path.exists(), "the user's file survives");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "// a user file that happens to share our name"
        );
    }
}
