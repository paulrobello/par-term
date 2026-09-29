//! View/window toggle action handlers, delegated from `keybinding_actions`.
//!
//! Split out of `keybinding_actions.rs` (line-count gate); the dispatch
//! table there imports these by name, matching the
//! `keybinding_display_actions` / `keybinding_helpers` seam pattern.

use crate::app::window_state::WindowState;

pub(crate) fn toggle_fullscreen(s: &mut WindowState) -> bool {
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

pub(crate) fn maximize_vertically(s: &mut WindowState) -> bool {
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

pub(crate) fn toggle_search(s: &mut WindowState) -> bool {
    s.overlay_ui.search_ui.toggle();
    if s.overlay_ui.search_ui.visible {
        s.overlay_ui.search_ui.init_from_config(
            s.config.load().search.search_case_sensitive,
            s.config.load().search.search_regex,
            s.config.load().search.search_wrap_around,
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

pub(crate) fn toggle_ai_inspector(s: &mut WindowState) -> bool {
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

pub(crate) fn move_tab_to_new_window(s: &mut WindowState) -> bool {
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

pub(crate) fn paste_special(s: &mut WindowState) -> bool {
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

pub(crate) fn toggle_session_logging(s: &mut WindowState) -> bool {
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

pub(crate) fn toggle_copy_mode(s: &mut WindowState) -> bool {
    if s.is_copy_mode_active() {
        s.exit_copy_mode();
    } else {
        s.enter_copy_mode();
    }
    true
}
