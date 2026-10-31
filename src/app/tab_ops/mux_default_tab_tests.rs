//! Tests for the `use_mux_by_default` new-tab routing rule.

use super::WindowState;

/// The flip on routes an unattached window's new tab through mux.
#[test]
fn the_flip_on_routes_an_unattached_new_tab_through_mux() {
    assert!(WindowState::new_tab_wants_mux(true, false, false));
}

/// The flip off keeps the classic local tab.
#[test]
fn the_flip_off_keeps_the_local_tab() {
    assert!(!WindowState::new_tab_wants_mux(false, false, false));
}

/// An attached window's tabs are already daemon windows; the rule must
/// not reroute them.
#[test]
fn an_attached_window_is_already_routed() {
    assert!(!WindowState::new_tab_wants_mux(true, true, false));
}

/// A gateway window owns a multiplexer connection of its own; the
/// default flip must not stack a par-mux attach on top.
#[test]
fn a_gateway_window_is_not_rerouted() {
    assert!(!WindowState::new_tab_wants_mux(true, false, true));
}
