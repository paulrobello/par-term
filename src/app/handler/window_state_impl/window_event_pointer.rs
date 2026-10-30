//! Pointer arms of `WindowState::handle_window_event` (free functions per
//! the ARC-001 impl-block budget).

use crate::app::window_state::WindowState;

/// Route the wheel event to tab scrolling when over the tab bar, else to the
/// terminal (skipped while egui UI is visible or using the pointer).
pub(super) fn mouse_wheel(
    ws: &mut WindowState,
    delta: winit::event::MouseScrollDelta,
    any_ui_visible: bool,
) {
    // Check if mouse is over the tab bar — convert vertical wheel
    // to horizontal tab scrolling when the bar overflows.
    let mouse_position = ws
        .tab_manager
        .active_tab()
        .map(|t| t.active_mouse().position)
        .unwrap_or((0.0, 0.0));
    let tab_count = ws.tab_manager.visible_tab_count();

    if ws.is_mouse_in_tab_bar(mouse_position)
        && ws.tab_bar_ui.handle_mouse_wheel(
            &delta,
            ws.config.load().tab_colors.tab_min_width,
            tab_count,
        )
    {
        ws.request_redraw();
    } else if !any_ui_visible && !ws.is_egui_using_pointer() {
        // Skip terminal handling if egui UI is visible or using the pointer
        // Note: any_ui_visible check is needed because is_egui_using_pointer()
        // returns false before egui is initialized (e.g., at startup when
        // shader_install_ui is shown before first render)
        ws.handle_mouse_wheel(delta);
    }
}

/// Dispatch the mouse button event: eat the first focus-click press (a
/// zero-char tmux selection would clear the clipboard image), else run the
/// normal press/release path with UI pointer-consumption tracking.
pub(super) fn mouse_input(
    ws: &mut WindowState,
    button: winit::event::MouseButton,
    state: winit::event::ElementState,
    any_ui_visible: bool,
) {
    use winit::event::ElementState;
    use winit::event::MouseButton;

    // Eat the first mouse press that brings the window into focus.
    // Without this, the click is forwarded to the PTY where mouse-aware
    // apps (tmux with `mouse on`) trigger a zero-char selection that
    // clears the system clipboard — destroying any clipboard image.
    //
    // Some platforms deliver `Focused(true)` before the mouse press, others
    // can deliver it after the press/release. Treat a press that arrives while
    // we're still unfocused as a focus-click too, then avoid double-arming the
    // later focus event path.
    let is_focus_click_press = state == ElementState::Pressed
        && (ws.focus_state.focus_click_pending || !ws.focus_state.is_focused);
    if is_focus_click_press {
        ws.focus_state.focus_click_pending = false;
        if !ws.focus_state.is_focused {
            ws.focus_state.focus_click_suppressed_while_unfocused_at =
                Some(std::time::Instant::now());
        }
        // If the focus click landed in the tab bar, let it through so the
        // tab switch registers in egui. Only suppress clicks in the terminal
        // area to prevent PTY mouse-tracking apps from seeing the focus click.
        let mouse_position = ws
            .tab_manager
            .active_tab()
            .map(|t| t.active_mouse().position)
            .unwrap_or((0.0, 0.0));
        if ws.is_mouse_in_tab_bar(mouse_position) {
            // Direct hit-test against the tab rects cached from the last render
            // frame.  egui's clicked_by() can miss focus-clicks because pointer
            // state may be stale when the window was unfocused.  Storing the
            // target here lets post_render apply the switch as a fallback.
            let scale_factor = ws.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0) as f32;
            let logical_pos = egui::pos2(
                mouse_position.0 as f32 / scale_factor,
                mouse_position.1 as f32 / scale_factor,
            );
            ws.focus_state.pending_focus_tab_switch = ws.tab_bar_ui.tab_at_logical_pos(logical_pos);

            // Don't suppress — egui needs both press and release to fire clicked_by()
            ws.focus_state.ui_consumed_mouse_press = false;
            ws.begin_clipboard_image_click_guard(button, state);
            ws.handle_mouse_button(button, state);
            ws.finish_clipboard_image_click_guard(button, state);
        } else {
            ws.focus_state.ui_consumed_mouse_press = true; // Also suppress the release
            ws.request_redraw();
        }
    } else {
        // Track UI mouse consumption to prevent release events bleeding through
        // when UI closes during a click (e.g., drawer toggle)
        let ui_wants_pointer = any_ui_visible || ws.is_egui_using_pointer();

        if state == ElementState::Pressed {
            if ui_wants_pointer {
                ws.focus_state.ui_consumed_mouse_press = true;
                ws.request_redraw();
            } else {
                ws.focus_state.ui_consumed_mouse_press = false;
                ws.begin_clipboard_image_click_guard(button, state);
                ws.handle_mouse_button(button, state);
                ws.finish_clipboard_image_click_guard(button, state);
            }
        } else {
            // Release: block if we consumed the press OR if UI wants pointer
            if ws.focus_state.ui_consumed_mouse_press || ui_wants_pointer {
                ws.focus_state.ui_consumed_mouse_press = false;

                // Clear terminal mouse state when the release is consumed
                // here instead of in handle_mouse_button(). Without this,
                // a press that set button_pressed=true in the terminal
                // followed by a release consumed by egui (e.g., pointer
                // moved into the tab bar) leaves button_pressed stuck as
                // true. The next mouse-move into the terminal then starts
                // an accidental drag-selection.
                if button == MouseButton::Left
                    && let Some(tab) = ws.tab_manager.active_tab_mut()
                {
                    tab.active_mouse_mut().button_pressed = false;
                    tab.selection_mouse_mut().is_selecting = false;
                }

                ws.request_redraw();
            } else {
                ws.begin_clipboard_image_click_guard(button, state);
                ws.handle_mouse_button(button, state);
                ws.finish_clipboard_image_click_guard(button, state);
            }
        }
    }
}

/// Always store the newest mouse position (hit-testing reads it even when
/// egui claims the pointer), then route the move to egui or the terminal.
pub(super) fn cursor_moved(
    ws: &mut WindowState,
    position: winit::dpi::PhysicalPosition<f64>,
    any_ui_visible: bool,
) {
    // Always update the stored mouse position so that hit-testing
    // (is_mouse_in_tab_bar, pixel_to_cell, etc.) uses the latest
    // coordinates even when egui claims the pointer and
    // handle_mouse_move() is skipped.  Without this, a stale position
    // can cause a subsequent MouseInput press to bypass the tab-bar
    // guard and leak through to tmux mouse tracking.
    if let Some(tab) = ws.tab_manager.active_tab_mut() {
        tab.active_mouse_mut().position = (position.x, position.y);
    }

    // Skip terminal handling if egui UI is visible or using the pointer
    if any_ui_visible || ws.is_egui_using_pointer() {
        // Request redraw so egui can update hover states
        ws.request_redraw();
    } else {
        ws.handle_mouse_move((position.x, position.y));
    }
}
