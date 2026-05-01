use crate::app::window_state::WindowState;
use crate::url_detection;
use std::sync::Arc;
use winit::event::{ElementState, MouseButton};

impl WindowState {
    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        // Get mouse position from active tab for shader interaction
        let mouse_position = self
            .tab_manager
            .active_tab()
            .map(|t| t.active_mouse().position)
            .unwrap_or((0.0, 0.0));

        let suppress_terminal_mouse_click = self
            .should_suppress_terminal_mouse_click_for_image_guard(button, state, mouse_position);

        // On left button release, always clear button_pressed and end any active selection
        // drag BEFORE any early returns so that state stays consistent when the release
        // lands in the tab bar, profile drawer, or context menu (all of which return early
        // without reaching the normal cleanup further below).
        //
        // Without button_pressed=false: dragging_divider can stay set and the next
        // mouse-move would continue phantom-dragging the divider with the button up.
        //
        // Without is_selecting=false: if the user dragged from the terminal into the tab
        // bar and released there, handle_left_mouse_release is never called, leaving
        // is_selecting=true and a visible selection highlight until the next terminal click.
        if button == MouseButton::Left
            && state == ElementState::Released
            && let Some(tab) = self.tab_manager.active_tab_mut()
        {
            tab.active_mouse_mut().button_pressed = false;
            tab.selection_mouse_mut().is_selecting = false;
        }

        // Check if profile drawer is open - let egui handle all mouse events
        if self.overlay_ui.profile_drawer_ui.expanded {
            self.request_redraw();
            return;
        }

        // Check if click is on the profile drawer toggle button
        let in_toggle_button = self.with_window(|window| {
            let size = window.inner_size();
            self.overlay_ui.profile_drawer_ui.is_point_in_toggle_button(
                mouse_position.0 as f32,
                mouse_position.1 as f32,
                size.width as f32,
                size.height as f32,
            )
        });
        if in_toggle_button == Some(true) {
            // Let egui handle the toggle button click
            self.request_redraw();
            return;
        }

