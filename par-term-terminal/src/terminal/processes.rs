//! Process-table inspection for job detection and close confirmation.
//!
//! Split from `terminal/mod.rs` for the repo's 800-line production ceiling.

use super::TerminalManager;

impl TerminalManager {
    /// Return names of child processes running under the shell, using the OS
    /// process table.  Skips processes whose name (case-insensitive) is in
    /// `jobs_to_ignore`.
    ///
    /// Returns an empty Vec when the shell PID is unknown, the process has no
    /// children, or `sysinfo` cannot inspect the process table.
    pub fn get_running_child_processes(&self, jobs_to_ignore: &[String]) -> Vec<String> {
        let shell_pid = match self.get_shell_pid() {
            Some(pid) => pid,
            None => return Vec::new(),
        };

        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

        let Ok(mut sys) = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
        ) else {
            return Vec::new();
        };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        let parent = Pid::from_u32(shell_pid);
        let ignore_lower: Vec<String> = jobs_to_ignore.iter().map(|s| s.to_lowercase()).collect();

        sys.processes()
            .values()
            .filter(|p| p.parent() == Some(parent))
            .filter_map(|p| p.name().to_str().map(|s| s.to_string()))
            .filter(|name| !ignore_lower.contains(&name.to_lowercase()))
            .collect()
    }

    /// Check if tab close should show a confirmation dialog.
    ///
    /// Uses OS process tree inspection as the primary method (works without
    /// shell integration), falling back to shell integration markers.
    /// Returns the name of the first concerning job, or `None` if clean.
    pub fn should_confirm_close(&self, jobs_to_ignore: &[String]) -> Option<String> {
        // Primary: OS process tree — reliable regardless of shell integration
        let children = self.get_running_child_processes(jobs_to_ignore);
        if let Some(first) = children.into_iter().next() {
            return Some(first);
        }

        // Fallback: shell integration markers (OSC 133) when no PID or no
        // children were found via sysinfo (e.g. shells that don't fork).
        let command_name = self.get_running_command_name()?;
        let command_lower = command_name.to_lowercase();
        if jobs_to_ignore
            .iter()
            .any(|i| i.to_lowercase() == command_lower)
        {
            return None;
        }
        Some(command_name)
    }
}
