//! Keybinding action dispatch for WindowState.
//!
//! - `execute_keybinding_action`: dispatches named actions (toggle shaders,
//!   new tab, copy, paste, etc.) through the [`ACTION_HANDLERS`] table.
//!
//! Visual notification helpers (`show_toast`, `show_pane_indices`), shader
//! toggle helpers (`toggle_background_shader`, `toggle_cursor_shader`), and
//! the clear actions (`clear_scrollback`, `clear_screen`) live in
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

use super::keybinding_helpers::{clear_screen, clear_scrollback};
use super::keybinding_view_actions::{
    maximize_vertically, move_tab_to_new_window, paste_special, toggle_ai_inspector,
    toggle_copy_mode, toggle_fullscreen, toggle_search, toggle_session_logging,
};
use super::pane_actions;
use super::tab_nav_actions;

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
        // One runtime-row builder (UX.md OV6), shared with the egui open
        // path.
        let rows = crate::app::overlay::palette_rows::palette_runtime_rows(
            &s.status_bar_ui,
            &s.agent_commands,
            &mut s.crash_triage,
            &s.config.load(),
            &s.overlay_ui.profile_manager,
            #[cfg(feature = "mux")]
            &s.tmux_state,
        );
        // The listed par-mux sessions come from a cached directory; the
        // open refreshes it for next time.
        #[cfg(feature = "mux")]
        s.refresh_mux_directory();
        s.overlay_ui
            .command_palette
            .toggle(rows, &s.keybinding_registry);
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
        // I15 cascade tail: closing the LAST tab closes the window, the
        // same contract the menu arm already has (MenuAction::CloseTab →
        // close_window). The old has_multiple_tabs() guard made the chord
        // a dead key on the last tab — Ctrl+Alt+W on Linux/Windows.
        // (Also UX.md TW4: one close vocabulary — the same rule as the
        // menu's Close Tab and `close_pane`.)
        s.is_shutting_down |= s.close_current_tab();
        log::info!("Tab closed via keybinding");
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
    ("split_down", |s: &mut WindowState| {
        s.split_pane_horizontal();
        true
    }),
    ("split_right", |s: &mut WindowState| {
        s.split_pane_vertical();
        true
    }),
    ("close_pane", |s: &mut WindowState| {
        // I15 cascade: the last pane of the last tab closes the window.
        s.is_shutting_down |= s.close_focused_pane();
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
    ("swap_pane_left", |s: &mut WindowState| {
        s.swap_pane(crate::pane::NavigationDirection::Left);
        true
    }),
    ("swap_pane_right", |s: &mut WindowState| {
        s.swap_pane(crate::pane::NavigationDirection::Right);
        true
    }),
    ("swap_pane_up", |s: &mut WindowState| {
        s.swap_pane(crate::pane::NavigationDirection::Up);
        true
    }),
    ("swap_pane_down", |s: &mut WindowState| {
        s.swap_pane(crate::pane::NavigationDirection::Down);
        true
    }),
    ("toggle_pane_zoom", pane_actions::toggle_pane_zoom),
    ("next_pane", pane_actions::next_pane),
    ("prev_pane", pane_actions::prev_pane),
    ("last_pane", pane_actions::last_pane),
    ("restart_pane", pane_actions::restart_pane),
    ("equalize_panes", pane_actions::equalize_panes),
    ("cycle_layout", pane_actions::cycle_layout),
    ("split_left", pane_actions::split_left),
    ("split_up", pane_actions::split_up),
    ("enter_resize_mode", pane_actions::enter_resize_mode),
    ("toggle_session_picker", |s: &mut WindowState| {
        s.overlay_ui.tmux_session_picker_ui.toggle();
        #[cfg(feature = "mux")]
        if s.overlay_ui.tmux_session_picker_ui.visible {
            s.refresh_mux_directory();
        }
        s.request_redraw();
        log::info!(
            "Session picker toggled via keybinding: {}",
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
    (
        "toggle_broadcast_input",
        pane_actions::toggle_broadcast_input,
    ),
    ("toggle_pane_broadcast", pane_actions::toggle_pane_broadcast),
    ("promote_pane_to_tab", |s: &mut WindowState| {
        s.promote_pane_to_tab();
        true
    }),
    ("demote_tab_to_pane", |s: &mut WindowState| {
        s.start_demote_tab();
        true
    }),
    // Open Profiles… (UX.md PR1, Cmd+O). The id predates the launcher; the
    // native menu item and saved bindings keep reaching it.
    ("toggle_profile_drawer", |s: &mut WindowState| {
        s.toggle_profile_drawer();
        log::info!(
            "Open Profiles toggled via keybinding: {}",
            if s.overlay_ui.profile_launcher_ui.visible {
                "open"
            } else {
                "closed"
            }
        );
        true
    }),
    // The Profiles drawer: Open Profiles pinned to the right edge (PR2).
    ("toggle_profiles_panel", |s: &mut WindowState| {
        s.toggle_profiles_drawer();
        true
    }),
    // UX.md PR3/PR5/PR6 profile actions; the per-profile ids
    // (`open_profile:<id>` …) resolve in `dispatch_profile_action`.
    ("manage_profiles", |s: &mut WindowState| {
        s.manage_profiles();
        true
    }),
    ("edit_tab_profile", |s: &mut WindowState| {
        s.edit_tab_profile();
        true
    }),
    ("toggle_tab_profile_pin", |s: &mut WindowState| {
        s.toggle_tab_profile_pin();
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
    ("clear_screen", clear_screen),
    ("scroll_up_page", |s: &mut WindowState| {
        s.scroll_up_page();
        s.request_redraw();
        true
    }),
    ("scroll_down_page", |s: &mut WindowState| {
        s.scroll_down_page();
        s.request_redraw();
        true
    }),
    ("scroll_to_top", |s: &mut WindowState| {
        s.scroll_to_top();
        s.request_redraw();
        true
    }),
    ("scroll_to_bottom", |s: &mut WindowState| {
        s.scroll_to_bottom();
        s.request_redraw();
        true
    }),
    ("scroll_to_previous_mark", |s: &mut WindowState| {
        s.scroll_to_previous_mark();
        s.request_redraw();
        true
    }),
    ("scroll_to_next_mark", |s: &mut WindowState| {
        s.scroll_to_next_mark();
        s.request_redraw();
        true
    }),
    // Menu-parity actions. These need the `WindowManager` — the event loop and
    // every window — not just a `WindowState`, so they go through the same queue
    // the in-app menu uses. Routing both through it means a keybinding and its
    // menu item cannot drift apart.
    ("new_window", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::NewWindow);
        true
    }),
    ("close_window", tab_nav_actions::close_window),
    ("close_tab_or_window", tab_nav_actions::close_tab_or_window),
    ("next_window", tab_nav_actions::next_window),
    ("prev_window", tab_nav_actions::prev_window),
    ("switch_to_window_1", tab_nav_actions::switch_to_window_1),
    ("switch_to_window_2", tab_nav_actions::switch_to_window_2),
    ("switch_to_window_3", tab_nav_actions::switch_to_window_3),
    ("switch_to_window_4", tab_nav_actions::switch_to_window_4),
    ("switch_to_window_5", tab_nav_actions::switch_to_window_5),
    ("switch_to_window_6", tab_nav_actions::switch_to_window_6),
    ("switch_to_window_7", tab_nav_actions::switch_to_window_7),
    ("switch_to_window_8", tab_nav_actions::switch_to_window_8),
    ("switch_to_window_9", tab_nav_actions::switch_to_window_9),
    ("last_tab", tab_nav_actions::last_tab),
    ("go_to_last_tab", tab_nav_actions::go_to_last_tab),
    ("rename_tab", tab_nav_actions::rename_tab),
    ("close_other_tabs", tab_nav_actions::close_other_tabs),
    (
        "move_tab_to_window_picker",
        tab_nav_actions::move_tab_to_window_picker,
    ),
    ("close_tabs_to_right", tab_nav_actions::close_tabs_to_right),
    ("quit", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::Quit);
        true
    }),
    ("select_all", |_s: &mut WindowState| {
        crate::menu::dispatch(crate::menu::MenuAction::SelectAll);
        true
    }),
    // UX.md A15: windows, tabs, and panes (hidden par-mux tabs included) in
    // one fuzzy list; the manager fills it while it is open.
    ("toggle_tree_picker", |s: &mut WindowState| {
        let chord = s.live_chord_hint("toggle_tree_picker");
        s.overlay_ui.tree_picker_ui.set_toggle_chord(chord);
        s.overlay_ui.tree_picker_ui.toggle();
        s.focus_state.needs_redraw = true;
        s.request_redraw();
        true
    }),
    // UX.md A17: jump to the next agent that is blocked, then done-unseen.
    ("focus_next_attention_agent", |s: &mut WindowState| {
        #[cfg(feature = "mux")]
        if s.focus_next_attention_agent() {
            return true;
        }
        s.show_toast("No agent needs attention");
        true
    }),
    // UX.md A22: a new par-mux session, no profile. Named session-N (the
    // picker takes a typed name); attaches this window, switching if needed.
    ("new_mux_session", |s: &mut WindowState| {
        #[cfg(feature = "mux")]
        {
            let name = crate::session_picker_mux::free_session_name(
                s.tmux_state
                    .mux_directory
                    .as_ref()
                    .map_or(&[][..], |d| &d.sessions[..]),
            );
            s.handle_mux_session_request(crate::session_picker_mux::MuxPickerAction::Create(name));
        }
        #[cfg(not(feature = "mux"))]
        s.show_toast("This build has no par-mux support");
        true
    }),
    // UX.md A23: the public detach id (`mux-detach` stays an alias through
    // ACTION_RENAMES). No transport means nothing to detach: say so.
    ("detach", |s: &mut WindowState| {
        #[cfg(feature = "mux")]
        if s.detach_mux_session() {
            log::info!("Detached from par-mux session");
            return true;
        }
        s.show_toast("Not attached to a par-mux session");
        true
    }),
    ("toggle_menu", |s: &mut WindowState| {
        crate::menu::request_toggle();
        s.request_redraw();
        true
    }),
];

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
        let action = par_term_config::config::keybindings_methods::current_action_id(action);
        // Open Profiles' Cmd+D / Cmd+Shift+D arrive here from the native
        // menu while the launcher is open (UX.md PR1).
        if self.launcher_takes_split(action) {
            return true;
        }
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
        } else if let Some(result) = pane_actions::layout_by_name(self, action) {
            result
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
        } else if let Some(result) = self.dispatch_profile_action(action) {
            result
        } else if action.starts_with("move_tab_to_window:") {
            self.dispatch_move_tab_to_window(action)
        } else if action.starts_with("attach_mux_session:") {
            #[cfg(feature = "mux")]
            return self.dispatch_attach_mux_session(action);
            #[cfg(not(feature = "mux"))]
            false
        } else if action == "mux-restart-pane" {
            self.palette_restart_mux_pane()
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
