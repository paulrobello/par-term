//! Script lifecycle management for the window manager.
//!
//! This module handles starting and stopping scripts on the active tab,
//! forwarding terminal events to scripts, reading script commands/output,
//! and syncing their running state to the settings window UI.

use std::process::Stdio;

use super::WindowManager;

/// Actions deferred out of the script-command loop so they can be executed
/// after the `Tab`/`WindowState` borrows are released.
///
/// Commands that need to call methods on `WindowState` (e.g. notifications,
/// badge updates, PTY writes) cannot be executed while the first block of
/// `sync_script_running_state` holds mutable borrows on `self.windows` and
/// `ws.tab_manager`. They are collected here and dispatched in a second pass.
enum PendingScriptAction {
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
fn tokenise_command(command: &str) -> Option<(String, Vec<String>)> {
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
    /// Start a script by config index on the focused window's active tab.
    pub fn start_script(&mut self, config_index: usize) {
        crate::debug_info!(
            "SCRIPT",
            "start_script called with config_index={}",
            config_index
        );
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            crate::debug_info!(
                "SCRIPT",
                "start_script: ws.config.scripts.len()={}, tab.script_ids.len()={}",
                ws.config.scripts.len(),
                tab.script_ids.len()
            );
            if config_index >= ws.config.scripts.len() {
                crate::debug_error!(
                    "SCRIPT",
                    "Script config index {} out of range (scripts.len={})",
                    config_index,
                    ws.config.scripts.len()
                );
                return;
            }
            let script_config = &ws.config.scripts[config_index];
            crate::debug_info!(
                "SCRIPT",
                "start_script: found config name='{}' path='{}' enabled={} args={:?}",
                script_config.name,
                script_config.script_path,
                script_config.enabled,
                script_config.args
            );
            if !script_config.enabled {
                crate::debug_info!(
                    "SCRIPT",
                    "Script '{}' is disabled, not starting",
                    script_config.name
                );
                return;
            }

            // Build subscription filter from config
            let subscription_filter = if script_config.subscriptions.is_empty() {
                None
            } else {
                Some(
                    script_config
                        .subscriptions
                        .iter()
                        .cloned()
                        .collect::<std::collections::HashSet<String>>(),
                )
            };

            // Create the event forwarder and register it as an observer
            let forwarder = std::sync::Arc::new(
                crate::scripting::observer::ScriptEventForwarder::new(subscription_filter),
            );

            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            let observer_id = {
                let term = tab.terminal.blocking_write();
                term.add_observer(forwarder.clone())
            };

            // Start the script process
            crate::debug_info!("SCRIPT", "start_script: spawning process...");
            match tab.script_manager.start_script(script_config) {
                Ok(script_id) => {
                    crate::debug_info!(
                        "SCRIPT",
                        "start_script: SUCCESS script_id={} observer_id={:?}",
                        script_id,
                        observer_id
                    );

                    // Ensure vecs are large enough
                    while tab.script_ids.len() <= config_index {
                        tab.script_ids.push(None);
                    }
                    while tab.script_observer_ids.len() <= config_index {
                        tab.script_observer_ids.push(None);
                    }
                    while tab.script_forwarders.len() <= config_index {
                        tab.script_forwarders.push(None);
                    }

                    tab.script_ids[config_index] = Some(script_id);
                    tab.script_observer_ids[config_index] = Some(observer_id);
                    tab.script_forwarders[config_index] = Some(forwarder);
                }
                Err(e) => {
                    let err_msg = format!("Failed to start: {}", e);
                    crate::debug_error!(
                        "SCRIPT",
                        "start_script: FAILED to start '{}': {}",
                        script_config.name,
                        e
                    );

                    // Acceptable risk: blocking_lock() in error cleanup path.
                    // See docs/CONCURRENCY.md for mutex strategy.
                    let term = tab.terminal.blocking_write();
                    term.remove_observer(observer_id);
                    drop(term);

                    // Show error in settings UI
                    if let Some(sw) = &mut self.settings_window {
                        let errors = &mut sw.settings_ui.script_errors;
                        while errors.len() <= config_index {
                            errors.push(String::new());
                        }
                        errors[config_index] = err_msg;
                        sw.request_redraw();
                    }
                    return;
                }
            }
            // Update running state in settings window
            self.sync_script_running_state();
        } else {
            crate::debug_error!(
                "SCRIPT",
                "start_script: no focused window or active tab found"
            );
        }
    }

