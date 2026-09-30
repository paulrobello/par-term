//! Live routing: the real `route_overlay_key` + `apply_overlay_route` path
//! on a `WindowState` — the same two calls `handle_window_event` and the
//! `--ui-test` chord injector make. The pure decision table is tested in
//! `routing::tests`; these prove the effects land on real overlay state.

use super::OverlayId;
use super::routing::{KeyFacts, KeyRoute};
use super::stack_tests::{open, window};
use crate::app::window_state::WindowState;

fn press(ws: &mut WindowState, facts: KeyFacts) -> KeyRoute {
    let route = ws.route_overlay_key(&facts);
    ws.apply_overlay_route(&route);
    route
}

fn chord(action: &str) -> KeyFacts {
    KeyFacts {
        is_escape: false,
        bound_action: Some(action.to_string()),
        is_command_chord: true,
    }
}

fn escape() -> KeyFacts {
    KeyFacts {
        is_escape: true,
        ..KeyFacts::default()
    }
}

#[test]
fn every_toggle_action_names_a_dispatchable_action() {
    let dispatchable: Vec<&str> = crate::app::input_events::keybinding_actions::ACTION_HANDLERS
        .iter()
        .map(|(id, _)| *id)
        .chain(
            crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS
                .iter()
                .map(|(id, _)| *id),
        )
        .collect();
    for &id in OverlayId::ALL {
        if let Some(action) = id.toggle_action() {
            assert!(
                dispatchable.contains(&action),
                "{id:?} names toggle action {action}, which nothing dispatches"
            );
            assert_eq!(OverlayId::opened_by_action(action), Some(id));
        }
    }
}

#[test]
fn live_toggle_chord_closes_its_own_popup() {
    let mut ws = window();
    open(&mut ws, OverlayId::CommandHistory);
    let route = press(&mut ws, chord("toggle_command_history"));
    assert_eq!(route, KeyRoute::CloseTop(OverlayId::CommandHistory));
    assert!(!ws.overlay_is_open(OverlayId::CommandHistory));
}

#[test]
fn live_escape_closes_only_the_top_overlay() {
    let mut ws = window();
    open(&mut ws, OverlayId::TreePicker);
    open(&mut ws, OverlayId::CommandPalette);
    press(&mut ws, escape());
    assert!(!ws.overlay_is_open(OverlayId::CommandPalette));
    assert!(
        ws.overlay_is_open(OverlayId::TreePicker),
        "the overlay beneath stays"
    );
    press(&mut ws, escape());
    assert!(!ws.overlay_is_open(OverlayId::TreePicker));
}

#[test]
fn live_modal_consumes_other_overlays_chords() {
    // F1 must not dismiss the quit dialog, and must not open help over it.
    let mut ws = window();
    open(&mut ws, OverlayId::QuitConfirm);
    let route = press(&mut ws, chord("toggle_help"));
    assert_eq!(route, KeyRoute::Consume);
    assert!(!route.continues_to_key_dispatch(), "never reaches the PTY");
    assert!(ws.overlay_is_open(OverlayId::QuitConfirm));
    assert!(!ws.overlay_is_open(OverlayId::Help));
}

#[test]
fn live_escape_cancels_the_quit_dialog() {
    let mut ws = window();
    open(&mut ws, OverlayId::QuitConfirm);
    press(&mut ws, escape());
    assert!(!ws.overlay_is_open(OverlayId::QuitConfirm));
}

#[test]
fn live_escape_does_not_answer_a_trigger_prompt() {
    let mut ws = window();
    open(&mut ws, OverlayId::TriggerConfirm);
    assert_eq!(press(&mut ws, escape()), KeyRoute::Consume);
    assert!(
        ws.overlay_is_open(OverlayId::TriggerConfirm),
        "the pending trigger action must not be silently denied"
    );
}

#[test]
fn live_another_popups_chord_replaces_the_top_popup() {
    let mut ws = window();
    open(&mut ws, OverlayId::CommandHistory);
    press(&mut ws, chord("toggle_tree_picker"));
    assert!(!ws.overlay_is_open(OverlayId::CommandHistory));
    assert!(ws.overlay_is_open(OverlayId::TreePicker));
}

#[test]
fn live_unused_keys_under_a_popup_never_reach_key_dispatch() {
    let mut ws = window();
    open(&mut ws, OverlayId::Search);
    for facts in [
        KeyFacts::default(),
        chord("new_tab"),
        KeyFacts {
            bound_action: None,
            is_command_chord: true,
            is_escape: false,
        },
    ] {
        let route = ws.route_overlay_key(&facts);
        assert!(!route.continues_to_key_dispatch(), "{facts:?} -> {route:?}");
    }
}

#[test]
fn live_no_overlay_leaves_keys_to_the_terminal() {
    let ws = window();
    assert_eq!(ws.route_overlay_key(&escape()), KeyRoute::Terminal);
    assert_eq!(ws.route_overlay_key(&chord("new_tab")), KeyRoute::Terminal);
}

#[test]
fn live_the_most_recently_opened_popup_is_on_top() {
    // Tree picker over the palette this time: Escape closes the picker.
    let mut ws = window();
    open(&mut ws, OverlayId::CommandPalette);
    open(&mut ws, OverlayId::TreePicker);
    press(&mut ws, escape());
    assert!(!ws.overlay_is_open(OverlayId::TreePicker));
    assert!(ws.overlay_is_open(OverlayId::CommandPalette));
}

#[test]
fn live_escape_cancels_the_demote_pick() {
    // The demote pick guards the terminal, so its key handler behind the
    // guard never runs: Escape must be resolved by the stack itself.
    let mut ws = window();
    open(&mut ws, OverlayId::DemotePick);
    press(&mut ws, escape());
    assert!(
        !ws.overlay_is_open(OverlayId::DemotePick),
        "Escape must cancel the demote pick"
    );
}

#[test]
fn no_guarded_mode_delegates_escape_to_a_handler_behind_the_guard() {
    // A guarded Mode's key handler sits in handle_key_event, behind the
    // modal guard; delegating Escape to it is dead code. Panels and
    // popups that delegate handle Escape in egui, which still sees it.
    use super::OverlayKind;
    use super::routing::{EscapeBehavior, escape_behavior};
    for &id in OverlayId::ALL {
        if id.guards_terminal() && id.kind() == OverlayKind::Mode {
            assert_ne!(
                escape_behavior(id),
                EscapeBehavior::Delegate,
                "{id:?}: guarded mode delegating Escape to an unreachable handler"
            );
        }
    }
}
