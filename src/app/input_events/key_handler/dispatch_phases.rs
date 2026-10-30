//! Phase functions of `WindowState::handle_key_event` (free functions per
//! the ARC-001 impl-block budget: no new `impl WindowState` blocks).
//!
//! Each function is a verbatim body move from `handle_key_event` at one of
//! its natural ownership boundaries — modal-mode gating, tab liveness,
//! prefix systems, clipboard-convention keys, terminal key-mode reads, and
//! key-byte routing — so the dispatcher reads as an ordered gate list and
//! adding a mode or a routing path stops growing its branch count.
//! Gate-style functions return true when the key was consumed and the
//! dispatcher must stop.

use std::sync::Arc;

use winit::event::{ElementState, KeyEvent};

use winit::keyboard::{Key, NamedKey};

use crate::app::window_state::WindowState;

pub(super) fn modal_mode_gate(ws: &mut WindowState, event: &KeyEvent) -> bool {
    let modal_guard_active = ws.any_modal_ui_visible();
    if modal_guard_active {
        return true;
    }

    // Check if egui UI wants keyboard input (e.g., text fields, ComboBoxes)
    if ws.is_egui_using_keyboard() {
        return true;
    }

    // Copy mode intercepts all keyboard input
    if ws.is_copy_mode_active() {
        if event.state == ElementState::Pressed {
            ws.handle_copy_mode_key(event);
        }
        return true;
    }

    // Pane-hint selection is a modal mode above every plugin overlay
    // (mode-stack contract, docs/plans/2026-09-24-overlay-plugin-design.md):
    // while armed it captures the next key press regardless of focus.
    if ws.pane_hint_select.is_active() {
        if event.state == ElementState::Pressed {
            ws.handle_pane_hint_select_key(event);
        }
        return true;
    }

    // Resize mode (A6) is the same kind of modal mode: it owns every
    // key until Escape or Enter.
    if ws.pane_resize_mode.is_active() {
        ws.handle_pane_resize_mode_key(event);
        return true;
    }

    // A focused plugin overlay is focus consumer 2 in the mode stack:
    // it swallows every key (the plugin owns no raw keys — design
    // constitutional) except Escape, which returns focus to the
    // terminal. Above terminal input, below modal modes (checked
    // above).
    if ws.status_bar_ui.plugin_host().focused_overlay().is_some() {
        let is_escape = matches!(&event.logical_key, Key::Named(NamedKey::Escape));
        if event.state == ElementState::Pressed {
            ws.resolve_focused_overlay_key(is_escape);
        }
        return true;
    }
    false
}

pub(super) fn active_tab_all_panes_running(ws: &WindowState) -> bool {
    if let Some(tab) = ws.tab_manager.active_tab() {
        // Tmux-managed tabs (gateway and display) have no local PTY process —
        // panes are created via Pane::new_for_tmux() which does not spawn a shell,
        // so PtySession::running is initialized to false. Never treat these as
        // exited; the actual process is the remote tmux session.
        if tab.tmux.tmux_gateway_active || tab.tmux.tmux_pane_id.is_some() {
            true
        } else if let Some(pm) = tab.pane_manager() {
            pm.all_panes().iter().any(|p| p.is_running())
        } else {
            // Fallback: no pane manager, check tab.terminal directly
            if let Ok(term) = tab.terminal.try_read() {
                term.is_running()
            } else {
                true
            }
        }
    } else {
        true
    }
}

