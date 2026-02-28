//! Per-tab multi-script orchestrator.
//!
//! [`ScriptManager`] manages multiple [`ScriptProcess`] instances for a single tab,
//! providing lifecycle management, event broadcasting, and panel state tracking.

use std::collections::HashMap;

use super::process::ScriptProcess;
use super::protocol::{ScriptCommand, ScriptEvent};
use par_term_config::ScriptConfig;

/// Unique identifier for a managed script process.
pub type ScriptId = u64;

/// Default maximum `WriteText` writes per second.
pub const DEFAULT_WRITE_TEXT_RATE: u32 = 10;
/// Default maximum `RunCommand` executions per second.
pub const DEFAULT_RUN_COMMAND_RATE: u32 = 1;

/// Manages multiple script subprocess instances for a single tab.
///
/// Each script is assigned a unique [`ScriptId`] and can be individually started,
/// stopped, and communicated with. Supports panel state tracking per script and
/// event broadcasting to all running scripts.
pub struct ScriptManager {
    /// Next ID to assign to a new script process.
    next_id: ScriptId,
    /// Map of active script processes keyed by their assigned ID.
    processes: HashMap<ScriptId, ScriptProcess>,
    /// Panel state per script: script_id -> (title, content).
    panels: HashMap<ScriptId, (String, String)>,
    /// Last `WriteText` dispatch time per script (for rate limiting).
    write_text_times: HashMap<ScriptId, std::time::Instant>,
    /// Last `RunCommand` dispatch time per script (for rate limiting).
    run_command_times: HashMap<ScriptId, std::time::Instant>,
}

