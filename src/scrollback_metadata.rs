use std::collections::HashMap;

use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;
use par_term_emu_core_rust::terminal::CommandExecution;

/// Lightweight snapshot of a completed command taken from the core library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSnapshot {
    pub id: usize,
    pub command: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

impl CommandSnapshot {
    pub fn from_core(command: &CommandExecution, id: usize) -> Self {
        Self {
            id,
            command: Some(command.command.clone()),
            start_time: command.start_time,
            end_time: command.end_time,
            exit_code: command.exit_code,
            duration_ms: command.duration_ms,
        }
    }
}

/// Public-facing metadata for a mark anchored to a scrollback line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollbackMark {
    pub line: usize,
    pub exit_code: Option<i32>,
    pub start_time: Option<u64>,
    pub duration_ms: Option<u64>,
    pub command: Option<String>,
}

/// Metadata for displaying timing/command info for a specific line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineMetadata {
    pub line: usize,
    pub exit_code: Option<i32>,
    pub start_time: Option<u64>,
    pub duration_ms: Option<u64>,
    pub command: Option<String>,
}

#[derive(Default)]
pub struct ScrollbackMetadata {
    /// Map of prompt/mark line -> command id
    line_to_command: HashMap<usize, usize>,
    /// Map of command id -> command info
    commands: HashMap<usize, CommandSnapshot>,
    /// Prompt/mark line indices sorted ascending
    prompt_lines: Vec<usize>,
    /// Optional timestamp for lines (ms since epoch)
    line_timestamps: HashMap<usize, u64>,
    /// Current command start line (absolute)
    current_command_start: Option<usize>,
    /// Last marker we processed to avoid duplicate events
    last_marker: Option<ShellIntegrationMarker>,
    /// Number of commands already recorded (matches command_history.len())
    last_recorded_history_len: usize,
}

