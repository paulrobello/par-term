//! Keyboard event handler and key-shortcut sub-handlers.
//!
//! This module handles all keyboard input routing:
//! - `handle_key_event`: main key dispatch entry point (this file)
//! - `clipboard`: clipboard history toggle, `paste_text`
//! - `command_history`: the command history toggle
//!
//! Every shipped chord resolves through the registry as a default keybinding
//! (`defaults::menu_chords`, `defaults::layer_chords`, UX K2): the scroll,
//! reload, UI-toggle, utility, and tab chord layers all dissolved into those
//! defaults. The paste/copy branch at the end of `handle_key_event` is the
//! one deliberate exemption.
//!
//! No layer here handles a dialog, picker, or panel's keys. Those overlays
//! are Popups and Modals in the overlay stack (UX.md OV2), which consumes
//! every key they do not close on before this handler runs and feeds it to
//! egui, where each overlay's `show()` reads its own keys. The per-overlay
//! layers that used to sit in `KEY_LAYERS` were unreachable behind the stack
//! and were removed (MP1 Q4). The last layer, per-profile hotkeys, was
//! removed by UX MP3 (B59): profile shortcuts are registry bindings to
//! `open_profile:<id>` (see `crate::profile::actions`), so `KEY_LAYERS` is
//! gone with it.

pub(crate) mod claims;
mod clipboard;
mod command_history;
mod config_reload;

#[cfg(test)]
mod chord_tests;

use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

/// What a key press in a tab whose panes have all exited should do (B63).
///
/// The old fallback quit the whole app via `event_loop.exit()` on any stray
/// key — killing every other window and skipping crash capture. The key now
/// closes just the dead tab, or the window when it is the last tab, through
/// the same steps `handle_shell_exit` uses. A tab with a restart pending
/// (RestartWithPrompt / RestartAfterDelay) keeps the key: the restart
/// prompt's Enter handler further down `handle_key_event` owns it.
#[derive(Debug, PartialEq, Eq)]
enum ExitedTabKeypress {
    /// Another key consumer owns this event (restart prompt).
    Keep,
    /// Close the dead tab; the other tabs and the app stay open.
    CloseTab,
    /// Last tab: close the window instead.
    CloseWindow,
}

fn exited_tab_keypress_action(
    restart_pending: bool,
    visible_tab_count: usize,
) -> ExitedTabKeypress {
    if restart_pending {
        ExitedTabKeypress::Keep
    } else if visible_tab_count <= 1 {
        ExitedTabKeypress::CloseWindow
    } else {
        ExitedTabKeypress::CloseTab
    }
}

