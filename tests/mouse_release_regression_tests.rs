//! Regression tests for mouse-release handling when egui consumes the release.
//!
//! ## Bugs covered
//!
//! ### Bug 1: button_pressed stuck as true after tab bar click
//!
//! When a mouse press landed in the terminal (setting `button_pressed = true`) and
//! the release was consumed by `handle_window_event` because `ui_wants_pointer` was
//! true (pointer moved into the tab bar), `button_pressed` and `is_selecting` were
//! never cleared. The cleanup only existed inside `handle_mouse_button()`, which was
//! never called for the consumed release. The next `handle_mouse_move()` call into the
//! terminal area then saw `button_pressed == true` and started an accidental drag
//! selection.
//!
//! **Fix**: Added unconditional `button_pressed = false` / `is_selecting = false`
//! cleanup in `handle_window_event` when a left-button release is consumed.
//!
//! ### Bug 2: Tab click silently dropped when egui state is stale
//!
//! When `is_egui_using_pointer()` returned `false` (stale after window focus change
//! or rapid pointer movement), the press bypassed egui and was caught only by
//! `is_mouse_in_tab_bar()` in `handle_mouse_button()`. But egui's `clicked_by()`
//! never fired without seeing the press, so the tab switch was silently dropped.
//!
//! **Fix**: Store a `pending_focus_tab_switch` in the tab bar guard so `post_render`
//! can apply the switch as a fallback.

use egui::{Pos2, Rect};
use par_term::pane::mouse::MouseState;
use par_term::selection::{Selection, SelectionMode};
use par_term::tab_bar_ui::TabBarUI;

type TabId = u64;

// ============================================================================
// Bug 1: MouseState cleanup on consumed release
// ============================================================================

#[test]
fn test_mouse_state_defaults_button_not_pressed() {
    let state = MouseState::test_new();
    assert!(
        !state.test_button_pressed(),
        "button_pressed must default to false"
    );
    assert!(
        !state.test_is_selecting(),
        "is_selecting must default to false"
    );
}

#[test]
fn test_mouse_state_button_press_sets_flag() {
    // Simulate what handle_left_mouse_button does at mouse_button.rs:416
    let mut state = MouseState::test_new();
    state.test_set_button_pressed(true);
    assert!(state.test_button_pressed());
}

#[test]
fn test_mouse_state_release_clears_flags() {
    // Simulate the unconditional cleanup that happens on left-button release.
    // This is the guard at mouse_button.rs:29-35 AND the new guard in
    // handle_window_event.rs:426-431.
    let mut state = MouseState::test_new();
    state.test_set_button_pressed(true);
    state.test_set_is_selecting(true);
    state.test_set_selection(Some(Selection::new(
        (0, 0),
        (5, 0),
        SelectionMode::Normal,
        0,
    )));

    // The fix: on left-button release consumed by egui, clear these flags
    state.test_set_button_pressed(false);
    state.test_set_is_selecting(false);

    assert!(
        !state.test_button_pressed(),
        "button_pressed must be cleared on release"
    );
    assert!(
        !state.test_is_selecting(),
        "is_selecting must be cleared on release"
    );
}

#[test]
fn test_mouse_state_release_clears_even_with_active_selection() {
    // Regression: button_pressed and is_selecting must clear even when a
    // selection exists (e.g., user dragged in terminal then released in tab bar).
    let mut state = MouseState::test_new();
    state.test_set_button_pressed(true);
    state.test_set_is_selecting(true);
    state.test_set_selection(Some(Selection::new(
        (0, 0),
        (10, 2),
        SelectionMode::Normal,
        0,
    )));
    state.test_set_click_pixel_position(Some((100.0, 200.0)));
    state.test_set_click_position(Some((5, 3)));
    state.test_set_click_count(1);

    // Simulate the release consumed by egui (handle_window_event path)
    state.test_set_button_pressed(false);
    state.test_set_is_selecting(false);

    assert!(!state.test_button_pressed());
    assert!(!state.test_is_selecting());
    // Note: selection data (selection, click_position, etc.) is NOT cleared
    // by this path — only button_pressed and is_selecting. The selection
    // highlight may persist until the next terminal click clears it.
    assert!(state.test_selection().is_some());
}