impl ScriptManager {
    /// Create a new empty `ScriptManager`.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            processes: HashMap::new(),
            panels: HashMap::new(),
            write_text_times: HashMap::new(),
            run_command_times: HashMap::new(),
        }
    }

    /// Start a script subprocess from the given configuration.
    ///
    /// If `script_path` ends with `.py`, the command is `python3` with the script path
    /// prepended to the args. Otherwise, `script_path` is used as the command directly.
    ///
    /// Returns the assigned [`ScriptId`] on success.
    ///
    /// # Errors
    /// Returns an error string if the subprocess cannot be spawned.
    pub fn start_script(&mut self, config: &ScriptConfig) -> Result<ScriptId, String> {
        let (command, args) = if config.script_path.ends_with(".py") {
            let mut full_args = vec![config.script_path.as_str()];
            let arg_refs: Vec<&str> = config.args.iter().map(String::as_str).collect();
            full_args.extend(arg_refs);
            (
                "python3".to_string(),
                full_args.into_iter().map(String::from).collect::<Vec<_>>(),
            )
        } else {
            let arg_refs: Vec<String> = config.args.to_vec();
            (config.script_path.clone(), arg_refs)
        };

        let arg_strs: Vec<&str> = args.iter().map(String::as_str).collect();
        let process = ScriptProcess::spawn(&command, &arg_strs, &config.env_vars)?;

        let id = self.next_id;
        self.next_id += 1;
        self.processes.insert(id, process);

        Ok(id)
    }

    /// Check if a script with the given ID is still running.
    ///
    /// Returns `false` if the script ID is unknown or the process has exited.
    pub fn is_running(&mut self, id: ScriptId) -> bool {
        self.processes.get_mut(&id).is_some_and(|p| p.is_running())
    }

    /// Send a [`ScriptEvent`] to a specific script by ID.
    ///
    /// # Errors
    /// Returns an error if the script ID is unknown or the write fails.
    pub fn send_event(&mut self, id: ScriptId, event: &ScriptEvent) -> Result<(), String> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| format!("No script with id {}", id))?;
        process.send_event(event)
    }

    /// Broadcast a [`ScriptEvent`] to all running scripts.
    ///
    /// Errors on individual scripts are silently ignored; the event is sent on a
    /// best-effort basis to all processes.
    pub fn broadcast_event(&mut self, event: &ScriptEvent) {
        for process in self.processes.values_mut() {
            let _ = process.send_event(event);
        }
    }

    /// Drain pending [`ScriptCommand`]s from a specific script's stdout buffer.
    ///
    /// Returns an empty `Vec` if the script ID is unknown.
    pub fn read_commands(&self, id: ScriptId) -> Vec<ScriptCommand> {
        self.processes
            .get(&id)
            .map(|p| p.read_commands())
            .unwrap_or_default()
    }

    /// Drain pending error lines from a specific script's stderr buffer.
    ///
    /// Returns an empty `Vec` if the script ID is unknown.
    pub fn read_errors(&self, id: ScriptId) -> Vec<String> {
        self.processes
            .get(&id)
            .map(|p| p.read_errors())
            .unwrap_or_default()
    }

    /// Stop and remove a specific script by ID.
    ///
    /// Also clears the associated panel state and rate-limit tracking.
    /// Does nothing if the ID is unknown.
    pub fn stop_script(&mut self, id: ScriptId) {
        if let Some(mut process) = self.processes.remove(&id) {
            process.stop();
        }
        self.panels.remove(&id);
        self.write_text_times.remove(&id);
        self.run_command_times.remove(&id);
    }

    /// Stop and remove all managed scripts.
    pub fn stop_all(&mut self) {
        for (_, mut process) in self.processes.drain() {
            process.stop();
        }
        self.panels.clear();
        self.write_text_times.clear();
        self.run_command_times.clear();
    }

    /// Check whether a `WriteText` command from `id` is within rate limits.
    ///
    /// Returns `true` (allowed) if at least `1000 / limit_per_sec` ms have
    /// elapsed since the last allowed write. Updates the last-write timestamp
    /// on success. `limit_per_sec == 0` uses the [`DEFAULT_WRITE_TEXT_RATE`].
    pub fn check_write_text_rate(&mut self, id: ScriptId, limit_per_sec: u32) -> bool {
        let rate = if limit_per_sec == 0 {
            DEFAULT_WRITE_TEXT_RATE
        } else {
            limit_per_sec
        };
        let min_interval_ms = 1000u64 / rate as u64;
        let now = std::time::Instant::now();
        if self
            .write_text_times
            .get(&id)
            .is_some_and(|last| (now.duration_since(*last).as_millis() as u64) < min_interval_ms)
        {
            return false;
        }
        self.write_text_times.insert(id, now);
        true
    }

    /// Check whether a `RunCommand` from `id` is within rate limits.
    ///
    /// Returns `true` (allowed) if at least `1000 / limit_per_sec` ms have
    /// elapsed since the last allowed run. Updates the last-run timestamp on
    /// success. `limit_per_sec == 0` uses the [`DEFAULT_RUN_COMMAND_RATE`].
    pub fn check_run_command_rate(&mut self, id: ScriptId, limit_per_sec: u32) -> bool {
        let rate = if limit_per_sec == 0 {
            DEFAULT_RUN_COMMAND_RATE
        } else {
            limit_per_sec
        };
        let min_interval_ms = 1000u64 / rate as u64;
        let now = std::time::Instant::now();
        if self
            .run_command_times
            .get(&id)
            .is_some_and(|last| (now.duration_since(*last).as_millis() as u64) < min_interval_ms)
        {
            return false;
        }
        self.run_command_times.insert(id, now);
        true
    }

    /// Get the panel state for a script.
    ///
    /// Returns `None` if the script ID has no panel set.
    pub fn get_panel(&self, id: ScriptId) -> Option<&(String, String)> {
        self.panels.get(&id)
    }

    /// Set the panel state (title, content) for a script.
    pub fn set_panel(&mut self, id: ScriptId, title: String, content: String) {
        self.panels.insert(id, (title, content));
    }

    /// Clear the panel state for a script.
    pub fn clear_panel(&mut self, id: ScriptId) {
        self.panels.remove(&id);
    }

    /// Get the IDs of all currently managed scripts.
    pub fn script_ids(&self) -> Vec<ScriptId> {
        self.processes.keys().copied().collect()
    }
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScriptManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}
