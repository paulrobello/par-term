//! Persistent command history for fuzzy search.
//!
//! Tracks commands captured via OSC 133 shell integration markers and persists
//! them across sessions to `~/.config/par-term/command_history.yaml`.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single command history entry persisted across sessions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandHistoryEntry {
    /// The command text
    pub command: String,
    /// Timestamp in milliseconds since epoch
    pub timestamp_ms: u64,
    /// Exit code (if known)
    pub exit_code: Option<i32>,
    /// Duration in milliseconds (if known)
    pub duration_ms: Option<u64>,
}

/// Manages a persistent, deduplicated command history with a configurable max size.
#[derive(Debug)]
pub struct CommandHistory {
    entries: VecDeque<CommandHistoryEntry>,
    max_entries: usize,
    path: PathBuf,
    dirty: bool,
}

/// YAML wrapper for serialization
#[derive(Debug, Serialize, Deserialize)]
struct CommandHistoryFile {
    commands: Vec<CommandHistoryEntry>,
}

impl CommandHistory {
    /// Create a new command history with the given max entries and persistence path.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            path: Self::default_path(),
            dirty: false,
        }
    }

    /// Get the default persistence path.
    fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("par-term")
            .join("command_history.yaml")
    }

    /// Load history from disk, merging with any existing in-memory entries.
    pub fn load(&mut self) {
        if !self.path.exists() {
            return;
        }
        match fs::read_to_string(&self.path) {
            Ok(contents) => match serde_yaml::from_str::<CommandHistoryFile>(&contents) {
                Ok(file) => {
                    // Load entries, newest first (file stores newest first)
                    self.entries = file.commands.into();
                    self.truncate();
                    log::info!("Loaded {} command history entries", self.entries.len());
                }
                Err(e) => {
                    log::error!("Failed to parse command history: {}", e);
                }
            },
            Err(e) => {
                log::error!("Failed to read command history file: {}", e);
            }
        }
    }

    /// Save history to disk.
    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        let file = CommandHistoryFile {
            commands: self.entries.iter().cloned().collect(),
        };
        if let Some(parent) = self.path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            log::error!("Failed to create command history directory: {}", e);
            return;
        }
        match serde_yaml::to_string(&file) {
            Ok(yaml) => {
                if let Err(e) = fs::write(&self.path, yaml) {
                    log::error!("Failed to write command history: {}", e);
                } else {
                    self.dirty = false;
                    log::debug!("Saved {} command history entries", self.entries.len());
                }
            }
            Err(e) => {
                log::error!("Failed to serialize command history: {}", e);
            }
        }
    }

    /// Serialize history and spawn a background thread to write it to disk.
    /// Used during shutdown to avoid blocking the main thread.
    pub fn save_background(&mut self) {
        if !self.dirty {
            return;
        }
        let file = CommandHistoryFile {
            commands: self.entries.iter().cloned().collect(),
        };
        self.dirty = false;
        let path = self.path.clone();
        let _ = std::thread::Builder::new()
            .name("cmd-history-save".into())
            .spawn(move || {
                if let Some(parent) = path.parent()
                    && let Err(e) = fs::create_dir_all(parent)
                {
                    log::error!("Failed to create command history directory: {}", e);
                    return;
                }
                match serde_yaml::to_string(&file) {
                    Ok(yaml) => {
                        if let Err(e) = fs::write(&path, yaml) {
                            log::error!("Failed to write command history: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize command history: {}", e);
                    }
                }
            });
    }

    /// Add a command to history, deduplicating by command text.
    /// If the command already exists, it is moved to the front with updated metadata.
    pub fn add(&mut self, command: String, exit_code: Option<i32>, duration_ms: Option<u64>) {
        let trimmed = command.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        // Remove existing duplicate (we'll re-add it at the front)
        self.entries.retain(|e| e.command != trimmed);

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.entries.push_front(CommandHistoryEntry {
            command: trimmed,
            timestamp_ms,
            exit_code,
            duration_ms,
        });

        self.truncate();
        self.dirty = true;
    }

    /// Get all entries (newest first).
    pub fn entries(&self) -> &VecDeque<CommandHistoryEntry> {
        &self.entries
    }

    /// Update max entries and truncate if needed.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max;
        self.truncate();
    }

    /// Whether the history has been modified since last save.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Get number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn truncate(&mut self) {
        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_deduplicate() {
        let mut history = CommandHistory::new(100);
        history.add("ls -la".to_string(), Some(0), Some(10));
        history.add("cd /tmp".to_string(), Some(0), Some(5));
        history.add("ls -la".to_string(), Some(0), Some(15));

        assert_eq!(history.len(), 2);
        // Most recent should be first
        assert_eq!(history.entries()[0].command, "ls -la");
        assert_eq!(history.entries()[1].command, "cd /tmp");
    }

    #[test]
    fn test_max_entries() {
        let mut history = CommandHistory::new(3);
        history.add("cmd1".to_string(), None, None);
        history.add("cmd2".to_string(), None, None);
        history.add("cmd3".to_string(), None, None);
        history.add("cmd4".to_string(), None, None);

        assert_eq!(history.len(), 3);
        assert_eq!(history.entries()[0].command, "cmd4");
        assert_eq!(history.entries()[2].command, "cmd2");
    }

    #[test]
    fn test_empty_command_ignored() {
        let mut history = CommandHistory::new(100);
        history.add("".to_string(), None, None);
        history.add("  ".to_string(), None, None);
        assert!(history.is_empty());
    }

    #[test]
    fn test_whitespace_trimmed() {
        let mut history = CommandHistory::new(100);
        history.add("  ls -la  ".to_string(), Some(0), None);
        assert_eq!(history.entries()[0].command, "ls -la");
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("command_history.yaml");

        let mut history = CommandHistory::new(100);
        history.path = path.clone();
        history.add("echo hello".to_string(), Some(0), Some(100));
        history.add("ls -la".to_string(), Some(0), Some(50));
        history.save();

        let mut loaded = CommandHistory::new(100);
        loaded.path = path;
        loaded.load();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.entries()[0].command, "ls -la");
        assert_eq!(loaded.entries()[1].command, "echo hello");
    }

    #[test]
    fn test_set_max_entries_truncates() {
        let mut history = CommandHistory::new(10);
        for i in 0..10 {
            history.add(format!("cmd{i}"), None, None);
        }
        assert_eq!(history.len(), 10);

        history.set_max_entries(5);
        assert_eq!(history.len(), 5);
        // Newest entries should remain
        assert_eq!(history.entries()[0].command, "cmd9");
    }
}
