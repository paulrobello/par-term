use crate::app::window_state::WindowState;
use crate::selection::{Selection, SelectionMode};
use crate::ui_constants::DRAG_THRESHOLD_PX;
use crate::url_detection;
use std::sync::Arc;

impl WindowState {
    pub(crate) fn handle_mouse_move(&mut self, position: (f64, f64)) {
        // Update mouse position in active tab (always needed for egui)
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_mouse_mut().position = position;
        }

        // If a protected image-clipboard click turns into a drag, restore normal terminal
        // mouse behavior by sending the press event once movement proves drag intent.
        self.maybe_forward_guarded_terminal_mouse_press_on_drag(position);

        // Notify status bar of mouse activity (for auto-hide timer)
        self.status_bar_ui.on_mouse_activity();

        // Check if profile drawer is open - let egui handle mouse events
        if self.overlay_ui.profile_drawer_ui.expanded {
            self.request_redraw();
            return;
        }

        // Check if mouse is in the tab bar area - if so, skip terminal-specific processing
        // Position update above is still needed for proper event handling
        // Tab bar height is in logical pixels (egui); position is physical pixels (winit)
        let tab_bar_height = self
            .tab_bar_ui
            .get_height(self.tab_manager.tab_count(), &self.config);
        let scale_factor = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        if position.1 < tab_bar_height as f64 * scale_factor {
            // Request redraw so egui can update hover states
            self.request_redraw();
            return; // Mouse is on tab bar, let egui handle it
        }

        // --- 1. Shader Uniform Updates ---
        // Update current mouse position for custom shaders (iMouse.xy)
        if let Some(ref mut renderer) = self.renderer {
            renderer.set_shader_mouse_position(position.0 as f32, position.1 as f32);
        }

