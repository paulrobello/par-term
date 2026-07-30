//! Script state synchronization for the window manager.
//!
//! This module handles forwarding terminal events to scripts, reading script
//! commands/output, and syncing their running state to the settings window UI.
//!
//! ## Sub-modules
//!
//! - `config_change` — `PendingScriptAction` enum, command tokenisation, and
//!   allowlisted config-key application
//! - `scripting_lifecycle` — `start_script` and `stop_script` implementations
//! - `write_text` — queueing a `WriteText` payload behind the confirmation dialog

mod config_change;
mod scripting_lifecycle;
mod write_text;

use std::process::Stdio;

use winit::window::WindowId;

use config_change::{PendingScriptAction, tokenise_command};

use super::WindowManager;
use crate::app::window_state::AutomationTarget;
use crate::tab::TabId;

/// A script command that could not be executed while `self.windows` was borrowed,
/// tagged with the window and tab whose script produced it.
///
/// The tag is what makes servicing background tabs safe: every deferred action
/// has a sink that is specific to one tab (`SetBadge`), one window (`Notify`,
/// `ChangeConfig`), or one terminal (`WriteText`), and resolving it against the
/// *active* tab would silently aim it at the wrong terminal.
type TaggedAction = (WindowId, TabId, PendingScriptAction);

impl WindowManager {
    /// Maximum number of output lines kept per script in the UI.
    const SCRIPT_OUTPUT_MAX_LINES: usize = 200;