        // Check if click is in the tab bar area - if so, let egui handle it
        // IMPORTANT: Do this BEFORE setting button_pressed to avoid selection state issues
        if self.is_mouse_in_tab_bar(mouse_position) {
            // Mark press as consumed so the matching release is also blocked,
            // even if is_egui_using_pointer() was stale when the press arrived
            // and didn't set ui_consumed_mouse_press in handle_window_event.
            if state == ElementState::Pressed {
                self.focus_state.ui_consumed_mouse_press = true;

                // Store a fallback tab switch: if the press bypassed egui's
                // wants_pointer_input() (stale state after window focus change
                // or rapid pointer movement), egui won't see the press and
                // clicked_by() won't fire. post_render checks this field and
                // applies the switch as a fallback.
                let scale_factor = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor())
                    .unwrap_or(1.0) as f32;
                let logical_pos = egui::pos2(
                    mouse_position.0 as f32 / scale_factor,
                    mouse_position.1 as f32 / scale_factor,
                );
                self.focus_state.pending_focus_tab_switch =
                    self.tab_bar_ui.tab_at_logical_pos(logical_pos);
            }
            // Request redraw so egui can process the click event
            self.request_redraw();
            return; // Click is on tab bar, don't process as terminal event
        }

        // Check if tab context menu is open - if so, let egui handle all clicks.
        // Request a redraw so egui can process click-away dismissal immediately.
        if self.tab_bar_ui.is_context_menu_open() {
            self.request_redraw();
            return;
        }

        // --- 1. Shader Interaction ---
        // Update shader mouse state for left button (matches Shadertoy iMouse convention)
        if button == MouseButton::Left
            && let Some(ref mut renderer) = self.renderer
        {
            renderer.set_shader_mouse_button(
                state == ElementState::Pressed,
                mouse_position.0 as f32,
                mouse_position.1 as f32,
            );
        }

        match button {
            MouseButton::Left => {
                self.handle_left_mouse_button(state, mouse_position, suppress_terminal_mouse_click);
            }
            MouseButton::Middle => {
                // When middle_click_paste is enabled, paste takes priority over mouse
                // tracking — matching iTerm2 behaviour. This prevents the common case
                // where middle-click silently forwards a mouse event to a TUI app (vim,
                // less, etc.) instead of pasting, causing apparent "different content"
                // compared to Cmd+V.
                if self.config.load().middle_click_paste {
                    if state == ElementState::Pressed {
                        // Phase 1: Focus the pane under the click, if it differs from the
                        // current focus. Must run before Phase 2's terminal lookup so that
                        // focused_pane() returns the clicked pane, not the old one.
                        {
                            let prev_focused = self
                                .tab_manager
                                .active_tab()
                                .filter(|t| t.has_multiple_panes())
                                .and_then(|t| t.focused_pane_id());

                            if let Some(tab) = self.tab_manager.active_tab_mut()
                                && tab.has_multiple_panes()
                                && let Some(pane_id) = tab
                                    .focus_pane_at(mouse_position.0 as f32, mouse_position.1 as f32)
                                && prev_focused != Some(pane_id)
                            {
                                tab.selection_mouse_mut().is_selecting = false;
                                if let Some(old_id) = prev_focused
                                    && let Some(pm) = tab.pane_manager.as_mut()
                                    && let Some(old_pane) = pm.get_pane_mut(old_id)
                                {
                                    old_pane.mouse.button_pressed = false;
                                }
                                // tab borrow ends here (NLL) — safe to call self methods.
                                self.set_tmux_focused_pane_from_native(pane_id);
                                self.set_scroll_target(0);
                                self.focus_state.needs_redraw = true;
                            }
                        }

                        // Phase 2: Read primary selection and paste into the now-focused pane.
                        if let Some(text) = self.input_handler.paste_from_primary_selection()
                            && let Some(tab) = self.tab_manager.active_tab()
                        {
                            let text = crate::paste_transform::sanitize_paste_content(&text);

                            // Route paste to the focused pane's terminal and compute
                            // pane-local click coordinates for the mouse-tracking focus-click.
                            // Falls back to tab.terminal + window coords (single-pane mode).
                            let (terminal_clone, click_cell) = if let Some(ref pm) =
                                tab.pane_manager
                                && let Some(focused_pane) = pm.focused_pane()
                            {
                                let terminal = Arc::clone(&focused_pane.terminal);
                                let cell = self.pixel_to_pane_cell(
                                    mouse_position.0,
                                    mouse_position.1,
                                    &focused_pane.bounds,
                                );
                                (terminal, cell)
                            } else {
                                (
                                    Arc::clone(&tab.terminal),
                                    self.pixel_to_cell(mouse_position.0, mouse_position.1),
                                )
                            };

                            self.runtime.spawn(async move {
                                let term = terminal_clone.write().await;

                                // If mouse tracking is active (e.g., tmux with mouse on),
                                // send a left-click press then release at the cursor
                                // position to focus the pane before the paste lands.
                                if term.is_mouse_tracking_enabled()
                                    && let Some((col, row)) = click_cell
                                {
                                    let press = term.encode_mouse_event(0, col, row, true, 0);
                                    let release = term.encode_mouse_event(0, col, row, false, 0);
                                    if !press.is_empty() {
                                        let _ = term.write(&press);
                                    }
                                    if !release.is_empty() {
                                        let _ = term.write(&release);
                                    }
                                }

                                let _ = term.paste(&text);
                            });
                        }
                    }
                    // Don't forward the middle button press/release itself to mouse
                    // tracking: the button is reserved for paste when this option is
                    // enabled (sending it would also trigger tmux's own paste-buffer).
                } else {
                    // Paste disabled — forward to terminal if mouse tracking is active.
                    self.try_send_mouse_event(1, state == ElementState::Pressed);
                }
            }
            MouseButton::Right => {
                // Try to send to terminal if mouse tracking is enabled
                let _ = self.try_send_mouse_event(2, state == ElementState::Pressed);
                // Event consumed by terminal (or ignored)
            }
            _ => {}
        }
    }

    fn handle_left_mouse_button(
        &mut self,
        state: ElementState,
        mouse_position: (f64, f64),
        suppress_terminal_mouse_click: bool,
    ) {
        // --- 2. URL Clicking ---
        // Check for modifier+Click on URL to open it in default browser
        // macOS: Cmd+Click (matches iTerm2 and system conventions)
        // Windows/Linux: Ctrl+Click (matches platform conventions)
        #[cfg(target_os = "macos")]
        let url_modifier_pressed = self.input_handler.modifiers.state().super_key();
        #[cfg(not(target_os = "macos"))]
        let url_modifier_pressed = self.input_handler.modifiers.state().control_key();

        // Use pane-local coordinates when split panes are active so the col/row
        // match the URL positions detected from the focused pane's terminal.
        let click_cell = self
            .tab_manager
            .active_tab()
            .and_then(|tab| {
                tab.pane_manager.as_ref().and_then(|pm| {
                    pm.focused_pane().and_then(|pane| {
                        self.pixel_to_pane_cell(mouse_position.0, mouse_position.1, &pane.bounds)
                    })
                })
            })
            .or_else(|| self.pixel_to_cell(mouse_position.0, mouse_position.1));

        if state == ElementState::Pressed
            && url_modifier_pressed
            && let Some((col, row)) = click_cell
            && let Some(tab) = self.tab_manager.active_tab()
        {
            let adjusted_row = row + tab.active_scroll_state().offset;

            if let Some(item) = url_detection::find_url_at_position(
                &tab.active_mouse().detected_urls,
                col,
                adjusted_row,
            ) {
                match &item.item_type {
                    url_detection::DetectedItemType::Url => {
                        if let Err(e) = url_detection::open_url(
                            &item.url,
                            &self.config.load().link_handler_command,
                        ) {
                            log::error!("Failed to open URL: {}", e);
                        }
                    }
                    url_detection::DetectedItemType::FilePath { line, column } => {
                        let editor_mode = self.config.load().semantic_history_editor_mode;
                        let editor_cmd = &self.config.load().semantic_history_editor;
                        let cwd = tab.get_cwd();
                        crate::debug_info!(
                            "SEMANTIC",
                            "Opening file path: {:?} line={:?} col={:?} mode={:?} editor_cmd={:?} cwd={:?}",
                            item.url,
                            line,
                            column,
                            editor_mode,
                            editor_cmd,
                            cwd
                        );
                        if let Err(e) = url_detection::open_file_in_editor(
                            &item.url,
                            *line,
                            *column,
                            editor_mode,
                            editor_cmd,
                            cwd.as_deref(),
                        ) {
                            crate::debug_error!("SEMANTIC", "Failed to open file: {}", e);
                        }
                    }
                }
                return; // Exit early: click handled
            }
        }

        // --- 3. Option+Click Cursor Positioning ---
        // NOTE: This must be checked BEFORE setting button_pressed to avoid triggering selection
        // Move cursor to clicked position when Option/Alt is pressed (without Cmd/Super)
        // This sends arrow key sequences to move the cursor within the shell line
        // macOS: Option+Click (matches iTerm2)
        // Windows/Linux: Alt+Click
        // Note: Option+Cmd is reserved for rectangular selection (matching iTerm2)
        if state == ElementState::Pressed
            && self.config.load().mouse.option_click_moves_cursor
            && self.input_handler.modifiers.state().alt_key()
            && !self.input_handler.modifiers.state().super_key() // Not Cmd/Super (that's for rectangular selection)
            && let Some((target_col, _target_row)) =
                self.pixel_to_cell(mouse_position.0, mouse_position.1)
            && let Some(tab) = self.tab_manager.active_tab()
        {
            // Only move cursor if we're at the bottom of scrollback (current view)
            // and not on the alternate screen (where apps handle their own cursor)
            let at_bottom = tab.active_scroll_state().offset == 0;
            // try_lock: intentional — double-click cursor-position query in sync loop.
            // On miss: defaults to (alt_screen=true, col=0) which skips the arrow-key
            // reposition logic. The cursor stays where it was — acceptable UX.
            let (is_alt_screen, current_col) = tab
                .terminal
                .try_write()
                .ok()
                .map(|t| (t.is_alt_screen_active(), t.cursor_position().0))
                .unwrap_or((true, 0));

            if at_bottom && !is_alt_screen {
                // Calculate horizontal movement needed
                // Send arrow keys: \x1b[C (right) or \x1b[D (left)
                let move_seq = if target_col > current_col {
                    // Move right
                    let count = target_col - current_col;
                    "\x1b[C".repeat(count)
                } else if target_col < current_col {
                    // Move left
                    let count = current_col - target_col;
                    "\x1b[D".repeat(count)
                } else {
                    // Already at target column
                    String::new()
                };

                if !move_seq.is_empty() {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.write().await;
                        let _ = t.write(move_seq.as_bytes());
                    });
                }
                return; // Exit early: cursor move handled
            }
        }

        // --- 4. Mouse Tracking Forwarding ---
        // Forward events to the PTY if terminal application requested tracking.
        // Shift held bypasses mouse tracking to allow local text selection
        // (standard terminal convention: iTerm2, Kitty, Alacritty all honour this).
        let shift_held = self.input_handler.modifiers.state().shift_key();
        if !suppress_terminal_mouse_click
            && !shift_held
            && self.try_send_mouse_event(0, state == ElementState::Pressed)
        {
            // Still track button state so mouse motion reporting works correctly.
            // ButtonEvent mode only reports motion when button_pressed is true,
            // so we must set this even though the click was consumed by tracking.
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_mouse_mut().button_pressed = state == ElementState::Pressed;
                // Record (or clear) the press position consumed by mouse tracking.
                // handle_mouse_move uses this to suppress button=32 drag events within
                // the dead zone, preventing trackpad jitter from making tmux treat a
                // pane-focus click as a drag-selection that wipes the clipboard.
                if state == ElementState::Pressed {
                    tab.active_mouse_mut().tracking_press_position = Some(mouse_position);
                    // Clear any stale local selection state.  When mouse tracking
                    // (e.g. tmux) owns the press, handle_left_mouse_press is never
                    // called, so any previous local selection would otherwise stay
                    // visible as a stuck highlight spanning tmux virtual panes.
                    // Also reset the click anchor fields so a stale click_pixel_position
                    // from a prior non-tracking press cannot trigger an accidental drag
                    // selection if a later motion event misses the tracking lock.
                    let sm = tab.selection_mouse_mut();
                    sm.selection = None;
                    sm.is_selecting = false;
                    sm.click_position = None;
                    sm.click_pixel_position = None;
                    sm.click_count = 0;
                } else {
                    tab.active_mouse_mut().tracking_press_position = None;
                    // Ensure dragging flag is cleared when tracking consumes the release.
                    tab.selection_mouse_mut().is_selecting = false;
                }
            }
            // If tracking consumed this release but a local selection was already in
            // progress (which happens when the press's try_lock missed while the PTY
            // reader held the terminal write lock, so try_send_mouse_event returned false
            // for the press but true for the release), finish the selection now.
            // get_selected_text() uses blocking_write() so this always succeeds.
            if state == ElementState::Released {
                let has_pending_selection = self
                    .tab_manager
                    .active_tab()
                    .is_some_and(|t| t.selection_mouse().selection.is_some());
                if has_pending_selection {
                    crate::debug_log!(
                        "MOUSE",
                        "Tracking consumed release but local selection pending — finishing selection"
                    );
                    self.handle_left_mouse_release();
                }
            }
            return; // Exit early: terminal app handled the input
        }
        if suppress_terminal_mouse_click {
            crate::debug_log!(
                "MOUSE",
                "Suppressing terminal mouse click forwarding to preserve image clipboard"
            );
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Fully consume the protected click so it doesn't become a local
                // selection anchor and affect the next drag-selection gesture.
                tab.active_mouse_mut().button_pressed = false;
                tab.selection_mouse_mut().is_selecting = false;
            }
            return;
        }

        // Track button press state for motion tracking logic (drag selection, motion reporting)
        // This is set AFTER special handlers (URL click, Option+click, mouse tracking) to avoid
        // triggering selection when those features handle the click.
        // Also clear tracking_press_position on release: the press was not forwarded to mouse
        // tracking (either tracking was disabled, or focus switched between panes), so any
        // stale position from a prior tracking interaction is cleaned up.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_mouse_mut().button_pressed = state == ElementState::Pressed;
            if state == ElementState::Released {
                tab.active_mouse_mut().tracking_press_position = None;
            }
        }

        if state == ElementState::Pressed {
            self.handle_left_mouse_press(mouse_position);
        } else {
            self.handle_left_mouse_release();
        }
    }

    /// Returns true if the given physical-pixel position falls within the tab bar area.
    ///
    /// Tab bar dimensions come from `TabBarUI` (logical pixels) and are scaled to physical
    /// pixels using the window's scale factor, matching the coordinate space of winit mouse events.
    pub(crate) fn is_mouse_in_tab_bar(&self, mouse_position: (f64, f64)) -> bool {
        let tab_count = self.tab_manager.tab_count();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config.load());
        let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &self.config.load());
        let scale_factor = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        match self.config.load().tab_bar_position {
            crate::config::TabBarPosition::Top => {
                mouse_position.1 < tab_bar_height as f64 * scale_factor
            }
            crate::config::TabBarPosition::Bottom => {
                let window_height = self
                    .window
                    .as_ref()
                    .map(|w| w.inner_size().height as f64)
                    .unwrap_or(0.0);
                mouse_position.1 > window_height - tab_bar_height as f64 * scale_factor
            }
            crate::config::TabBarPosition::Left => {
                mouse_position.0 < tab_bar_width as f64 * scale_factor
            }
        }
    }
}