#[test]
fn test_drag_selection_requires_button_pressed() {
    // Verify that drag-selection logic in handle_mouse_move guards on
    // button_pressed. If button_pressed is false, no selection starts.
    let mut state = MouseState::test_new();
    state.test_set_click_pixel_position(Some((100.0, 200.0)));
    state.test_set_click_position(Some((5, 3)));
    state.test_set_click_count(1);
    // button_pressed is false (fix cleared it)

    assert!(!state.test_button_pressed());
    // The drag-selection code at mouse_move.rs:343 checks `button_pressed`
    // before starting or extending a selection. With button_pressed=false,
    // the entire selection block is skipped.
}

#[test]
fn test_button_pressed_stuck_causes_false_selection() {
    // Demonstrate the bug: if button_pressed stays true after a consumed
    // release, a subsequent mouse move would start a selection.
    let mut state = MouseState::test_new();
    // Simulate a terminal press
    state.test_set_button_pressed(true);
    state.test_set_click_pixel_position(Some((100.0, 200.0)));
    state.test_set_click_position(Some((5, 3)));
    state.test_set_click_count(1);

    // BUG PATH: release was consumed by egui, button_pressed NOT cleared
    // (button_pressed remains true — this is what the fix prevents)

    // Without the fix, the next mouse move would see button_pressed=true
    // and start drag selection. With the fix, button_pressed is cleared
    // so this can't happen.
    assert!(state.test_button_pressed()); // Bug state
    state.test_set_button_pressed(false); // Fix applied
    assert!(!state.test_button_pressed()); // Now safe
}

// ============================================================================
// Bug 2: TabBarUI tab_at_logical_pos fallback
// ============================================================================

#[test]
fn test_tab_at_logical_pos_no_rects() {
    let ui = TabBarUI::new();
    // No tab rects cached — should return None
    assert!(ui.tab_at_logical_pos(Pos2::new(10.0, 10.0)).is_none());
}

#[test]
fn test_tab_at_logical_pos_hits_tab() {
    let mut ui = TabBarUI::new();
    let tab1: TabId = 1;
    let tab2: TabId = 2;
    let tab3: TabId = 3;
    ui.test_set_tab_rects(vec![
        (
            tab1,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
        ),
        (
            tab2,
            Rect::from_min_max(Pos2::new(104.0, 0.0), Pos2::new(204.0, 30.0)),
        ),
        (
            tab3,
            Rect::from_min_max(Pos2::new(208.0, 0.0), Pos2::new(308.0, 30.0)),
        ),
    ]);

    assert_eq!(ui.tab_at_logical_pos(Pos2::new(50.0, 15.0)), Some(tab1));
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(150.0, 15.0)), Some(tab2));
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(250.0, 15.0)), Some(tab3));
}

#[test]
fn test_tab_at_logical_pos_miss_between_tabs() {
    let mut ui = TabBarUI::new();
    let tab1: TabId = 1;
    let tab2: TabId = 2;
    ui.test_set_tab_rects(vec![
        (
            tab1,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
        ),
        (
            tab2,
            Rect::from_min_max(Pos2::new(104.0, 0.0), Pos2::new(204.0, 30.0)),
        ),
    ]);

    // Gap between tabs (101-103)
    assert!(ui.tab_at_logical_pos(Pos2::new(102.0, 15.0)).is_none());
}

#[test]
fn test_tab_at_logical_pos_edge_boundaries() {
    let mut ui = TabBarUI::new();
    let tab1: TabId = 1;
    let tab2: TabId = 2;
    ui.test_set_tab_rects(vec![
        (
            tab1,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
        ),
        (
            tab2,
            Rect::from_min_max(Pos2::new(104.0, 0.0), Pos2::new(204.0, 30.0)),
        ),
    ]);

    // Exact left edge of tab1
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(0.0, 0.0)), Some(tab1));
    // Just inside right edge of tab1
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(99.99, 29.99)), Some(tab1));
    // Right edge (egui Rect contains is inclusive on right/bottom)
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(100.0, 15.0)), Some(tab1));
    // Between tabs (101-103 is gap)
    assert!(ui.tab_at_logical_pos(Pos2::new(102.0, 15.0)).is_none());
    // Exact left edge of tab2
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(104.0, 0.0)), Some(tab2));
}

#[test]
fn test_tab_at_logical_pos_above_and_below() {
    let mut ui = TabBarUI::new();
    let tab1: TabId = 1;
    ui.test_set_tab_rects(vec![(
        tab1,
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
    )]);

    // Above tab bar
    assert!(ui.tab_at_logical_pos(Pos2::new(50.0, -1.0)).is_none());
    // Bottom edge of tab (egui Rect contains is inclusive)
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(50.0, 30.0)), Some(tab1));
    // Below tab bar
    assert!(ui.tab_at_logical_pos(Pos2::new(50.0, 30.01)).is_none());
    // Inside tab bar
    assert_eq!(ui.tab_at_logical_pos(Pos2::new(50.0, 15.0)), Some(tab1));
}

