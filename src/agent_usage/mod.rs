//! Agent usage panel: a display surface over a directory of JSON usage
//! records written by external collectors.
//!
//! The file contract is omarchy's (ported verbatim — see
//! `docs/features/AGENT_USAGE.md` once Task 6 lands): one `<agent-id>.json`
//! per agent under the records directory, parsed by [`records`], watched and
//! snapshotted by [`store`], and rendered by the status-bar widget and popup
//! panel (Tasks 3–4). par-term itself ships no collectors.

pub(crate) mod records;
pub(crate) mod store;

use std::path::PathBuf;

/// Resolve the records directory.
///
/// `PAR_TERM_AGENT_USAGE_RECORDS_DIR` overrides everything (hermetic ui-test
/// runs and debugging — the same isolation pattern as `XDG_CONFIG_HOME` for
/// the config). Otherwise the platform default, mirroring
/// `Config::state_file_path`'s cfg split: `~/.local/state/par-term/agents/usage`
/// on Linux/macOS (the design's XDG-state contract) and
/// `%LOCALAPPDATA%\par-term\agents\usage` on Windows.
pub(crate) fn default_records_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PAR_TERM_AGENT_USAGE_RECORDS_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .map(|d| d.join("par-term").join("agents").join("usage"))
            .unwrap_or_else(|| PathBuf::from("agents").join("usage"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .map(|h| {
                h.join(".local")
                    .join("state")
                    .join("par-term")
                    .join("agents")
                    .join("usage")
            })
            .unwrap_or_else(|| {
                PathBuf::from(".local")
                    .join("state")
                    .join("par-term")
                    .join("agents")
                    .join("usage")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    /// SAFETY: sole writer of this key is the caller, momentarily.
    unsafe fn restore(key: &str, saved: Option<&OsString>) {
        match saved {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    /// One test for both scenarios because env mutation is process-global:
    /// two tests racing on the same variable would flake. Edition 2024 makes
    /// set_var/remove_var unsafe (they are not thread-safe); the unsafety is
    /// contained by this being the only writer of the variable, and lib tests
    /// touching nothing else that reads it.
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn env_override_wins_and_unix_default_follows() {
        let key = "PAR_TERM_AGENT_USAGE_RECORDS_DIR";
        let saved = std::env::var_os(key);

        let overridden = {
            // SAFETY: sole writer of this key for the duration of the test.
            unsafe { std::env::set_var(key, "/tmp/agent-usage-override-test") };
            let resolved = default_records_dir();
            unsafe { restore(key, saved.as_ref()) };
            resolved
        };
        assert_eq!(overridden, PathBuf::from("/tmp/agent-usage-override-test"));

        let defaulted = {
            // SAFETY: as above.
            unsafe { std::env::remove_var(key) };
            let resolved = default_records_dir();
            unsafe { restore(key, saved.as_ref()) };
            resolved
        };
        let expected = dirs::home_dir()
            .expect("home resolves in tests")
            .join(".local")
            .join("state")
            .join("par-term")
            .join("agents")
            .join("usage");
        assert_eq!(defaulted, expected);
    }
}
