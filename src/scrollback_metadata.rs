use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
    /// Custom color override (from trigger marks). When set, overrides exit_code-based coloring.
    pub color: Option<(u8, u8, u8)>,
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
    /// Line where last marker was observed (to de-dupe identical repeats)
    last_marker_line: Option<usize>,
    /// Last exit code seen from shell integration (for synthetic finishes)
    last_exit_code: Option<i32>,
    /// Line where we last consumed an exit code event
    last_exit_code_line: Option<usize>,
    /// Number of commands already recorded (matches command_history.len())
    last_recorded_history_len: usize,
    /// Wall-clock start time for the current command (ms since epoch)
    current_command_start_time_ms: Option<u64>,
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
        last_exit_code: Option<i32>,
    ) {
        let last_command_clone = last_command.clone();
        let absolute_line = scrollback_len.saturating_add(cursor_row);
        let repeat_marker =
            marker == self.last_marker && Some(absolute_line) == self.last_marker_line;
        let mut finished_command = false;

        match marker {
            Some(ShellIntegrationMarker::PromptStart) => {
                if !repeat_marker {
                    self.record_prompt_line(
                        absolute_line,
                        last_command.as_ref().map(|c| c.start_time),
                    );
                }
            }
            Some(ShellIntegrationMarker::CommandStart)
            | Some(ShellIntegrationMarker::CommandExecuted) => {
                if !repeat_marker {
                    // Record a mark even if the prompt (A) never arrived.
                    self.record_prompt_line(absolute_line, Some(now_ms()));
                }
                self.current_command_start = Some(absolute_line);
                self.current_command_start_time_ms = Some(now_ms());
            }
            Some(ShellIntegrationMarker::CommandFinished) => {
                #[allow(clippy::collapsible_if)]
                if history_len > self.last_recorded_history_len {
                    if let Some(cmd) = last_command {
                        let start_line = self.finish_command(absolute_line, cmd);
                        self.last_recorded_history_len = history_len;
                        self.last_exit_code_line = Some(start_line);
                        finished_command = true;
                    }
                } else if let Some(exit_code) = last_exit_code {
                    // Shell reported completion but core history did not advance
                    // (common when shell integration markers are emitted but
                    // command history tracking is not wired up). Synthesize a
                    // minimal snapshot so mark indicators still render.
                    let start_time = self.current_command_start_time_ms.unwrap_or_else(now_ms);
                    let end_time = now_ms();
                    let duration_ms = end_time.saturating_sub(start_time);
                    let id = self.last_recorded_history_len;
                    let synthetic = CommandSnapshot {
                        id,
                        command: None,
                        start_time,
                        end_time: Some(end_time),
                        exit_code: Some(exit_code),
                        duration_ms: Some(duration_ms),
                    };
                    let start_line = self.finish_command(absolute_line, synthetic);
                    // Keep ids monotonic to avoid duplicate marks on repeated frames
                    self.last_recorded_history_len =
                        self.last_recorded_history_len.saturating_add(1);
                    self.last_exit_code_line = Some(start_line);
                    finished_command = true;
                }
            }
            _ => {}
        }

        // Fallback: if command history advanced but we didn't see a CommandFinished marker,
        // still record a mark at the current line so users get indicators when shell integration
        // scripts emit timestamps but markers are missing.
        if history_len > self.last_recorded_history_len
            && let Some(ref cmd) = last_command_clone
        {
            let start_line = self.finish_command(absolute_line, cmd.clone());
            self.last_recorded_history_len = history_len;
            self.last_exit_code_line = Some(start_line);
            finished_command = true;
        }

        // Some shells emit the exit code but not a CommandFinished marker. If the exit code
        // changed or arrived on a new prompt line, synthesize a completion using the latest
        // marker location so scrollbar marks get colored correctly.
        if !finished_command && let Some(code) = last_exit_code {
            let candidate_line = self
                .current_command_start
                .or_else(|| self.prompt_lines.last().copied())
                .unwrap_or(absolute_line);

            let exit_event_is_new = Some(candidate_line) != self.last_exit_code_line
                || Some(code) != self.last_exit_code;

            if exit_event_is_new {
                let start_time = self.current_command_start_time_ms.unwrap_or_else(now_ms);
                let end_time = now_ms();
                let duration_ms = end_time.saturating_sub(start_time);
                let id = self.last_recorded_history_len;
                let synthetic = CommandSnapshot {
                    id,
                    command: last_command_clone.as_ref().and_then(|c| c.command.clone()),
                    start_time,
                    end_time: Some(end_time),
                    exit_code: Some(code),
                    duration_ms: Some(duration_ms),
                };
                self.finish_command(candidate_line, synthetic);
                self.last_recorded_history_len = self.last_recorded_history_len.saturating_add(1);
                self.last_exit_code_line = Some(candidate_line);
            }
        }

        self.last_marker = marker;
        self.last_marker_line = Some(absolute_line);
        self.last_exit_code = last_exit_code;
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
                color: None,
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

    /// Set the command text on the most recent mark that doesn't have one.
    ///
    /// Called after `apply_event()` when the frontend has extracted command text
    /// from the terminal grid. This fills in the `command` field on synthetic
    /// snapshots that would otherwise have `command: None`.
    pub fn set_latest_mark_command(&mut self, command: String) {
        if let Some(&line) = self.prompt_lines.last()
            && let Some(id) = self.line_to_command.get(&line)
            && let Some(snapshot) = self.commands.get_mut(id)
            && snapshot.command.is_none()
        {
            snapshot.command = Some(command);
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

    fn finish_command(&mut self, end_line: usize, command: CommandSnapshot) -> usize {
        let start_line = self
            .current_command_start
            .take()
            .or_else(|| self.prompt_lines.last().copied())
            .unwrap_or(end_line);
        self.current_command_start_time_ms = None;

        // Ensure a mark exists even if no prompt marker was recorded.
        self.record_prompt_line(start_line, Some(command.start_time));

        self.line_to_command.insert(start_line, command.id);
        let start_time = command.start_time;
        self.commands.insert(command.id, command);
        self.line_timestamps.entry(start_line).or_insert(start_time);
        start_line
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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

        meta.apply_event(
            Some(ShellIntegrationMarker::PromptStart),
            10,
            5,
            0,
            None,
            None,
        );
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandExecuted),
            10,
            5,
            0,
            None,
            None,
        );

        meta.apply_event(
            Some(ShellIntegrationMarker::CommandFinished),
            12,
            3,
            1,
            Some(snapshot(0, 0, 1_000, 500)),
            None,
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

        meta.apply_event(
            Some(ShellIntegrationMarker::PromptStart),
            5,
            0,
            0,
            None,
            None,
        );
        meta.apply_event(
            Some(ShellIntegrationMarker::PromptStart),
            8,
            2,
            0,
            None,
            None,
        );

        assert_eq!(meta.previous_mark(7), Some(5));
        assert_eq!(meta.next_mark(5), Some(10));
    }

    #[test]
    fn records_when_history_advances_without_marker() {
        let mut meta = ScrollbackMetadata::new();
        let cmd = snapshot(0, 1, 2_000, 300);

        // No marker but history length increased
        meta.apply_event(None, 12, 3, 1, Some(cmd), Some(1));

        let marks = meta.marks();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].line, 15);
        assert_eq!(marks[0].exit_code, Some(1));
    }

    #[test]
    fn records_when_exit_code_arrives_without_history() {
        let mut meta = ScrollbackMetadata::new();

        // Simulate prompt and command start
        meta.apply_event(
            Some(ShellIntegrationMarker::PromptStart),
            20,
            0,
            0,
            None,
            None,
        );
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandStart),
            20,
            0,
            0,
            None,
            None,
        );

        // Command finishes, shell sends exit code but core history does not advance
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandFinished),
            22,
            1,
            0,
            None,
            Some(42),
        );

        let marks = meta.marks();
        assert_eq!(marks.len(), 1);
        // start line = prompt at 20 + cursor 0 = 20
        assert_eq!(marks[0].line, 20);
        assert_eq!(marks[0].exit_code, Some(42));
        assert!(marks[0].start_time.is_some());
        assert!(marks[0].duration_ms.is_some());
    }

    #[test]
    fn synthesizes_exit_code_without_finished_marker() {
        let mut meta = ScrollbackMetadata::new();

        meta.apply_event(
            Some(ShellIntegrationMarker::CommandStart),
            0,
            0,
            0,
            None,
            None,
        );

        meta.apply_event(None, 0, 1, 0, None, Some(7));

        let marks = meta.marks();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].line, 0);
        assert_eq!(marks[0].exit_code, Some(7));
    }

    #[test]
    fn records_multiple_commands_when_history_missing() {
        let mut meta = ScrollbackMetadata::new();

        // First command (exit 0)
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandStart),
            0,
            0,
            0,
            None,
            None,
        );
        meta.apply_event(None, 0, 0, 0, None, Some(0));

        // Second command, same exit code but new prompt line
        meta.apply_event(
            Some(ShellIntegrationMarker::CommandStart),
            10,
            0,
            0,
            None,
            None,
        );
        meta.apply_event(None, 10, 0, 0, None, Some(0));

        let marks = meta.marks();
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].exit_code, Some(0));
        assert_eq!(marks[1].exit_code, Some(0));
        assert_eq!(marks[1].line, 10);
    }
}
