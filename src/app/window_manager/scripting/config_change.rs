//! Script command tokenisation and config-change application.
//!
//! Contains:
//! - `PendingScriptAction` — deferred script command actions
//! - `tokenise_command` — split a command string into program + args
//! - `apply_script_config_change` — apply an allowlisted config key from a script

use super::WindowManager;

/// Actions deferred out of the script-command loop so they can be executed
/// after the `Tab`/`WindowState` borrows are released.
///
/// Commands that need to call methods on `WindowState` (e.g. notifications,
/// badge updates, PTY writes) cannot be executed while the first block of
/// `sync_script_running_state` holds mutable borrows on `self.windows` and
/// `ws.tab_manager`. They are collected here and dispatched in a second pass.
pub(super) enum PendingScriptAction {
    /// Show a desktop/in-app notification.
    Notify { title: String, body: String },
    /// Set the active tab's badge text override.
    SetBadge { text: String },
    /// Set a named user variable in the badge/session context.
    SetVariable { name: String, value: String },
    /// Inject sanitised text into the active PTY (gated by `allow_write_text`).
    WriteText { text: String, config_index: usize },
    /// Spawn an external process (gated by `allow_run_command`).
    RunCommand {
        command: String,
        config_index: usize,
    },
    /// Modify a runtime config key (gated by `allow_change_config`).
    ChangeConfig {
        key: String,
        value: serde_json::Value,
        config_index: usize,
    },
}

/// Tokenise a command string into `(program, args)` without invoking a shell.
///
/// Splits on ASCII whitespace and respects double-quoted spans.  Single-char
/// escape sequences inside quotes are **not** handled — the intent is purely to
/// separate tokens, not to emulate a full shell parser.
///
/// **Limitations**: Single quotes (`'arg with spaces'`) and backslash escapes
/// (`arg\ with\ spaces`) are not supported.  Scripts should use double quotes
/// for arguments containing whitespace.
///
/// Returns `None` if the string is empty or contains only whitespace.
pub(super) fn tokenise_command(command: &str) -> Option<(String, Vec<String>)> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in command.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return None;
    }
    let program = tokens.remove(0);
    Some((program, tokens))
}

impl WindowManager {
    /// Apply a `ChangeConfig` command to the given window state.
    ///
    /// Only keys in the runtime allowlist are accepted; unknown or unsafe keys
    /// are rejected with a warning log and no state is modified.
    ///
    /// # Allowlisted keys
    ///
    /// | Key                      | Type  | Constraints         |
    /// |--------------------------|-------|---------------------|
    /// | `font_size`              | f64   | clamped 6 – 72      |
    /// | `window_opacity`         | f64   | clamped 0.0 – 1.0   |
    /// | `scrollback_lines`       | u64   | unconstrained       |
    /// | `cursor_blink`           | bool  |                     |
    /// | `notification_bell_desktop` | bool |                  |
    /// | `notification_bell_visual`  | bool |                  |
    pub(super) fn apply_script_config_change(
        ws: &mut crate::app::window_state::WindowState,
        key: &str,
        value: &serde_json::Value,
        config_index: usize,
    ) {
        match key {
            "font_size" => {
                if let Some(v) = value.as_f64() {
                    let new_size = (v as f32).clamp(6.0, 72.0);
                    ws.config.font_size = new_size;
                    ws.focus_state.needs_redraw = true;
                    ws.request_redraw();
                    log::info!(
                        "Script[{}] ChangeConfig: font_size = {}",
                        config_index,
                        new_size
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: font_size expected number, got {:?}",
                        config_index,
                        value
                    );
                }
            }
            "window_opacity" => {
                if let Some(v) = value.as_f64() {
                    let new_opacity = (v as f32).clamp(0.0, 1.0);
                    ws.config.window_opacity = new_opacity;
                    if let Some(renderer) = &mut ws.renderer {
                        renderer.update_opacity(new_opacity);
                    }
                    ws.focus_state.needs_redraw = true;
                    ws.request_redraw();
                    log::info!(
                        "Script[{}] ChangeConfig: window_opacity = {}",
                        config_index,
                        new_opacity
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: window_opacity expected number, got {:?}",
                        config_index,
                        value
                    );
                }
            }
            "scrollback_lines" => {
                if let Some(v) = value.as_u64() {
                    ws.config.scrollback_lines = v as usize;
                    log::info!(
                        "Script[{}] ChangeConfig: scrollback_lines = {}",
                        config_index,
                        v
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: scrollback_lines expected integer, got {:?}",
                        config_index,
                        value
                    );
                }
            }
            "cursor_blink" => {
                if let Some(v) = value.as_bool() {
                    ws.config.cursor_blink = v;
                    ws.focus_state.needs_redraw = true;
                    ws.request_redraw();
                    log::info!(
                        "Script[{}] ChangeConfig: cursor_blink = {}",
                        config_index,
                        v
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: cursor_blink expected bool, got {:?}",
                        config_index,
                        value
                    );
                }
            }
            "notification_bell_desktop" => {
                if let Some(v) = value.as_bool() {
                    ws.config.notification_bell_desktop = v;
                    log::info!(
                        "Script[{}] ChangeConfig: notification_bell_desktop = {}",
                        config_index,
                        v
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: notification_bell_desktop expected bool, \
                         got {:?}",
                        config_index,
                        value
                    );
                }
            }
            "notification_bell_visual" => {
                if let Some(v) = value.as_bool() {
                    ws.config.notification_bell_visual = v;
                    ws.focus_state.needs_redraw = true;
                    ws.request_redraw();
                    log::info!(
                        "Script[{}] ChangeConfig: notification_bell_visual = {}",
                        config_index,
                        v
                    );
                } else {
                    log::warn!(
                        "Script[{}] ChangeConfig: notification_bell_visual expected bool, \
                         got {:?}",
                        config_index,
                        value
                    );
                }
            }
            _ => {
                log::warn!(
                    "Script[{}] ChangeConfig: key '{}' not in runtime allowlist",
                    config_index,
                    key
                );
            }
        }
    }
}
