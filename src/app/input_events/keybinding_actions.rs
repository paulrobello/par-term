//! Keybinding action dispatch for WindowState.
//!
//! - `execute_keybinding_action`: dispatches named actions (toggle shaders,
//!   new tab, copy, paste, etc.) through the [`ACTION_HANDLERS`] table.
//!
//! Visual notification helpers (`show_toast`, `show_pane_indices`) and shader
//! toggle helpers (`toggle_background_shader`, `toggle_cursor_shader`) live in
//! `keybinding_helpers`.
//!
//! Display/navigation actions (font size, cursor style, tab index switching,
//! throughput mode, etc.) live in `keybinding_display_actions`.
//!
//! Snippet and custom action execution live in `snippet_actions`.
//!
//! # Why a table and not a `match`
//!
//! Actions arrive as `&str` (from `config.yaml` keybindings, from generated
//! `snippet:`/`action:` names), so a `match` never had an exhaustiveness check
//! to lose. What a `match` on string literals *did* give was
//! `unreachable_patterns`: a duplicated arm was a compile-time warning. A
//! linear-scan table makes a duplicate key silently first-wins instead, so
//! `dispatch_tests::action_table_has_no_duplicate_keys` replaces that
//! guarantee, and `dispatch_tests` additionally asserts the table's key set
//! against a frozen inventory and against the display table's keys — neither
//! of which the `match` form could express at all.

use crate::app::window_state::WindowState;
use crate::command_palette::catalog::plugin_palette_entries;

/// Handler for one named keybinding action.
///
/// Every entry in [`ACTION_HANDLERS`] returns `true`; the `bool` exists so the
/// signature matches `execute_keybinding_action`'s contract, where a name that
/// no handler claims returns `false`.
pub(crate) type ActionHandler = fn(&mut WindowState) -> bool;

