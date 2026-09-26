//! Recovery from global display-configuration changes.
//!
//! macOS posts `NSApplicationDidChangeScreenParametersNotification` whenever
//! the display topology changes (monitor plugged in or unplugged, resolution
//! or color space changed). Winit frequently does **not** translate this into
//! per-window `ScaleFactorChanged`/`Moved`/`Resized` events when the window's
//! DPI and physical size are unchanged — yet the window may now live on a
//! different GPU or a differently-configured Metal drawable chain, leaving the
//! terminal strobing until something forces a surface rebuild.
//!
//! This module closes that gap:
//!
//! - [`DisplayChangeGate`] — a pure, edge-triggered coalescer. macOS can post
//!   several screen-parameters notifications for a single physical change
//!   (e.g. display-link negotiation during monitor hot-plug); the gate
//!   collapses each burst into exactly one dispatched recovery event.
//! - The macOS-only observer (see [`macos`]) registers a global
//!   `NSApplicationDidChangeScreenParametersNotification` observer on the main
//!   thread that consults the gate and forwards
//!   [`AppEvent::DisplayConfigurationChanged`](crate::app::AppEvent::DisplayConfigurationChanged)
//!   into the winit event loop via an `EventLoopProxy`.
//! - The event loop side ([`crate::app::handler`]) reacts by calling
//!   [`WindowState::force_surface_reconfigure`](crate::app::window_state::WindowState::force_surface_reconfigure)
//!   for every terminal window, then re-arms the gate via
//!   [`DisplayChangeGate::clear`].
//!
//! Non-macOS platforms compile this module (the gate and its tests) but never
//! dispatch the event; the observer itself is `cfg(target_os = "macos")`.

#[cfg(any(target_os = "macos", test))]
use std::sync::atomic::{AtomicBool, Ordering};

/// Edge-triggered coalescer for display-configuration notifications.
///
/// Semantics:
///
/// - `record()` returns `true` only for the **first** notification since the
///   last `clear()` — exactly when a recovery event should be dispatched.
///   Further notifications in the same burst are suppressed.
/// - `clear()` re-arms the gate once recovery has been applied, so a genuinely
///   later display change dispatches again.
///
/// The `swap` in `record()` is atomic: even if notifications arrived from
/// multiple threads (they do not — both the AppKit callback and the winit
/// `user_event` handler run on the main thread), only one caller can observe
/// the pre-swap `false`.
///
/// This type is deliberately free of any platform dependency so its burst
/// behavior is unit-testable on every platform (see the tests below).
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Default)]
pub(crate) struct DisplayChangeGate {
    /// Whether a recovery event has been dispatched and not yet applied.
    pending: AtomicBool,
}

#[cfg(any(target_os = "macos", test))]
impl DisplayChangeGate {
    pub(crate) const fn new() -> Self {
        Self {
            pending: AtomicBool::new(false),
        }
    }

    /// Record one display-configuration notification.
    ///
    /// Returns `true` when a recovery event should be dispatched (first
    /// notification of a burst), `false` when one is already outstanding.
    pub(crate) fn record(&self) -> bool {
        !self.pending.swap(true, Ordering::AcqRel)
    }

