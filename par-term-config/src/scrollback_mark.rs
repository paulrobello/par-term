/// Public-facing metadata for a mark anchored to a scrollback line.
///
/// This is a shared type used by both the terminal module (which creates marks
/// from shell integration events) and the renderer module (which displays
/// marks in the scrollbar and separator lines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollbackMark {
    /// The absolute scrollback line index this mark is anchored to.
    pub line: usize,
    /// Exit code of the command at this mark, if captured by shell integration.
    pub exit_code: Option<i32>,
    /// Unix timestamp (seconds) when the command started, if captured.
    pub start_time: Option<u64>,
    /// Elapsed time in milliseconds for the command, if captured.
    pub duration_ms: Option<u64>,
    /// The command string at this mark, if captured.
    pub command: Option<String>,
    /// Custom color override (from trigger marks). When set, overrides exit_code-based coloring.
    pub color: Option<(u8, u8, u8)>,
    /// Trigger ID that created this mark (None for shell integration marks).
    /// Used for deduplication: the same trigger matching the same physical line
    /// across multiple scans produces marks at different absolute positions.
    pub trigger_id: Option<u64>,
}