        // --- 2. URL Hover Detection ---
        // Identify if mouse is over a clickable link and update window UI (cursor/title)
        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1) {
            // Get scroll offset and terminal title from active tab (clone to avoid borrow conflicts)
            let (scroll_offset, terminal_title, detected_urls, hovered_url) = self
                .tab_manager
                .active_tab()
                .map(|t| {
                    (
                        t.active_scroll_state().offset,
                        t.active_cache().terminal_title.clone(),
                        t.active_mouse().detected_urls.clone(),
                        t.active_mouse().hovered_url.clone(),
                    )
                })
                .unwrap_or((0, String::new(), Vec::new(), None));

            let adjusted_row = row + scroll_offset;
            let url_opt = url_detection::find_url_at_position(&detected_urls, col, adjusted_row);

            if let Some(url) = url_opt {
                // Hovering over a new/different URL
                if hovered_url.as_ref() != Some(&url.url) {
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.active_mouse_mut().hovered_url = Some(url.url.clone());
                    }
                    if let Some(window) = &self.window {
                        // Visual feedback: hand pointer + URL tooltip in title
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                        let base_title = self.format_title(&self.config.window_title);
                        let tooltip_title = format!("{} - {}", base_title, url.url);
                        window.set_title(&tooltip_title);
                    }
                }
            } else if hovered_url.is_some() {
                // Mouse left a URL area: restore default state
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_mouse_mut().hovered_url = None;
                }
                if let Some(window) = &self.window {
                    window.set_cursor(winit::window::CursorIcon::Text);
                    // Restore terminal-controlled title or config default
                    if self.config.allow_title_change && !terminal_title.is_empty() {
                        window.set_title(&self.format_title(&terminal_title));
                    } else {
                        window.set_title(&self.format_title(&self.config.window_title));
                    }
                }
            }
        }

        // --- 3. Mouse Motion Reporting ---
        // Forward motion events to PTY if requested by terminal app (e.g., mouse tracking in vim)
        // In split pane mode, only forward when mouse is inside the focused pane's bounds.
        // Clicks outside the focused pane (on dividers or other panes) must fall through
        // to divider drag and hover handlers.
        // Shift held bypasses mouse tracking so the user can drag-select even inside
        // apps like `less` that enable mouse tracking on the alternate screen.
        let shift_held = self.input_handler.modifiers.state().shift_key();
        if let Some(tab) = self.tab_manager.active_tab() {
            let resolved = if let Some(ref pm) = tab.pane_manager
                && let Some(focused_pane) = pm.focused_pane()
            {
                // Split pane mode: only report motion inside the focused pane
                let btn = tab.active_mouse().button_pressed;
                self.pixel_to_pane_cell(position.0, position.1, &focused_pane.bounds)
                    .map(|(col, row)| (Arc::clone(&focused_pane.terminal), col, row, btn))
            } else {
                // Single pane mode: use tab's terminal with global coordinates
                let btn = tab.active_mouse().button_pressed;
                self.pixel_to_cell(position.0, position.1)
                    .map(|(col, row)| (Arc::clone(&tab.terminal), col, row, btn))
            };

            if let Some((terminal_arc, col, row, button_pressed)) = resolved {
                // try_lock: intentional — should_report_mouse_motion query from mouse-move
                // handler in the sync event loop. On miss: assumes no tracking (false) so
                // the motion event is skipped this frame. High-frequency; acceptable loss.
                let should_report = terminal_arc
                    .try_write()
                    .ok()
                    .is_some_and(|term| term.should_report_mouse_motion(button_pressed));

                // try_lock: intentional — second lock attempt to encode/write the event.
                // On miss: mouse motion encoding is skipped this frame. Same rationale.
                if should_report
                    && !shift_held
                    && let Ok(term) = terminal_arc.try_write()
                {
                    // Encode button+motion (button 32 marker)
                    let button = if button_pressed {
                        32 // Motion while button pressed
                    } else {
                        35 // Motion without button pressed
                    };

                    let encoded = term.encode_mouse_event(button, col, row, true, 0);
                    if !encoded.is_empty() {
                        let terminal_clone = Arc::clone(&terminal_arc);
                        let runtime = Arc::clone(&self.runtime);
                        runtime.spawn(async move {
                            let t = terminal_clone.write().await;
                            let _ = t.write(&encoded);
                        });
                    }
                    return; // Exit early: terminal app is handling mouse motion
                }
            }
        }

        // --- 4. Scrollbar Dragging ---
        let is_dragging = self
            .tab_manager
            .active_tab()
            .map(|t| t.active_scroll_state().dragging)
            .unwrap_or(false);

        if is_dragging {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_scroll_state_mut().last_activity = std::time::Instant::now();
            }
            self.drag_scrollbar_to(position.1 as f32);
            return; // Exit early: scrollbar dragging takes precedence over selection
        }

        // --- 4b. Divider Dragging ---
        // Handle pane divider drag resize
        let divider_dragging = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.active_mouse().dragging_divider);

        if let Some(divider_index) = divider_dragging {
            // Actively dragging a divider
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.drag_divider(divider_index, position.0 as f32, position.1 as f32);
            }
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return; // Exit early: divider dragging takes precedence
        }

        // --- 4c. Divider Hover Detection ---
        // Check if mouse is hovering over a pane divider
        let is_on_divider = self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.is_on_divider(position.0 as f32, position.1 as f32));

        let was_hovering = self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.active_mouse().divider_hover);

        if is_on_divider != was_hovering {
            // Hover state changed
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                let new_idx = if is_on_divider {
                    tab.find_divider_at(position.0 as f32, position.1 as f32)
                } else {
                    None
                };
                tab.active_mouse_mut().divider_hover = is_on_divider;
                tab.active_mouse_mut().hovered_divider_index = new_idx;
            }
            if let Some(window) = &self.window {
                if is_on_divider {
                    // Get divider orientation to set correct cursor
                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Some(divider_idx) =
                            tab.find_divider_at(position.0 as f32, position.1 as f32)
                        && let Some(divider) = tab.get_divider(divider_idx)
                    {
                        let cursor = if divider.is_horizontal {
                            winit::window::CursorIcon::RowResize
                        } else {
                            winit::window::CursorIcon::ColResize
                        };
                        window.set_cursor(cursor);
                    }
                } else {
                    window.set_cursor(winit::window::CursorIcon::Text);
                }
            }
        }

        // --- 5. Drag Selection Logic ---
        // Perform local text selection if mouse tracking is NOT active
        // try_lock: intentional — alt-screen query during mouse-move in sync event loop.
        // On miss: is_some_and returns false, treating as not on alt screen — local
        // selection will proceed even on alt screen for this one motion event. Benign.
        let alt_screen_active = self.tab_manager.active_tab().is_some_and(|tab| {
            tab.terminal
                .try_write()
                .ok()
                .is_some_and(|term| term.is_alt_screen_active())
        });

        // Get mouse state for selection logic (per-pane in split mode)
        let (
            button_pressed,
            click_count,
            is_selecting,
            click_position,
            click_pixel_position,
            selection_mode,
        ) = self
            .tab_manager
            .active_tab()
            .map(|t| {
                let sm = t.selection_mouse();
                (
                    t.active_mouse().button_pressed,
                    sm.click_count,
                    sm.is_selecting,
                    sm.click_position,
                    sm.click_pixel_position,
                    sm.selection.as_ref().map(|s| s.mode),
                )
            })
            .unwrap_or((false, 0, false, None, None, None));

        // Use pane-relative coordinates in split-pane mode so drag selection
        // coordinates match the focused pane's terminal buffer.
        if let Some((col, row)) = self.pixel_to_selection_cell(position.0, position.1)
            && button_pressed
            && (!alt_screen_active || shift_held)
        {
            // Minimum pixel distance before a click becomes a drag selection.
            // Prevents accidental micro-drags (e.g. trackpad taps) from creating
            // tiny selections that overwrite clipboard content (including images).
            // Slightly larger dead zone to avoid accidental selection starts from
            // trackpad jitter / tap-to-click movement noise.
            let past_drag_threshold = click_pixel_position.is_some_and(|(cx, cy)| {
                let dx = position.0 - cx;
                let dy = position.1 - cy;
                (dx * dx + dy * dy) >= DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX
            });

            if click_count == 1
                && !is_selecting
                && let Some(click_pos) = click_position
                && click_pos != (col, row)
                && past_drag_threshold
            {
                // Initial drag move: Start selection if we've moved past the pixel drag threshold
                // Option+Cmd (Alt+Super) triggers Rectangular/Block selection mode (matches iTerm2)
                // Option alone is for cursor positioning, not selection
                let mode = if self.input_handler.modifiers.state().alt_key()
                    && self.input_handler.modifiers.state().super_key()
                {
                    SelectionMode::Rectangular
                } else {
                    SelectionMode::Normal
                };

                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    let sm = tab.selection_mouse_mut();
                    sm.is_selecting = true;
                    sm.selection = Some(Selection::new(click_pos, (col, row), mode));
                }
                self.request_redraw();
            } else if is_selecting && let Some(mode) = selection_mode {
                // Dragging in progress: Update selection endpoints
                if mode == SelectionMode::Line {
                    // Triple-click mode: Selection always covers whole lines
                    self.extend_line_selection(row);
                    self.request_redraw();
                } else {
                    // Normal/Rectangular mode: update end cell
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && let Some(ref mut sel) = tab.selection_mouse_mut().selection
                    {
                        sel.end = (col, row);
                    }
                    self.request_redraw();
                }
            }
        }
    }
}
