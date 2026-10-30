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
mod dispatch_phases;

#[cfg(test)]
mod chord_tests;

use self::dispatch_phases::{
    active_tab_all_panes_running, handle_clipboard_convention_keys, modal_mode_gate,
    prefix_systems_gate, read_active_key_modes, route_bytes_via_tmux, shift_enter_route,
    write_bytes_to_active_tab,
};
use crate::app::window_state::WindowState;
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
        if modal_mode_gate(self, &event) {
            return;
        }

        // Check if active tab's shell has exited.
        // Must check pane_manager panes (not tab.terminal) because after split pane
        // session restore, tab.terminal may be orphaned/dead while restored panes
        // have their own independent PTYs.
        let is_running = active_tab_all_panes_running(self);

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

        if prefix_systems_gate(self, &event) {
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
        if event.state == ElementState::Pressed
            && !passthrough
            && handle_clipboard_convention_keys(self, &event)
        {
            return;
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
            read_active_key_modes(self);

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
            if is_shift_enter && shift_enter_route(self, &mut bytes, alt_screen_active) {
                return;
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

            if route_bytes_via_tmux(self, &bytes) {
                return; // Input was routed through tmux or queued for the gateway
            }

            write_bytes_to_active_tab(self, bytes);
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