    /// Mark the outstanding recovery as applied and re-arm the gate so the
    /// next display change dispatches again.
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn clear(&self) {
        self.pending.store(false, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
}

/// Per-window gate for `WindowEvent::Moved`, answering "did this move put the
/// window on a monitor it was not on before?".
///
/// winit delivers `Moved` for every position update during a window drag —
/// dozens per second — and the handler must not treat each one as a display
/// change: a same-config `Surface::configure` heals nothing (see
/// `CellRenderer::reconfigure_after_display_change` in par-term-render) while
/// being a compositor-class disturbance under wgpu 30, so a per-move
/// configure storm is pure cost and a strobe candidate. Only an actual
/// monitor crossing warrants recovery, and this gate detects it
/// edge-triggered.
///
/// Like [`DisplayChangeGate`], deliberately free of platform dependencies so
/// its semantics are unit-testable everywhere; the caller feeds it
/// `window.current_monitor()` and reacts only to `true`.
#[derive(Debug, Default)]
pub(crate) struct WindowMoveGate<M> {
    /// The monitor the window was last known to be on. `None` until the
    /// first observation (and never returned to once set — a transient
    /// `current_monitor() == None` keeps the baseline rather than erasing it).
    last: Option<M>,
}

impl<M: PartialEq> WindowMoveGate<M> {
    pub(crate) const fn new() -> Self {
        Self { last: None }
    }

    /// Record the window's current monitor and report whether it differs
    /// from the previously recorded one.
    ///
    /// - The first observation establishes the baseline and returns `false`.
    /// - `None` (monitor transiently unknown) returns `false` and keeps the
    ///   baseline.
    /// - A change from one known monitor to another returns `true` once.
    pub(crate) fn observe(&mut self, current: Option<M>) -> bool {
        let Some(current) = current else {
            return false;
        };
        match &self.last {
            Some(last) if *last == current => false,
            _ => {
                let changed = self.last.is_some();
                self.last = Some(current);
                changed
            }
        }
    }
}

/// Process-wide display-change gate, shared between the macOS notification
/// observer (AppKit main-thread callback) and the winit event loop
/// (`ApplicationHandler::user_event`).
#[cfg(target_os = "macos")]
static DISPLAY_CHANGE_GATE: DisplayChangeGate = DisplayChangeGate::new();

/// Access the shared display-change gate.
#[cfg(target_os = "macos")]
pub(crate) fn display_change_gate() -> &'static DisplayChangeGate {
    &DISPLAY_CHANGE_GATE
}

#[cfg(target_os = "macos")]
mod macos {
    //! Global `NSApplicationDidChangeScreenParametersNotification` observer.

    use std::sync::OnceLock;

    use objc2::rc::Retained;
    use objc2::runtime::NSObjectProtocol;
    use objc2::{define_class, extern_methods};
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject};
    use winit::event_loop::EventLoopProxy;

    use crate::app::AppEvent;

    use super::display_change_gate;

    /// Proxy the observer uses to wake the event loop. Set once during app
    /// startup (before the run loop starts, so before any notification can be
    /// delivered).
    static PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();

    define_class!(
        // SAFETY:
        // - The superclass NSObject has no subclassing requirements.
        // - `DisplayChangeObserver` has no ivars and does not implement `Drop`.
        #[unsafe(super = NSObject)]
        struct DisplayChangeObserver;

        // SAFETY: `NSObjectProtocol` has no safety requirements.
        unsafe impl NSObjectProtocol for DisplayChangeObserver {}

        impl DisplayChangeObserver {
            // SAFETY: The signature matches `-(void)displayParametersChanged:(NSNotification *)`,
            // the selector this method is registered under with the
            // notification center (see `register_display_change_observer`).
            #[unsafe(method(displayParametersChanged:))]
            fn display_parameters_changed(&self, _notification: &NSNotification) {
                // Coalesce: skip if a recovery event is already outstanding.
                if !display_change_gate().record() {
                    return;
                }
                log::info!("macOS display configuration changed; dispatching recovery event");
                match PROXY.get() {
                    Some(proxy) => {
                        if let Err(e) = proxy.send_event(AppEvent::DisplayConfigurationChanged) {
                            log::warn!("Failed to send display-change event to event loop: {e}");
                            // Re-arm so a later change can retry delivery.
                            display_change_gate().clear();
                        }
                    }
                    None => {
                        // Unreachable in practice: registration stores the
                        // proxy before the observer can fire. Re-arm anyway.
                        display_change_gate().clear();
                    }
                }
            }
        }
    );

    impl DisplayChangeObserver {
        extern_methods!(
            #[unsafe(method(new))]
            fn new() -> Retained<Self>;
        );
    }

    /// Owns the registered notification observer.
    ///
    /// `NSNotificationCenter` does not retain selector-based observers, so the
    /// handle must outlive the registration; dropping it unregisters.
    pub(crate) struct DisplayChangeObserverHandle {
        observer: Option<Retained<DisplayChangeObserver>>,
    }

    impl Drop for DisplayChangeObserverHandle {
        fn drop(&mut self) {
            if let Some(observer) = self.observer.take() {
                let center = NSNotificationCenter::defaultCenter();
                // SAFETY: `observer` is a live NSObject registered with this
                // center by `register_display_change_observer`.
                unsafe { center.removeObserver(&observer) };
            }
        }
    }