impl WindowState {
    pub(crate) fn handle_key_event(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        // Synthesize modifier state from physical key events.  On Windows, WM_NCACTIVATE can
        // cause ModifiersChanged(empty) without a matching WM_KILLFOCUS, leaving modifier state
        // permanently zeroed until the key is re-pressed.  Synthesizing here ensures the state
        // is always in sync with actual key press/release events regardless of ModifiersChanged
        // delivery reliability.
        self.input_handler.sync_modifier_from_key_event(&event);

        // Track Alt key press/release for Option key mode detection
        self.input_handler.track_alt_key(&event);

        // handle_window_event routed this key through the overlay stack
        // first (UX.md OV2): only keys no overlay owns, and keys for a
        // keyboard mode, arrive here. A dialog, picker, or menu (the MP0
        // modal guard) never lets a key through, so this is a backstop that
        // keeps the B61 guarantee even if a caller skips the stack. It does
        // not cover the three Popups outside the MP0 set (paste special,
        // pane rename, the in-app menu): for those the stack is the only
        // guard (`removed_key_layer_overlays_never_reach_key_dispatch`).
        let modal_guard_active = self.any_modal_ui_visible();
        if modal_guard_active {
            return;
        }

        // Check if egui UI wants keyboard input (e.g., text fields, ComboBoxes)
        if self.is_egui_using_keyboard() {
            return;
        }

        // Copy mode intercepts all keyboard input
        if self.is_copy_mode_active() {
            if event.state == ElementState::Pressed {
                self.handle_copy_mode_key(&event);
            }
            return;
        }

        // Pane-hint selection is a modal mode above every plugin overlay
        // (mode-stack contract, docs/plans/2026-09-24-overlay-plugin-design.md):
        // while armed it captures the next key press regardless of focus.
        if self.pane_hint_select.is_active() {
            if event.state == ElementState::Pressed {
                self.handle_pane_hint_select_key(&event);
            }
            return;
        }

        // Resize mode (A6) is the same kind of modal mode: it owns every
        // key until Escape or Enter.
        if self.pane_resize_mode.is_active() {
            self.handle_pane_resize_mode_key(&event);
            return;
        }

        // A focused plugin overlay is focus consumer 2 in the mode stack:
        // it swallows every key (the plugin owns no raw keys — design
        // constitutional) except Escape, which returns focus to the
        // terminal. Above terminal input, below modal modes (checked
        // above).
        if self.status_bar_ui.plugin_host().focused_overlay().is_some() {
            let is_escape = matches!(&event.logical_key, Key::Named(NamedKey::Escape));
            if event.state == ElementState::Pressed {
                self.resolve_focused_overlay_key(is_escape);
            }
            return;
        }

        // Check if active tab's shell has exited.
        // Must check pane_manager panes (not tab.terminal) because after split pane
        // session restore, tab.terminal may be orphaned/dead while restored panes
        // have their own independent PTYs.
        let is_running = if let Some(tab) = self.tab_manager.active_tab() {
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
        };

        // B63: a key press in a tab whose panes have all exited must not
        // quit the app. The old fallback aborted every tab's refresh task
        // and called event_loop.exit() here — killing all windows on a
        // stray key, and firing before the restart prompt's Enter handler
        // further down, so RestartWithPrompt could never receive its key.
        if !is_running && event.state == ElementState::Pressed {
            let restart_pending = self.tab_manager.active_tab().is_some_and(|tab| {
                tab.pane_manager()
                    .is_some_and(|pm| pm.all_panes().iter().any(|p| p.restart_state.is_some()))
            });
            match exited_tab_keypress_action(restart_pending, self.tab_manager.visible_tab_count())
            {
                ExitedTabKeypress::Keep => {}
                ExitedTabKeypress::CloseTab => {
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let id = tab.id;
                        log::info!("All panes of tab {id} exited; closing the tab on keypress");
                        let _ = self.tab_manager.close_tab(id);
                    }
                    return;
                }
                ExitedTabKeypress::CloseWindow => {
                    log::info!("Last tab's panes exited; closing the window on keypress");
                    self.is_shutting_down = true;
                    for tab in self.tab_manager.tabs_mut() {
                        tab.stop_refresh_task();
                    }
                    event_loop.exit();
                    return;
                }
            }
        }

        // Update last key press time for cursor blink reset and shader effects
        if event.state == ElementState::Pressed {
            self.cursor_anim.last_key_press = Some(std::time::Instant::now());
            // Update shader key press time for visual effects (iTimeKeyPress uniform)
            if let Some(renderer) = &mut self.renderer {
                renderer.update_key_press_time();
            }
        }

        // When custom-action prefix mode is armed, it owns the next key press.
        // Swallow all follow-up key events while armed so they can't leak through
        // to tmux shortcuts, user keybindings, or the terminal PTY.
        if self.custom_action_prefix_state.is_active() {
            crate::debug_info!(
                "PREFIX_ACTION",
                "Mode active, handling follow-up key={:?} state={:?}",
                event.logical_key,
                event.state
            );
            let handled = self.handle_custom_action_prefix_key(&event);
            if handled || self.custom_action_prefix_state.is_active() {
                return;
            }
        }

        // Cancel pane transfer pick mode on Escape
        if self.pane_transfer_state.is_active()
            && matches!(event.logical_key, Key::Named(NamedKey::Escape))
        {
            self.cancel_pane_transfer();
            return;
        }

        // Prefix systems must run before normal keybindings so the follow-up key
        // is consumed by the two-stroke action instead of another shortcut.
        // The leader (UX.md K4) owns its chord, the tmux prefix in a gateway
        // tab, and every key while armed — ahead of the registry, so a user
        // binding on the leader chord cannot shadow it.
        if self
            .handle_leader_press(crate::app::leader::LeaderPress::from_event(&event))
            .is_some()
        {
            return;
        }

        if self.handle_custom_action_prefix_key(&event) {
            return;
        }

        // Unbind/passthrough (UX K2/K27): set when the registry matched a
        // `pass_to_terminal` row below. The paste/copy branch must not take
        // the key after that — it must reach the PTY encoding at the end of
        // this handler.
        let mut passthrough = false;