impl ScrollbackMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the latest shell integration marker and update internal metadata.
    ///
    /// `scrollback_len` is the current scrollback length (lines off-screen).
    /// `cursor_row` is the zero-based cursor row on the visible screen.
    /// `history_len` is the current command history length from the core library.
    /// `last_command` should contain the most recent command when `history_len` > 0.
    pub fn apply_event(
        &mut self,
        marker: Option<ShellIntegrationMarker>,
        scrollback_len: usize,
        cursor_row: usize,
        history_len: usize,
        last_command: Option<CommandSnapshot>,
    ) {
        let absolute_line = scrollback_len.saturating_add(cursor_row);

        match marker {
            Some(ShellIntegrationMarker::PromptStart) => {
                self.record_prompt_line(absolute_line, last_command.as_ref().map(|c| c.start_time));
            }
            Some(ShellIntegrationMarker::CommandStart)
            | Some(ShellIntegrationMarker::CommandExecuted) => {
                self.current_command_start = Some(absolute_line);
            }
            Some(ShellIntegrationMarker::CommandFinished) => {
                #[allow(clippy::collapsible_if)]
                if history_len > self.last_recorded_history_len {
                    if let Some(cmd) = last_command {
                        self.finish_command(absolute_line, cmd);
                        self.last_recorded_history_len = history_len;
                    }
                }
            }
            _ => {}
        }

        self.last_marker = marker;
    }

    /// Produce a list of marks suitable for rendering or navigation.
    pub fn marks(&self) -> Vec<ScrollbackMark> {
        let mut marks = Vec::with_capacity(self.prompt_lines.len());

        for line in &self.prompt_lines {
            let command_id = self.line_to_command.get(line);
            let (exit_code, start_time, duration_ms, command) = command_id
                .and_then(|id| self.commands.get(id))
                .map(|cmd| {
                    (
                        cmd.exit_code,
                        Some(cmd.start_time),
                        cmd.duration_ms,
                        cmd.command.clone(),
                    )
                })
                .unwrap_or((None, None, None, None));

            marks.push(ScrollbackMark {
                line: *line,
                exit_code,
                start_time,
                duration_ms,
                command,
            });
        }

        marks
    }

    /// Retrieve metadata for a specific absolute line index, if available.
    pub fn metadata_for_line(&self, line: usize) -> Option<LineMetadata> {
        let command_id = self.line_to_command.get(&line);
        let base = command_id
            .and_then(|id| self.commands.get(id))
            .map(|cmd| LineMetadata {
                line,
                exit_code: cmd.exit_code,
                start_time: Some(cmd.start_time),
                duration_ms: cmd.duration_ms,
                command: cmd.command.clone(),
            });

        if base.is_some() {
            return base;
        }

        self.line_timestamps.get(&line).map(|ts| LineMetadata {
            line,
            exit_code: None,
            start_time: Some(*ts),
            duration_ms: None,
            command: None,
        })
    }

    /// Find the previous mark (prompt) before the given absolute line.
    pub fn previous_mark(&self, line: usize) -> Option<usize> {
        match self.prompt_lines.binary_search(&line) {
            Ok(idx) => {
                if idx > 0 {
                    Some(self.prompt_lines[idx - 1])
                } else {
                    None
                }
            }
            Err(idx) => idx
                .checked_sub(1)
                .and_then(|i| self.prompt_lines.get(i).copied()),
        }
    }

    /// Find the next mark (prompt) after the given absolute line.
    pub fn next_mark(&self, line: usize) -> Option<usize> {
        match self.prompt_lines.binary_search(&line) {
            Ok(idx) => self.prompt_lines.get(idx + 1).copied(),
            Err(idx) => self.prompt_lines.get(idx).copied(),
        }
    }

    fn record_prompt_line(&mut self, line: usize, timestamp: Option<u64>) {
        if let Err(pos) = self.prompt_lines.binary_search(&line) {
            self.prompt_lines.insert(pos, line);
        }
        if let Some(ts) = timestamp {
            self.line_timestamps.entry(line).or_insert(ts);
        }
    }

    fn finish_command(&mut self, end_line: usize, command: CommandSnapshot) {
        let start_line = self
            .current_command_start
            .take()
            .or_else(|| self.prompt_lines.last().copied())
            .unwrap_or(end_line);

        self.line_to_command.insert(start_line, command.id);
        let start_time = command.start_time;
        self.commands.insert(command.id, command);
        self.line_timestamps.entry(start_line).or_insert(start_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(id: usize, exit_code: i32, start_time: u64, duration_ms: u64) -> CommandSnapshot {
        CommandSnapshot {
            id,
            command: Some(format!("cmd-{id}")),
            start_time,
            end_time: Some(start_time + duration_ms),
            exit_code: Some(exit_code),
            duration_ms: Some(duration_ms),
        }
    }

    #[test]
    fn records_prompt_and_command() {
        let mut meta = ScrollbackMetadata::new();

        meta.apply_event(Some(ShellIntegrationMarker::PromptStart), 10, 5, 0, None);
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandExecuted),
            10,
            5,
            0,
            None,
        );

        meta.apply_event(
            Some(ShellIntegrationMarker::CommandFinished),
            12,
            3,
            1,
            Some(snapshot(0, 0, 1_000, 500)),
        );

        let marks = meta.marks();
        assert_eq!(marks.len(), 1);
        let mark = &marks[0];
        assert_eq!(mark.line, 15); // scrollback_len 10 + cursor_row 5
        assert_eq!(mark.exit_code, Some(0));
        assert_eq!(mark.start_time, Some(1_000));
    }

    #[test]
    fn navigation_prev_next() {
        let mut meta = ScrollbackMetadata::new();

        meta.apply_event(Some(ShellIntegrationMarker::PromptStart), 5, 0, 0, None);
        meta.apply_event(Some(ShellIntegrationMarker::PromptStart), 8, 2, 0, None);

        assert_eq!(meta.previous_mark(7), Some(5));
        assert_eq!(meta.next_mark(5), Some(10));
    }
}