/// Exact-match dispatch table for named keybinding actions.
///
/// Entry order is irrelevant to behavior — every key is a distinct literal and
/// lookup is by equality — but keys must stay unique, which
/// `dispatch_tests::action_table_has_no_duplicate_keys` enforces.
///
/// Names not found here fall through to `execute_display_keybinding_action`
/// and then to the `snippet:` / `action:` / `restore_arrangement:` /
/// `plugin-action:` prefix forms; see `execute_keybinding_action`.
pub(crate) static ACTION_HANDLERS: &[(&str, ActionHandler)] = &[
    ("toggle_background_shader", |s: &mut WindowState| {
        s.toggle_background_shader();
        true
    }),
    ("toggle_cursor_shader", |s: &mut WindowState| {
        s.toggle_cursor_shader();
        true
    }),
    ("cycle_background_shader", |s: &mut WindowState| {
        s.cycle_background_shader();
        true
    }),
    ("toggle_shader_animation", |s: &mut WindowState| {
        s.toggle_shader_animation();
        true
    }),
    ("toggle_shader_readability_mode", |s: &mut WindowState| {
        s.toggle_shader_readability_mode();
        true
    }),
    ("reload_config", |s: &mut WindowState| {
        s.reload_config();
        true
    }),
    ("open_settings", |s: &mut WindowState| {
        s.overlay_state.open_settings_window_requested = true;
        s.request_redraw();
        log::info!("Settings window requested via keybinding");
        true
    }),
    ("toggle_fullscreen", toggle_fullscreen),
    ("maximize_vertically", maximize_vertically),
    ("toggle_help", |s: &mut WindowState| {
        s.overlay_ui.help_ui.toggle();
        s.request_redraw();
        log::info!(
            "Help UI toggled via keybinding: {}",
            if s.overlay_ui.help_ui.visible {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    ("toggle_fps_overlay", |s: &mut WindowState| {
        s.debug.show_fps_overlay = !s.debug.show_fps_overlay;
        s.request_redraw();
        log::info!(
            "FPS overlay toggled via keybinding: {}",
            if s.debug.show_fps_overlay {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    ("toggle_search", toggle_search),
    ("toggle_command_palette", |s: &mut WindowState| {
        // The `mut` serves only the mux arm's extend below.
        #[cfg_attr(not(feature = "mux"), allow(unused_mut))]
        let mut plugin_rows =
            plugin_palette_entries(&s.status_bar_ui.plugin_host().palette_actions());
        // Agent-authored commands join as runtime rows, hot-reloaded by the
        // commands-dir watcher (design 2026-09-24).
        plugin_rows.extend(s.agent_commands.palette_rows());
        // Configured launchable agents join the same way (the `agents:`
        // config list — config data, not a dispatch-table built-in).
        plugin_rows.extend(crate::command_palette::catalog::agent_palette_entries(
            &s.config.load().agents,
        ));
        // Captured crashes join as runtime rows too — the triage consent
        // surface (crash_triage module).
        plugin_rows.extend(s.crash_triage.palette_entries());
        // Rostered agents join the palette at open time (A2b task 3): the
        // rows are runtime data from the cache, like the plugin rows —
        // scoped to panes the app maps, so every offered row is focusable.
        #[cfg(feature = "mux")]
        {
            let map = &s.tmux_state.tmux_pane_owners;
            plugin_rows.extend(
                s.tmux_state
                    .agent_roster
                    .palette_rows(&|pane| map.contains_key(&pane)),
            );
        }
        // The attached par-mux session's detach row joins the same way —
        // a runtime row, present only while a transport is installed.
        #[cfg(feature = "mux")]
        plugin_rows.extend(s.tmux_state.mux_palette_rows());
        s.overlay_ui.command_palette.toggle(plugin_rows);
        s.focus_state.needs_redraw = true;
        s.request_redraw();
        log::info!(
            "Command palette toggled via keybinding: {}",
            if s.overlay_ui.command_palette.visible {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    ("toggle_agent_usage_panel", |s: &mut WindowState| {
        s.overlay_ui.agent_usage_panel.toggle();
        s.focus_state.needs_redraw = true;
        s.request_redraw();
        true
    }),
    ("toggle_ai_inspector", toggle_ai_inspector),
    ("rename_pane", |s: &mut WindowState| {
        // Open the rename popup on the focused pane (the same popup a
        // right-click on its title bar opens). The popup anchors at the
        // pane's title bar, or its top edge when title bars are off.
        let found = s.tab_manager.active_tab().and_then(|tab| {
            let pm = tab.pane_manager.as_ref()?;
            let pane = pm.focused_pane()?;
            Some((pane.id, pane.title.clone(), pane.bounds))
        });
        if let Some((pane_id, title, bounds)) = found {
            let scale = s.renderer.as_ref().map(|r| r.scale_factor()).unwrap_or(1.0);
            let panes_cfg = &s.config.load().panes;
            let bar_y = match panes_cfg.pane_title_position {
                par_term_config::PaneTitlePosition::Top => bounds.y,
                par_term_config::PaneTitlePosition::Bottom => {
                    bounds.y + bounds.height - panes_cfg.pane_title_height * scale
                }
            };
            let pos = egui::pos2(bounds.x / scale, bar_y / scale);
            s.overlay_ui.pane_rename_ui.open(pane_id, &title, pos);
            s.request_redraw();
            log::info!("Pane rename popup opened via keybinding for pane {pane_id}");
        }
        true
    }),
    ("new_tab", |s: &mut WindowState| {
        s.new_tab_or_show_profiles();
        true
    }),
    ("close_tab", |s: &mut WindowState| {
        if s.has_multiple_tabs() {
            s.close_current_tab();
            log::info!("Tab closed via keybinding");
        }
        true
    }),
    ("duplicate_tab", |s: &mut WindowState| {
        s.duplicate_tab();
        log::info!("Tab duplicated via keybinding");
        true
    }),
    ("move_tab_to_new_window", move_tab_to_new_window),
    ("next_tab", |s: &mut WindowState| {
        s.next_tab();
        log::debug!("Switched to next tab via keybinding");
        true
    }),
    ("prev_tab", |s: &mut WindowState| {
        s.prev_tab();
        log::debug!("Switched to previous tab via keybinding");
        true
    }),
    ("paste_special", paste_special),
    ("toggle_session_logging", toggle_session_logging),
    ("split_horizontal", |s: &mut WindowState| {
        s.split_pane_horizontal();
        true
    }),
    ("split_vertical", |s: &mut WindowState| {
        s.split_pane_vertical();
        true
    }),
    ("close_pane", |s: &mut WindowState| {
        s.close_focused_pane();
        true
    }),
    ("navigate_pane_left", |s: &mut WindowState| {
        s.navigate_pane(crate::pane::NavigationDirection::Left);
        true
    }),
    ("navigate_pane_right", |s: &mut WindowState| {
        s.navigate_pane(crate::pane::NavigationDirection::Right);
        true
    }),
    ("navigate_pane_up", |s: &mut WindowState| {
        s.navigate_pane(crate::pane::NavigationDirection::Up);
        true
    }),
    ("navigate_pane_down", |s: &mut WindowState| {
        s.navigate_pane(crate::pane::NavigationDirection::Down);
        true
    }),
    ("select_pane_hint", |s: &mut WindowState| {
        s.enter_pane_hint_select();
        true
    }),
    ("resize_pane_left", |s: &mut WindowState| {
        s.resize_pane(crate::pane::NavigationDirection::Left);
        true
    }),
    ("resize_pane_right", |s: &mut WindowState| {
        s.resize_pane(crate::pane::NavigationDirection::Right);
        true
    }),
    ("resize_pane_up", |s: &mut WindowState| {
        s.resize_pane(crate::pane::NavigationDirection::Up);
        true
    }),
    ("resize_pane_down", |s: &mut WindowState| {
        s.resize_pane(crate::pane::NavigationDirection::Down);
        true
    }),
    ("toggle_tmux_session_picker", |s: &mut WindowState| {
        s.overlay_ui.tmux_session_picker_ui.toggle();
        s.request_redraw();
        log::info!(
            "tmux session picker toggled via keybinding: {}",
            if s.overlay_ui.tmux_session_picker_ui.visible {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    // Deliberate alias pair: `toggle_copy_mode` and `enter_copy_mode` are two
    // public action names for one behavior (the handler toggles either way).
    // Do not "deduplicate" these into one entry — both names are documented and
    // bindable, and dropping either silently breaks existing user configs.
    ("toggle_copy_mode", toggle_copy_mode),
    ("enter_copy_mode", toggle_copy_mode),
    ("toggle_broadcast_input", |s: &mut WindowState| {
        s.broadcast_input = !s.broadcast_input;
        let message = if s.broadcast_input {
            "Broadcast Input: ON"
        } else {
            "Broadcast Input: OFF"
        };
        s.show_toast(message);
        log::info!(
            "Broadcast input mode {}",
            if s.broadcast_input {
                "enabled"
            } else {
                "disabled"
            }
        );
        true
    }),
    ("promote_pane_to_tab", |s: &mut WindowState| {
        s.promote_pane_to_tab();
        true
    }),
    ("demote_tab_to_pane", |s: &mut WindowState| {
        s.start_demote_tab();
        true
    }),
    ("toggle_profile_drawer", |s: &mut WindowState| {
        s.toggle_profile_drawer();
        log::info!(
            "Profile drawer toggled via keybinding: {}",
            if s.overlay_ui.profile_drawer_ui.expanded {
                "expanded"
            } else {
                "collapsed"
            }
        );
        true
    }),
    ("toggle_clipboard_history", |s: &mut WindowState| {
        s.toggle_clipboard_history();
        log::info!(
            "Clipboard history toggled via keybinding: {}",
            if s.overlay_ui.clipboard_history_ui.visible {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    ("toggle_command_history", |s: &mut WindowState| {
        s.toggle_command_history();
        log::info!(
            "Command history toggled via keybinding: {}",
            if s.overlay_ui.command_history_ui.visible {
                "visible"
            } else {
                "hidden"
            }
        );
        true
    }),
    ("clear_scrollback", clear_scrollback),
    // Menu-parity actions. These need the `WindowManager` — the event loop and
    // every window — not just a `WindowState`, so they go through the same queue
    // the in-app menu uses. Routing both through it means a keybinding and its
    // menu item cannot drift apart.
    ("new_window", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::NewWindow);
        true
    }),
    ("close_window", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::CloseWindow);
        true
    }),
    ("quit", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::Quit);
        true
    }),
    ("select_all", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::SelectAll);
        true
    }),
    ("toggle_menu", |s: &mut WindowState| {
        crate::menu::request_toggle();
        s.request_redraw();
        true
    }),
];

fn toggle_fullscreen(s: &mut WindowState) -> bool {
    if let Some(window) = &s.window {
        s.is_fullscreen = !s.is_fullscreen;
        if s.is_fullscreen {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            log::info!("Entering fullscreen mode via keybinding");
        } else {
            window.set_fullscreen(None);
            log::info!("Exiting fullscreen mode via keybinding");
        }
    }
    true
}

fn maximize_vertically(s: &mut WindowState) -> bool {
    if let Some(window) = &s.window {
        // Get current monitor to determine screen height
        if let Some(monitor) = window.current_monitor() {
            let monitor_pos = monitor.position();
            let monitor_size = monitor.size();
            let window_pos = window.outer_position().unwrap_or_default();
            let window_size = window.outer_size();

            // Set window to span full height while keeping current X position and width
            window.set_outer_position(winit::dpi::PhysicalPosition::new(
                window_pos.x,
                monitor_pos.y,
            ));
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                window_size.width,
                monitor_size.height,
            ));
            log::info!("Window maximized vertically via keybinding");
        }
    }
    true
}

fn toggle_search(s: &mut WindowState) -> bool {
    s.overlay_ui.search_ui.toggle();
    if s.overlay_ui.search_ui.visible {
        s.overlay_ui.search_ui.init_from_config(
            s.config.load().search.search_case_sensitive,
            s.config.load().search.search_regex,
        );
    }
    s.focus_state.needs_redraw = true;
    s.request_redraw();
    log::info!(
        "Search UI toggled via keybinding: {}",
        if s.overlay_ui.search_ui.visible {
            "visible"
        } else {
            "hidden"
        }
    );
    true
}

fn toggle_ai_inspector(s: &mut WindowState) -> bool {
    if s.config.load().ai_inspector.ai_inspector_enabled {
        let just_opened = s.overlay_ui.ai_inspector.toggle();
        s.sync_ai_inspector_width();
        if just_opened {
            if s.config.load().ai_inspector.ai_inspector_input_history_mode
                == par_term_config::AssistantInputHistoryMode::Persist
            {
                s.overlay_ui.ai_inspector.merge_persisted_input_history();
            }
            s.try_auto_connect_agent();
        }
        s.request_redraw();
    }
    true
}

fn move_tab_to_new_window(s: &mut WindowState) -> bool {
    if let Some(tab_id) = s.tab_manager.active_tab_id()
        && !s.is_gateway_active()
        && s.has_multiple_tabs()
    {
        // A mux tab mirrors a daemon window and carries no transport of its
        // own — moving it to another par-term window would strand the
        // mirror (input would have nowhere to go). Blocked, not carried.
        if s.mux_window_for_tab(tab_id).is_some() {
            s.show_toast(
                "par-mux: tabs attached to a par-mux session can't move between windows — \
                 detach first",
            );
            s.request_redraw();
            return true;
        }
        s.overlay_ui.pending_move_tab_request = Some(crate::app::window_manager::MoveTabRequest {
            tab_id,
            destination: crate::app::window_manager::MoveDestination::NewWindow,
        });
        log::info!("Move Tab to New Window triggered via keybinding");
    }
    true
}

fn paste_special(s: &mut WindowState) -> bool {
    // Get clipboard content and open paste special UI
    if let Some(text) = s.input_handler.paste_from_clipboard() {
        s.overlay_ui.paste_special_ui.open(text);
        s.focus_state.needs_redraw = true;
        s.request_redraw();
        log::info!("Paste special UI opened");
    } else {
        log::debug!("Paste special: no clipboard content");
    }
    true
}

fn toggle_session_logging(s: &mut WindowState) -> bool {
    if let Some(tab) = s.tab_manager.active_tab_mut() {
        match tab.toggle_session_logging(&s.config.load()) {
            Ok(is_active) => {
                let message = if is_active {
                    "⏺ Recording Started"
                } else {
                    "⏹ Recording Stopped"
                };
                log::info!(
                    "Session logging toggled: {}",
                    if is_active { "started" } else { "stopped" }
                );
                // Show toast after releasing tab borrow
                s.show_toast(message);
            }
            Err(e) => {
                log::error!("Failed to toggle session logging: {}", e);
                s.show_toast(format!("Recording Error: {}", e));
            }
        }
    }
    true
}

fn toggle_copy_mode(s: &mut WindowState) -> bool {
    if s.is_copy_mode_active() {
        s.exit_copy_mode();
    } else {
        s.enter_copy_mode();
    }
    true
}

fn clear_scrollback(s: &mut WindowState) -> bool {
    let cleared = if let Some(tab) = s.tab_manager.active_tab_mut() {
        // try_lock: intentional — keybinding action in sync event loop.
        // On miss: scrollback not cleared this invocation. User can retry.
        let did_clear = if let Ok(mut term) = tab.terminal.try_write() {
            term.clear_scrollback();
            term.clear_scrollback_metadata();
            true
        } else {
            false
        };
        if did_clear {
            tab.active_cache_mut().scrollback_len = 0;
            tab.scripting.trigger_marks.clear();
            let tab_terminal = std::sync::Arc::clone(&tab.terminal);
            if let Some(pm) = tab.pane_manager_mut() {
                for pane in pm.all_panes_mut() {
                    if std::sync::Arc::ptr_eq(&pane.terminal, &tab_terminal) {
                        pane.cache.invalidate_pane_cells();
                    }
                }
            }
        }
        did_clear
    } else {
        false
    };
    if cleared {
        s.set_scroll_target(0);
        log::info!("Cleared scrollback buffer via keybinding");
    }
    true
}

/// Extra environment variables an agent-command script receives: always
/// `PAR_TERM_COMMAND_ID`, plus `PAR_TERM_COMMAND_SOURCE_AGENT` for
/// agent-created commands (design 2026-09-24, owner decision 6).
pub(crate) fn agent_command_env(
    file: &par_term_config::agent_commands::AgentCommandFile,
) -> Vec<(String, String)> {
    let mut env = vec![("PAR_TERM_COMMAND_ID".to_string(), file.id().to_string())];
    if let Some(agent) = &file.source_agent {
        env.push(("PAR_TERM_COMMAND_SOURCE_AGENT".to_string(), agent.clone()));
    }
    env
}

/// Parse a `plugin-action:<plugin_id>:<action_id>` keybinding action name.
///
/// Pure core of the `plugin-action:` miss-path branch: strips the prefix,
/// splits on the first remaining colon, and rejects empty halves. An action
/// half containing a further colon parses (split-once) — whether such an id
/// exists is the manifest lookup's call, not the parser's.
pub(crate) fn parse_plugin_action_id(action: &str) -> Option<(&str, &str)> {
    let remainder = action.strip_prefix("plugin-action:")?;
    let (plugin_id, action_id) = remainder.split_once(':')?;
    if plugin_id.is_empty() || action_id.is_empty() {
        return None;
    }
    Some((plugin_id, action_id))
}

/// Parse an `agent-roster-focus:<pane_id>` keybinding action name (A2b
/// task 3's palette picker rows). Pure core, mirroring
/// [`parse_plugin_action_id`]: strips the prefix and parses the pane id,
/// rejecting empty or non-numeric remainders.
pub(crate) fn parse_agent_roster_focus_id(action: &str) -> Option<u64> {
    let remainder = action.strip_prefix("agent-roster-focus:")?;
    if remainder.is_empty() {
        return None;
    }
    remainder.parse().ok()
}

impl WindowState {
    /// Execute a keybinding action by name.
    ///
    /// Returns true if the action was handled, false if unknown.
    pub(crate) fn execute_keybinding_action(&mut self, action: &str) -> bool {
        if let Some((_, handler)) = ACTION_HANDLERS.iter().find(|(name, _)| *name == action) {
            return handler(self);
        }

        // Miss path — order is load-bearing and must not be rearranged.
        // Delegate display/navigation actions to the companion handler
        if let Some(result) = self.execute_display_keybinding_action(action) {
            return result;
        }
        // Check for snippet or action keybindings
        if let Some(snippet_id) = action.strip_prefix("snippet:") {
            self.execute_snippet(snippet_id)
        } else if let Some(action_id) = action.strip_prefix("action:") {
            self.execute_custom_action(action_id)
        } else if let Some(arrangement_name) = action.strip_prefix("restore_arrangement:") {
            // Restore arrangement by name - handled by WindowManager
            self.overlay_state.pending_arrangement_restore = Some(arrangement_name.to_string());
            self.request_redraw();
            log::info!(
                "Arrangement restore requested via keybinding: {}",
                arrangement_name
            );
            true
        } else if action.starts_with("plugin-action:") {
            match parse_plugin_action_id(action) {
                Some((plugin_id, action_id)) => self.dispatch_plugin_action(plugin_id, action_id),
                None => {
                    // Fires once per keypress, not per frame, so a plain warn
                    // cannot flood the log the way a render-path warn could.
                    log::warn!(
                        "Malformed plugin-action keybinding '{}' (expected \
                         plugin-action:<plugin_id>:<action_id>)",
                        action
                    );
                    false
                }
            }
        } else if let Some(cmd_id) = action.strip_prefix("agent-cmd:") {
            self.execute_agent_command(cmd_id, &[])
        } else if let Some(agent_id) = action.strip_prefix("launch-agent-autonomous:") {
            self.launch_agent_by_id(agent_id, true)
        } else if let Some(agent_id) = action.strip_prefix("launch-agent:") {
            self.launch_agent_by_id(agent_id, false)
        } else if action == "launch-default-agent" {
            self.launch_default_agent()
        } else if let Some(offer_id) = action.strip_prefix("triage-crash:") {
            self.triage_crash_by_id(offer_id)
        } else if let Some(pane_id) = parse_agent_roster_focus_id(action) {
            if self.focus_agent_roster_pane(pane_id) {
                log::info!("Focused agent roster pane {} via palette", pane_id);
                true
            } else {
                log::warn!(
                    "Agent roster pane {} has no native pane to focus (session ended \
                     or layout rebuilding)",
                    pane_id
                );
                false
            }
        } else if action == "mux-detach" {
            // The palette offers this row only while a transport is attached
            // (mux feature); a hand-bound keybinding on a build without the
            // feature, or after the session ended, lands here and no-ops.
            #[cfg(feature = "mux")]
            {
                if self.detach_mux_session() {
                    log::info!("Detached from par-mux session via palette");
                    return true;
                }
            }
            log::warn!("par-mux detach requested but no transport is attached");
            false
        } else {
            log::warn!("Unknown keybinding action: {}", action);
            false
        }
    }

    /// Execute an agent-authored command by id (`agent-cmd:<id>` dispatch).
    ///
    /// Macros replay through the existing custom-action executor; scripts
    /// check the confirmation ledger first — an unconfirmed body is queued
    /// for the first-run dialog instead of executing. `extra_args` come from
    /// the CLI fallthrough (`par-term <id> a b c`) and append to the stored
    /// args of a script command.
    pub(crate) fn execute_agent_command(&mut self, cmd_id: &str, extra_args: &[String]) -> bool {
        let file = match self.agent_commands.get(cmd_id) {
            Some(c) => c.file.clone(),
            None => {
                // The palette snapshot can lag a delete by one watcher poll;
                // a hand-bound chord on a deleted command lands here too.
                log::warn!("agent-cmd {:?} not found (deleted or invalid)", cmd_id);
                return false;
            }
        };

        match file.action.clone() {
            par_term_config::CustomActionConfig::ShellCommand {
                command,
                args,
                notify_on_success,
                timeout_secs,
                title,
                capture_output,
                ..
            } => {
                if !self.agent_commands.is_confirmed(&file) {
                    log::info!(
                        "agent-cmd {:?} body unconfirmed — queuing first-run dialog",
                        cmd_id
                    );
                    self.agent_commands.request_confirmation(file);
                    self.request_redraw();
                    return true;
                }
                let mut full_args = args;
                full_args.extend(extra_args.iter().cloned());
                let env = agent_command_env(&file);
                self.execute_shell_command_action_with_env(
                    command,
                    full_args,
                    notify_on_success,
                    timeout_secs,
                    title,
                    capture_output,
                    env,
                )
            }
            // Macros and Sequences replay through the same executor path the
            // `action:` prefix uses; they take no runtime input.
            action => self.execute_custom_action_payload(&action),
        }
    }

    /// Dispatch a plugin-contributed palette action through the status bar's
    /// plugin host.
    ///
    /// `true` means delivered to the plugin's running action process, not
    /// executed — the plugin acknowledges (or doesn't) through its next
    /// output. Every miss returns `false` behind the host's warn-once gates.
    pub(crate) fn dispatch_plugin_action(&mut self, plugin_id: &str, action_id: &str) -> bool {
        self.status_bar_ui
            .plugin_host_mut()
            .invoke_action(plugin_id, action_id)
    }

    /// Dispatch a `triage-crash:<id>` palette activation: consume the offer
    /// and hand its facts to the default agent. This is the consent point —
    /// nothing reaches an agent until here.
    pub(crate) fn triage_crash_by_id(&mut self, id: &str) -> bool {
        let Ok(id) = id.parse::<u64>() else {
            log::warn!("triage-crash: malformed offer id '{id}'");
            return false;
        };
        let Some(offer) = self.crash_triage.take(id) else {
            log::warn!("triage-crash: no live offer {id} (expired or consumed)");
            self.show_toast("That crash offer is no longer available".to_string());
            return false;
        };
        let Some(agent) =
            par_term_config::agent_launcher::default_agent(&self.config.load().agents).cloned()
        else {
            log::warn!("triage-crash: no default agent configured");
            self.show_toast("No default agent configured".to_string());
            return false;
        };
        // Always a plain launch — triage never arms the autonomous variant.
        let command_line = format!(
            "{} {}",
            agent.command,
            crate::crash_triage::shell_single_quote(&crate::crash_triage::triage_prompt(&offer))
        );
        crate::debug_info!("TAB_ACTION", "triage crash offer {id} via palette");
        #[cfg(feature = "mux")]
        {
            use crate::app::tmux_handler::MuxLaunchOutcome;
            match self.launch_agent_via_mux(&command_line) {
                MuxLaunchOutcome::NotMux => {}
                MuxLaunchOutcome::Launched => return true,
                MuxLaunchOutcome::Failed => return false,
            }
        }
        self.execute_new_tab_action(Some(command_line), agent.name.clone())
    }
}