    /// Register the global display-configuration observer.
    ///
    /// Must be called on the main thread (AppKit requirement); `App::run`
    /// does this before starting the event loop. The returned handle must be
    /// kept alive for the app lifetime — the notifications it recovers from
    /// can arrive at any point afterwards.
    pub(crate) fn register_display_change_observer(
        proxy: EventLoopProxy<AppEvent>,
    ) -> DisplayChangeObserverHandle {
        let _ = PROXY.set(proxy);

        let center = NSNotificationCenter::defaultCenter();
        let observer = DisplayChangeObserver::new();
        // SAFETY: `observer` is a live `NSObject` subclass, the selector
        // `displayParametersChanged:` is implemented on it (verified by
        // `define_class!` above), and the notification name is a framework
        // constant. `None` for the object means "any poster".
        unsafe {
            center.addObserver_selector_name_object(
                &observer,
                objc2::sel!(displayParametersChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
        }
        log::debug!("Registered NSApplicationDidChangeScreenParametersNotification observer");

        DisplayChangeObserverHandle {
            observer: Some(observer),
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::register_display_change_observer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_notification_dispatches() {
        let gate = DisplayChangeGate::new();
        assert!(!gate.is_pending());
        assert!(gate.record(), "first notification must dispatch");
        assert!(gate.is_pending());
    }

    #[test]
    fn burst_is_coalesced_to_one_dispatch() {
        // A single display change can post several notifications (display-link
        // negotiation during hot-plug); only the first may dispatch.
        let gate = DisplayChangeGate::new();
        assert!(gate.record());
        assert!(
            !gate.record(),
            "second notification in burst must be suppressed"
        );
        assert!(
            !gate.record(),
            "third notification in burst must be suppressed"
        );
        assert!(gate.is_pending());
    }

    #[test]
    fn clear_re_arms_the_gate() {
        let gate = DisplayChangeGate::new();
        assert!(gate.record());
        gate.clear();
        assert!(!gate.is_pending());
        assert!(
            gate.record(),
            "after clear, a new change must dispatch again"
        );
    }

    #[test]
    fn interleaved_handling_keeps_edge_trigger_semantics() {
        // Simulates: burst → dispatch → (suppressed duplicates) → recovery
        // applied → new physical change → dispatch again.
        let gate = DisplayChangeGate::new();

        // First burst of three notifications, one dispatch.
        assert!(gate.record());
        assert!(!gate.record());
        assert!(!gate.record());

        // Recovery applied while more duplicates arrive: duplicates before the
        // clear are still part of the handled burst.
        assert!(!gate.record());
        gate.clear();

        // A genuinely later display change dispatches exactly once more.
        assert!(gate.record());
        assert!(!gate.record());

        gate.clear();
        assert!(!gate.is_pending());
    }

    #[test]
    fn fresh_gate_is_not_pending() {
        let gate = DisplayChangeGate::default();
        assert!(!gate.is_pending());
    }

    #[test]
    fn move_gate_first_observation_records_without_firing() {
        let mut gate = WindowMoveGate::new();
        assert!(
            !gate.observe(Some("monitor-a")),
            "the first Moved event only establishes the baseline"
        );
    }

    #[test]
    fn move_gate_same_monitor_never_fires() {
        let mut gate = WindowMoveGate::new();
        gate.observe(Some("monitor-a"));
        for _ in 0..50 {
            assert!(
                !gate.observe(Some("monitor-a")),
                "moving within one monitor must not fire"
            );
        }
    }

    #[test]
    fn move_gate_fires_once_per_monitor_crossing() {
        let mut gate = WindowMoveGate::new();
        gate.observe(Some("monitor-a"));
        assert!(
            gate.observe(Some("monitor-b")),
            "crossing to another monitor must fire exactly once"
        );
        assert!(!gate.observe(Some("monitor-b")));
        assert!(gate.observe(Some("monitor-a")));
    }

    #[test]
    fn move_gate_transient_unknown_monitor_keeps_baseline() {
        // current_monitor() can briefly return None (window off-screen,
        // display tearing down). That must neither fire a recovery nor
        // forget which monitor the window was on: returning to the SAME
        // monitor stays silent, while the preserved baseline means a move
        // to a different one still fires.
        let mut gate = WindowMoveGate::new();
        gate.observe(Some("monitor-a"));
        assert!(!gate.observe(None));
        assert!(!gate.observe(Some("monitor-a")));
        assert!(gate.observe(Some("monitor-b")));
    }

    #[test]
    fn move_gate_baseline_from_none_via_none_is_inert() {
        let mut gate = WindowMoveGate::new();
        assert!(!gate.observe(None));
        assert!(!gate.observe(Some("monitor-a")), "still just a baseline");
    }
}
