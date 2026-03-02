//! Default values for terminal-behaviour settings.

pub fn scrollback() -> usize {
    10000
}

pub fn login_shell() -> bool {
    true
}

pub fn initial_text() -> String {
    String::new()
}

pub fn initial_text_delay_ms() -> u64 {
    100
}

pub fn initial_text_send_newline() -> bool {
    true
}

pub fn scrollbar_position() -> String {
    "right".to_string()
}

pub fn scrollbar_width() -> f32 {
    15.0
}

pub fn scrollbar_autohide_delay() -> u64 {
    0 // 0 = never auto-hide (always visible when scrollback exists)
}

pub fn paste_delay_ms() -> u64 {
    0 // No delay by default
}

pub fn clipboard_max_sync_events() -> usize {
    64 // Aligned with sister project
}

pub fn clipboard_max_event_bytes() -> usize {
    2048 // Aligned with sister project
}

pub fn activity_threshold() -> u64 {
    10 // Aligned with sister project (10 seconds)
}

pub fn anti_idle_seconds() -> u64 {
    60 // Default keep-alive interval: 60 seconds
}

pub fn anti_idle_code() -> u8 {
    0 // Default keep-alive code: NUL (0x00)
}

pub fn silence_threshold() -> u64 {
    300 // 5 minutes
}

pub fn notification_max_buffer() -> usize {
    64 // Aligned with sister project
}

pub fn scroll_speed() -> f32 {
    3.0 // Lines per scroll tick
}

pub fn double_click_threshold() -> u64 {
    500 // 500 milliseconds
}

pub fn triple_click_threshold() -> u64 {
    500 // 500 milliseconds (same as double-click)
}

pub fn cursor_blink_interval() -> u64 {
    500 // 500 milliseconds (blink twice per second)
}

pub fn bell_sound() -> u8 {
    50 // Default to 50% volume
}

pub fn word_characters() -> String {
    // Default characters considered part of a word (in addition to alphanumeric)
    // Matches iTerm2's default: /-+\~_.
    "/-+\\~_.".to_string()
}

pub fn smart_selection_enabled() -> bool {
    true // Smart selection enabled by default
}

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

pub fn command_history_max_entries() -> usize {
    1000 // Maximum number of commands to persist across sessions
}

pub fn session_undo_timeout_secs() -> u32 {
    5
}

pub fn session_undo_max_entries() -> usize {
    10
}

pub fn session_undo_preserve_shell() -> bool {
    false
}
