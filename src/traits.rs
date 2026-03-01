//! Shared trait definitions for par-term components.
//!
//! These traits document the contracts between major components and enable
//! mock implementations for unit testing the `app/` module without needing
//! a live PTY session or GPU context.
//!
//! # Status
//!
//! These traits are currently *definitions only* — the concrete types
//! (`TerminalManager`, `TabBarUI`, `StatusBarUI`, `WindowState`) have not yet
//! been refactored to `impl` these traits. The definitions serve as:
//!
//! 1. Documentation of the expected interface contract
//! 2. The foundation for mock implementations in tests (see AUD-050)
//! 3. A migration target for future refactoring (AUD-040, AUD-041, AUD-042)
//!
//! # Migration Path
//!
//! When a component is ready to implement a trait:
//! 1. Add `impl TerminalAccess for TerminalManager { ... }` delegating to existing methods
//! 2. Update call sites in `app/mouse_events/` and `app/input_events/` to use `T: TerminalAccess`
//! 3. Add mock implementations in `#[cfg(test)]` blocks for unit tests

// ── AUD-040: TerminalAccess ──────────────────────────────────────────────────

/// Provides read-only access to terminal mode and state for input/mouse handlers.
///
/// Implemented by `TerminalManager` (and mock types in tests). Decouples
/// `app/mouse_events/` and `app/input_events/` from the concrete terminal type,
/// enabling unit tests without a live PTY session.
///
/// # Notes
///
/// All methods take `&self` — this trait is intentionally read-only. State
/// mutation goes through `TerminalManager` directly (write, resize, etc.).
pub trait TerminalAccess {
    /// Returns `true` if the alternate screen buffer (DECSC/smcup) is active.
    ///
    /// Used by mouse handlers to suppress scrollback scrolling while TUI apps
    /// (vim, htop, etc.) own the display.
    fn is_alt_screen_active(&self) -> bool;

    /// Returns `true` if mouse motion events should be reported to the PTY.
    ///
    /// `button_pressed` — whether a mouse button is currently held. Some
    /// modes (ButtonEvent) only report motion while a button is pressed.
    fn should_report_mouse_motion(&self, button_pressed: bool) -> bool;

    /// Returns the current modifyOtherKeys level (0 = off, 1 = basic, 2 = full).
    ///
    /// Controls how modifier-key combinations are encoded in key sequences.
    fn modify_other_keys_mode(&self) -> u8;

    /// Returns `true` if DECCKM (application cursor key) mode is active.
    ///
    /// In application mode, cursor keys send `\x1bO[ABCD]` instead of `\x1b[ABCD]`.
    fn application_cursor(&self) -> bool;

    /// Encode a mouse event into the bytes to send to the PTY.
    ///
    /// Parameters match the X10 / SGR / UTF-8 encoding conventions used by
    /// `par-term-emu-core-rust`:
    /// - `button` — button index (0-2 for primary/middle/secondary, 64+ for scroll)
    /// - `col` / `row` — 0-based cell coordinates
    /// - `pressed` — `true` for press, `false` for release
    /// - `modifiers` — bitmask: bit 2 = Shift, bit 3 = Alt, bit 4 = Ctrl
    fn encode_mouse_event(
        &self,
        button: u8,
        col: usize,
        row: usize,
        pressed: bool,
        modifiers: u8,
    ) -> Vec<u8>;
}

// ── AUD-041: UIElement ───────────────────────────────────────────────────────

/// Lifecycle contract for egui-based UI overlay components.
///
/// Implemented by `TabBarUI`, `StatusBarUI`, `TmuxStatusBarUI`, and similar
/// overlay panels. Documents the expected init → update → draw → handle_input
/// lifecycle and enables generic overlay management in `WindowState`.
///
/// # Notes
///
/// The `Config` and `egui::Context` parameters are not included in the trait
/// signature here because they differ per component (some take `&Config`,
/// others `&mut Config`). Concrete `render()` methods retain their current
/// signatures. The trait captures the structural contract rather than the
/// exact signatures.
pub trait UIElement {
    /// Returns `true` if this element should be rendered this frame.
    ///
    /// Typically checks config flags (e.g., `config.tab_bar_enabled`) and
    /// current application state (e.g., number of tabs).
    fn is_visible(&self) -> bool;

    /// Returns the height this element occupies in logical pixels.
    ///
    /// Used by layout code to reserve space before the renderer borrow.
    /// Returns 0.0 when the element is not visible.
    fn height_logical(&self) -> f32;

    /// Returns the width this element occupies in logical pixels.
    ///
    /// Returns 0.0 for full-width elements (status bar, tab bar with
    /// `TabBarPosition::Top` or `Bottom`).
    fn width_logical(&self) -> f32;

    /// Returns `true` if this element is currently capturing user input
    /// (e.g., a context menu is open, a rename field is focused).
    ///
    /// When `true`, the main terminal should not process keyboard or mouse
    /// events that fall within the element's bounds.
    fn is_capturing_input(&self) -> bool;
}

// ── AUD-042: EventHandler ────────────────────────────────────────────────────

/// Marker interface for components that process winit `WindowEvent` messages.
///
/// `WindowState` dispatches events through several handler methods. This trait
/// documents the expected signature and return convention without requiring
/// immediate refactoring of the existing monolithic dispatch chain.
///
/// # Return value
///
/// `handle_event` returns `true` if the event was consumed (no further
/// handlers should process it), `false` to propagate to the next handler.
///
/// # Notes
///
/// The concrete event type is `winit::event::WindowEvent`. It is not
/// referenced here to keep `traits.rs` free of heavy imports; implementors
/// use the full type in their `impl` blocks.
pub trait EventHandler {
    /// The event type this handler processes.
    type Event;

    /// Process one event.
    ///
    /// Returns `true` if the event was consumed and should not be forwarded
    /// to subsequent handlers in the chain.
    fn handle_event(&mut self, event: Self::Event) -> bool;
}