        // Check user-defined keybindings first (before hardcoded shortcuts)
        if event.state == ElementState::Pressed
            && let Some(action) = self.keybinding_registry.lookup_with_options(
                &event,
                &self.input_handler.modifiers,
                &self.config.load().input.modifier_remapping,
                self.config.load().input.use_physical_keys,
            )
        {
            crate::debug_info!(
                "KEYBINDING",
                "Keybinding matched: action={}, key={:?}, modifiers={:?}",
                action,
                event.logical_key,
                self.input_handler.modifiers
            );
            // Clone to avoid borrow conflict
            let action = action.to_string();
            if action == par_term_keybindings::PASS_TO_TERMINAL {
                passthrough = true;
            } else if self.execute_keybinding_action(&action) {
                return; // Key was handled by user-defined keybinding
            }
        } else if event.state == ElementState::Pressed {
            crate::debug_log!(
                "KEYBINDING",
                "No keybinding match for key={:?}, modifiers={:?}",
                event.logical_key,
                self.input_handler.modifiers
            );
        }

        // Paste/copy: the one deliberate hardcoded exemption from the
        // registry-only chord rule (UX K2). These are OS clipboard
        // conventions every native app answers to, not terminal actions —
        // the menu's Copy/Paste accelerators are exempt from registry
        // backing for the same reason, and `claims::PASTE_COPY` models this
        // branch so chord precedence over it stays checkable.
        if event.state == ElementState::Pressed && !passthrough {
            // macOS: Cmd+V, NamedKey::Paste
            // Windows/Linux: Ctrl+Shift+V, Shift+Insert, NamedKey::Paste
            // (Ctrl+V is "literal next" in terminals, must not be intercepted)
            #[cfg(not(target_os = "macos"))]
            let is_paste = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                let shift = self.input_handler.modifiers.state().shift_key();
                matches!(event.logical_key, Key::Named(NamedKey::Paste))
                    || (ctrl
                        && shift
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
                    || (shift && matches!(event.logical_key, Key::Named(NamedKey::Insert)))
            };

            #[cfg(target_os = "macos")]
            let is_paste = {
                let cmd = self.input_handler.modifiers.state().super_key();
                matches!(event.logical_key, Key::Named(NamedKey::Paste))
                    || (cmd
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
            };

            if is_paste {
                if let Some(text) = self.input_handler.paste_from_clipboard() {
                    log::debug!("Paste: got {} chars of text from clipboard", text.len());
                    // paste_text sanitizes and routes: daemon pane in a mux
                    // tab, focused pane's terminal locally. The old inline
                    // paste wrote tab.terminal — the hidden shell in a mux
                    // tab.
                    self.paste_text(&text);
                } else if self.input_handler.clipboard_has_image() {
                    // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
                    // image-aware child processes (e.g., Claude Code) can handle image paste
                    log::debug!(
                        "Paste: clipboard has image but no text, forwarding Ctrl+V to terminal"
                    );
                    if let Some(tab) = self.tab_manager.active_tab() {
                        // A mux tab's `tab.terminal` is a hidden login shell —
                        // route the daemon pane first.
                        if !self.route_mux_tab_write(tab, b"\x16") {
                            let terminal_clone = Arc::clone(&tab.terminal);
                            self.runtime.spawn(async move {
                                let term = terminal_clone.read().await;
                                if let Err(e) = term.write(b"\x16") {
                                    crate::debug_error!(
                                        "INPUT",
                                        "PTY write failed (image paste): {e}"
                                    );
                                }
                            });
                        }
                    }
                } else {
                    log::debug!("Paste: clipboard has neither text nor image");
                }
                return;
            }

            // macOS: Cmd+C, NamedKey::Copy
            // Windows/Linux: Ctrl+Shift+C, NamedKey::Copy
            // (Ctrl+C is SIGINT in terminals, must not be intercepted)
            #[cfg(target_os = "macos")]
            let is_copy = {
                let cmd = self.input_handler.modifiers.state().super_key();
                matches!(event.logical_key, Key::Named(NamedKey::Copy))
                    || (cmd
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
            };

            #[cfg(not(target_os = "macos"))]
            let is_copy = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                let shift = self.input_handler.modifiers.state().shift_key();
                matches!(event.logical_key, Key::Named(NamedKey::Copy))
                    || (ctrl
                        && shift
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
            };

            if is_copy {
                if let Some(selected_text) = self.get_selected_text_for_copy() {
                    if let Err(e) = self.input_handler.copy_to_clipboard(&selected_text) {
                        log::error!("Failed to copy to clipboard: {}", e);
                    } else {
                        log::debug!("Copied {} chars via keyboard copy", selected_text.len());
                        // Sync to tmux paste buffer if connected
                        self.sync_clipboard_to_tmux(&selected_text);
                    }
                }
                return;
            }
        }

        // Clear selection on keyboard input (except for modifier-only keys and special keys handled above)
        // Don't clear selection when pressing just modifier keys (Ctrl, Alt, Shift, Cmd)
        let is_modifier_only = matches!(
            event.logical_key,
            Key::Named(
                NamedKey::Control
                    | NamedKey::Alt
                    | NamedKey::Shift
                    | NamedKey::Super
                    | NamedKey::Meta
            )
        );

        if event.state == ElementState::Pressed
            && !is_modifier_only
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.selection_mouse().selection.is_some()
        {
            tab.selection_mouse_mut().selection = None;
            self.request_redraw();
        }

        // B61: the PTY encoding tail below must never run while a modal
        // overlay is open. The early return above already guarantees it;
        // this re-check guards against an action run by a layer above that
        // opened a dialog mid-dispatch.
        if self.any_modal_ui_visible() {
            return;
        }

        // Get terminal modes (if available).
        //
        // try_read: intentional — only reading terminal mode flags, no mutation needed.
        // Multiple readers can hold the lock simultaneously, so this rarely blocks.
        //
        // Cache fallback: in release/LTO builds the renderer's per-frame `try_write`
        // collides with this `try_read` often enough that the previous "fall back to
        // (0, false, false)" behavior caused Shift+Enter under tmux to silently send
        // raw LF (which tmux re-encodes as Ctrl+J → \x1b[106;5u for mode-2 panes,
        // not recognized by Claude Code as Shift+Enter). `read_or_cached_modes`
        // returns the last successfully-read values on contention, so modifier-aware
        // encoding stays correct.
        // Read keyboard mode flags from the correct terminal.
        //
        // When pane splits exist, the focused pane may have a different mode state
        // than the primary pane (e.g., vim in primary with modifyOtherKeys=2, bash
        // in split with modifyOtherKeys=0). Reading from the wrong terminal causes
        // keys like Ctrl+U to be encoded as CSI-u sequences that the focused pane's
        // shell can't interpret, producing garbage control characters.
        //
        // Priority: focused pane's terminal → tab's cached modes (fallback).
        let (modify_other_keys_mode, application_cursor, alt_screen_active) =
            if let Some(tab) = self.tab_manager.active_tab() {
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
            };

        // Detect Shift+Enter before the event is consumed by handle_key_event_with_mode.
        // Par-term follows the iTerm2 convention: regular Enter emits CR (\r) so
        // shells submit the command line, Shift+Enter emits LF (\n) so chat-style
        // TUIs (Claude Code, pi agent, etc.) insert a soft newline.
        //
        // That LF convention breaks inside tmux because tmux converts raw 0x0a
        // (LF) into Ctrl+J (tty-keys.c: C0 control codes except HT/CR/ESC are
        // converted to Ctrl+key equivalents), then re-encodes Ctrl+J as
        // \x1b[106;5u for MODE_KEYS_EXTENDED_2 panes. The inner app never sees
        // a Shift+Enter it recognizes.
        //
        // Fix: send \x1b[13;2u (CSI-u Shift+Enter) whenever a TUI context is
        // active. tmux's extended-keys parser re-encodes for the pane's negotiated
        // protocol (kitty or modifyOtherKeys), and direct TUIs parse CSI-u
        // natively. In shell context (no alternate screen), keep the iTerm2 \n
        // convention for soft newlines.
        let is_shift_enter = self.input_handler.modifiers.state().shift_key()
            && matches!(event.logical_key, Key::Named(NamedKey::Enter));

        // Normal key handling - send to terminal (or via tmux if connected)
        if let Some(mut bytes) = self.input_handler.handle_key_event_with_mode(
            event,
            modify_other_keys_mode,
            application_cursor,
        ) {
            if is_shift_enter {
                // Gateway path: route raw LF via send-keys -H so it bypasses
                // tmux's per-pane re-encoding. The old C-j rewrite was being
                // turned into \x1b[27;5;106~ for mode-2 apps.
                if self.send_literal_bytes_via_tmux(b"\n") {
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.activity.anti_idle_last_activity = std::time::Instant::now();
                    }
                    return;
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
                } else if self.shell_has_tmux_child() {
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
                    bytes = b"\x1b[13;2u".to_vec();
                }
            }

            // Broadcast claims the key before single-pane routing: further
            // down, send_input_via_tmux would route it to the focused pane
            // only. Mux-guarded so gateway-tmux tabs keep their existing
            // ordering (their broadcast branch never ran; the send-keys
            // claim below owns the key there).
            if self.broadcast_bytes(&bytes) {
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.activity.anti_idle_last_activity = std::time::Instant::now();
                }
                return;
            }

            // A focused par-mux pane whose process exited is HELD by the
            // daemon: Enter restarts it (respawn-pane), other keys have no
            // process to reach — the mux twin of RestartWithPrompt below.
            #[cfg(feature = "mux")]
            if self.handle_key_for_exited_mux_pane(&bytes).is_some() {
                return;
            }

            // Try to send via tmux if connected (check before borrowing tab)
            if self.send_input_via_tmux(&bytes) {
                // Still need to reset anti-idle timer
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.activity.anti_idle_last_activity = std::time::Instant::now();
                }
                return; // Input was routed through tmux
            }

