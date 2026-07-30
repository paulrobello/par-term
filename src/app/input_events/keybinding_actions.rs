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

/// Handler for one named keybinding action.
///
/// Every entry in [`ACTION_HANDLERS`] returns `true`; the `bool` exists so the
/// signature matches `execute_keybinding_action`'s contract, where a name that
/// no handler claims returns `false`.
pub(super) type ActionHandler = fn(&mut WindowState) -> bool;

/// Exact-match dispatch table for named keybinding actions.
///
/// Entry order is irrelevant to behavior — every key is a distinct literal and
/// lookup is by equality — but keys must stay unique, which
/// `dispatch_tests::action_table_has_no_duplicate_keys` enforces.
///
/// Names not found here fall through to `execute_display_keybinding_action`
/// and then to the `snippet:` / `action:` / `restore_arrangement:` prefix
/// forms; see `execute_keybinding_action`.
pub(super) static ACTION_HANDLERS: &[(&str, ActionHandler)] = &[
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
    ("toggle_ai_inspector", toggle_ai_inspector),
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
        } else {
            log::warn!("Unknown keybinding action: {}", action);
            false
        }
    }
}