pub(super) fn prefix_systems_gate(ws: &mut WindowState, event: &KeyEvent) -> bool {
    // When custom-action prefix mode is armed, it owns the next key press.
    // Swallow all follow-up key events while armed so they can't leak through
    // to tmux shortcuts, user keybindings, or the terminal PTY.
    if ws.custom_action_prefix_state.is_active() {
        crate::debug_info!(
            "PREFIX_ACTION",
            "Mode active, handling follow-up key={:?} state={:?}",
            event.logical_key,
            event.state
        );
        let handled = ws.handle_custom_action_prefix_key(event);
        if handled || ws.custom_action_prefix_state.is_active() {
            return true;
        }
    }

    // Cancel pane transfer pick mode on Escape
    if ws.pane_transfer_state.is_active()
        && matches!(event.logical_key, Key::Named(NamedKey::Escape))
    {
        ws.cancel_pane_transfer();
        return true;
    }

    // Prefix systems must run before normal keybindings so the follow-up key
    // is consumed by the two-stroke action instead of another shortcut.
    // The leader (UX.md K4) owns its chord, the tmux prefix in a gateway
    // tab, and every key while armed — ahead of the registry, so a user
    // binding on the leader chord cannot shadow it.
    if ws
        .handle_leader_press(crate::app::leader::LeaderPress::from_event(event))
        .is_some()
    {
        return true;
    }

    if ws.handle_custom_action_prefix_key(event) {
        return true;
    }
    false
}