    /// Stop a script by config index on the focused window's active tab.
    pub fn stop_script(&mut self, config_index: usize) {
        log::debug!("stop_script called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            // Stop the script process
            if let Some(Some(script_id)) = tab.script_ids.get(config_index).copied() {
                tab.script_manager.stop_script(script_id);
                log::info!(
                    "Stopped script at index {} (id={})",
                    config_index,
                    script_id
                );
            }

            // Acceptable risk: blocking_lock() from sync event loop for infrequent
            // user-initiated operation. See docs/CONCURRENCY.md for mutex strategy.
            if let Some(Some(observer_id)) = tab.script_observer_ids.get(config_index).copied() {
                let term = tab.terminal.blocking_write();
                term.remove_observer(observer_id);
                drop(term);
            }

            // Clear tracking state
            if let Some(slot) = tab.script_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_observer_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_forwarders.get_mut(config_index) {
                *slot = None;
            }

            // Update running state in settings window
            self.sync_script_running_state();
        }
    }

    /// Maximum number of output lines kept per script in the UI.
    const SCRIPT_OUTPUT_MAX_LINES: usize = 200;

    /// Sync script running state to the settings window.
    ///
    /// Drains events from forwarders, sends them to scripts, reads commands
    /// and errors back, and updates the settings UI state.
    pub fn sync_script_running_state(&mut self) {
        let focused = self.get_focused_window_id();

        // Pass 1 — Collect state from the active tab.
        //
        // Safe commands (Log, SetPanel, ClearPanel) are executed immediately.
        // Commands that need `WindowState` methods (Notify, SetBadge, etc.) or
        // require permission checks (WriteText, RunCommand, ChangeConfig) are
        // deferred into `pending_actions` and processed in Pass 2.
        #[allow(clippy::type_complexity)]
        let (running_state, error_state, new_output, panel_state, pending_actions): (
            Vec<bool>,
            Vec<String>,
            Vec<Vec<String>>,
            Vec<Option<(String, String)>>,
            Vec<PendingScriptAction>,
        ) = if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            let script_count = ws.config.scripts.len();
            let mut running = Vec::with_capacity(script_count);
            let mut errors = Vec::with_capacity(script_count);
            let mut output = Vec::with_capacity(script_count);
            let mut panels = Vec::with_capacity(script_count);
            let mut pending: Vec<PendingScriptAction> = Vec::new();

            for i in 0..script_count {
                let has_script_id = tab.script_ids.get(i).and_then(|opt| *opt);
                let is_running = has_script_id.is_some_and(|id| tab.script_manager.is_running(id));

                // Drain events from forwarder and send to script
                if is_running && let Some(Some(forwarder)) = tab.script_forwarders.get(i) {
                    let events = forwarder.drain_events();
                    if let Some(script_id) = has_script_id {
                        for event in &events {
                            let _ = tab.script_manager.send_event(script_id, event);
                        }
                    }
                }

                // Read commands from script and process them
                let mut log_lines = Vec::new();
                let mut panel_val = tab
                    .script_manager
                    .get_panel(has_script_id.unwrap_or(0))
                    .cloned();

                if let Some(script_id) = has_script_id {
                    let commands = tab.script_manager.read_commands(script_id);
                    for cmd in commands {
                        match cmd {
                            crate::scripting::protocol::ScriptCommand::Log { level, message } => {
                                log_lines.push(format!("[{}] {}", level, message));
                            }
                            crate::scripting::protocol::ScriptCommand::SetPanel {
                                title,
                                content,
                            } => {
                                tab.script_manager.set_panel(
                                    script_id,
                                    title.clone(),
                                    content.clone(),
                                );
                                panel_val = Some((title, content));
                            }
                            crate::scripting::protocol::ScriptCommand::ClearPanel {} => {
                                tab.script_manager.clear_panel(script_id);
                                panel_val = None;
                            }
                            // Safe display-only commands — defer to Pass 2 so they can
                            // call `WindowState` methods without borrow conflicts.
                            crate::scripting::protocol::ScriptCommand::Notify { title, body } => {
                                pending.push(PendingScriptAction::Notify { title, body });
                            }
                            crate::scripting::protocol::ScriptCommand::SetBadge { text } => {
                                pending.push(PendingScriptAction::SetBadge { text });
                            }
                            crate::scripting::protocol::ScriptCommand::SetVariable {
                                name,
                                value,
                            } => {
                                pending.push(PendingScriptAction::SetVariable { name, value });
                            }
                            // Restricted commands — permission-checked in Pass 2.
                            crate::scripting::protocol::ScriptCommand::WriteText { text } => {
                                pending.push(PendingScriptAction::WriteText {
                                    text,
                                    config_index: i,
                                });
                            }
                            crate::scripting::protocol::ScriptCommand::RunCommand { command } => {
                                pending.push(PendingScriptAction::RunCommand {
                                    command,
                                    config_index: i,
                                });
                            }
                            crate::scripting::protocol::ScriptCommand::ChangeConfig {
                                key,
                                value,
                            } => {
                                pending.push(PendingScriptAction::ChangeConfig {
                                    key,
                                    value,
                                    config_index: i,
                                });
                            }
                        }
                    }
                }

                // Read errors from script
                let err_text = if let Some(script_id) = has_script_id {
                    if is_running {
                        // Drain any stderr lines even while running
                        let err_lines = tab.script_manager.read_errors(script_id);
                        if !err_lines.is_empty() {
                            err_lines.join("\n")
                        } else {
                            String::new()
                        }
                    } else {
                        let err_lines = tab.script_manager.read_errors(script_id);
                        err_lines.join("\n")
                    }
                } else if let Some(sw) = &self.settings_window
                    && let Some(existing) = sw.settings_ui.script_errors.get(i)
                    && !existing.is_empty()
                {
                    existing.clone()
                } else {
                    String::new()
                };

                running.push(is_running);
                errors.push(err_text);
                output.push(log_lines);
                panels.push(panel_val);
            }

            (running, errors, output, panels, pending)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        // Pass 2 — Execute deferred actions that need `WindowState` access.
        //
        // The mutable borrow of `self.windows` from Pass 1 has been released,
        // so we can take a fresh mutable borrow here.
        if !pending_actions.is_empty()
            && let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
        {
            for action in pending_actions {
                match action {
                    // ── Notify ──────────────────────────────────────────────────
                    PendingScriptAction::Notify { title, body } => {
                        crate::debug_info!(
                            "SCRIPT",
                            "AUDIT Script Notify title={:?} body={:?}",
                            title,
                            body
                        );
                        ws.deliver_notification(&title, &body);
                    }

                    // ── SetBadge ────────────────────────────────────────────────
                    PendingScriptAction::SetBadge { text } => {
                        if let Some(tab) = ws.tab_manager.active_tab_mut() {
                            tab.badge_override = Some(text.clone());
                        }
                        ws.request_redraw();
                        crate::debug_info!("SCRIPT", "SetBadge text={:?}", text);
                    }

                    // ── SetVariable ─────────────────────────────────────────────
                    PendingScriptAction::SetVariable { name, value } => {
                        {
                            let mut vars = ws.badge_state.variables_mut();
                            vars.custom.insert(name.clone(), value.clone());
                        }
                        ws.badge_state.mark_dirty();
                        ws.request_redraw();
                        crate::debug_info!("SCRIPT", "SetVariable {}={:?}", name, value);
                    }

                    // ── WriteText ───────────────────────────────────────────────
                    // NOTE: Uses `try_write()` for the terminal lock.  If the
                    // lock is held (e.g. by the PTY reader), the write is
                    // silently skipped this frame.  The script receives no
                    // failure signal — it may retry on the next event cycle.
                    PendingScriptAction::WriteText { text, config_index } => {
                        // Permission check (copy value to release config borrow)
                        let allow = ws
                            .config
                            .scripts
                            .get(config_index)
                            .map(|s| s.allow_write_text)
                            .unwrap_or(false);
                        let rate_limit = ws
                            .config
                            .scripts
                            .get(config_index)
                            .map(|s| s.write_text_rate_limit)
                            .unwrap_or(0);

                        if !allow {
                            log::warn!(
                                "Script[{}] WriteText DENIED: allow_write_text=false",
                                config_index
                            );
                            continue;
                        }

                        // Strip VT/ANSI sequences before PTY injection
                        let clean = crate::scripting::protocol::strip_vt_sequences(&text);
                        if clean.is_empty() {
                            continue;
                        }

                        // Rate limit and write
                        if let Some(tab) = ws.tab_manager.active_tab_mut() {
                            let script_id = tab.script_ids.get(config_index).and_then(|o| *o);
                            if let Some(sid) = script_id
                                && !tab.script_manager.check_write_text_rate(sid, rate_limit)
                            {
                                log::warn!("Script[{}] WriteText RATE-LIMITED", config_index);
                                continue;
                            }
                            // try_lock: acceptable — script WriteText in sync event
                            // loop. On miss the write is skipped this frame; the
                            // script can retry.
                            if let Ok(term) = tab.terminal.try_write()
                                && let Err(e) = term.write_str(&clean)
                            {
                                log::error!(
                                    "Script[{}] WriteText write failed: {}",
                                    config_index,
                                    e
                                );
                            }
                            crate::debug_info!(
                                "SCRIPT",
                                "AUDIT Script[{}] WriteText wrote {} bytes",
                                config_index,
                                clean.len()
                            );
                        }
                    }

                    // ── RunCommand ──────────────────────────────────────────────
                    // NOTE: Spawned processes run fire-and-forget with
                    // stdout/stderr discarded (`Stdio::null()`).  Scripts that
                    // need command output should read it from the PTY stream
                    // or use a side-channel (e.g. writing to a temp file).
                    PendingScriptAction::RunCommand {
                        command,
                        config_index,
                    } => {
                        let allow = ws
                            .config
                            .scripts
                            .get(config_index)
                            .map(|s| s.allow_run_command)
                            .unwrap_or(false);
                        let rate_limit = ws
                            .config
                            .scripts
                            .get(config_index)
                            .map(|s| s.run_command_rate_limit)
                            .unwrap_or(0);

                        if !allow {
                            log::warn!(
                                "Script[{}] RunCommand DENIED: allow_run_command=false",
                                config_index
                            );
                            continue;
                        }

                        // Tokenise without invoking a shell
                        let Some((program, args)) = tokenise_command(&command) else {
                            log::warn!("Script[{}] RunCommand DENIED: empty command", config_index);
                            continue;
                        };

                        // Command denylist check
                        if let Some(pattern) =
                            par_term_config::check_command_denylist(&program, &args)
                        {
                            log::error!(
                                "Script[{}] RunCommand DENIED: '{}' matches denylist \
                                     pattern '{}'",
                                config_index,
                                command,
                                pattern
                            );
                            continue;
                        }

                        // Rate limit check
                        if let Some(tab) = ws.tab_manager.active_tab_mut() {
                            let script_id = tab.script_ids.get(config_index).and_then(|o| *o);
                            if let Some(sid) = script_id
                                && !tab.script_manager.check_run_command_rate(sid, rate_limit)
                            {
                                log::warn!(
                                    "Script[{}] RunCommand RATE-LIMITED: '{}'",
                                    config_index,
                                    command
                                );
                                continue;
                            }
                        }

                        crate::debug_info!(
                            "SCRIPT",
                            "AUDIT Script[{}] RunCommand program={} args={:?}",
                            config_index,
                            program,
                            args
                        );

                        match std::process::Command::new(&program)
                            .args(&args)
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                        {
                            Ok(child) => {
                                log::debug!(
                                    "Script[{}] RunCommand spawned PID={}",
                                    config_index,
                                    child.id()
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "Script[{}] RunCommand failed to spawn '{}': {}",
                                    config_index,
                                    command,
                                    e
                                );
                            }
                        }
                    }

                    // ── ChangeConfig ────────────────────────────────────────────
                    PendingScriptAction::ChangeConfig {
                        key,
                        value,
                        config_index,
                    } => {
                        let allow = ws
                            .config
                            .scripts
                            .get(config_index)
                            .map(|s| s.allow_change_config)
                            .unwrap_or(false);

                        if !allow {
                            log::warn!(
                                "Script[{}] ChangeConfig DENIED: \
                                     allow_change_config=false",
                                config_index
                            );
                            continue;
                        }

                        Self::apply_script_config_change(ws, &key, &value, config_index);
                    }
                }
            }
        }

        // Pass 3 — Update settings window state
        if let Some(sw) = &mut self.settings_window {
            let running_changed = sw.settings_ui.script_running != running_state;
            let errors_changed = sw.settings_ui.script_errors != error_state;
            let has_new_output = new_output.iter().any(|lines| !lines.is_empty());
            let panels_changed = sw.settings_ui.script_panels != panel_state;

            if running_changed || errors_changed {
                crate::debug_info!(
                    "SCRIPT",
                    "sync: state change - running={:?} errors_changed={}",
                    running_state,
                    errors_changed
                );
            }

            let count = running_state.len();
            sw.settings_ui.script_output.resize_with(count, Vec::new);
            sw.settings_ui.script_output_expanded.resize(count, false);
            sw.settings_ui.script_panels.resize_with(count, || None);

            // Append new output lines, capping at max
            for (i, lines) in new_output.into_iter().enumerate() {
                if !lines.is_empty() {
                    let buf = &mut sw.settings_ui.script_output[i];
                    buf.extend(lines);
                    let overflow = buf.len().saturating_sub(Self::SCRIPT_OUTPUT_MAX_LINES);
                    if overflow > 0 {
                        buf.drain(..overflow);
                    }
                }
            }

            if running_changed || errors_changed || has_new_output || panels_changed {
                sw.settings_ui.script_running = running_state;
                sw.settings_ui.script_errors = error_state;
                sw.settings_ui.script_panels = panel_state;
                sw.request_redraw();
            }
        }
    }

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
    fn apply_script_config_change(
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