            // When tmux is connected, send_input_via_tmux may have failed because
            // the gateway terminal's RwLock is held (e.g., by a prior async write
            // blocked on the inner parking_lot::Mutex while the PTY reader processes
            // tmux output). Do NOT fall through to the direct PTY write path — that
            // writes to the wrong terminal (the tmux display tab instead of the
            // gateway). Instead, retry asynchronously so the keystroke is delivered
            // as soon as the gateway lock becomes available.
            if self.is_tmux_connected() {
                crate::debug_info!(
                    "TMUX_INPUT",
                    "Gateway lock contention — queuing {} bytes for async delivery",
                    bytes.len()
                );
                // Format the send-keys command while we still have access to the
                // tmux session state (synchronous borrow), then write the
                // pre-formatted command to the gateway PTY asynchronously.
                let cmd = if let Some(session) = &self.tmux_state.tmux_session {
                    match session.format_send_keys(&bytes) {
                        Some(c) => c,
                        None => {
                            let args = crate::tmux::send_keys_arguments(&bytes);
                            format!("send-keys {}\n", args)
                        }
                    }
                } else {
                    let args = crate::tmux::send_keys_arguments(&bytes);
                    format!("send-keys {}\n", args)
                };
                if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
                    && let Some(tab) = self.tab_manager.get_tab(gateway_tab_id)
                {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    let cmd_bytes = cmd.into_bytes();
                    self.runtime.spawn(async move {
                        let term = terminal_clone.read().await;
                        if let Err(e) = term.write(&cmd_bytes) {
                            crate::debug_error!("INPUT", "PTY write failed (tmux send-keys): {e}");
                        }
                    });
                }
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.activity.anti_idle_last_activity = std::time::Instant::now();
                }
                return;
            }

            // Broadcast input to all panes or just the focused pane
            if let Some(tab) = self.tab_manager.active_tab_mut() {
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
                        if let Err(e) = focused_pane.respawn_shell(&self.config.load()) {
                            log::error!(
                                "Failed to respawn shell in pane {}: {}",
                                focused_pane.id,
                                e
                            );
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
                self.runtime.spawn(async move {
                    let term = terminal_clone.read().await;
                    if let Err(e) = term.write(&bytes) {
                        crate::debug_error!("INPUT", "PTY write failed (key input): {e}");
                    }
                });
            }
        }
    }

    /// Resolve a key press while a plugin overlay holds focus (mode stack
    /// consumer 2): only Escape acts — it returns focus to the terminal.
    /// Every other key is swallowed (the caller returns before terminal
    /// input); a focused overlay never receives raw keys (design
    /// constitutional). Split out of `handle_key_event` so tests can drive
    /// the resolution without fabricating a winit `KeyEvent`.
    pub(crate) fn resolve_focused_overlay_key(&mut self, is_escape: bool) {
        if is_escape {
            self.status_bar_ui.plugin_host_mut().unfocus_overlay();
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }
}