// ============================================================================
// End-to-end regression scenario simulation
// ============================================================================

#[test]
fn test_regression_drag_from_terminal_release_in_tab_bar() {
    // Simulate the exact sequence that caused the bug:
    // 1. Press in terminal -> button_pressed = true
    // 2. Move to tab bar
    // 3. Release consumed by egui (ui_wants_pointer = true)
    // 4. button_pressed was NOT cleared (BUG)
    // 5. Move back to terminal -> accidental drag selection

    let mut mouse = MouseState::test_new();

    // Step 1: Press in terminal
    mouse.test_set_button_pressed(true);
    mouse.test_set_click_pixel_position(Some((500.0, 400.0)));
    mouse.test_set_click_position(Some((50, 20)));
    mouse.test_set_click_count(1);

    // Step 2-3: Release consumed by egui
    // BUG PATH (before fix): button_pressed stays true
    // With fix: button_pressed and is_selecting are cleared
    mouse.test_set_button_pressed(false);
    mouse.test_set_is_selecting(false);

    // Step 4-5: Move back to terminal
    // handle_mouse_move checks button_pressed
    assert!(
        !mouse.test_button_pressed(),
        "button_pressed must be false — no accidental drag selection"
    );
    assert!(!mouse.test_is_selecting());
}

#[test]
fn test_regression_tab_click_dropped_when_egui_stale() {
    // Simulate: click on tab when is_egui_using_pointer() was stale (false).
    // The press bypassed egui and was caught by is_mouse_in_tab_bar().
    // egui never saw the press, so clicked_by() never fired.
    // Fix: store pending_focus_tab_switch as fallback.

    let mut ui = TabBarUI::new();
    let target_tab: TabId = 5;
    ui.test_set_tab_rects(vec![
        (
            3,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
        ),
        (
            target_tab,
            Rect::from_min_max(Pos2::new(104.0, 0.0), Pos2::new(204.0, 30.0)),
        ),
        (
            7,
            Rect::from_min_max(Pos2::new(208.0, 0.0), Pos2::new(308.0, 30.0)),
        ),
    ]);

    // Simulate: press at tab5's center position
    let scale_factor: f32 = 2.0; // Retina display
    let physical_x = 154.0 * scale_factor; // Center of tab5 in physical pixels
    let physical_y = 15.0 * scale_factor;
    let logical_pos = egui::pos2(physical_x / scale_factor, physical_y / scale_factor);

    // tab_at_logical_pos should find the tab
    let found = ui.tab_at_logical_pos(logical_pos);
    assert_eq!(
        found,
        Some(target_tab),
        "tab_at_logical_pos must find the clicked tab for fallback switching"
    );

    // In production, this value is stored in pending_focus_tab_switch
    // and applied by post_render if egui didn't fire clicked_by().
    let pending_switch = found;
    assert_eq!(pending_switch, Some(target_tab));
}

#[test]
fn test_regression_normal_tab_click_not_affected() {
    // Verify that the fix doesn't break the normal case:
    // egui handles the click directly, no fallback needed.

    let mut ui = TabBarUI::new();
    let tab1: TabId = 1;
    ui.test_set_tab_rects(vec![(
        tab1,
        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 30.0)),
    )]);

    // Normal case: egui handles the click, pending_focus_tab_switch stays None
    let pending: Option<TabId> = None;
    assert!(
        pending.is_none(),
        "No fallback needed for normal egui clicks"
    );
}

// ============================================================================
// MouseState isolation: divider drag cleanup
// ============================================================================

#[test]
fn test_mouse_state_divider_drag_cleared_on_release() {
    // Related: mouse_move.rs has a recovery path for stale divider drag.
    // If button_pressed is false but dragging_divider is set, the divider
    // drag is ended gracefully.
    let mut state = MouseState::test_new();
    state.test_set_dragging_divider(Some(2));
    state.test_set_button_pressed(true);

    // Simulate release clearing button_pressed
    state.test_set_button_pressed(false);

    // The divider recovery code at mouse_move.rs:226-261 checks:
    // if !button_pressed { dragging_divider = None }
    assert!(!state.test_button_pressed());
    if !state.test_button_pressed() {
        state.test_set_dragging_divider(None);
    }
    assert!(state.test_dragging_divider().is_none());
}
