//! Trigger management state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use regex::Regex;
use std::collections::HashMap;
use std::time::Instant;

/// State for managing terminal triggers and their spawned processes.
#[derive(Default)]
pub(crate) struct TriggerState {
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: HashMap<u32, Instant>,
    /// Compiled regex cache for prettify trigger patterns (command_filter and block_end).
    /// Keyed by pattern string; avoids recompiling the same pattern every frame.
    pub(crate) trigger_regex_cache: HashMap<String, Regex>,
}
