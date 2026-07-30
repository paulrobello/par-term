//! Delivery channel for menu actions raised inside the process.
//!
//! The native menu delivers activations through muda's process-global
//! [`muda::MenuEvent`] channel, which `MenuManager::poll_events` drains once per
//! event-loop iteration. The in-app egui menu ([`super::egui_menu`]) is drawn
//! deep inside a window's egui pass, far from the [`super::MenuManager`] that
//! owns the dispatch table, so it needs the same shape of channel — this module
//! is it. Both queues are drained together by
//! `WindowManager::process_menu_events`, so an action behaves identically no
//! matter which menu raised it.

use super::MenuAction;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Actions raised by the in-app menu, waiting to be dispatched.
static PENDING: Mutex<Vec<MenuAction>> = Mutex::new(Vec::new());

/// Set when something outside the egui pass asks the menu to open or close.
static TOGGLE_REQUEST: AtomicBool = AtomicBool::new(false);

/// Queue an action raised by the in-app menu.
pub fn dispatch(action: MenuAction) {
    match PENDING.lock() {
        Ok(mut pending) => pending.push(action),
        Err(poisoned) => poisoned.into_inner().push(action),
    }
}

/// Take every queued action, leaving the queue empty.
pub fn drain_pending_actions() -> Vec<MenuAction> {
    match PENDING.lock() {
        Ok(mut pending) => std::mem::take(&mut *pending),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    }
}

/// Ask the in-app menu to open (or close, if already open).
///
/// The request is applied by the next egui pass that draws the menu, so this is
/// safe to call from the input handler — for example from a `toggle_menu`
/// keybinding action.
pub fn request_toggle() {
    TOGGLE_REQUEST.store(true, Ordering::Relaxed);
}

/// Consume a pending toggle request, if any.
pub fn take_toggle_request() -> bool {
    TOGGLE_REQUEST.swap(false, Ordering::Relaxed)
}

/// Serialises the tests that exercise these process-global queues.
///
/// Without it, `cargo test`'s parallelism lets one test drain the other's
/// action or toggle request.
#[cfg(test)]
pub(super) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_and_toggle_round_trip() {
        let _guard = TEST_LOCK.lock();
        let _ = drain_pending_actions();
        let _ = take_toggle_request();

        assert!(drain_pending_actions().is_empty());
        dispatch(MenuAction::Quit);
        dispatch(MenuAction::NewWindow);
        assert_eq!(
            drain_pending_actions(),
            vec![MenuAction::Quit, MenuAction::NewWindow]
        );
        assert!(drain_pending_actions().is_empty());

        assert!(!take_toggle_request());
        request_toggle();
        assert!(take_toggle_request());
        assert!(!take_toggle_request());
    }
}
