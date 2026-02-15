use serde::{Deserialize, Serialize};

/// Scope for terminal state snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotScope {
    Visible,
    Recent(usize),
    Full,
}

impl SnapshotScope {
    pub fn from_config_str(s: &str) -> Self {
        if s == "visible" {
            Self::Visible
        } else if s == "full" {
            Self::Full
        } else if let Some(n) = s.strip_prefix("recent_") {
            Self::Recent(n.parse().unwrap_or(10))
        } else {
            Self::Visible
        }
    }

    pub fn to_config_str(&self) -> String {
        match self {
            Self::Visible => "visible".to_string(),
            Self::Recent(n) => format!("recent_{n}"),
            Self::Full => "full".to_string(),
        }
    }
}

/// A single command entry from shell integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub cwd: Option<String>,
    pub output: Option<String>,
    pub output_line_count: usize,
}

/// Environment metadata from shell integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
}

/// Terminal dimensions and cursor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub cols: usize,
    pub rows: usize,
    pub cursor: (usize, usize),
}

/// Complete terminal state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    pub timestamp: String,
    pub scope: String,
    pub environment: EnvironmentInfo,
    pub terminal: TerminalInfo,
    pub commands: Vec<CommandEntry>,
}

impl SnapshotData {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Gather a snapshot from the terminal manager.
    ///
    /// Pulls command history, environment info, and terminal state from
    /// the `TerminalManager` and packages it into a `SnapshotData` according
    /// to the requested `scope`.
    ///
    /// # Arguments
    /// * `terminal` - The terminal manager to read state from
    /// * `scope` - Controls how much history to include
    /// * `_max_output_lines` - Reserved for future per-command output capture
    pub fn gather(
        terminal: &crate::terminal::TerminalManager,
        scope: &SnapshotScope,
        _max_output_lines: usize,
    ) -> Self {
        // Get command history from core library via shell integration.
        // Each entry is (command_text, exit_code, duration_ms).
        let history = terminal.core_command_history();

        let commands_to_include: Vec<_> = match scope {
            SnapshotScope::Visible => {
                // Take recent commands (approximate visible window)
                history.iter().rev().take(10).rev().cloned().collect()
            }
            SnapshotScope::Recent(n) => {
                history.iter().rev().take(*n).rev().cloned().collect()
            }
            SnapshotScope::Full => history,
        };

        let cwd = terminal.shell_integration_cwd();

        let commands: Vec<CommandEntry> = commands_to_include
            .into_iter()
            .map(|(cmd, exit_code, duration_ms)| CommandEntry {
                command: cmd,
                exit_code,
                duration_ms: duration_ms.unwrap_or(0),
                cwd: cwd.clone(),
                output: None,
                output_line_count: 0,
            })
            .collect();

        let (cursor_col, cursor_row) = terminal.cursor_position();

        let environment = EnvironmentInfo {
            hostname: terminal.shell_integration_hostname(),
            username: terminal.shell_integration_username(),
            cwd,
            shell: None,
        };

        let (cols, rows) = terminal.dimensions();

        let terminal_info = TerminalInfo {
            cols,
            rows,
            cursor: (cursor_col, cursor_row),
        };

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            scope: scope.to_config_str(),
            environment,
            terminal: terminal_info,
            commands,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_from_config_str() {
        assert_eq!(
            SnapshotScope::from_config_str("visible"),
            SnapshotScope::Visible
        );
        assert_eq!(
            SnapshotScope::from_config_str("full"),
            SnapshotScope::Full
        );
        assert_eq!(
            SnapshotScope::from_config_str("recent_10"),
            SnapshotScope::Recent(10)
        );
        assert_eq!(
            SnapshotScope::from_config_str("recent_25"),
            SnapshotScope::Recent(25)
        );
        assert_eq!(
            SnapshotScope::from_config_str("unknown"),
            SnapshotScope::Visible
        );
    }

    #[test]
    fn test_scope_roundtrip() {
        let scopes = vec![
            SnapshotScope::Visible,
            SnapshotScope::Full,
            SnapshotScope::Recent(10),
        ];
        for scope in scopes {
            let s = scope.to_config_str();
            assert_eq!(SnapshotScope::from_config_str(&s), scope);
        }
    }

    #[test]
    fn test_snapshot_to_json() {
        let snapshot = SnapshotData {
            timestamp: "2026-02-15T10:00:00Z".to_string(),
            scope: "visible".to_string(),
            environment: EnvironmentInfo {
                hostname: Some("test-host".to_string()),
                username: Some("user".to_string()),
                cwd: Some("/home/user".to_string()),
                shell: Some("zsh".to_string()),
            },
            terminal: TerminalInfo {
                cols: 80,
                rows: 24,
                cursor: (0, 0),
            },
            commands: vec![CommandEntry {
                command: "echo hello".to_string(),
                exit_code: Some(0),
                duration_ms: 100,
                cwd: Some("/home/user".to_string()),
                output: Some("hello\n".to_string()),
                output_line_count: 1,
            }],
        };
        let json = snapshot.to_json().unwrap();
        assert!(json.contains("echo hello"));
        assert!(json.contains("test-host"));
    }
}