    /// Sync script running state to the settings window.
    ///
    /// Drains events from forwarders, sends them to scripts, reads commands
    /// and errors back, and updates the settings UI state.
    ///
    /// Every tab of every window is serviced, not just the focused window's
    /// active tab. A script is attached to the tab it was started on and keeps
    /// receiving terminal events there whether or not the user is looking at it,
    /// so servicing only one tab left every other script's commands unread and
    /// its event forwarder filling up untouched.
    ///
    /// This runs once per event-loop wake, so the sweep leads with the two
    /// cheapest possible rejections: a window whose config declares no scripts,
    /// and a tab that has no script attached.
    pub fn sync_script_running_state(&mut self) {
        let focused = self.get_focused_window_id();

        // Pass 1 — Service every tab that has a script attached.
        //
        // Safe commands (SetPanel, ClearPanel) are executed immediately.
        // Commands that need `WindowState` methods (Notify, SetBadge, etc.) or
        // require permission checks (WriteText, RunCommand, ChangeConfig) are
        // deferred into `pending_actions` and processed in Pass 2.
        //
        // All state is aggregated per script *config index*, because that is how
        // the settings window indexes it. The same script running in several tabs
        // therefore reports as running if any tab has it running, and its output
        // lines are merged rather than dropped.
        let mut running_state: Vec<bool> = Vec::new();
        let mut error_lines: Vec<Vec<String>> = Vec::new();
        let mut new_output: Vec<Vec<String>> = Vec::new();
        let mut panel_state: Vec<Option<(String, String)>> = Vec::new();
        let mut has_live_slot: Vec<bool> = Vec::new();
        let mut pending_actions: Vec<TaggedAction> = Vec::new();

        for (window_id, ws) in self.windows.iter_mut() {
            let script_count = ws.config.load().automation.scripts.len();
            if script_count == 0 {
                continue;
            }

            // Windows hold their own config, so size the aggregates to the
            // largest script list seen rather than assuming they all match.
            if running_state.len() < script_count {
                running_state.resize(script_count, false);
                error_lines.resize_with(script_count, Vec::new);
                new_output.resize_with(script_count, Vec::new);
                panel_state.resize_with(script_count, || None);
                has_live_slot.resize(script_count, false);
            }

            let window_id = *window_id;
            let is_focused_window = focused == Some(window_id);
            let active_tab_id = ws.tab_manager.active_tab_id();

            for tab in ws.tab_manager.tabs_mut() {
                // A tab that never started a script has an empty `script_ids`,
                // and one whose scripts were all stopped has all-`None` slots.
                if tab.scripting.script_ids.iter().all(Option::is_none) {
                    continue;
                }

                let tab_id = tab.id;
                // Only one panel can be shown per script, so the tab the user is
                // actually looking at is the one whose panel wins.
                let tab_is_authoritative = is_focused_window && active_tab_id == Some(tab_id);

                let slot_count = script_count.min(tab.scripting.script_ids.len());
                for i in 0..slot_count {
                    let Some(script_id) = tab.scripting.script_ids[i] else {
                        continue;
                    };
                    has_live_slot[i] = true;

                    let is_running = tab.scripting.script_manager.is_running(script_id);
                    running_state[i] |= is_running;

                    // Drain events from forwarder and send to script.
                    if is_running
                        && let Some(forwarder) = tab
                            .scripting
                            .script_forwarders
                            .get(i)
                            .and_then(|s| s.clone())
                    {
                        for event in forwarder.drain_events() {
                            let _ = tab.scripting.script_manager.send_event(script_id, &event);
                        }
                    }

                    // Read commands from script and process them.
                    let mut panel_val = tab.scripting.script_manager.get_panel(script_id).cloned();

                    // Bound to a local first: temporaries in a `for` head live for
                    // the whole loop, which would hold a borrow of `script_manager`
                    // across the `set_panel`/`clear_panel` calls in the body.
                    let commands = tab.scripting.script_manager.read_commands(script_id);
                    for cmd in commands {
                        match cmd {
                            par_term_scripting::protocol::ScriptCommand::Log { level, message } => {
                                new_output[i].push(format!("[{}] {}", level, message));
                            }
                            par_term_scripting::protocol::ScriptCommand::SetPanel {
                                title,
                                content,
                            } => {
                                tab.scripting.script_manager.set_panel(
                                    script_id,
                                    title.clone(),
                                    content.clone(),
                                );
                                panel_val = Some((title, content));
                            }
                            par_term_scripting::protocol::ScriptCommand::ClearPanel {} => {
                                tab.scripting.script_manager.clear_panel(script_id);
                                panel_val = None;
                            }
                            // Safe display-only commands — defer to Pass 2 so they can
                            // call `WindowState` methods without borrow conflicts.
                            par_term_scripting::protocol::ScriptCommand::Notify { title, body } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::Notify { title, body },
                                ));
                            }
                            par_term_scripting::protocol::ScriptCommand::SetBadge { text } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::SetBadge { text },
                                ));
                            }
                            par_term_scripting::protocol::ScriptCommand::SetVariable {
                                name,
                                value,
                            } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::SetVariable { name, value },
                                ));
                            }
                            // Restricted commands — permission-checked in Pass 2.
                            par_term_scripting::protocol::ScriptCommand::WriteText { text } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::WriteText {
                                        text,
                                        config_index: i,
                                    },
                                ));
                            }
                            par_term_scripting::protocol::ScriptCommand::RunCommand { command } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::RunCommand {
                                        command,
                                        config_index: i,
                                    },
                                ));
                            }
                            par_term_scripting::protocol::ScriptCommand::ChangeConfig {
                                key,
                                value,
                            } => {
                                pending_actions.push((
                                    window_id,
                                    tab_id,
                                    PendingScriptAction::ChangeConfig {
                                        key,
                                        value,
                                        config_index: i,
                                    },
                                ));
                            }
                        }
                    }

                    if tab_is_authoritative || panel_state[i].is_none() {
                        panel_state[i] = panel_val;
                    }

                    // Read errors from script.
                    let errors = tab.scripting.script_manager.read_errors(script_id);
                    if !errors.is_empty() {
                        error_lines[i].extend(errors);
                    }
                }
            }
        }

        // A script index with no live slot in any tab keeps whatever error the
        // settings window is already showing: nothing will produce that text
        // again, and clearing it would erase the reason the script is not running.
        let mut error_state: Vec<String> =
            error_lines.iter().map(|lines| lines.join("\n")).collect();
        if let Some(sw) = &self.settings_window {
            for (i, err) in error_state.iter_mut().enumerate() {
                if !has_live_slot[i]
                    && err.is_empty()
                    && let Some(existing) = sw.settings_ui.script_errors.get(i)
                {
                    err.clone_from(existing);
                }
            }
        }

        // Pass 2 — Execute deferred actions that need `WindowState` access.
        //
        // The mutable borrow of `self.windows` from Pass 1 has been released,
        // so we can take a fresh mutable borrow here. Each action is resolved
        // against the window and tab that produced it, never against whichever
        // tab happens to be active now.
        for (window_id, tab_id, action) in pending_actions {
            let Some(ws) = self.windows.get_mut(&window_id) else {
                continue;
            };

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
                    if let Some(tab) = ws.tab_manager.get_tab_mut(tab_id) {
                        tab.profile.badge_override = Some(text.clone());
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
                // SEC-013: sanitising the payload does not make it safe —
                // what runs a command is printable text plus a newline, and
                // both survive `strip_vt_sequences` by design. So the write
                // goes to the confirmation dialog unless the script opted
                // out via `prompt_before_write_text: false` or the user
                // chose "Always Allow" for this exact text in this tab.
                //
                // NOTE: the direct write uses `try_read()` for the terminal
                // lock.  If the lock is held (e.g. by the PTY reader), the
                // write is silently skipped this frame.  The script receives
                // no failure signal — it may retry on the next event cycle.
                PendingScriptAction::WriteText { text, config_index } => {
                    // Copy the settings out to release the config borrow.
                    let (allow, rate_limit, prompt_before_write, script_name) = {
                        let config = ws.config.load();
                        match config.automation.scripts.get(config_index) {
                            Some(script) => (
                                script.allow_write_text,
                                script.write_text_rate_limit,
                                script.prompt_before_write_text,
                                script.name.clone(),
                            ),
                            None => (false, 0, true, String::new()),
                        }
                    };

                    if !allow {
                        log::warn!(
                            "Script[{}] WriteText DENIED: allow_write_text=false",
                            config_index
                        );
                        continue;
                    }

                    // Strip VT/ANSI sequences before PTY injection
                    let clean = par_term_scripting::protocol::strip_vt_sequences(&text);
                    if clean.is_empty() {
                        continue;
                    }

                    // Rate limit before either sink, so a flood cannot spend
                    // the user's attention any faster than it could spend
                    // the PTY.
                    if let Some(tab) = ws.tab_manager.get_tab_mut(tab_id) {
                        let script_id = tab.scripting.script_ids.get(config_index).and_then(|o| *o);
                        if let Some(sid) = script_id
                            && !tab
                                .scripting
                                .script_manager
                                .check_write_text_rate(sid, rate_limit)
                        {
                            log::warn!("Script[{}] WriteText RATE-LIMITED", config_index);
                            continue;
                        }
                    }

                    // The confirmation carries the tab it belongs to, so the
                    // dialog can name that tab and the approved write can reach
                    // it. The id is scoped to the target for the same reason:
                    // two tabs running one script are two separate decisions.
                    let target = AutomationTarget { window_id, tab_id };
                    let action_id =
                        write_text::tab_scoped_write_text_action_id(&script_name, &clean, target);
                    let session_approved = ws
                        .trigger_state
                        .always_allow_trigger_ids
                        .contains(&action_id);

                    if par_term_scripting::confirm::write_text_needs_confirmation(
                        prompt_before_write,
                        session_approved,
                    ) {
                        Self::queue_script_write_text(
                            ws,
                            config_index,
                            &script_name,
                            action_id,
                            target,
                            clean,
                        );
                        continue;
                    }

                    if let Some(tab) = ws.tab_manager.get_tab(tab_id) {
                        // try_lock: acceptable — script WriteText in sync event
                        // loop. On miss the write is skipped this frame; the
                        // script can retry.
                        if let Ok(term) = tab.terminal.try_read()
                            && let Err(e) = term.write_str(&clean)
                        {
                            log::error!("Script[{}] WriteText write failed: {}", config_index, e);
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
                        .load()
                        .automation
                        .scripts
                        .get(config_index)
                        .map(|s| s.allow_run_command)
                        .unwrap_or(false);
                    let rate_limit = ws
                        .config
                        .load()
                        .automation
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
                    if let Some(pattern) = par_term_config::check_command_denylist(&program, &args)
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
                    if let Some(tab) = ws.tab_manager.get_tab_mut(tab_id) {
                        let script_id = tab.scripting.script_ids.get(config_index).and_then(|o| *o);
                        if let Some(sid) = script_id
                            && !tab
                                .scripting
                                .script_manager
                                .check_run_command_rate(sid, rate_limit)
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
                        .load()
                        .automation
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
}
