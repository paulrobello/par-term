//! Default values for terminal-behaviour settings.

/// Default scrollback buffer size in lines.
pub fn scrollback() -> usize {
    10000
}

/// Default login shell flag (true = start as login shell).
pub fn login_shell() -> bool {
    true
}

/// Default initial text sent to the shell on startup.
pub fn initial_text() -> String {
    String::new()
}

/// Default delay in milliseconds before sending initial text.
pub fn initial_text_delay_ms() -> u64 {
    100
}

/// Default flag controlling whether a newline is appended to initial text.
pub fn initial_text_send_newline() -> bool {
    true
}

/// Default scrollbar position (`"right"` or `"left"`).
pub fn scrollbar_position() -> String {
    "right".to_string()
}

/// Default scrollbar width in pixels.
pub fn scrollbar_width() -> f32 {
    15.0
}

/// Default scrollbar auto-hide delay in milliseconds (0 = never auto-hide).
pub fn scrollbar_autohide_delay() -> u64 {
    0 // 0 = never auto-hide (always visible when scrollback exists)
}

/// Default paste delay in milliseconds between chunks (0 = no delay).
pub fn paste_delay_ms() -> u64 {
    0 // No delay by default
}

/// Default maximum number of clipboard sync events to buffer.
pub fn clipboard_max_sync_events() -> usize {
    64 // Aligned with sister project
}

/// Default maximum bytes per clipboard sync event.
pub fn clipboard_max_event_bytes() -> usize {
    2048 // Aligned with sister project
}

/// Default activity threshold in seconds before a tab is considered idle.
pub fn activity_threshold() -> u64 {
    10 // Aligned with sister project (10 seconds)
}

/// Default anti-idle keep-alive interval in seconds.
pub fn anti_idle_seconds() -> u64 {
    60 // Default keep-alive interval: 60 seconds
}

/// Default anti-idle keep-alive byte code sent to the PTY.
pub fn anti_idle_code() -> u8 {
    0 // Default keep-alive code: NUL (0x00)
}

/// Default silence threshold in seconds before a silence notification fires.
pub fn silence_threshold() -> u64 {
    300 // 5 minutes
}

/// Default maximum number of notification lines to buffer.
pub fn notification_max_buffer() -> usize {
    64 // Aligned with sister project
}

/// Default mouse scroll speed in lines per scroll tick.
pub fn scroll_speed() -> f32 {
    3.0 // Lines per scroll tick
}

/// Default double-click interval threshold in milliseconds.
pub fn double_click_threshold() -> u64 {
    500 // 500 milliseconds
}

/// Default triple-click interval threshold in milliseconds.
pub fn triple_click_threshold() -> u64 {
    500 // 500 milliseconds (same as double-click)
}

/// Default cursor blink interval in milliseconds.
pub fn cursor_blink_interval() -> u64 {
    500 // 500 milliseconds (blink twice per second)
}

/// Default bell sound volume (0–100 percent).
pub fn bell_sound() -> u8 {
    50 // Default to 50% volume
}

/// Default set of non-alphanumeric characters treated as part of a word for double-click selection.
pub fn word_characters() -> String {
    // Default characters considered part of a word (in addition to alphanumeric)
    // Matches iTerm2's default: /-+\~_.
    "/-+\\~_.".to_string()
}

/// Default flag enabling smart selection for URLs and file paths.
pub fn smart_selection_enabled() -> bool {
    true // Smart selection enabled by default
}

/// Default answerback string sent in response to ENQ (empty = disabled).
pub fn answerback_string() -> String {
    String::new() // Empty/disabled by default for security
}

/// Default semantic history editor command
/// Empty string means auto-detect from $EDITOR or use system default
pub fn semantic_history_editor() -> String {
    String::new() // Auto-detect by default
}

/// Default list of jobs/processes to ignore when checking for running jobs
/// These are common shells and utilities that shouldn't block tab close
pub fn jobs_to_ignore() -> Vec<String> {
    vec![
        // Common shells - these are the parent process, not "jobs"
        "bash".to_string(),
        "zsh".to_string(),
        "fish".to_string(),
        "sh".to_string(),
        "dash".to_string(),
        "ksh".to_string(),
        "tcsh".to_string(),
        "csh".to_string(),
        // Common pagers and viewers
        "less".to_string(),
        "more".to_string(),
        "man".to_string(),
        // Common utilities that are often left running
        "cat".to_string(),
        "sleep".to_string(),
    ]
}

/// Default session log directory (XDG-compliant: `~/.local/share/par-term/logs/`).
pub fn session_log_directory() -> String {
    // XDG-compliant default: ~/.local/share/par-term/logs/
    if let Some(home) = dirs::home_dir() {
        home.join(".local")
            .join("share")
            .join("par-term")
            .join("logs")
            .to_string_lossy()
            .to_string()
    } else {
        "logs".to_string()
    }
}

/// Default maximum number of command history entries to persist across sessions.
pub fn command_history_max_entries() -> usize {
    1000 // Maximum number of commands to persist across sessions
}

/// Default session undo timeout in seconds.
pub fn session_undo_timeout_secs() -> u32 {
    5
}

/// Default maximum number of session undo entries.
pub fn session_undo_max_entries() -> usize {
    10
}

/// Default flag controlling whether the shell process is preserved on session undo.
pub fn session_undo_preserve_shell() -> bool {
    false
}