pub(super) fn handle_clipboard_convention_keys(ws: &mut WindowState, event: &KeyEvent) -> bool {
    // macOS: Cmd+V, NamedKey::Paste
    // Windows/Linux: Ctrl+Shift+V, Shift+Insert, NamedKey::Paste
    // (Ctrl+V is "literal next" in terminals, must not be intercepted)
    #[cfg(not(target_os = "macos"))]
    let is_paste = {
        let ctrl = ws.input_handler.modifiers.state().control_key();
        let shift = ws.input_handler.modifiers.state().shift_key();
        matches!(event.logical_key, Key::Named(NamedKey::Paste))
            || (ctrl
                && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
            || (shift && matches!(event.logical_key, Key::Named(NamedKey::Insert)))
    };

    #[cfg(target_os = "macos")]
    let is_paste = {
        let cmd = ws.input_handler.modifiers.state().super_key();
        matches!(event.logical_key, Key::Named(NamedKey::Paste))
            || (cmd
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
    };

    if is_paste {
        if let Some(text) = ws.input_handler.paste_from_clipboard() {
            log::debug!("Paste: got {} chars of text from clipboard", text.len());
            // paste_text sanitizes and routes: daemon pane in a mux
            // tab, focused pane's terminal locally. The old inline
            // paste wrote tab.terminal — the hidden shell in a mux
            // tab.
            ws.paste_text(&text);
        } else if ws.input_handler.clipboard_has_image() {
            // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
            // image-aware child processes (e.g., Claude Code) can handle image paste
            log::debug!("Paste: clipboard has image but no text, forwarding Ctrl+V to terminal");
            if let Some(tab) = ws.tab_manager.active_tab() {
                // A mux tab's `tab.terminal` is a hidden login shell —
                // route the daemon pane first.
                if !ws.route_mux_tab_write(tab, b"\x16") {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    ws.runtime.spawn(async move {
                        let term = terminal_clone.read().await;
                        if let Err(e) = term.write(b"\x16") {
                            crate::debug_error!("INPUT", "PTY write failed (image paste): {e}");
                        }
                    });
                }
            }
        } else {
            log::debug!("Paste: clipboard has neither text nor image");
        }
        return true;
    }

    // macOS: Cmd+C, NamedKey::Copy
    // Windows/Linux: Ctrl+Shift+C, NamedKey::Copy
    // (Ctrl+C is SIGINT in terminals, must not be intercepted)
    #[cfg(target_os = "macos")]
    let is_copy = {
        let cmd = ws.input_handler.modifiers.state().super_key();
        matches!(event.logical_key, Key::Named(NamedKey::Copy))
            || (cmd
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
    };

    #[cfg(not(target_os = "macos"))]
    let is_copy = {
        let ctrl = ws.input_handler.modifiers.state().control_key();
        let shift = ws.input_handler.modifiers.state().shift_key();
        matches!(event.logical_key, Key::Named(NamedKey::Copy))
            || (ctrl
                && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
    };

    if is_copy {
        if let Some(selected_text) = ws.get_selected_text_for_copy() {
            if let Err(e) = ws.input_handler.copy_to_clipboard(&selected_text) {
                log::error!("Failed to copy to clipboard: {}", e);
            } else {
                log::debug!("Copied {} chars via keyboard copy", selected_text.len());
                // Sync to tmux paste buffer if connected
                ws.sync_clipboard_to_tmux(&selected_text);
            }
        }
        return true;
    }
    false
}

pub(super) fn read_active_key_modes(ws: &WindowState) -> (u8, bool, bool) {
    if let Some(tab) = ws.tab_manager.active_tab() {
        if let Some(ref pane_manager) = tab.pane_manager {
            if let Some(focused_pane) = pane_manager.focused_pane() {
                if let Ok(term) = focused_pane.terminal.try_read() {
                    (
                        term.modify_other_keys_mode(),
                        term.application_cursor(),
                        term.is_alt_screen_active(),
                    )
                } else {
                    // Lock contention on focused pane — fall back to tab's cache
                    tab.read_or_cached_modes()
                }
            } else {
                tab.read_or_cached_modes()
            }
        } else {
            tab.read_or_cached_modes()
        }
    } else {
        (0, false, false)
    }
}

pub(super) fn shift_enter_route(
    ws: &mut WindowState,
    bytes: &mut Vec<u8>,
    alt_screen_active: bool,
) -> bool {
    // Gateway path: route raw LF via send-keys -H so it bypasses
    // tmux's per-pane re-encoding. The old C-j rewrite was being
    // turned into \x1b[27;5;106~ for mode-2 apps.
    if ws.send_literal_bytes_via_tmux(b"\n") {
        if let Some(tab) = ws.tab_manager.active_tab_mut() {
            tab.activity.anti_idle_last_activity = std::time::Instant::now();
        }
        return true;
    }

    // Non-gateway path: decide between \n (iTerm2 convention for
    // shells) and \x1b[13;2u (CSI-u for TUIs).
    //
    // Use alternate screen buffer as the primary signal: TUI apps
    // (including tmux wrapping a TUI) always enter alternate screen.
    // When active, send CSI-u so tmux can re-encode for the inner
    // pane's negotiated protocol, or so direct kitty TUIs parse it.
    //
    // Fall back to process-tree tmux detection for edge cases where
    // tmux is running but alternate screen hasn't been entered yet.
    let send_csi_u = if alt_screen_active {
        crate::debug_info!(
            "SHIFTENTER",
            "alt-screen active — sending CSI-u \\x1b[13;2u"
        );
        true
    } else if ws.shell_has_tmux_child() {
        crate::debug_info!(
            "SHIFTENTER",
            "tmux child detected (no alt-screen) — sending CSI-u \\x1b[13;2u"
        );
        true
    } else {
        crate::debug_info!(
            "SHIFTENTER",
            "shell context — sending LF (iTerm2 convention)"
        );
        false
    };

    if send_csi_u {
        *bytes = b"\x1b[13;2u".to_vec();
    }
    false
}

pub(super) fn route_bytes_via_tmux(ws: &mut WindowState, bytes: &[u8]) -> bool {
    // Try to send via tmux if connected (check before borrowing tab)
    if ws.send_input_via_tmux(bytes) {
        // Still need to reset anti-idle timer
        if let Some(tab) = ws.tab_manager.active_tab_mut() {
            tab.activity.anti_idle_last_activity = std::time::Instant::now();
        }
        return true; // Input was routed through tmux
    }

    // When tmux is connected, send_input_via_tmux may have failed because
    // the gateway terminal's RwLock is held (e.g., by a prior async write
    // blocked on the inner parking_lot::Mutex while the PTY reader processes
    // tmux output). Do NOT fall through to the direct PTY write path — that
    // writes to the wrong terminal (the tmux display tab instead of the
    // gateway). Instead, retry asynchronously so the keystroke is delivered
    // as soon as the gateway lock becomes available.
    if ws.is_tmux_connected() {
        crate::debug_info!(
            "TMUX_INPUT",
            "Gateway lock contention — queuing {} bytes for async delivery",
            bytes.len()
        );
        // Format the send-keys command while we still have access to the
        // tmux session state (synchronous borrow), then write the
        // pre-formatted command to the gateway PTY asynchronously.
        let cmd = if let Some(session) = &ws.tmux_state.tmux_session {
            match session.format_send_keys(bytes) {
                Some(c) => c,
                None => {
                    let args = crate::tmux::send_keys_arguments(bytes);
                    format!("send-keys {}\n", args)
                }
            }
        } else {
            let args = crate::tmux::send_keys_arguments(bytes);
            format!("send-keys {}\n", args)
        };
        if let Some(gateway_tab_id) = ws.tmux_state.tmux_gateway_tab_id
            && let Some(tab) = ws.tab_manager.get_tab(gateway_tab_id)
        {
            let terminal_clone = Arc::clone(&tab.terminal);
            let cmd_bytes = cmd.into_bytes();
            ws.runtime.spawn(async move {
                let term = terminal_clone.read().await;
                if let Err(e) = term.write(&cmd_bytes) {
                    crate::debug_error!("INPUT", "PTY write failed (tmux send-keys): {e}");
                }
            });
        }
        if let Some(tab) = ws.tab_manager.active_tab_mut() {
            tab.activity.anti_idle_last_activity = std::time::Instant::now();
        }
        return true;
    }
    false
}

pub(super) fn write_bytes_to_active_tab(ws: &mut WindowState, bytes: Vec<u8>) {
    // Broadcast input to all panes or just the focused pane
    if let Some(tab) = ws.tab_manager.active_tab_mut() {
        // Reset anti-idle timer on keyboard input
        tab.activity.anti_idle_last_activity = std::time::Instant::now();

        // Check if focused pane is awaiting restart input (Enter key to restart)
        if let Some(ref mut pane_manager) = tab.pane_manager
            && let Some(focused_pane) = pane_manager.focused_pane_mut()
            && matches!(
                focused_pane.restart_state,
                Some(crate::pane::RestartState::AwaitingInput)
            )
        {
            // Check if this is an Enter key (bytes == "\r" or "\n")
            if bytes == b"\r" || bytes == b"\n" || bytes == b"\r\n" {
                log::info!(
                    "Enter pressed, restarting shell in pane {}",
                    focused_pane.id
                );
                if let Err(e) = focused_pane.respawn_shell(&ws.config.load()) {
                    log::error!("Failed to respawn shell in pane {}: {}", focused_pane.id, e);
                }
                return;
            }
            // For any other key, ignore it while awaiting input
            return;
        }

        // Get the terminal to write to:
        // - If split panes exist, use the focused pane's terminal
        // - Otherwise, use the tab's main terminal
        let terminal_clone = if let Some(ref pane_manager) = tab.pane_manager {
            if let Some(focused_pane) = pane_manager.focused_pane() {
                Arc::clone(&focused_pane.terminal)
            } else {
                Arc::clone(&tab.terminal)
            }
        } else {
            Arc::clone(&tab.terminal)
        };

        // read() not write(): TerminalManager::write() takes &self (shared
        // reference) because mutation is serialized by the inner
        // parking_lot::Mutex.  Using a read lock here prevents the keyboard
        // write from exclusively holding the outer RwLock while blocked on the
        // inner Mutex, which would starve the refresh task (try_read) and the
        // render pipeline (try_write) of their generation checks.
        ws.runtime.spawn(async move {
            let term = terminal_clone.read().await;
            if let Err(e) = term.write(&bytes) {
                crate::debug_error!("INPUT", "PTY write failed (key input): {e}");
            }
        });
    }
}
